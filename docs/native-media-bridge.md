# Native media bridge

`src-tauri/src/audio/media` projects the authoritative audio snapshot into OS media controls. It owns no decoder or queue. React keeps in-app keyboard shortcuts but does not register browser Media Session handlers or metadata.

## Commands and lifecycle

The bridge initializes once during Tauri setup. Platform callbacks enqueue `RemoteCommand` directly into the audio worker's bounded command channel, in arrival order with UI commands. Availability is checked again when the worker consumes the command. Play, pause, toggle, next, and previous reuse `session::Command`; previous restarts at five seconds, and next at the final item without repeat pauses, matching the app. Stop pauses and resets position without deleting the queue. Seeks reject non-finite values and clamp to duration.

Page epochs invalidate queued UI commands on navigation; native callbacks survive WebView reattachment. A separate session epoch invalidates pre-logout commands from either source. Queue clear/logout clears metadata when the worker publishes its empty snapshot. On app exit the bridge closes, discards queued updates, removes native callback registrations, and clears metadata. Android Activity destruction alone does not release the process-owned session.

Presentation includes revision, track and attempt identity, title, artist, native artwork path, actual transport status, duration, position sampled against `Instant`, rate, and available actions. Only actual `playing` state advances the OS clock. Position updates coalesce to one second; command effects and state changes publish immediately. A single pending main-thread task consumes the latest presentation, avoiding an unbounded UI task backlog. Older revisions and artwork for an obsolete track/attempt are rejected. Platform work runs outside the audio and PCM callbacks and their locks. Adapter initialization/update errors are logged without stopping audio.

## Platform behavior

| Platform | Adapter | Artwork | Transport differences |
| --- | --- | --- | --- |
| macOS | MediaPlayer through objc2; main-thread object lifetime and retained command tokens | Cached file → NSImage → MPMediaItemArtwork | Loading/buffering maps to Interrupted with rate zero; error/end maps to Stopped |
| Windows | SMTC bound to the main Tauri HWND; event tokens removed on teardown | Cached bytes → native memory stream (file URLs are not supported by CreateFromUri) | Buffering maps to Changing; seek range controls timeline availability |
| Linux | zbus MPRIS session-bus service, unique process name | Local file URL; attempt-specific MPRIS track path | MPRIS has no Buffering/Error states: buffering maps to Paused and errors to Stopped; rate is fixed at 1, volume writes are unsupported |
| Android | Application-owned framework MediaSession; callbacks cross JNI directly into Rust | Cached file decoded off the main thread, identity checked on completion | Native buffering/error states and per-session action mask |

Each adapter implements `new`, `update`, and `Drop`; an iOS implementation can be added behind the same platform boundary. iOS is not implemented in this step. Android exposes the existing session token through `NativeMediaSession.getOrCreate(context).token`: step 3's foreground service must reuse this owner rather than create a second session. This step does not promise Android background process survival or foreground-service notifications.

## Verification

Automated coverage checks action availability, native command mapping, queue/duplicate-attempt behavior, invalid and stale seeks, revision ordering, update coalescing, stale artwork after replacement/clear/shutdown, page/session invalidation, and absence of browser Media Session access across React reattachment. Existing native session tests cover repeat and shuffle policy.

The implementation was checked with macOS Rust tests/Clippy, frontend tests/lint/typecheck, an Android arm64 Rust check, Kotlin compilation against Android API 36, and isolated compile checks of the Windows and Linux adapter APIs. Isolated checks are not full Windows/Linux application builds; the repository's three-OS CI performs those. No OS surface/headset/device acceptance test is claimed by these checks.

On each OS, run the app and verify:

1. Title, artist, cover, duration and progress; pause/resume and buffering clock behavior.
2. Media keyboard/headset play, pause, stop, next and previous; each event executes once.
3. Seek, duplicate queue entries, repeat one/all and shuffle; previous restarts after five seconds.
4. Rapid A → B → A selection while cover downloads are pending; an old attempt's cover never wins.
5. Clear queue and logout remove metadata; app exit removes the media control owner.
6. Suspend JavaScript in the inspector or reload/detach the UI while playing. Native controls and queue advancement continue; reattachment shows the same native session.
7. On Android, recreate the Activity and confirm a single session/token remains. Test foreground-service/background guarantees separately in step 3.
