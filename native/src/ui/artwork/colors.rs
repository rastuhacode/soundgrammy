//! Artwork sampling and color layers. Image work runs on the blocking executor,
//! once per cover; animation only composites cached GPU textures.
use gpui_kit::RenderImage;
use std::{collections::HashMap, sync::Arc};
#[derive(Clone, Copy, PartialEq)]
pub(in crate::ui) struct Palette {
    pub colors: [u32; 3],
    pub accent: Option<u32>,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            colors: [0x141414, 0x202020, 0x272727],
            accent: None,
        }
    }
}
fn pack(c: [u8; 3]) -> u32 {
    (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}
fn unpack(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}
fn hue(c: [u8; 3]) -> f64 {
    let [r, g, b] = c.map(|v| v as f64 / 255.);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    if delta == 0. {
        return 0.;
    }
    let sector = if max == r {
        (g - b) / delta
    } else if max == g {
        (b - r) / delta + 2.
    } else {
        (r - g) / delta + 4.
    };
    (sector * 60.).rem_euclid(360.)
}
#[derive(Default)]
struct Bucket {
    color: [u8; 3],
    count: u32,
    sum: [u32; 3],
    chroma: u32,
}
pub(super) fn extract(image: &image::RgbaImage) -> Palette {
    let mut buckets: Vec<Bucket> = vec![];
    let mut positions = HashMap::new();
    let mut groups: [([u32; 3], u32, u32); 12] = Default::default();
    for pixel in image.pixels().filter(|p| p[3] >= 180) {
        let c = [pixel[0], pixel[1], pixel[2]];
        let chroma = *c.iter().max().unwrap() - *c.iter().min().unwrap();
        let color = c.map(|v| ((v as f64 / 32.).round() * 32.).min(255.) as u8);
        let index = *positions.entry(color).or_insert_with(|| {
            buckets.push(Bucket {
                color,
                ..Default::default()
            });
            buckets.len() - 1
        });
        let bucket = &mut buckets[index];
        bucket.count += 1;
        for (sum, v) in bucket.sum.iter_mut().zip(c) {
            *sum += v as u32;
        }
        bucket.chroma += chroma as u32;
        if chroma >= 12 {
            let group = &mut groups[(hue(c) / 30.).round() as usize % 12];
            for (sum, v) in group.0.iter_mut().zip(c) {
                *sum += v as u32;
            }
            group.1 += 1;
            group.2 += chroma as u32;
        }
    }
    let minimum = (image.width() * image.height()) as f64 * 0.02;
    let minimum = minimum.ceil() as u32;
    buckets.sort_by_key(|b| std::cmp::Reverse(b.count));
    let preferred: Vec<[u8; 3]> = buckets
        .iter()
        .filter_map(|b| {
            if *b.color.iter().max().unwrap() <= 32 {
                if b.count < minimum {
                    return None;
                }
                let average = b.sum.map(|v| (v as f64 / b.count as f64).round() as u8);
                let max = *average.iter().max().unwrap() as f64;
                let min = *average.iter().min().unwrap() as f64;
                if max - min
                    >= 3f64
                        .max((max + min) * 0.08)
                        .max(b.chroma as f64 / b.count as f64 * 0.65)
                {
                    return Some(average);
                }
            }
            let brightness = b.color.iter().map(|v| *v as f64).sum::<f64>() / 3.;
            (12. ..=244.).contains(&brightness).then_some(b.color)
        })
        .collect();
    let candidates = if preferred.is_empty() {
        buckets.iter().map(|b| b.color).collect::<Vec<_>>()
    } else {
        preferred
    };
    let mut selected: Vec<[u8; 3]> = vec![];
    for c in candidates {
        if selected.iter().all(|v| {
            v.iter()
                .zip(c)
                .map(|(a, b)| (*a as f64 - b as f64).powi(2))
                .sum::<f64>()
                .sqrt()
                >= 72.
        }) {
            selected.push(c);
        }
        if selected.len() == 3 {
            break;
        }
    }
    let visible: Vec<f64> = selected
        .iter()
        .filter(|c| *c.iter().max().unwrap() - *c.iter().min().unwrap() >= 16)
        .map(|c| hue(*c))
        .collect();
    let mut accents: Vec<_> = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| g.1 >= minimum)
        .collect();
    accents.sort_by_key(|(_, g)| std::cmp::Reverse(g.2));
    let accent = if visible.len() >= 2 {
        None
    } else {
        accents
            .iter()
            .find(|(i, _)| {
                visible.iter().all(|h| {
                    let d = (*i as f64 * 30. - h).abs();
                    d.min(360. - d) >= 30.
                })
            })
            .or(accents.first())
            .map(|(_, g)| pack(g.0.map(|v| (v as f64 / g.1 as f64).round() as u8)))
    };
    let colors = super::complete(&selected);
    if colors.len() != 3 {
        return Palette::default();
    }
    Palette {
        colors: std::array::from_fn(|i| pack(colors[i])),
        accent,
    }
}
fn hsl(c: [u8; 3]) -> (f64, f64, f64) {
    let max = *c.iter().max().unwrap() as f64 / 255.;
    let min = *c.iter().min().unwrap() as f64 / 255.;
    let light = (max + min) / 2.;
    let delta = max - min;
    (
        hue(c),
        if delta == 0. {
            0.
        } else {
            delta / (1. - (2. * light - 1.).abs())
        },
        light,
    )
}
fn tone(h: f64, s: f64, l: f64) -> [f32; 3] {
    let chroma = (1. - (2. * l - 1.).abs()) * s;
    let x = chroma * (1. - ((h / 60.) % 2. - 1.).abs());
    let m = l - chroma / 2.;
    let c = match h as u32 {
        0..=59 => [chroma, x, 0.],
        60..=119 => [x, chroma, 0.],
        120..=179 => [0., chroma, x],
        180..=239 => [0., x, chroma],
        240..=299 => [x, 0., chroma],
        _ => [chroma, 0., x],
    };
    c.map(|v| (v + m) as f32)
}
#[cfg(any(test, not(target_os = "macos")))]
fn luminosity(c: [f32; 3]) -> f32 {
    c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11
}
/// W3C nonseparable color blend: retain the backdrop's luminosity.
#[cfg(any(test, not(target_os = "macos")))]
fn color_blend(mut c: [f32; 3], backdrop: [f32; 3]) -> [f32; 3] {
    let d = luminosity(backdrop) - luminosity(c);
    for v in &mut c {
        *v += d;
    }
    let l = luminosity(c);
    let n = c.iter().copied().fold(f32::INFINITY, f32::min);
    let x = c.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if n < 0. {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1. {
        c = c.map(|v| l + (v - l) * (1. - l) / (x - l));
    }
    c
}
#[derive(Clone, PartialEq, Default)]
pub(in crate::ui) struct Background {
    pub rest: Option<Arc<RenderImage>>,
    pub bands: [Option<Arc<RenderImage>>; 3],
    pub peaks: [Option<Arc<RenderImage>>; 3],
    #[cfg(target_os = "macos")]
    pub gpu: Option<gpui_apple::effects::Image>,
}
#[cfg(not(target_os = "macos"))]
fn texture(mut image: image::RgbaImage) -> Arc<RenderImage> {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
}
fn color_range(c: u32, recovered: bool) -> ([f32; 3], Option<[f32; 3]>, Option<[f32; 3]>) {
    let (h, s, l) = hsl(unpack(c));
    let level = (l * 0.65).min(0.36);
    let rest = s * 0.45;
    if s < 0.05 {
        return (tone(h, rest, level), None, None);
    }
    let chroma = 2. * level;
    let ambient = rest * chroma;
    let base_peak = ((s * 1.35 + 0.25).min(1.) * chroma).max(ambient + 0.10 * (s / 0.25).min(1.));
    let peak = base_peak
        + if recovered {
            (0.40 - base_peak).max(0.) * 0.75
        } else {
            0.
        };
    let active = ((s * 1.05 + 0.12).min(1.) * chroma).max(ambient + (base_peak - ambient) * 0.55);
    (
        tone(h, rest, level),
        Some(tone(h, active, 0.5)),
        Some(tone(h, peak, 0.5)),
    )
}

#[cfg(not(target_os = "macos"))]
pub(in crate::ui) fn background(path: &std::path::Path, palette: Palette) -> Background {
    let Ok(image) = image::open(path) else {
        return Background::default();
    };
    let image = image
        .resize_to_fill(256, 160, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let image = image::imageops::blur(&image, 11.);
    let accent = palette.accent.and_then(|c| color_range(c, true).2);
    let ranges = palette.colors.map(|c| {
        let (rest, active, peak) = color_range(c, false);
        (rest, active, peak.or(accent))
    });
    let mut rest = image::RgbaImage::new(256, 160);
    for (x, y, pixel) in rest.enumerate_pixels_mut() {
        let original = image.get_pixel(x, y);
        let l = (original[0] as f32 + original[1] as f32 + original[2] as f32) / 3.;
        let mut c: [f32; 3] = std::array::from_fn(|i| {
            ((l + (original[i] as f32 - l) * 1.25).clamp(0., 255.) * 0.5 + 10.) / 255.
        });
        for (index, cx, cy, radius) in [
            (2, 0.45, 1., 0.75),
            (1, 0.78, 0.48, 0.72),
            (0, 0.22, 0.08, 0.72),
        ] {
            let dx = x as f32 / 256. - cx;
            let dy = y as f32 / 160. - cy;
            let a = (1. - (dx * dx + dy * dy).sqrt() / radius).clamp(0., 1.) * 0.9;
            for (i, v) in c.iter_mut().enumerate() {
                *v = *v * (1. - a) + ranges[index].0[i] * a;
            }
        }
        pixel.0 = [
            (c[0] * 255. * 0.55) as u8,
            (c[1] * 255. * 0.55) as u8,
            (c[2] * 255. * 0.55) as u8,
            255,
        ];
    }
    let make_layer = |i: usize, peak: bool| {
        let range = ranges[2 - i];
        let active = if peak { range.2? } else { range.1? };
        let mut layer = rest.clone();
        let (cx, cy) = [(0.42, 1.), (0.72, 0.5), (0.30, 0.)][i];
        for (x, y, pixel) in layer.enumerate_pixels_mut() {
            let base = std::array::from_fn(|j| pixel[j] as f32 / 255.);
            let blend = color_blend(active, base);
            let dx = (x as f32 / 256. - cx) / 0.85;
            let dy = (y as f32 / 160. - cy) / 0.60;
            let alpha = (1. - (dx * dx + dy * dy).sqrt() / 0.85).clamp(0., 1.);
            pixel.0 = [
                (blend[0] * 255.) as u8,
                (blend[1] * 255.) as u8,
                (blend[2] * 255.) as u8,
                (alpha * 255.) as u8,
            ];
        }
        Some(texture(layer))
    };
    let bands = std::array::from_fn(|i| make_layer(i, false));
    let peaks = std::array::from_fn(|i| make_layer(i, true));
    Background {
        rest: Some(texture(rest)),
        bands,
        peaks,
        #[cfg(target_os = "macos")]
        gpu: None,
    }
}
#[cfg(target_os = "macos")]
pub(super) fn gpu_uniforms(palette: Palette) -> gpui_apple::effects::PaletteUniforms {
    let accent = palette.accent.and_then(|c| color_range(c, true).2);
    let ranges = palette.colors.map(|c| {
        let (rest, active, peak) = color_range(c, false);
        (rest, active, peak.or(accent))
    });
    let rgba = |c: Option<[f32; 3]>| c.map_or([0.; 4], |v| [v[0], v[1], v[2], 1.]);
    gpui_apple::effects::PaletteUniforms {
        rest: ranges.map(|range| rgba(Some(range.0))),
        active: std::array::from_fn(|i| rgba(ranges[2 - i].1)),
        peak: std::array::from_fn(|i| rgba(ranges[2 - i].2)),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{color_blend, extract, luminosity, unpack};
    #[test]
    fn repeated_shadow_tint_survives_quantization() {
        let image = image::RgbaImage::from_pixel(48, 48, image::Rgba([3, 12, 9, 255]));
        let p = extract(&image);
        assert_eq!(p.colors[0], 0x030c09);
        assert!(p
            .colors
            .iter()
            .all(|c| unpack(*c).iter().copied().max().unwrap() < 48));
    }
    #[test]
    fn repeated_accent_survives_gray_majority() {
        let mut image = image::RgbaImage::from_pixel(48, 48, image::Rgba([128, 128, 128, 255]));
        for i in 0..100 {
            image.put_pixel(i % 48, i / 48, image::Rgba([100, 20, 40, 255]));
        }
        assert_eq!(extract(&image).accent, Some(0x641428));
    }
    #[test]
    fn color_blend_preserves_luminosity() {
        let b = [0.05, 0.08, 0.09];
        let c = color_blend([0.8, 0.1, 0.3], b);
        assert!((luminosity(c) - luminosity(b)).abs() < 0.0001);
    }
}
