//! Initialize Android's process-owned MediaSession when playback starts.
use crate::runtime::AppHost;
use jni::{objects::JString, JNIEnv};
use std::sync::OnceLock;

static MEDIA_READY: OnceLock<()> = OnceLock::new();

fn initialization_error(env: &mut JNIEnv<'_>, error: jni::errors::Error) -> String {
    if !matches!(error, jni::errors::Error::JavaException) {
        return format!("Android media session: {error}");
    }
    let throwable = env.exception_occurred().ok();
    let _ = env.exception_clear();
    let detail = throwable.and_then(|throwable| {
        let value = env
            .call_method(throwable, "toString", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        env.get_string(&JString::from(value))
            .ok()
            .map(|text| text.into())
    });
    let _ = env.exception_clear();
    format!(
        "Android media session: {}",
        detail.unwrap_or_else(|| error.to_string())
    )
}

pub(super) async fn initialize(app: &AppHost) -> Result<(), String> {
    if MEDIA_READY.get().is_some() {
        return Ok(());
    }
    let (vm, context) = crate::android_keyring::java_context()?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let result = super::media::platform::initialize(&mut env, context.as_obj(), app)
        .map_err(|error| initialization_error(&mut env, error));
    if result.is_ok() {
        let _ = MEDIA_READY.set(());
    }
    result
}
