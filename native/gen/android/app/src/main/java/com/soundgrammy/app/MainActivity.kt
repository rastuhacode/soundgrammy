package com.soundgrammy.app

import android.os.Bundle

/** GPUI's native Activity hosts rendering; playback belongs to PlaybackService. */
class MainActivity : dev.gpui.mobile.GpuiActivity() {
    companion object {
        private var current = java.lang.ref.WeakReference<MainActivity>(null)
        @JvmStatic fun background() { current.get()?.let { activity -> activity.runOnUiThread { activity.moveTaskToBack(true) } } }
    }
    private external fun nativeBack(): Boolean
    private fun handleBack() { if (!nativeBack()) moveTaskToBack(true) }
    @Deprecated("Compatibility for Android 12 and earlier")
    override fun onBackPressed() { handleBack() }
    private external fun initializeNativeStorage(context: android.content.Context)
    override fun onCreate(savedInstanceState: Bundle?) {
        System.loadLibrary("soundgrammy_lib")
        initializeNativeStorage(applicationContext)
        super.onCreate(savedInstanceState)
        current = java.lang.ref.WeakReference(this)
        if (android.os.Build.VERSION.SDK_INT >= 33) {
            onBackInvokedDispatcher.registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT) { handleBack() }
        }
    }
}
