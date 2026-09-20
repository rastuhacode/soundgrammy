//! Initialize Android's process-owned MediaSession when playback starts.
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

static MEDIA_READY: OnceLock<Result<(), String>> = OnceLock::new();

pub(super) async fn initialize(app: &AppHandle) -> Result<(), String> {
    if let Some(result) = MEDIA_READY.get() {
        return result.clone();
    }
    let webview = app
        .get_webview_window("main")
        .ok_or("Android audio initialization requires the main WebView")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let app = app.clone();
    webview
        .with_webview(move |webview| {
            webview.jni_handle().exec(move |env, activity, _| {
                let result = MEDIA_READY.get_or_init(|| {
                    super::media::platform::initialize(env, activity, &app)
                        .map_err(|error| format!("Android media session: {error}"))
                });
                if result.is_err() {
                    let _ = env.exception_clear();
                }
                let _ = send.send(result.clone());
            });
        })
        .map_err(|error| error.to_string())?;
    receive
        .await
        .map_err(|_| "Android audio initialization interrupted".to_string())?
}
