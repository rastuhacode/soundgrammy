# Native methods and classes resolved by name from Rust.
-keep class com.soundgrammy.app.MainActivity { *; }
-keep class com.soundgrammy.app.NativeMediaSession { *; }
-keep class com.soundgrammy.app.NativeMediaSession$Companion { *; }
-keep class com.soundgrammy.app.AndroidDownloads { *; }
-keep class com.soundgrammy.app.NativeFiles { *; }
-keep class dev.gpui.mobile.** { *; }
-keepclasseswithmembernames class * { native <methods>; }
