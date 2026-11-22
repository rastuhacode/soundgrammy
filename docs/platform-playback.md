# Platform playback

Native adapters project the Rust playback snapshot into system media controls. The audio worker handles OS and UI commands through the same queue. OS callbacks carry the presented attempt identity so a late command cannot affect the next track. Availability is checked again when the worker consumes the command. See the [transport and reattachment contract](native-audio.md#queue-and-ui-boundary).

Metadata and artwork are scoped to the current track and attempt. Position advances in the OS only while the transport is playing. Position updates coalesce, while state changes publish immediately. Adapter errors leave playback running. Logout and queue clear remove metadata. App exit releases the media-control owner.

| Platform | Adapter | Behavior |
| --- | --- | --- |
| macOS | [Apple MediaPlayer](../src-tauri/src/audio/media/apple.rs) | Now Playing and remote commands. Buffering reports an interruption with playback rate zero. |
| Windows | [SMTC](../src-tauri/src/audio/media/windows.rs) | Bound to the main window. Artwork uses a native memory stream. Buffering maps to Changing. |
| Linux | [MPRIS](../src-tauri/src/audio/media/linux.rs) | Process-specific session-bus name and local artwork URLs. Buffering maps to Paused and errors map to Stopped. |
| Android | [MediaSession](../src-tauri/src/audio/media/android.rs) | Application-owned session and JNI callbacks. The foreground service shares the same session token. |
| iOS | [Apple MediaPlayer](../src-tauri/src/audio/media/apple.rs) | Shares Apple's metadata and command adapter and uses a playback audio session. |

See the [README](../README.md#supported-platforms) for distribution support and [CI](../.github/workflows/ci.yml) for automated build coverage.

## Android lifecycle

Android requires API 26 or newer for CPAL's AAudio backend. Tao initializes the shared NDK context during Activity startup. The Rust [Android audio entry point](../src-tauri/src/audio/android.rs) attaches the media session lazily through the main WebView. Later OS commands reach the native worker directly.

Before PCM starts, the worker waits for [`PlaybackService`](../src-tauri/gen/android/app/src/main/java/com/soundgrammy/app/PlaybackService.kt) to enter the foreground and obtain audio focus. Foreground-start failure, focus denial, and initialization timeout fail playback. Transient focus loss pauses output while preserving play intent. Permanent loss and becoming noisy clear that intent. Active playback or buffering holds a partial CPU wake lock.

Pausing releases focus and the wake lock, demotes the service, and leaves a dismissible notification. A prepared track can retain its decoder and PCM buffer for resume. Moving to the background and recreating the Activity preserve the process-owned session. Swiping the task from Recents stops the service and ends the process. Force-stop and process death also leave the next launch idle.

## iOS lifecycle

The [iOS plist](../src-tauri/Info.ios.plist) declares the audio background mode. Rust activates the playback audio session before opening CPAL and deactivates it when output is released. The Rust interruption observer gates playback intent while CPAL manages the AudioUnit. A route or media-services failure releases output. An explicit Play can rebuild it at the retained position. See the [iOS audio integration](../src-tauri/src/audio/ios.rs).

## Device verification

Record the device, OS version, app build, cache state, and observed result for each run. CI build results establish compilation. Use a running app to check timing, system controls, and background behavior.

| Scenario | Expected behavior |
| --- | --- |
| Cached and streamed tracks, including indexed M4A and variable-bitrate MP3, over a throttled connection | Playback starts, seeks, recovers from buffering, and reaches EOF. |
| Media keys, headset, notification, and lock-screen actions where available | Each action executes once. Position freezes on pause and buffering. Previous and next match the app's queue policy. |
| Rapid track switches during artwork fetch, then clear queue or logout | The OS shows artwork for the current attempt and clears metadata when the session ends. |
| WebView reload or Android Activity recreation during playback | Audio and OS controls continue. The UI reattaches to the same queue, position, and attempt. |
| Mobile background and screen lock for 30 minutes or more | Cached and streamed playback advance without an active WebView, including repeat, shuffle, and duplicate tracks. |
| Calls or competing audio, pause during interruption, and headset removal | Resume follows current user intent. Route loss fails or pauses safely. |
| Wi-Fi to cellular, offline, then reconnect | Verified chunks are retained and playback recovers or reports a recoverable error. |
| Android Recents swipe, force-stop, explicit exit, and service destruction | Playback stops and releases platform resources. After process exit the next launch starts idle. |
| Desktop device changes, sleep/wake, and no-device scenarios | Output transitions or reports a recoverable failure. |
| Seek, buffer, repeat, clear statistics, and resume during actual output | Local history respects the settings boundary. Last.fm queues at most one scrobble per qualified attempt. |
| Extended mixed-format playback | Record CPU, memory, underruns, and worker count. On mobile, also record battery use and wake-lock duration. |
