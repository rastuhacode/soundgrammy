//! Retain Android JNI handles and initialize credentials after Tao sets up ndk-context.
use jni::{objects::JObject, JNIEnv};
use std::sync::OnceLock;

// Downloads use these handles independently of the Activity lifecycle.
static CONTEXT: OnceLock<jni::objects::GlobalRef> = OnceLock::new();
static VM: OnceLock<jni::JavaVM> = OnceLock::new();
static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();

pub(crate) fn java_context(
) -> Result<(&'static jni::JavaVM, &'static jni::objects::GlobalRef), String> {
    Ok((
        VM.get().ok_or("Android Java VM is unavailable")?,
        CONTEXT
            .get()
            .ok_or("Android application context is unavailable")?,
    ))
}

/// Tauri setup runs after Tao initializes the process-owned ndk-context and
/// before session repair or any frontend command can access credentials.
pub(crate) fn initialize_store() -> Result<(), String> {
    let store = android_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_MainActivity_initializeNativeStorage(
    mut env: JNIEnv<'_>,
    _activity: JObject<'_>,
    application: JObject<'_>,
) {
    let result = INITIALIZED.get_or_init(|| {
        let context = env.new_global_ref(application).map_err(|e| e.to_string())?;
        let vm = env.get_java_vm().map_err(|e| e.to_string())?;
        let _ = CONTEXT.set(context);
        let _ = VM.set(vm);
        Ok(())
    });
    if let Err(error) = result {
        let _ = env.throw_new(
            "java/lang/IllegalStateException",
            format!("Android JNI context initialization failed: {error}"),
        );
    }
}
