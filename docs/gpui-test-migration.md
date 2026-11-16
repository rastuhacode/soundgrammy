# Frontend regression migration inventory

> Historical migration evidence. The [current audit](gpui-audit.md) supersedes development commands, baseline references and test counts below.

Baseline: `develop` commit `6238384`, 388 passing frontend tests across 66 suites. The React/Vitest implementation is available in that commit's history.

Queue policy, shuffle, listening accounting, retries, streaming, database edits and export already had native Rust tests; those tests remain. Presentation contracts now use pure Rust model tests and deterministic GPUI windows. GPUI tests render the production view tree at desktop and compact sizes. JavaScript engine fakes, React effect cleanup, JSON IPC validation and browser media/session wrappers are retired because those layers no longer exist. Their domain guarantees are exercised by the native owner rather than a duplicate JavaScript engine.

This inventory is a review map, not a claim that every old test has a line-for-line equivalent. Physical Android playback, IME composition, notification and lifecycle acceptance is deferred to the user; builds and JVM tests do not prove those behaviors on a device.

| Previous suite | Native coverage / owner |
|---|---|
| `src/App.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/components/audio/AudioPlayer.test.ts` | `native/src/display_wake.rs`; `ui/{artwork,tests}.rs` |
| `src/components/audio/AudioProgressBar.test.ts` | `native/src/ui/model.rs`, `views.rs`; GPUI Slider handles native pointer and keyboard events |
| `src/components/audio/queue/queue-list-items.test.ts` | `native/src/ui/model.rs`; `ui/tests.rs` |
| `src/components/playlist/PlaylistTrackDropdownMenu.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/components/playlist/PlaylistTrackRow.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/components/playlist/PlaylistTracksTable.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/components/playlist/PlaylistView.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/components/playlist/track-actions.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/components/settings/SettingsDialog.test.ts` | `native/src/{cache,listen_stats,proxy_settings}.rs`; `ui/{actions,tests}.rs` |
| `src/hooks/audio/engine-factory.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/fake-engine.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/native-engine.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/native-resume.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/player-integration.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/react-engine.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/transport-engine.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/use-audio-controls.native.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/audio/use-audio-controls.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/hooks/use-android-back.test.ts` | `native/src/ui/views.rs` (`dismiss_layer`); Android `MainActivity.kt` |
| `src/hooks/use-app-session.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/hooks/use-cached-thumbnail.resume.test.ts` | `native/src/cache/media_art.rs`, `ui/{mod,artwork}.rs` |
| `src/hooks/use-cached-thumbnail.test.ts` | `native/src/cache/media_art.rs`, `ui/{mod,artwork}.rs` |
| `src/hooks/use-cached-thumbnail.upgrade.test.ts` | `native/src/cache/media_art.rs`, `ui/{mod,artwork}.rs` |
| `src/hooks/use-image-palette.test.ts` | `native/src/cache/media_art.rs`, `ui/{mod,artwork}.rs` |
| `src/hooks/use-playlist-form.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/hooks/use-playlist-view.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/hooks/use-profile-music-sync.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/hooks/use-proxy-settings.test.ts` | `native/src/proxy_settings.rs`; `ui/actions.rs` |
| `src/hooks/use-user-avatar.test.ts` | `native/src/cache/media_art.rs`, `ui/{mod,artwork}.rs` |
| `src/lib/android-back.test.ts` | `native/src/ui/views.rs` (`dismiss_layer`); Android `MainActivity.kt` |
| `src/lib/api.test.ts` | `native/src/runtime.rs`; `audio/mod.rs` presentation test. JSON validation retired with typed in-process values |
| `src/lib/app-logger.test.ts` | `native/src/ui/diagnostics.rs` (bounded opt-in history); typed calls carry no argument logs |
| `src/lib/auth/login-error.test.ts` | `native/src/telegram/auth.rs`; `ui/mod.rs` |
| `src/lib/auth/login-schemas.test.ts` | `native/src/telegram/auth.rs`; `ui/mod.rs` |
| `src/lib/bounce.test.ts` | `native/src/cache/media_art.rs`, `ui/{mod,artwork}.rs` |
| `src/lib/events.test.ts` | `native/src/runtime.rs`; `audio/mod.rs` presentation test. JSON validation retired with typed in-process values |
| `src/lib/export-track.test.ts` | `native/src/{export,playlist_recipe}.rs`; `ui/file_dialog.rs` |
| `src/lib/fullscreen-wake-lock.test.ts` | `native/src/display_wake.rs`; `ui/{artwork,tests}.rs` |
| `src/lib/library-hydration.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/lib/playlist-form.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/lib/playlist-playback.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/lib/playlist-recipe-io.test.ts` | `native/src/{export,playlist_recipe}.rs`; `ui/file_dialog.rs` |
| `src/lib/playlist-sort.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/lib/playlist-visibility.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/lib/proxy-link.test.ts` | `native/src/proxy_settings.rs`; `ui/actions.rs` |
| `src/lib/proxy-status.test.ts` | `native/src/proxy_settings.rs`; `ui/actions.rs` |
| `src/lib/queue/playing-source-index.test.ts` | `native/src/ui/model.rs`; `ui/tests.rs` |
| `src/lib/shuffle/index.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/lib/shuffle/modes.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/lib/telegram-reconnect.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/lib/volume.test.ts` | `native/src/ui/model.rs`, `views.rs`; GPUI Slider handles native pointer and keyboard events |
| `src/stores/cache-store.test.ts` | `native/src/cache.rs`; `ui/{mod,actions}.rs` account-scoped jobs |
| `src/stores/connectivity-store.sync.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/stores/connectivity-store.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/stores/fullscreen-store.test.ts` | `native/src/display_wake.rs`; `ui/{artwork,tests}.rs` |
| `src/stores/library-store.test.ts` | `native/src/telegram/saved_music.rs`; `ui/{mod,model,tests}.rs` |
| `src/stores/listen-stats-store.test.ts` | `native/src/listen_stats.rs`, `audio/activity.rs`, `ui/model.rs` |
| `src/stores/player-store.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/stores/playlist-jobs-store.test.ts` | `native/src/cache.rs`; `ui/{mod,actions}.rs` account-scoped jobs |
| `src/stores/playlists-store.test.ts` | `native/src/db/tests.rs`, `audio/session.rs`, `playlist_recipe.rs`; `ui/{model,actions,tests}.rs` |
| `src/stores/settings-mutations.test.ts` | `native/src/{cache,listen_stats,proxy_settings}.rs`; `ui/{actions,tests}.rs` |
| `src/stores/snapshot-reconciliation.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/test-support/legacy-listen-tracker.test.ts` | `native/src/listen_stats.rs`, `audio/activity.rs`, `ui/model.rs` |
| `src/test-support/legacy-queue.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
| `src/test-support/legacy-shuffle-base.test.ts` | `native/src/audio/{mod,session,shuffle,activity}.rs`; `ui/transport.rs` |
