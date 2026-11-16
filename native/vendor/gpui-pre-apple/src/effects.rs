//! GPU effect surfaces and opt-in actual presentation measurements.
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
pub struct FrameMeasurements {
    pub presented_seconds: Vec<f64>,
    pub cpu_draw_ms: Vec<f64>,
    pub scene_signatures: Vec<u64>,
}
static MEASUREMENTS: Mutex<FrameMeasurements> = Mutex::new(FrameMeasurements {
    presented_seconds: Vec::new(),
    cpu_draw_ms: Vec::new(),
    scene_signatures: Vec::new(),
});
pub(crate) fn measuring() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("SOUNDGRAMMY_PREVIEW_PERF").is_some()
            && std::env::args().any(|arg| arg == "--preview-fixture")
    })
}
pub(crate) fn record_presented(seconds: f64, signature: u64) {
    if seconds > 0. {
        let mut metrics = MEASUREMENTS.lock().unwrap_or_else(|e| e.into_inner());
        metrics.presented_seconds.push(seconds);
        metrics.scene_signatures.push(signature);
    }
}
pub(crate) fn record_draw(milliseconds: f64) {
    MEASUREMENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .cpu_draw_ms
        .push(milliseconds);
}
pub fn take_measurements() -> FrameMeasurements {
    std::mem::take(&mut *MEASUREMENTS.lock().unwrap_or_else(|e| e.into_inner()))
}

use objc2_core_foundation::CFRetained;
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferCreate, kCVPixelFormatType_32BGRA, kCVReturnSuccess,
};
use std::{collections::HashMap, ptr::NonNull, sync::Arc};

/// Immutable source prepared off the UI thread; BGRA8, already blurred once.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<u8>>,
}
/// Float uniforms stay unquantized until the final display render target.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaletteUniforms {
    pub rest: [[f32; 4]; 3],
    pub active: [[f32; 4]; 3],
    pub peak: [[f32; 4]; 3],
    pub energy: [f32; 4],
    pub spread: [f32; 4],
    pub drift: [f32; 4],
    pub ambient: [f32; 4],
    pub viewport: [f32; 4],
}
#[derive(Clone)]
pub enum Effect {
    BackdropBlur {
        radius: f32,
        opacity: f32,
    },
    Palette {
        image: Image,
        uniforms: PaletteUniforms,
    },
}
static SURFACES: OnceLock<Mutex<HashMap<usize, Effect>>> = OnceLock::new();
fn surfaces() -> &'static Mutex<HashMap<usize, Effect>> {
    SURFACES.get_or_init(Mutex::default)
}
/// A typed effect carried through GPUI's existing ordered surface primitive.
/// Only registered buffers are intercepted; ordinary video surfaces are unchanged.
pub struct Surface {
    buffer: CFRetained<CVPixelBuffer>,
}
impl Surface {
    pub fn new(effect: Effect) -> Option<Self> {
        let mut buffer = std::ptr::null_mut();
        let status = unsafe {
            CVPixelBufferCreate(
                None,
                1,
                1,
                kCVPixelFormatType_32BGRA,
                None,
                NonNull::from(&mut buffer),
            )
        };
        if status != kCVReturnSuccess {
            return None;
        }
        let buffer = unsafe { CFRetained::from_raw(NonNull::new(buffer)?) };
        let surface = Self { buffer };
        surface.update(effect);
        Some(surface)
    }
    pub fn update(&self, effect: Effect) {
        surfaces()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(self.key(), effect);
    }
    fn key(&self) -> usize {
        &*self.buffer as *const CVPixelBuffer as usize
    }
    pub fn buffer(&self) -> CFRetained<CVPixelBuffer> {
        self.buffer.clone()
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        surfaces()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.key());
    }
}
pub(crate) fn lookup(buffer: &CVPixelBuffer) -> Option<Effect> {
    surfaces()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(buffer as *const CVPixelBuffer as usize))
        .cloned()
}

/// A dropped effect can still be retained by the previously painted GPUI scene.
/// Treat that stale marker as empty rather than decoding it as a video frame.
pub(crate) fn is_marker(buffer: &CVPixelBuffer) -> bool {
    objc2_core_video::CVPixelBufferGetWidth(buffer) == 1
        && objc2_core_video::CVPixelBufferGetHeight(buffer) == 1
        && objc2_core_video::CVPixelBufferGetPixelFormatType(buffer) == kCVPixelFormatType_32BGRA
}
