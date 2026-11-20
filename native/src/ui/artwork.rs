//! The artwork palette and motion policy ported from the web player.
#[cfg(test)]
use super::bounce::sample;
pub(super) use super::bounce::BounceProfile;
use std::collections::HashMap;
pub fn complete(colors: &[[u8; 3]]) -> Vec<[u8; 3]> {
    fn shift(c: [u8; 3], a: f64) -> [u8; 3] {
        c.map(|v| {
            (v as f64 + ((if a > 0. { 255. } else { 0. }) - v as f64) * a.abs()).round() as u8
        })
    }
    match colors {
        [] => vec![],
        [c] => {
            let brightness = c.iter().map(|v| *v as f64).sum::<f64>() / 3.;
            if *c.iter().max().unwrap() < 48 {
                vec![*c, shift(*c, -0.14), shift(*c, -0.25)]
            } else if brightness < 128. {
                vec![*c, shift(*c, 0.18), shift(*c, -0.25)]
            } else {
                vec![*c, shift(*c, -0.18), shift(*c, 0.12)]
            }
        }
        [a, b] => vec![
            *a,
            *b,
            std::array::from_fn(|i| ((a[i] as f64 + b[i] as f64) / 2.).round() as u8),
        ],
        _ => colors[..3].to_vec(),
    }
}
mod colors;
pub(super) use colors::{Background, Palette};

/// Embedded ID3 art must survive a late remote-thumbnail response.
pub(super) fn prefer_thumbnail(
    current: Option<&std::path::Path>,
    candidate: &std::path::Path,
) -> bool {
    let embedded = |path: &std::path::Path| path.to_string_lossy().ends_with(".embed.jpg");
    current.is_none_or(|path| !embedded(path)) || embedded(candidate)
}

pub(super) struct Prepared {
    pub cover: Arc<RenderImage>,
    pub palette: Palette,
    pub background: Background,
}
/// Prepare the current cover before fullscreen is requested. Decode only once,
/// retain a ready-to-paint cover, and perform image work on the blocking pool.
pub(super) fn prepare(path: &std::path::Path) -> Option<Prepared> {
    let decoded = image::open(path).ok()?;
    let palette = colors::extract(
        &decoded
            .resize_exact(48, 48, image::imageops::FilterType::Triangle)
            .to_rgba8(),
    );
    let background = colors::background(&decoded, palette);
    let decoded = if decoded.width() > 2048 || decoded.height() > 2048 {
        decoded.resize(2048, 2048, image::imageops::FilterType::Lanczos3)
    } else {
        decoded
    };
    let mut cover = decoded.to_rgba8();
    for pixel in cover.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Prepared {
        cover: Arc::new(RenderImage::new(vec![image::Frame::new(cover)])),
        palette,
        background,
    })
}

/// Small blurred cover used behind the active row's play indicator.
pub fn active_cover(path: &std::path::Path) -> Option<std::sync::Arc<gpui_kit::RenderImage>> {
    let image = image::open(path)
        .ok()?
        .resize_exact(96, 96, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let mut image = image::imageops::blur(&image, 4.);
    for pixel in image.pixels_mut() {
        for channel in &mut pixel.0[..3] {
            *channel = (*channel as f32 * 0.6) as u8;
        }
        pixel.0.swap(0, 2);
    }
    Some(std::sync::Arc::new(gpui_kit::RenderImage::new(vec![
        image::Frame::new(image),
    ])))
}

use super::{model::BounceSettings, views::CARD};
use gpui_kit::{base::animation::cubic_bezier, prelude::*, *};
use std::{path::PathBuf, sync::Arc, time::Instant};

const MAX_DECODED_COVERS: usize = 96;

/// The virtual list limits live rows. Bound the decoded images separately so
/// visiting a new section cannot keep all of its covers in RAM and the atlas.
#[derive(Default)]
pub(super) struct ThumbnailCache {
    images: HashMap<Resource, (u64, ImageCacheItem)>,
    access: u64,
}

impl ThumbnailCache {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self::default());
        cx.observe_release(&cache, |cache, cx| {
            for (_, item) in cache.images.values() {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        cache
    }
}

impl ImageCache for ThumbnailCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        self.access += 1;
        if let Some((access, image)) = self.images.get_mut(resource) {
            *access = self.access;
            return image.use_image(window);
        }
        if self.images.len() == MAX_DECODED_COVERS {
            let oldest = self
                .images
                .iter()
                .min_by_key(|(_, (access, _))| *access)
                .map(|(resource, _)| resource.clone())
                .unwrap();
            let (_, image) = self.images.remove(&oldest).unwrap();
            if let Some(Ok(image)) = image.get() {
                window.defer(cx, move |window, cx| {
                    cx.drop_image(image, Some(window));
                    // Cached sibling views may still have paint commands for
                    // this atlas tile. Rebuild them before reusing that paint.
                    window.refresh();
                });
            }
        }
        let image = ImageCacheItem::new(resource, cx);
        let result = image.use_image(window);
        self.images.insert(resource.clone(), (self.access, image));
        result
    }
}

