# Local GPUI macOS frame pacing patch

Based on the published gpui-pre-macos 0.3.10 crate. Original licenses retained.

- Prefer NSView CADisplayLink on macOS 14+ and request the screen maximum refresh rate, including common run-loop modes during tracking. Keep CVDisplayLink fallback for older systems.
- Implement GPUI frame_waker/schedule_frame and stop the clock after 50 ms without demand. Continuous animations/input retain native display cadence. A former one-second grace kept the display clock continuously awake between 500 ms caret blinks; the shorter grace preserves short input gaps while sparse updates can sleep. Invalidating the modern link breaks its retained target reference during idle, occlusion, resize and close.
- Keep the upstream immortal CVDisplayLink registry and its teardown protections intact.

Remove this patch when upstream provides equivalent pacing and idle behavior.
