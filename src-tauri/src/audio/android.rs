//! CPAL's AAudio backend needs a process-wide Android application context.
use jni::objects::GlobalRef;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

// Retain an Application, not an Activity, across WebView/activity recreation.
// This reference intentionally lives until process exit, like ndk-context.
static CONTEXT: OnceLock<Result<GlobalRef, String>> = OnceLock::new();

pub(super) async fn initialize(app: &AppHandle) -> Result<(), String> {
    if let Some(result) = CONTEXT.get() {
        return result.as_ref().map(|_| ()).map_err(Clone::clone);
    }
    let webview = app
        .get_webview_window("main")
        .ok_or("Android audio initialization requires the main WebView")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let app = app.clone();
    webview
        .with_webview(move |webview| {
            webview.jni_handle().exec(move |env, activity, _| {
                let result = CONTEXT.get_or_init(|| {
                    let mut initialize = || -> jni::errors::Result<GlobalRef> {
                        if let Err(error) = super::media::platform::initialize(env, activity, &app)
                        {
                            tracing::warn!(%error, "Android media session unavailable");
                            let _ = env.exception_clear();
                        }
                        let application = env
                            .call_method(
                                activity,
                                "getApplicationContext",
                                "()Landroid/content/Context;",
                                &[],
                            )?
                            .l()?;
                        let context = env.new_global_ref(application)?;
                        let vm = env.get_java_vm()?;
                        // SAFETY: OnceLock serializes the only initialization. The JVM
                        // and retained Application outlive every CPAL operation, and
                        // all audio commands await this initialization before use.
                        unsafe {
                            ndk_context::initialize_android_context(
                                vm.get_java_vm_pointer().cast(),
                                context.as_obj().as_raw().cast(),
                            );
                        }
                        Ok(context)
                    };
                    initialize().map_err(|error| format!("Android audio context: {error}"))
                });
                let _ = send.send(result.as_ref().map(|_| ()).map_err(Clone::clone));
            });
        })
        .map_err(|error| error.to_string())?;
    receive
        .await
        .map_err(|_| "Android audio initialization interrupted".to_string())?
}
