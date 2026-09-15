# Audio engine boundary

Desktop playback always uses `NativeRustAudioEngine`, created by `engine-factory.tsx`. The provider owns one adapter per committed lifetime and awaits asynchronous destruction before creating a replacement. Tests can inject a fake engine through the factory.

Queue, repeat, shuffle, requested play/pause, and listening attempts remain in the existing Zustand stores. `player-integration.ts` connects them to the transport contract in `engine.ts`. Components consume snapshots through `use-audio-engine.ts` and never own an audio element.

The native adapter subscribes to backend events before fetching its initial snapshot, validates payloads, serializes commands, and rejects stale revisions and track/attempt identities. Commands wait for initialization. Scrubbing previews the cursor immediately and sends the final seek when scrubbing ends.

Unload releases playback while leaving the adapter reusable. Destroy is terminal, unloads the backend, and removes event subscriptions. Backend page navigation handling stops native playback. Creation failures from injected factories use `AudioEngineCreationError`; playback failures are exposed in the engine snapshot without fallback to another engine.

Fake transport contract tests cover state semantics. Native adapter tests cover IPC ordering, stale events, failures, and teardown. React integration tests cover provider lifetimes, volume preferences, and listening activity. Rust fixture tests cover decoding, seeking, source cancellation, and output accounting. Audible output still requires desktop device verification.

See [native audio](native-audio.md) for implementation details and platform limitations.
