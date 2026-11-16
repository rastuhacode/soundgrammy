//! Ordered GPU compositing. No drawable readback, CPU blur, or GPU waits.
use crate::effects::{Effect, Image};
use gpui::{DevicePixels, PaintSurface, Size};
use metal::*;
use std::{ffi::c_void, mem, sync::Weak};

pub(crate) struct Effects {
    palette: RenderPipelineState,
    copy: RenderPipelineState,
    blur: RenderPipelineState,
    overlay: RenderPipelineState,
    target: Option<Texture>,
    ping: Option<Texture>,
    pong: Option<Texture>,
    source: Option<Texture>,
    source_pixels: Weak<Vec<u8>>,
}
fn pipeline(device: &DeviceRef, library: &LibraryRef, fragment: &str) -> RenderPipelineState {
    let descriptor = RenderPipelineDescriptor::new();
    descriptor.set_vertex_function(Some(&library.get_function("sg_vertex", None).unwrap()));
    descriptor.set_fragment_function(Some(&library.get_function(fragment, None).unwrap()));
    let attachment = descriptor.color_attachments().object_at(0).unwrap();
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::One);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .expect("GPU effects pipeline")
}
fn texture(device: &DeviceRef, slot: &mut Option<Texture>, width: u64, height: u64) -> Texture {
    if slot
        .as_ref()
        .is_none_or(|texture| texture.width() != width || texture.height() != height)
    {
        let desc = TextureDescriptor::new();
        desc.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        desc.set_width(width);
        desc.set_height(height);
        desc.set_storage_mode(MTLStorageMode::Private);
        desc.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        *slot = Some(device.new_texture(&desc));
    }
    slot.as_ref().unwrap().clone()
}
fn encoder<'a>(
    commands: &'a CommandBufferRef,
    texture: &TextureRef,
) -> &'a RenderCommandEncoderRef {
    let descriptor = RenderPassDescriptor::new();
    let color = descriptor.color_attachments().object_at(0).unwrap();
    color.set_texture(Some(texture));
    color.set_load_action(MTLLoadAction::DontCare);
    color.set_store_action(MTLStoreAction::Store);
    commands.new_render_command_encoder(descriptor)
}
impl Effects {
    pub fn new(device: &DeviceRef, library: &LibraryRef) -> Self {
        Self {
            palette: pipeline(device, library, "sg_palette"),
            copy: pipeline(device, library, "sg_copy"),
            blur: pipeline(device, library, "sg_blur"),
            overlay: pipeline(device, library, "sg_overlay"),
            target: None,
            ping: None,
            pong: None,
            source: None,
            source_pixels: Weak::new(),
        }
    }
    pub fn target(&mut self, device: &DeviceRef, size: Size<DevicePixels>) -> Texture {
        texture(
            device,
            &mut self.target,
            i32::from(size.width).max(1) as u64,
            i32::from(size.height).max(1) as u64,
        )
    }
    pub fn release_backdrop(&mut self) {
        self.target = None;
        self.ping = None;
        self.pong = None;
    }
    pub fn copy(&self, source: &TextureRef, encoder: &RenderCommandEncoderRef) {
        encoder.set_render_pipeline_state(&self.copy);
        encoder.set_fragment_texture(0, Some(source));
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
    }
    fn source(&mut self, device: &DeviceRef, image: &Image) -> Texture {
        let length = (image.width as usize)
            .checked_mul(image.height as usize)
            .and_then(|v| v.checked_mul(4));
        assert!(
            image.width > 0 && image.height > 0 && length == Some(image.pixels.len()),
            "invalid GPU effect image dimensions/bytes"
        );
        if self
            .source_pixels
            .upgrade()
            .is_none_or(|pixels| !std::sync::Arc::ptr_eq(&pixels, &image.pixels))
        {
            let desc = TextureDescriptor::new();
            desc.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            desc.set_width(image.width as u64);
            desc.set_height(image.height as u64);
            desc.set_usage(MTLTextureUsage::ShaderRead);
            desc.set_storage_mode(if device.has_unified_memory() {
                MTLStorageMode::Shared
            } else {
                MTLStorageMode::Managed
            });
            let texture = device.new_texture(&desc);
            texture.replace_region(
                MTLRegion::new_2d(0, 0, image.width as u64, image.height as u64),
                0,
                image.pixels.as_ptr().cast(),
                u64::from(image.width) * 4,
            );
            self.source = Some(texture);
            self.source_pixels = std::sync::Arc::downgrade(&image.pixels);
        }
        self.source.as_ref().unwrap().clone()
    }
    pub fn palette(
        &mut self,
        device: &DeviceRef,
        effect: Effect,
        surface: &PaintSurface,
        encoder: &RenderCommandEncoderRef,
    ) {
        let Effect::Palette {
            image,
            mut uniforms,
        } = effect
        else {
            return;
        };
        let source = self.source(device, &image);
        let clip = surface.content_mask.bounds.intersect(&surface.bounds);
        encoder.set_scissor_rect(MTLScissorRect {
            x: clip.origin.x.0.max(0.) as u64,
            y: clip.origin.y.0.max(0.) as u64,
            width: clip.size.width.0.max(0.) as u64,
            height: clip.size.height.0.max(0.) as u64,
        });
        let b = surface.bounds;
        let x = b.origin.x.0.max(0.) as u64;
        let y = b.origin.y.0.max(0.) as u64;
        let width = b.size.width.0.max(1.);
        let height = b.size.height.0.max(1.);
        uniforms.viewport = [width, height, 0., 0.];
        encoder.set_viewport(MTLViewport {
            originX: x as f64,
            originY: y as f64,
            width: width as f64,
            height: height as f64,
            znear: 0.,
            zfar: 1.,
        });
        encoder.set_render_pipeline_state(&self.palette);
        encoder.set_fragment_texture(0, Some(&source));
        encoder.set_fragment_bytes(
            0,
            mem::size_of_val(&uniforms) as u64,
            (&uniforms as *const _ as *const c_void).cast(),
        );
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
    }
    pub fn blur(
        &mut self,
        device: &DeviceRef,
        commands: &CommandBufferRef,
        target: &TextureRef,
        radius: f32,
    ) -> Texture {
        // Quarter resolution: radius 16 logical px becomes a small Gaussian.
        // Three ordered GPU passes use a fixed tap count regardless of window size.
        let w = target.width().div_ceil(4).max(1);
        let h = target.height().div_ceil(4).max(1);
        let ping = texture(device, &mut self.ping, w, h);
        let pong = texture(device, &mut self.pong, w, h);
        let e = encoder(commands, &ping);
        self.copy(target, e);
        e.end_encoding();
        for (source, destination, step) in [
            (&ping, &pong, [radius / (16. * w as f32), 0., 0., 0.]),
            (&pong, &ping, [0., radius / (16. * h as f32), 0., 0.]),
        ] {
            let e = encoder(commands, destination);
            e.set_render_pipeline_state(&self.blur);
            e.set_fragment_texture(0, Some(source));
            e.set_fragment_bytes(0, 16, step.as_ptr().cast());
            e.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
            e.end_encoding();
        }
        ping
    }
    pub fn overlay(&self, source: &TextureRef, opacity: f32, encoder: &RenderCommandEncoderRef) {
        let parameters = [0f32, 0., 0., opacity];
        encoder.set_render_pipeline_state(&self.overlay);
        encoder.set_fragment_texture(0, Some(source));
        encoder.set_fragment_bytes(0, 16, parameters.as_ptr().cast());
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
    }
}
