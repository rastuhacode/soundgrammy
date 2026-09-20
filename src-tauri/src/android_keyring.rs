//! Initialize the Android credential store before the WebView can sign in.
use jni::{objects::JObject, JNIEnv};
use std::sync::OnceLock;

// ndk-context retains a raw jobject, so keep its Application global reference
// alive for the entire process, including activity recreation.
static CONTEXT: OnceLock<jni::objects::GlobalRef> = OnceLock::new();
static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();

#[no_mangle]
pub extern "system" fn Java_com_soundgrammy_app_MainActivity_initializeNativeStorage(
    mut env: JNIEnv<'_>,
    _activity: JObject<'_>,
    application: JObject<'_>,
) {
    let result = INITIALIZED.get_or_init(|| {
        let context = env.new_global_ref(application).map_err(|e| e.to_string())?;
        let vm = env.get_java_vm().map_err(|e| e.to_string())?;
        let context = CONTEXT.get_or_init(|| context);
        // SAFETY: INITIALIZED is set once, and CONTEXT holds a global JNI ref
        // for the lifetime of the process. The VM is owned by Android.
        unsafe {
            ndk_context::initialize_android_context(
                vm.get_java_vm_pointer().cast(),
                context.as_obj().as_raw().cast(),
            );
        }
        let store = android_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
        keyring_core::set_default_store(store);
        Ok(())
    });
    if let Err(error) = result {
        let _ = env.throw_new(
            "java/lang/IllegalStateException",
            format!("Android secure storage initialization failed: {error}"),
        );
    }
}