pub(super) fn release_texture(image: Option<Arc<RenderImage>>, window: &Window, cx: &mut App) {
    if let Some(image) = image {
        window.defer(cx, move |window, cx| cx.drop_image(image, Some(window)));
    }
}

pub(super) fn release_background(background: Background, window: &Window, cx: &mut App) {
    release_texture(background.rest, window, cx);
    for layer in background.bands.into_iter().chain(background.peaks) {
        release_texture(layer, window, cx);
    }
}

#[derive(PartialEq)]
pub(super) struct FullscreenFrame {
    pub track_id: Option<i64>,
    pub cover: Option<PathBuf>,
    pub image: Option<Arc<RenderImage>>,
    pub backdrop: Background,
    pub palette: Palette,
    pub settings: BounceSettings,
    pub profile: Option<Arc<BounceProfile>>,
    pub profile_ready_at: Option<Instant>,
    pub seconds: f64,
    pub playing: bool,
}

/// Animate the artwork in its own view so a visual frame does not re-render the
/// library, playback controls, tooltips, or queue.
pub(super) struct FullscreenVisual {
    artwork_cache: Entity<ThumbnailCache>,
    frame: Option<FullscreenFrame>,
    clock: Option<(Instant, Instant, Instant)>, // entrance, playback anchor, previous frame
    motion: super::bounce::Motion,
}

impl FullscreenVisual {
    pub fn new(artwork_cache: Entity<ThumbnailCache>) -> Self {
        Self {
            artwork_cache,
            frame: None,
            clock: None,
            motion: Default::default(),
        }
    }
    pub fn clear(&mut self) {
        *self = Self::new(self.artwork_cache.clone());
    }
    pub fn set_frame(&mut self, frame: FullscreenFrame, cx: &mut Context<Self>) {
        if self.frame.as_ref() == Some(&frame) {
            return;
        }
        let now = cx.background_executor().now();
        let (entered, observed, sampled) = self.clock.get_or_insert((now, now, now));
        if self
            .frame
            .as_ref()
            .is_none_or(|previous| previous.track_id != frame.track_id)
        {
            *entered = now;
            self.motion = Default::default();
        }
        if self.frame.as_ref().is_none_or(|previous| {
            previous.track_id != frame.track_id
                || previous.seconds != frame.seconds
                || previous.playing != frame.playing
        }) {
            if self.frame.as_ref().is_some_and(|previous| {
                (frame.seconds
                    - previous.seconds
                    - if previous.playing {
                        now.saturating_duration_since(*observed).as_secs_f64()
                    } else {
                        0.
                    })
                .abs()
                    > 0.5
            }) {
                self.motion = Default::default();
            }
            *observed = now;
        }
        if !frame.settings.enabled {
            self.motion = Default::default();
            *sampled = now;
        }
        self.frame = Some(frame);
        cx.notify();
    }
}

