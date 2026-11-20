plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val appVersion = providers.gradleProperty("appVersion").getOrElse("3.6.0")
val appVersionCode = providers.gradleProperty("appVersionCode").getOrElse("3060000").toInt()
val releaseKeystore = System.getenv("ANDROID_KEYSTORE_PATH")

android {
    compileSdk = 36
    namespace = "com.soundgrammy.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.soundgrammy.app"
        minSdk = 26 // CPAL uses AAudio (Android 8.0+).
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersion
    }
    if (releaseKeystore != null) {
        signingConfigs {
            create("release") {
                storeFile = file(releaseKeystore)
                storePassword = System.getenv("ANDROID_KEY_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            applicationIdSuffix = ".dev"
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (releaseKeystore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.core:core-splashscreen:1.0.1")
    implementation("androidx.media:media:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}
