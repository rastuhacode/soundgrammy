# Mobile background playback

The Android GPUI Activity hosts rendering and composing-text input. The Rust runtime owns playback, queue intent and listening attempts; a view subscribes to display snapshots. `MainActivity` initializes the process JNI context before the keyring/session store can be used, then inherits `GpuiActivity` → `GpuiInputActivity` → `NativeActivity`.

`native/gen/android/app/src/main/java/dev/gpui/mobile` contains the pinned upstream host and package helpers, with Apache license and revision notices. The inheritance adjustment retains the InputConnection needed for Android soft keyboards and multistage IMEs.

## Lifetime

The existing `PlaybackService` remains a foreground media-playback service and owns a partial wake lock while background playback needs it. `NativeMediaSession` publishes notifications and lock-screen controls. JNI commands go directly to the Rust audio owner, including while no GPUI view is present. Android media publication attaches to the Java VM directly rather than depending on an active view/main-thread notification receiver.

A process-level Rust resource owner survives Activity recreation. Reattached views adopt authoritative snapshots. Home/backgrounding keeps playback active; task removal is an explicit close and stops/kills through `TaskRemoval`. The service remains non-sticky: process death/force-stop requires an explicit user launch and play. No durable queue recovery or automatic process resurrection is added.

Back first goes through GPUI key routing so an active menu may dismiss before its parent. The app then closes a dialog, fullscreen player, selection mode or compact playlist in order. At the root it backgrounds the Activity. Nested dialogs retain their parent. Android 13+ predictive Back and older Back callbacks share the same route; the system IME retains priority for hiding its keyboard.

## Storage and platform integration

Release and debug package IDs retain their previous values. Database/session/cache paths stay compatible with the old app. The app loads Android Keystore-backed credentials before repairing any restored unreadable session. Backup rules continue excluding credential/session material. User downloads retain the existing Android Downloads bridge; recipe file pickers copy SAF documents into controlled temporary storage before Rust reads or writes them.

The existing audio-focus, noisy-route and interruption policy remains in the service and native audio lifecycle modules. Minimum SDK remains 26 for AAudio. SDK 36 and NDK 29 are configured in the host/build tooling.

## Verification

`cargo xtask android --debug` cross-compiles arm64 Rust, stages the library, builds a debug APK and runs JVM tests. Host tests check composing-input inheritance; existing task-removal and instrumented media tests remain.

Physical-device checks are deferred to the user. Test login/QR/2FA, keyboard composition, cached and streamed playback, seeks, Home/lock screen, notification controls, interruptions/headset disconnect, Back, Activity recreation, task removal and restart. APK/JVM build success does not establish those real-device behaviors.

The Rust iOS audio/lifecycle adapter and `Info.ios.plist` are retained, but an iOS GPUI application host and signing pipeline are not configured. iOS was not an officially supported application target before this migration.
