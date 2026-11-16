//! Android's native activity hosts the same GPUI view tree as desktop.
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("soundgrammy_lib=info")
        .try_init();
    gpui_mobile::android::jni::install_panic_hook();
    let _platform = gpui_mobile::android::jni::init_platform(&app);
    let Some(shared) = gpui_mobile::android::jni::shared_platform() else {
        tracing::error!("Android GPUI platform unavailable");
        return;
    };
    gpui_kit::Application::with_platform(shared.into_rc())
        .with_assets(gpui_kit::assets::AllAssets)
        .run(crate::ui::launch);
}

#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_MainActivity_nativeBack(
    _: jni::JNIEnv<'_>,
    _: jni::objects::JObject<'_>,
) -> jni::sys::jboolean {
    if crate::runtime::request_mobile_back() {
        1
    } else {
        0
    }
}
