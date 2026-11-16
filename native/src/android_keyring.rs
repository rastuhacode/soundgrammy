//! Retain Android JNI handles and initialize credentials after the native Activity initializes its JNI context.
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

/// The native runtime starts after Android initializes the process-owned ndk-context and
/// before session repair or any UI call can access credentials.
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

/// Match the previous application's data/cache locations exactly. Android's
/// application data directory is the parent of getFilesDir(), not getFilesDir().
pub(crate) fn application_paths(
) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf), String> {
    let files = gpui_mobile::packages::path_provider::support_directory()?;
    let data = files
        .parent()
        .ok_or("Application data directory unavailable")?
        .to_path_buf();
    let cache = gpui_mobile::packages::path_provider::cache_directory()?;
    let downloads = files.join("Downloads");
    Ok((data, cache, downloads))
}

pub(crate) fn copy_document(uri: &str, path: &std::path::Path, write: bool) -> Result<(), String> {
    use jni::objects::{JClass, JValue};
    let (vm, context) = java_context()?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let result: jni::errors::Result<()> = env.with_local_frame(16, |env| {
        let loader = env
            .call_method(
                context.as_obj(),
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        let name = env.new_string("com.soundgrammy.app.NativeFiles")?;
        let class = env
            .call_method(
                loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?;
        let uri = env.new_string(uri)?;
        let path = env.new_string(path.to_string_lossy().as_ref())?;
        env.call_static_method(
            JClass::from(class),
            "copyDocument",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Z)V",
            &[
                JValue::Object(context.as_obj()),
                JValue::Object(&uri),
                JValue::Object(&path),
                JValue::Bool(write.into()),
            ],
        )?;
        Ok(())
    });
    if result.is_err() {
        let _ = env.exception_clear();
    }
    result.map_err(|_| "The selected document could not be read or written.".into())
}

/// Root Back navigation backgrounds the Activity while the native player stays alive.
pub(crate) fn background_activity() -> Result<(), String> {
    let (vm, context) = java_context()?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let result: jni::errors::Result<()> = env.with_local_frame(8, |env| {
        let loader = env
            .call_method(
                context.as_obj(),
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        let name = env.new_string("com.soundgrammy.app.MainActivity")?;
        let class = env
            .call_method(
                loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[jni::objects::JValue::Object(&name)],
            )?
            .l()?;
        env.call_static_method(jni::objects::JClass::from(class), "background", "()V", &[])?;
        Ok(())
    });
    if result.is_err() {
        let _ = env.exception_clear();
    }
    result.map_err(|e| e.to_string())
}
