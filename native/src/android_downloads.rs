//! Publish a cached track through Android's Downloads collection.

use std::path::PathBuf;

use jni::{
    objects::{JClass, JString, JValue},
    Env,
};

use crate::error::{AppError, AppResult};

fn java_error(env: &mut Env<'_>, error: jni::errors::Error) -> String {
    if !matches!(error, jni::errors::Error::JavaException) {
        return error.to_string();
    }
    let throwable = env.exception_occurred();
    env.exception_clear();
    let detail = throwable.and_then(|throwable| {
        let text = env
            .call_method(
                throwable,
                jni::jni_str!("toString"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        env.cast_local::<JString>(text)
            .ok()?
            .try_to_string(env)
            .ok()
    });
    env.exception_clear();
    detail.unwrap_or_else(|| error.to_string())
}

fn save_blocking(source: PathBuf, name: String, mime: String) -> AppResult<String> {
    let (vm, context) = crate::android_keyring::java_context().map_err(AppError::msg)?;
    vm.attach_current_thread(|env| -> jni::errors::Result<_> {
        let result = (|| -> jni::errors::Result<String> {
            // Native worker threads cannot resolve app classes with FindClass.
            let loader = env
                .call_method(
                    context.as_ref(),
                    jni::jni_str!("getClassLoader"),
                    jni::jni_sig!("()Ljava/lang/ClassLoader;"),
                    &[],
                )?
                .l()?;
            let class_name = env.new_string("com.soundgrammy.app.AndroidDownloads")?;
            let class = env
                .call_method(
                    loader,
                    jni::jni_str!("loadClass"),
                    jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
                    &[JValue::Object(&class_name)],
                )?
                .l()?;
            let source = env.new_string(source.to_string_lossy().as_ref())?;
            let name = env.new_string(name)?;
            let mime = env.new_string(mime)?;
            let class = env.cast_local::<JClass>(class)?;
            let location = env
                .call_static_method(
                    class,
                    jni::jni_str!("save"),
                    jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                    &[
                        JValue::Object(context.as_ref()),
                        JValue::Object(&source),
                        JValue::Object(&name),
                        JValue::Object(&mime),
                    ],
                )?
                .l()?;
            env.cast_local::<JString>(location)?.try_to_string(env)
        })();
        Ok(result.map_err(|error| {
            AppError::msg(format!("Cannot save track to Downloads: {}", java_error(env, error)))
        }))
    })
    .map_err(|error| AppError::msg(format!("Cannot access Android Downloads: {error}")))?
}

pub async fn save(source: PathBuf, name: String, mime: String) -> AppResult<String> {
    tokio::task::spawn_blocking(move || save_blocking(source, name, mime))
        .await
        .map_err(|error| AppError::msg(format!("Android download interrupted: {error}")))?
}
