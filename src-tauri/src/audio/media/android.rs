//! JNI calls a process-owned Android MediaSession, never a React listener.
use super::{dispatch, Presentation, RemoteCommand};
use jni::{
    objects::{GlobalRef, JClass, JObject, JValue},
    sys::{jdouble, jint},
    JNIEnv, JavaVM,
};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
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
    // JNI FindClass uses the system loader on Wry's native dispatch thread.
    // Resolve app classes through the Activity's loader instead.
    let name = env.new_string("com.soundgrammy.app.NativeMediaSession")?;
    let class = env
        .call_method(
            activity,
            "getAppClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )?
        .l()?;
    let session = env
        .call_static_method(
            JClass::from(class),
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

/// Start the foreground lifetime and obtain focus before rendering any PCM.
pub fn acquire() -> bool {
    let Some((vm, session)) = SESSION.get() else {
        return false;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return false;
    };
    let result = env
        .call_method(session.as_obj(), "acquire", "()Z", &[])
        .and_then(|v| v.z());
    if result.is_err() {
        let _ = env.exception_clear();
    }
    result.unwrap_or(false)
}
#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_NativeMediaSession_nativeLifecycle(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    event: jint,
    token: jni::sys::jlong,
) {
    use crate::audio::lifecycle::Event;
    let event = match event {
        0 if token > 0 => Event::Begin(token as u64),
        1 if token > 0 => Event::End(token as u64, true),
        2 => Event::PermanentLoss,
        _ => return,
    };
    if let Some(app) = APP.get() {
        let _ = app
            .state::<crate::state::AppState>()
            .audio
            .lifecycle_event(event);
    }
}
