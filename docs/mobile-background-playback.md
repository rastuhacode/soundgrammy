# Mobile background playback

The Rust control worker owns decoding, output, streaming, queue progression, user
intent, and listen attempts. React subscribes to snapshots and aggregates; WebView
suspension, navigation, and component cleanup do not stop the player. Mobile
window-driven exit requests are prevented; explicit application exit is honored.

## Android lifetime

`PlaybackService` is a non-exported `mediaPlayback` foreground service. It reuses
the process-owned `NativeMediaSession` and sends JNI commands directly to the
existing Rust worker. No second decoder, queue, or Activity reference is created.
The first UI audio command initializes the application context/JNI bridge. Later
native controls need neither an Activity nor JavaScript.

Before enabling PCM, Rust waits for foreground promotion and a granted audio-focus
request. Restricted/failed service starts, denied focus, and initialization timeout
fail playback safely. Transient loss (including duck requests) pauses output;
matching focus gain removes the interruption gate without changing user intent.
Permanent loss and becoming-noisy clear playback intent. Old focus-request callbacks
are discarded. The notification shares the MediaSession token and has previous,
pause, next, and stop actions; seek is exposed through MediaSession.

Swiping the task away from Android Recents stops the foreground service and ends
the process, so playback stops and the next launch starts with a fresh WebView.
Moving the app to the background still permits playback, and ordinary Activity
recreation reattaches through native snapshots. Pause/final completion/stop/logout
release focus and stop the foreground service; service destruction releases its
receiver and wake lock.
Unexpected service destruction sends native Stop. Temporary interruptions retain
the service but release its CPU wake lock. Active playback/buffering holds a partial
wake lock for Rust decoding/network work; it never keeps the display awake.

The service is `START_NOT_STICKY`. Process death and force-stop do not resume audio.
Optional persisted queue/cursor/position restoration is deliberately not enabled:
only repeat/shuffle preferences persist. A fresh process starts idle and requires
an explicit user playback action.

## iOS lifetime and CPAL ownership

`Info.ios.plist`, referenced by Tauri's `bundle.iOS.infoPlist`, enables the `audio`
background mode. The shared Apple adapter now supplies Now Playing metadata,
artwork, and remote commands on iOS as well as macOS. Rust activates the playback
AVAudioSession before opening CPAL, and deactivates it when output is released on
pause, stop, final completion, or failure.

CPAL 0.18.2's `host/coreaudio/ios/session_event_manager.rs` was inspected: CPAL
already stops/resumes its AudioUnit on interruption and reactivates the session
when `ShouldResume` is present. Our notification observer only gates Rust intent;
it never starts an AudioUnit or activates the session on interruption end. CPAL
also reports headphone/route removal, media-service loss/reset, and invalidation
through the output error callback. These fail safely; explicit Play rebuilds the
output at the retained position. Automatic reset recovery is intentionally avoided.

## Accounting and networking

Native PCM frame counts exclude seeks, buffering silence, and paused time. A single
ordered Last.fm task preserves start → qualification → end order; existing SQLite
queue deduplication/delivery remain in use. Changing Last.fm configuration invalidates
pending qualification work. Local statistics clear/enable changes are serialized
with playback transitions. UI reattachment refreshes aggregates without recording
another attempt. See [listen-statistics.md](listen-statistics.md).

Streaming lazily initializes the Telegram client in Rust. Ferogram 0.6.5 owns home
and media-DC connection recovery in its native async tasks. Playback file requests
retry transient IO/dropped/server errors at the same verified chunk offset, with
bounded backoff and cancellation polling. Auth, malformed-response, and file-reference
errors are not indiscriminately retried. The existing file-reference refresh path
is retained. No playback-critical retry requires browser `online` events.

## Validation record (2026-09-19)

Compilation/unit tests and device acceptance are separate:

- 159 host Rust tests and Clippy pass; 260 frontend tests, TypeScript, and ESLint pass.
- Android Kotlin compilation and `cargo check --target aarch64-linux-android` pass.
- iOS generation/compilation is **blocked** on this host: full Xcode, XcodeGen,
  Apple Rust targets, and signing setup are unavailable. `tauri ios init --ci`
  failed installing the target; `--skip-targets-install` failed obtaining XcodeGen.
  No generated Apple project or successful iOS build is claimed. On a configured
  Mac, run `bun tauri ios init`, set the development team, then build and verify the
  generated app's Info.plist includes `UIBackgroundModes = [audio]`.
- No real-device background, network-transition, battery, or CPU result is claimed.

## Required real-device acceptance

Run the following on physical iOS and Android devices, recording device/OS/build,
track format/cache state, observed result, logs, and resource measurements. All
rows remain **not run** for this change.

| Scenario | Expected evidence |
|---|---|
| Cached and streamed tracks, lock for 30+ minutes | Audible output and queue advancement without a mounted/active WebView |
| Repeat-one/all, shuffle, duplicate tracks | Correct native membership progression and one attempt per play |
| Lock-screen seek/previous/next/play/pause | Direct native actions; no restart on UI reattachment |
| Calls/focus competition; pause during interruption | Resume only if still intended; no stale resume after user pause |
| Wired headphones/Bluetooth removal and route changes | Safe pause/failure, explicit recovery; no unexpected speaker output |
| Wi-Fi ↔ cellular, offline then online | Verified chunks retained; native recovery or bounded recoverable error |
| Activity recreation without task removal | Same process session and position; no extra decoder/queue/accounting task |
| Swipe task from Recents, then reopen | Audio and notification stop; fresh process shows the UI and starts idle |
| Logout, notification stop, service destruction | Output, foreground status, focus, and wake resources released |
| Seek repeatedly; buffer; pause; clear/disable statistics | No time inflation or pre-clear history reappearance; Last.fm at most once per attempt |
| Force-stop/process death | No automatic playback; fresh process idle |
| Extended cached/streamed runs | Record battery delta, CPU, worker counts, wake-lock duration, and memory; check for growth |

Platform references: [Android background playback](https://developer.android.com/media/media3/session/background-playback),
[Android audio focus](https://developer.android.com/media/optimize/audio-focus),
[Apple audio lifecycle](https://developer.apple.com/library/archive/documentation/AudioVideo/Conceptual/MediaPlaybackGuide/Contents/Resources/en.lproj/RefiningTheUserExperience/RefiningTheUserExperience.html).
