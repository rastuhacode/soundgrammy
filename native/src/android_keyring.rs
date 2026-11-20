//! Retain Android JNI handles and initialize credentials after the native Activity initializes its JNI context.
use jni::{
    objects::{Global, JObject},
    EnvUnowned,
};
use std::sync::OnceLock;

// Downloads use these handles independently of the Activity lifecycle.
static CONTEXT: OnceLock<Global<JObject<'static>>> = OnceLock::new();
static VM: OnceLock<jni::JavaVM> = OnceLock::new();
static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();

pub(crate) fn java_context(
) -> Result<(&'static jni::JavaVM, &'static Global<JObject<'static>>), String> {
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
    mut env: EnvUnowned<'_>,
    _activity: JObject<'_>,
    application: JObject<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let result = INITIALIZED.get_or_init(|| {
            let context = env.new_global_ref(application).map_err(|e| e.to_string())?;
            let vm = env.get_java_vm().map_err(|e| e.to_string())?;
            let _ = CONTEXT.set(context);
            let _ = VM.set(vm);
            Ok(())
        });
        if let Err(error) = result {
            env.throw_new(
                jni::jni_str!("java/lang/IllegalStateException"),
                jni::strings::JNIString::new(format!(
                    "Android JNI context initialization failed: {error}"
                )),
            )?;
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
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
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let result = (|| {
            let loader = env
                .call_method(
                    context.as_ref(),
                    jni::jni_str!("getClassLoader"),
                    jni::jni_sig!("()Ljava/lang/ClassLoader;"),
                    &[],
                )?
                .l()?;
            let name = env.new_string("com.soundgrammy.app.NativeFiles")?;
            let class = env
                .call_method(
                    loader,
                    jni::jni_str!("loadClass"),
                    jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
                    &[JValue::Object(&name)],
                )?
                .l()?;
            let uri = env.new_string(uri)?;
            let path = env.new_string(path.to_string_lossy().as_ref())?;
            let class = env.cast_local::<JClass>(class)?;
            env.call_static_method(
                class,
                jni::jni_str!("copyDocument"),
                jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Z)V"),
                &[
                    JValue::Object(context.as_ref()),
                    JValue::Object(&uri),
                    JValue::Object(&path),
                    JValue::Bool(write),
                ],
            )?;
            Ok(())
        })();
        if result.is_err() {
            env.exception_clear();
        }
        result
    })
    .map_err(|_| "The selected document could not be read or written.".into())
}

/// Root Back navigation backgrounds the Activity while the native player stays alive.
pub(crate) fn background_activity() -> Result<(), String> {
    let (vm, context) = java_context()?;
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let result = (|| {
            let loader = env
                .call_method(
                    context.as_ref(),
                    jni::jni_str!("getClassLoader"),
                    jni::jni_sig!("()Ljava/lang/ClassLoader;"),
                    &[],
                )?
                .l()?;
            let name = env.new_string("com.soundgrammy.app.MainActivity")?;
            let class = env
                .call_method(
                    loader,
                    jni::jni_str!("loadClass"),
                    jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
                    &[jni::objects::JValue::Object(&name)],
                )?
                .l()?;
            let class = env.cast_local::<jni::objects::JClass>(class)?;
            env.call_static_method(
                class,
                jni::jni_str!("background"),
                jni::jni_sig!("()V"),
                &[],
            )?;
            Ok(())
        })();
        if result.is_err() {
            env.exception_clear();
        }
        result
    })
    .map_err(|e| e.to_string())
}
