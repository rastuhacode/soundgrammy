//! JNI calls a process-owned Android MediaSession, never a React listener.
use super::{dispatch, Presentation, RemoteCommand};
use crate::runtime::AppHost;
use jni::{
    objects::{Global, JClass, JObject, JValue},
    sys::{jdouble, jint},
    Env, EnvUnowned, JavaVM,
};
use std::sync::OnceLock;
static SESSION: OnceLock<(JavaVM, Global<JObject<'static>>)> = OnceLock::new();
static APP: OnceLock<AppHost> = OnceLock::new();
pub fn initialize(
    env: &mut Env<'_>,
    activity: &JObject<'_>,
    app: &AppHost,
) -> jni::errors::Result<()> {
    if SESSION.get().is_some() {
        return Ok(());
    }
    let _ = APP.set(app.clone());
    let loader = env
        .call_method(
            activity,
            jni::jni_str!("getClassLoader"),
            jni::jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let name = env.new_string("com.soundgrammy.app.NativeMediaSession")?;
    let class = env
        .call_method(
            loader,
            jni::jni_str!("loadClass"),
            jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)],
        )?
        .l()?;
    let class = env.cast_local::<JClass>(class)?;
    let session = env
        .call_static_method(
            class,
            jni::jni_str!("getOrCreate"),
            jni::jni_sig!("(Landroid/content/Context;)Lcom/soundgrammy/app/NativeMediaSession;"),
            &[JValue::Object(activity)],
        )?
        .l()?;
    let session = env.new_global_ref(session)?;
    let _ = SESSION.set((env.get_java_vm()?, session));
    Ok(())
}
pub fn publish(p: &Presentation) -> Result<(), String> {
    let Some((vm, session)) = SESSION.get() else {
        return Ok(());
    };
    let mut current = p.clone();
    current.position = p.position_now();
    let json = serde_json::to_string(&current).map_err(|e| e.to_string())?;
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let result = (|| {
            let json = env.new_string(json)?;
            env.call_method(
                session.as_ref(),
                jni::jni_str!("update"),
                jni::jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&json)],
            )?;
            Ok(())
        })();
        if result.is_err() {
            env.exception_clear();
        }
        result
    })
    .map_err(|e| e.to_string())
}

pub fn shutdown() {
    if let Some((vm, session)) = SESSION.get() {
        let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            if env
                .call_method(
                    session.as_ref(),
                    jni::jni_str!("release"),
                    jni::jni_sig!("()V"),
                    &[],
                )
                .is_err()
            {
                env.exception_clear();
            }
            Ok(())
        });
    }
}

/// JNI ABI matches NativeMediaSession.nativeCommand. No JVM object escapes this call.
#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_NativeMediaSession_nativeCommand(
    _env: EnvUnowned<'_>,
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
    vm.attach_current_thread(|env| -> jni::errors::Result<bool> {
        let result = env
            .call_method(
                session.as_ref(),
                jni::jni_str!("acquire"),
                jni::jni_sig!("()Z"),
                &[],
            )
            .and_then(|v| v.z());
        if result.is_err() {
            env.exception_clear();
        }
        result
    })
    .unwrap_or(false)
}
#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_NativeMediaSession_nativeLifecycle(
    _env: EnvUnowned<'_>,
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
