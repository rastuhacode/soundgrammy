package com.soundgrammy.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private external fun initializeNativeStorage(context: android.content.Context)

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    // Retain JNI handles for Downloads. Tao initializes ndk-context in
    // super.onCreate; Rust app setup then initializes the credential store.
    System.loadLibrary("soundgrammy_lib")
    initializeNativeStorage(applicationContext)
    super.onCreate(savedInstanceState)
  }
}