impl Render for FullscreenVisual {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(frame) = self.frame.as_ref() else {
            return div();
        };
        let now = cx.background_executor().now();
        let (entered, observed, sampled) = self.clock.as_mut().unwrap();
        let age = now.saturating_duration_since(*entered).as_secs_f64();
        let dt_ms = now.saturating_duration_since(*sampled).as_secs_f64() * 1000.;
        *sampled = now;
        let reduce_motion = cx.reduce_motion();
        let (scale, lift) = if frame.settings.enabled && !reduce_motion {
            frame.profile.as_ref().map_or((1., 0.), |profile| {
                let seconds = frame.seconds
                    + if frame.playing {
                        now.saturating_duration_since(*observed).as_secs_f64()
                    } else {
                        0.
                    };
                let fade = frame.profile_ready_at.map_or(1., |ready| {
                    (now.saturating_duration_since(ready).as_secs_f64() / 0.75).min(1.)
                });
                self.motion
                    .sample(profile, seconds, frame.playing, fade, dt_ms)
            })
        } else {
            self.motion = Default::default();
            (1., 0.)
        };
        let enter = if reduce_motion {
            1.
        } else {
            cubic_bezier(0.22, 1., 0.36, 1.)((age / 0.8).clamp(0., 1.) as f32)
        };
        if !reduce_motion
            && window.is_visible()
            && (frame.backdrop.rest.is_some()
                || enter < 1.
                || (frame.settings.enabled && (frame.playing || self.motion.active())))
        {
            window.request_animation_frame();
        }
        let scale = scale * (0.965 + 0.035 * enter);
        let lift = lift - 10. * (1. - enter);
        let viewport = window.viewport_size();
        let art_size = (f32::from(viewport.height) * 0.58)
            .min(f32::from(viewport.width) * 0.54)
            .max(256.);
        let drift = if reduce_motion {
            0.
        } else {
            (1. - (age as f32 * std::f32::consts::PI / 28.).cos()) / 2.
        };
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .bg(linear_gradient(
                135.,
                linear_color_stop(rgb(frame.palette.colors[0]), 0.),
                linear_color_stop(rgb(frame.palette.colors[1]), 1.),
            ))
            .when_some(frame.backdrop.rest.clone(), |el, backdrop| {
                el.child(
                    img(backdrop)
                        .absolute()
                        .left(relative(-0.25 * drift))
                        .top(relative(-0.25 * drift * 0.7))
                        .w(relative(1.25))
                        .h(relative(1.25))
                        .object_fit(ObjectFit::Cover),
                )
            })
            .children((0..3).filter_map(|i| {
                let active = frame.backdrop.bands[i].clone();
                let peak = frame.backdrop.peaks[i].clone();
                if active.is_none() && peak.is_none() {
                    return None;
                }
                let energy = self.motion.energy[i] as f32;
                let spread = self.motion.spread[i] as f32;
                let (ox, oy, direction) =
                    [(0.42, 1., -0.12), (0.72, 0.5, -0.02), (0.30, 0., 0.12)][i];
                let drift = if reduce_motion {
                    0.5
                } else {
                    (1. - ((age as f32 + [0., 11., 21.][i]) * std::f32::consts::PI / 32.).cos())
                        / 2.
                };
                Some(
                    div()
                        .absolute()
                        .left(relative(
                            -0.30 - 1.04 * spread * ox + (-0.03 + 0.06 * drift) * 1.6,
                        ))
                        .top(relative(
                            -0.30 - 1.04 * spread * oy + spread * direction + 0.02 * drift,
                        ))
                        .w(relative(1.6 + spread * 1.04))
                        .h(relative(1.6 + spread * 1.04))
                        .opacity((0.14 + energy * 0.72) * 0.9)
                        .when_some(active, |el, image| {
                            el.child(img(image).absolute().inset_0().size_full())
                        })
                        .when_some(peak, |el, image| {
                            el.child(
                                img(image)
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .opacity(energy * 0.86),
                            )
                        }),
                )
            }))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_10()
                    .pt_12()
                    .pb(px(176.))
                    .child(
                        div()
                            .id("fullscreen-artwork")
                            .test_support()
                            .role(Role::Image)
                            .aria_label("Track artwork")
                            .size(px(art_size))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .id("fullscreen-bounce-cover")
                                    .test_support()
                                    .size(px(art_size * scale))
                                    .flex_shrink_0()
                                    .relative()
                                    .top(px(-lift))
                                    .rounded(px(16.))
                                    .bg(rgb(CARD))
                                    .shadow_lg()
                                    .border_1()
                                    .border_color(rgba(0xffffff1a))
                                    .overflow_hidden()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(frame.image.is_none(), |el| {
                                        el.child(super::views::glyph(
                                            gpui_kit::assets::IconName::Music,
                                            80.,
                                        ))
                                    })
                                    .when_some(frame.image.clone(), |el, image| {
                                        el.child(
                                            img(image)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                                .rounded(px(16.)),
                                        )
                                    }),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[gpui_kit::test]
    fn thumbnail_cache_evicts_the_least_recently_used_cover(cx: &mut TestAppContext) {
        struct CoverSweep;

        impl Render for CoverSweep {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let mut cache = ThumbnailCache::default();
                let resource = |index| Resource::Embedded(format!("fixture-cover-{index}").into());
                for index in 0..MAX_DECODED_COVERS {
                    let _ = cache.load(&resource(index), window, cx);
                }
                let _ = cache.load(&resource(0), window, cx);
                let _ = cache.load(&resource(MAX_DECODED_COVERS), window, cx);
                assert_eq!(cache.images.len(), MAX_DECODED_COVERS);
                assert!(cache.images.contains_key(&resource(0)));
                assert!(!cache.images.contains_key(&resource(1)));
                Empty
            }
        }

        let (_, cx) = cx.add_window_view(|_, _| CoverSweep);
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[test]
    fn profile_interpolation_clamps_and_handles_missing_data() {
        assert_eq!(sample(&[], 0., 50), 0.);
        assert_eq!(sample(&[0, 255], 0.025, 50), 0.5);
        assert_eq!(sample(&[0, 255], 100., 50), 0.);
        assert_eq!(sample(&[128], -1., 50), 128. / 255.);
        assert_eq!(sample(&[255], f64::NAN, 50), 0.);
        assert_eq!(sample(&[255], 1., 0), 0.);
    }
    #[test]
    fn palette_completion_keeps_three_distinct_tones_for_monochrome_art() {
        assert_eq!(complete(&[]), Vec::<[u8; 3]>::new());
        assert_eq!(
            complete(&[[0, 0, 0]]),
            vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]]
        );
        assert_eq!(complete(&[[0, 0, 0], [255, 255, 255]])[2], [128, 128, 128]);
        assert_eq!(
            complete(&[[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]]).len(),
            3
        );
    }
}

