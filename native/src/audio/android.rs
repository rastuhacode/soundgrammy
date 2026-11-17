//! Initialize Android's process-owned MediaSession when playback starts.
use crate::runtime::AppHost;
use jni::{objects::JString, Env};
use std::sync::OnceLock;

static MEDIA_READY: OnceLock<()> = OnceLock::new();

fn initialization_error(env: &mut Env<'_>, error: jni::errors::Error) -> String {
    if !matches!(error, jni::errors::Error::JavaException) {
        return format!("Android media session: {error}");
    }
    let throwable = env.exception_occurred();
    env.exception_clear();
    let detail = throwable.and_then(|throwable| {
        let value = env
            .call_method(
                throwable,
                jni::jni_str!("toString"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        env.cast_local::<JString>(value)
            .ok()?
            .try_to_string(env)
            .ok()
    });
    env.exception_clear();
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
    let result = vm
        .attach_current_thread(|env| -> jni::errors::Result<_> {
            Ok(
                super::media::platform::initialize(env, context.as_ref(), app)
                    .map_err(|error| initialization_error(env, error)),
            )
        })
        .map_err(|e| e.to_string())?;
    if result.is_ok() {
        let _ = MEDIA_READY.set(());
    }
    result
}
