# Local Apple renderer effects patch

Based on published `gpui-pre-apple` 0.3.10. Original Apache license retained.

- `effects::Surface` carries a typed effect through GPUI's ordered `PaintSurface` primitive. A private registry recognizes its retained 1×1 BGRA marker; normal video surfaces keep their existing path. Dropped/stale markers render empty. This avoids a fork of GPUI's cross-platform scene ABI.
- `metal_effects` allocates/reuses offscreen textures only while a backdrop blur is present. It downsamples to quarter resolution, encodes horizontal/vertical Gaussian passes, composites the blur at the surface's paint position, resumes normal foreground drawing, then copies to the drawable. All passes share the frame's command buffer. There is no CPU framebuffer readback or GPU wait.
- The palette fragment shader at the end of `shaders.metal` evaluates CSS-style radial gradients, group opacity and W3C nonseparable `color` blend against the current backdrop per display pixel. Colours remain float until output; spatial dithering reduces 8-bit display banding. Prepared cover pixels upload only when the source changes; motion updates small uniforms.
- Effect pipelines compile with the existing metallib at build time and initialize with the renderer, avoiding a first-modal runtime compilation stall. Only the app's macOS UI currently uses effects; other platforms retain their existing rendering fallback.
- Opt-in fixture telemetry records `MTLDrawable.presentedTime`, CPU renderer time and scene-geometry signatures. It is inactive in ordinary app launches. Geometry signatures distinguish repeated presents from visibly changed scroll positions; they are not a general image-difference measurement.

Remove this patch when upstream supports the required compositing primitives.
