//! Publish a cached track through Android's Downloads collection.

use std::path::PathBuf;

use jni::{
    objects::{JClass, JString, JValue},
    JNIEnv,
};

use crate::error::{AppError, AppResult};

fn java_error(env: &mut JNIEnv<'_>, error: jni::errors::Error) -> String {
    if !matches!(error, jni::errors::Error::JavaException) {
        return error.to_string();
    }
    let throwable = env.exception_occurred().ok();
    let _ = env.exception_clear();
    let detail = throwable.and_then(|throwable| {
        let text = env
            .call_method(throwable, "toString", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        env.get_string(&JString::from(text))
            .ok()
            .map(|text| String::from(text))
    });
    let _ = env.exception_clear();
    detail.unwrap_or_else(|| error.to_string())
}

fn save_blocking(source: PathBuf, name: String, mime: String) -> AppResult<String> {
    let (vm, context) = crate::android_keyring::java_context().map_err(AppError::msg)?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|error| AppError::msg(format!("Cannot access Android Downloads: {error}")))?;
    let result = (|| -> jni::errors::Result<String> {
        // Native worker threads cannot resolve app classes with FindClass.
        let loader = env
            .call_method(
                context.as_obj(),
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        let class_name = env.new_string("com.soundgrammy.app.AndroidDownloads")?;
        let class = env
            .call_method(
                loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&class_name)],
            )?
            .l()?;
        let source = env.new_string(source.to_string_lossy().as_ref())?;
        let name = env.new_string(name)?;
        let mime = env.new_string(mime)?;
        let location = env
            .call_static_method(
                JClass::from(class),
                "save",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[
                    JValue::Object(context.as_obj()),
                    JValue::Object(&source),
                    JValue::Object(&name),
                    JValue::Object(&mime),
                ],
            )?
            .l()?;
        let location = JString::from(location);
        let location = String::from(env.get_string(&location)?);
        Ok(location)
    })();
    result.map_err(|error| {
        AppError::msg(format!(
            "Cannot save track to Downloads: {}",
            java_error(&mut env, error)
        ))
    })
}

pub async fn save(source: PathBuf, name: String, mime: String) -> AppResult<String> {
    tokio::task::spawn_blocking(move || save_blocking(source, name, mime))
        .await
        .map_err(|error| AppError::msg(format!("Android download interrupted: {error}")))?
}
