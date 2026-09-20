package com.soundgrammy.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private external fun initializeNativeStorage(context: android.content.Context)

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    // Rust's generated loader runs from super.onCreate; initialize the store
    // first so the WebView cannot race an auth/session read.
    System.loadLibrary("soundgrammy_lib")
    initializeNativeStorage(applicationContext)
    super.onCreate(savedInstanceState)
  }
}
