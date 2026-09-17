//! JNI calls a process-owned Android MediaSession, never a React listener.
use super::{dispatch, Presentation, RemoteCommand};
use jni::{
    objects::{GlobalRef, JClass, JObject, JValue},
    sys::{jdouble, jint},
    JNIEnv, JavaVM,
};
use std::sync::OnceLock;
use tauri::AppHandle;
static SESSION: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();
pub fn initialize(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    app: &AppHandle,
) -> jni::errors::Result<()> {
    if SESSION.get().is_some() {
        return Ok(());
    }
    let _ = APP.set(app.clone());
    let class = env.find_class("com/soundgrammy/app/NativeMediaSession")?;
    let session = env
        .call_static_method(
            class,
            "getOrCreate",
            "(Landroid/content/Context;)Lcom/soundgrammy/app/NativeMediaSession;",
            &[JValue::Object(activity)],
        )?
        .l()?;
    let session = env.new_global_ref(session)?;
    let _ = SESSION.set((env.get_java_vm()?, session));
    Ok(())
}
pub struct Adapter;
impl Adapter {
    pub fn new(_: &AppHandle) -> Result<Self, String> {
        Ok(Self)
    }
    pub fn update(&mut self, p: &Presentation) -> Result<(), String> {
        let Some((vm, session)) = SESSION.get() else {
            return Ok(());
        };
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let mut current = p.clone();
        current.position = p.position_now();
        let json = serde_json::to_string(&current).map_err(|e| e.to_string())?;
        // An already-attached app thread may live for the entire process; bound
        // JNI local references instead of accumulating one string per update.
        let result: jni::errors::Result<()> = env.with_local_frame(8, |env| {
            let json = env.new_string(json)?;
            env.call_method(
                session.as_obj(),
                "update",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&json)],
            )?;
            Ok(())
        });
        if result.is_err() {
            let _ = env.exception_clear();
        }
        result.map(|_| ()).map_err(|e| e.to_string())
    }
}
impl Drop for Adapter {
    fn drop(&mut self) {
        if let Some((vm, session)) = SESSION.get() {
            if let Ok(mut env) = vm.attach_current_thread() {
                if env
                    .call_method(session.as_obj(), "release", "()V", &[])
                    .is_err()
                {
                    let _ = env.exception_clear();
                }
            }
        }
    }
}
/// JNI ABI matches NativeMediaSession.nativeCommand. No JVM object escapes this call.
#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_NativeMediaSession_nativeCommand(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    command: jint,
    seconds: jdouble,
) {
    let command = match command {
        0 => RemoteCommand::Play,
        1 => RemoteCommand::Pause,
        2 => RemoteCommand::Stop,
        3 => RemoteCommand::Next,
        4 => RemoteCommand::Previous,
        5 => RemoteCommand::Seek(seconds),
        6 => RemoteCommand::Toggle,
        _ => return,
    };
    if let Some(app) = APP.get() {
        dispatch(app, command);
    }
}
