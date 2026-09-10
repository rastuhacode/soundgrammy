# Audio engine boundary

This refactor keeps HTML audio as the default. It adds no native playback or new player features. Queue, repeat, shuffle, and requested play/pause remain in the existing Zustand stores.

## Implementation plan and resulting layout

1. Define serializable transport requests, snapshots, errors, and events in `src/hooks/audio/engine.ts`. Keep queue identity separate from track identity.
2. Preserve the source, sparse-cache, MSE append, MP3 frame-sync, seek, and WebKit priming helpers under `src/hooks/audio/html/`. Bind them through a private React host, rather than rewriting the streaming algorithm.
3. Synchronize store intent and engine facts in `player-integration.ts`. Subscribe synchronously so React batching cannot collapse A → B → A transitions. Use a lifetime/attempt sequence plus the store's same-track epoch. Natural completion creates a new attempt for repeat-one or adjacent duplicate rows.
4. Expose only values/actions from `useAudioEngine`. Keyboard, Media Session, previous-or-restart, listening statistics, Last.fm, and artwork timing use this surface. Artwork samples the existing precomputed profile against the engine timeline, interpolating between observations while actually playing. The existing provisional motion is the fallback when no profile is available.
5. Run the same contract suite against a fake transport and the HTML host. Add store integration, Strict Mode, listening lifetime, volume, source cleanup, and pending-play tests. Retain the existing focused MSE and queue tests.

The only composition decision is `createAudioEngine` in `engine-factory.tsx`. The provider creates an engine for its committed lifetime and destroys it on cleanup. The HTML host exists only for the HTML composition. An override returns `{ engine }` for an implementation that needs no React host. There is no platform branch in the player UI.

## Contract semantics

- `load` accepts a request without waiting for media readiness. A different attempt replaces the previous source. Repeating the same active request is a no-op; Play after a recoverable error rebuilds its source with a new internal generation.
- `attemptId` identifies a queue/listen attempt, not a Telegram track. A retry retains it; a duplicate-row skip or repeat creates a new one.
- `play`/`pause` request transport changes. `playing`, `buffering`, `paused`, and `error` describe actual activity. Buffering preserves play intent; external pause and failure pause store intent. Listening clocks consume actual transitions.
- Methods can be called before readiness. HTML queues early play, volume, and seek commands until its private host attaches. `beginSeek`/`endSeek` retain existing scrub behavior; a native implementation without batching can implement these as no-ops.
- Times are seconds. Finite seeks clamp to zero and the known duration; non-finite seek/volume input rejects with `RangeError`. Unknown duration is zero. Volume is `0..100`; UI persistence remains the JSON number at `soundgrammy-volume`.
- Snapshots are stable between events and read synchronously with `getSnapshot`. Revisions increase within an engine lifetime. Subscribe does not emit an artificial event; read the initial snapshot after subscribing. Listener failures cannot disrupt other listeners.
- `initialLoading` distinguishes initial source acquisition from later buffering. Buffered ranges report playable time, not partial-download byte estimates. The existing disjoint-range calculations are retained.
- Natural end emits once for the active attempt. Seek-to-duration, failure, replacement, and unload do not advance the queue.
- Replacement invalidates the generation before disposing the old source. Each HTML generation owns a separate element, so queued DOM events cannot be attributed to a later load of the same track. Asynchronous seek and source work also checks generation.
- Unload releases the source and leaves the engine reusable. Destroy is terminal and idempotent. Source teardown pauses audio, removes its source, disposes MSE, closes the backend session, and removes event subscriptions. Page hide and window close also close the backend session.
- Errors log one safe record per failure containing kind, track, attempt, and code. Implementation exceptions, URLs, cache paths, and Telegram documents do not cross the error surface.

## Adding a native proxy later

Implement `AudioEngine` and return it from the composition factory. Translate Tauri commands/events through `src/lib/api.ts`; do not expose raw payloads or native decoder/output types. A proxy must correlate events to the request attempt and a generation, reject stale responses, preserve command ordering, and report actual state. Run `audioEngineContract` with a harness for that implementation before selecting it.

No consumer changes are needed for a proxy satisfying this contract. Native resource acquisition, audio interruptions, platform lifecycle, and native media controls belong to that implementation. This refactor does not implement or validate those platform integrations, and it does not move queue ownership out of React. Background queue ownership would be a separate playback-service milestone.

Creation failure is exposed as `AudioEngineCreationError`. An override must release any partially created resources before throwing. There is no automatic mid-attempt fallback: select the HTML composition before starting playback, or first destroy the previous engine through an explicit replacement flow.

## Verification

Use `bun lint`, `bun typecheck`, and `bun run test`. For a checkout without `.env.local`, use the same build-only placeholders as CI: `TELEGRAM_API_ID=1 TELEGRAM_API_HASH=ci-placeholder`. These tests do not log into Telegram. `bun run test` invokes the repository's Vitest and Cargo scripts; bare `bun test` selects Bun's different built-in runner, which does not honor this project's Vitest mocks or jsdom environments.

The HTML contract tests run the real hooks with simulated media decoding. Existing MSE tests exercise the append/range/session algorithms independently. Audible output and platform-specific WebKit/AVFoundation behavior still require playback in the desktop app; simulated DOM tests cannot validate device audio.
