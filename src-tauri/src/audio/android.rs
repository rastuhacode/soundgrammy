//! Initialize Android's process-owned MediaSession when playback starts.
use jni::{objects::JString, JNIEnv};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

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

pub(super) async fn initialize(app: &AppHandle) -> Result<(), String> {
    if MEDIA_READY.get().is_some() {
        return Ok(());
    }
    let webview = app
        .get_webview_window("main")
        .ok_or("Android audio initialization requires the main WebView")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let app = app.clone();
    webview
        .with_webview(move |webview| {
            webview.jni_handle().exec(move |env, activity, _| {
                let result = super::media::platform::initialize(env, activity, &app)
                    .map_err(|error| initialization_error(env, error));
                if result.is_ok() {
                    let _ = MEDIA_READY.set(());
                }
                let _ = send.send(result);
            });
        })
        .map_err(|error| error.to_string())?;
    receive
        .await
        .map_err(|_| "Android audio initialization interrupted".to_string())?
}