#[cfg(test)]
mod quality_tests {
    use super::prefer_thumbnail;
    use std::path::Path;
    #[test]
    fn late_remote_response_cannot_replace_id3_cover() {
        let small = Path::new("track.jpg");
        let full = Path::new("track.full.jpg");
        let embedded = Path::new("track.embed.jpg");
        assert!(prefer_thumbnail(None, small));
        assert!(prefer_thumbnail(Some(small), full));
        assert!(prefer_thumbnail(Some(full), embedded));
        assert!(!prefer_thumbnail(Some(embedded), full));
        assert!(!prefer_thumbnail(Some(embedded), small));
    }
}

impl super::PlayerApp {
    pub(super) fn fullscreen_artwork_pending(&self, cx: &App) -> bool {
        let Some(id) = self.playback.read(cx).track_id else {
            return false;
        };
        if self.thumb_loading.contains(&id)
            && self.thumbs.get(&id).and_then(Option::as_ref).is_none()
        {
            return true;
        }
        let path = self.thumbs.get(&id).and_then(Option::as_ref);
        path.is_some()
            && !(self.prepared_artwork_track == Some(id)
                && (self.fullscreen_cover.is_some() || self.palette_path.as_ref() == path))
    }
    pub(super) fn prepare_fullscreen_artwork(
        &mut self,
        id: Option<i64>,
        path: Option<PathBuf>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.prepared_artwork_track != id {
            self.prepared_artwork_track = id;
            self.preparing_artwork_path = None;
            self.palette_path = None;
            self.palette = Default::default();
            release_texture(self.fullscreen_cover.take(), window, cx);
            release_background(std::mem::take(&mut self.fullscreen_backdrop), window, cx);
        }
        let (Some(id), Some(path)) = (id, path) else {
            return;
        };
        if self.palette_path.as_ref() == Some(&path)
            || self.preparing_artwork_path.as_ref() == Some(&path)
        {
            return;
        }
        self.preparing_artwork_path = Some(path.clone());
        let requested = path.clone();
        let handle = window.window_handle();
        self.quiet_task(
            cx,
            async move {
                Ok::<_, crate::error::AppError>(
                    crate::runtime::spawn_blocking(move || prepare(&path))
                        .await
                        .ok()
                        .flatten(),
                )
            },
            move |app, prepared, cx| {
                if app.prepared_artwork_track != Some(id)
                    || app.preparing_artwork_path.as_ref() != Some(&requested)
                {
                    return;
                }
                app.preparing_artwork_path = None;
                app.palette_path = Some(requested);
                if let Some(prepared) = prepared {
                    let old_cover = app.fullscreen_cover.replace(prepared.cover);
                    let old_background =
                        std::mem::replace(&mut app.fullscreen_backdrop, prepared.background);
                    app.palette = prepared.palette;
                    let _ = handle.update(cx, move |_, window, cx| {
                        release_texture(old_cover, window, cx);
                        release_background(old_background, window, cx);
                    });
                }
            },
        );
    }
}
