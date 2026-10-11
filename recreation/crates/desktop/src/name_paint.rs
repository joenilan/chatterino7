//! Bounded compositing of the existing shaped author run; text geometry stays authoritative.
use crate::seven_paints::Paint;
use gpui_kit::*;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Request {
    pub paint: Paint,
    pub cache: Rc<RefCell<Cache>>,
    /// Decoded image for image paints; animated paints advance with its frames.
    pub image: Option<Arc<crate::media::DecodedMedia>>,
}
struct Drawing {
    image: Arc<RenderImage>,
    offset: (i32, i32),
    width: u32,
    height: u32,
}
struct Entry {
    drawing: Option<Drawing>,
    used: u64,
    bytes: usize,
}
pub struct Cache {
    entries: HashMap<String, Entry>,
    used: u64,
    bytes: usize,
    epoch: Instant,
    created: usize,
    retired: Vec<Arc<RenderImage>>,
    retired_bytes: usize,
    retirement_pending: bool,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            used: 0,
            bytes: 0,
            epoch: Instant::now(),
            created: 0,
            retired: Vec::new(),
            retired_bytes: 0,
            retirement_pending: false,
        }
    }
}
impl Request {
    pub fn draw(
        &self,
        line: &ShapedLine,
        origin: Point<Pixels>,
        height: Pixels,
        base: u32,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if self.paint.limitation.is_some()
            || line.runs.iter().map(|r| r.glyphs.len()).sum::<usize>() > 64
        {
            return false;
        }
        let scale = window.scale_factor();
        if !scale.is_finite() || scale <= 0. || scale > 4. {
            return false;
        }
        let (ox, oy) = (f32::from(origin.x) * scale, f32::from(origin.y) * scale);
        let (ax, ay) = (ox.floor() as i32, oy.floor() as i32);
        // Negative raster positions have platform-specific tie behavior; retain plain text.
        if ox < 0. || oy < 0. {
            return false;
        }
        let phase = (ox - ax as f32, oy - ay as f32);
        // Image paints draw the current animation frame; reduced motion pins frame 0.
        let frame = match (&self.paint.image(), &self.image) {
            (None, _) => None,
            (Some(_), None) => return false,
            (Some(_), Some(media)) => {
                let index = media.frame(cx.reduce_motion());
                if media.image.frame_count() > 1 && !cx.reduce_motion() {
                    window.request_animation_frame();
                }
                Some(index)
            }
        };
        let key = format!(
            "{:?}|{frame:?}|{base}|{}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            self.paint,
            scale.to_bits(),
            f32::from(height).to_bits(),
            phase,
            line.font_size,
            line.width,
            line.ascent,
            line.descent,
            line.runs
        );
        let mut cache = self.cache.borrow_mut();
        cache.used = cache.used.wrapping_add(1);
        let used = cache.used;
        if !cache.entries.contains_key(&key) {
            if cache.epoch.elapsed() >= Duration::from_millis(16) {
                cache.epoch = Instant::now();
                cache.created = 0;
            }
            // Animated paints may build one frame per name each tick.
            if cache.created >= 2 + 6 * usize::from(frame.is_some()) {
                window.request_animation_frame();
                return false;
            }
            cache.created += 1;
            let pixels = frame.and_then(|index| {
                let media = self.image.as_ref()?;
                let size = media.image.size(index);
                Some((media.image.as_bytes(index)?, size))
            });
            let image_frame = pixels.map(|(bytes, size)| crate::seven_paints::Frame {
                bytes,
                width: size.width.0.max(0) as u32,
                height: size.height.0.max(0) as u32,
            });
            let drawing = compose(
                &self.paint,
                line,
                origin,
                height,
                base,
                image_frame.as_ref(),
                window,
            );
            let bytes = drawing
                .as_ref()
                .map(|d| d.width as usize * d.height as usize * 4)
                .unwrap_or(0);
            if cache.bytes + cache.retired_bytes + bytes > 32 * 1024 * 1024 {
                return false;
            }
            while cache.entries.len() >= 256 || cache.bytes + bytes > 16 * 1024 * 1024 {
                let victim = cache
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.used)
                    .map(|(key, _)| key.clone());
                let Some(victim) = victim else {
                    break;
                };
                if let Some(old) = cache.entries.remove(&victim) {
                    cache.bytes = cache.bytes.saturating_sub(old.bytes);
                    if let Some(d) = old.drawing {
                        cache.retired_bytes += old.bytes;
                        cache.retired.push(d.image);
                    }
                }
            }
            if !cache.retired.is_empty() && !cache.retirement_pending {
                cache.retirement_pending = true;
                let shared = self.cache.clone();
                cx.defer(move |cx| {
                    let images = {
                        let mut cache = shared.borrow_mut();
                        cache.retirement_pending = false;
                        cache.retired_bytes = 0;
                        std::mem::take(&mut cache.retired)
                    };
                    for image in images {
                        cx.drop_image(image, None);
                    }
                    // Cached GPUI paint ranges can contain old atlas tiles. Force fresh scenes.
                    cx.refresh_windows();
                });
            }
            cache.bytes += bytes;
            cache.entries.insert(
                key.clone(),
                Entry {
                    drawing,
                    used,
                    bytes,
                },
            );
        }
        let entry = cache.entries.get_mut(&key).unwrap();
        entry.used = used;
        let Some(drawing) = &entry.drawing else {
            return false;
        };
        let bounds = Bounds::new(
            point(
                px((ax + drawing.offset.0) as f32 / scale),
                px((ay + drawing.offset.1) as f32 / scale),
            ),
            size(
                px(drawing.width as f32 / scale),
                px(drawing.height as f32 / scale),
            ),
        );
        window
            .paint_image(
                bounds,
                bounds,
                Corners::all(px(0.)),
                drawing.image.clone(),
                0,
                false,
            )
            .is_ok()
    }
}
fn quantize(value: f32, variants: f32) -> f32 {
    ((value * variants).abs() - 0.5).ceil().copysign(value) / variants
}
fn compose(
    paint: &Paint,
    line: &ShapedLine,
    origin: Point<Pixels>,
    height: Pixels,
    base: u32,
    frame: Option<&crate::seven_paints::Frame>,
    window: &Window,
) -> Option<Drawing> {
    let scale = window.scale_factor();
    let ox = f32::from(origin.x) * scale;
    let oy = f32::from(origin.y) * scale;
    let (ax, ay) = (ox.floor() as i32, oy.floor() as i32);
    let baseline = origin.y + (height - line.ascent - line.descent) / 2. + line.ascent;
    let mut masks = Vec::new();
    let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for run in &line.runs {
        for glyph in &run.glyphs {
            if glyph.is_emoji {
                return None;
            }
            let gx = quantize(f32::from(origin.x + glyph.position.x) * scale, 4.);
            let gy = quantize(f32::from(baseline + glyph.position.y) * scale, 1.);
            let params = RenderGlyphParams {
                font_id: run.font_id,
                glyph_id: glyph.id,
                font_size: line.font_size,
                subpixel_variant: point((gx.fract() * 4.) as u8, gy.fract() as u8),
                scale_factor: scale,
                is_emoji: false,
                subpixel_rendering: false,
                dilation: 0,
            };
            let (offset, actual, bytes) = window
                .text_system()
                .rasterize_grayscale_glyph(&params, 128 * 1024)
                .ok()?;
            let (w, h) = (i32::from(actual.width), i32::from(actual.height));
            if w == 0 || h == 0 {
                continue;
            }
            let x = gx.trunc() as i32 + i32::from(offset.x) - ax;
            let y = gy.trunc() as i32 + i32::from(offset.y) - ay;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + w);
            bottom = bottom.max(y + h);
            if right - left > 2048 || bottom - top > 512 {
                return None;
            }
            masks.push((x, y, w, h, bytes));
        }
    }
    if masks.is_empty() {
        return None;
    }
    // Room around the text for the glow: offset plus about three blur deviations.
    let margin = paint
        .shadows
        .iter()
        .map(|s| ((s.x.abs().max(s.y.abs()) + s.radius * 1.5) * scale as f64).ceil() as i32)
        .max()
        .unwrap_or(0)
        .clamp(0, 32);
    let (left, top, right, bottom) = (left - margin, top - margin, right + margin, bottom + margin);
    let (width, height_px) = ((right - left) as usize, (bottom - top) as usize);
    if width.checked_mul(height_px)? > 1024 * 1024 {
        return None;
    }
    let mut coverage = vec![0u8; width * height_px];
    for (x, y, w, h, bytes) in masks {
        for row in 0..h {
            for col in 0..w {
                let at = (y - top + row) as usize * width + (x - left + col) as usize;
                let alpha = bytes[(row * w + col) as usize] as u16;
                let old = coverage[at] as u16;
                coverage[at] = (old + alpha - old * alpha / 255).min(255) as u8;
            }
        }
    }
    // Premultiplied RGBA in 0..1.
    let mut pixels = vec![[0f32; 4]; width * height_px];
    for y in 0..height_px {
        for x in 0..width {
            let cover = coverage[y * width + x];
            if cover == 0 {
                continue;
            }
            let local_x = (ax + left + x as i32) as f64 + 0.5 - ox as f64;
            let local_y = (ay + top + y as i32) as f64 + 0.5 - oy as f64;
            let rgb = paint.sample(
                local_x / scale as f64,
                local_y / scale as f64,
                f32::from(line.width()) as f64,
                f32::from(height) as f64,
                base,
                frame,
            )?;
            let a = cover as f32 / 255.;
            pixels[y * width + x] = [
                ((rgb >> 16) & 255) as f32 / 255. * a,
                ((rgb >> 8) & 255) as f32 / 255. * a,
                (rgb & 255) as f32 / 255. * a,
                a,
            ];
        }
    }
    // CSS chains `drop-shadow` filters: each shadow is cast by everything drawn so far.
    for shadow in &paint.shadows {
        let mut alpha: Vec<f32> = pixels.iter().map(|p| p[3]).collect();
        blur(
            &mut alpha,
            width,
            height_px,
            shadow.radius as f32 / 2. * scale,
        );
        let (dx, dy) = (
            (shadow.x as f32 * scale).round() as i32,
            (shadow.y as f32 * scale).round() as i32,
        );
        let color = [
            ((shadow.rgba >> 24) & 255) as f32 / 255.,
            ((shadow.rgba >> 16) & 255) as f32 / 255.,
            ((shadow.rgba >> 8) & 255) as f32 / 255.,
            (shadow.rgba & 255) as f32 / 255.,
        ];
        for y in 0..height_px as i32 {
            for x in 0..width as i32 {
                let (sx, sy) = (x - dx, y - dy);
                if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height_px as i32 {
                    continue;
                }
                let cast = alpha[sy as usize * width + sx as usize] * color[3];
                if cast <= 0. {
                    continue;
                }
                let p = &mut pixels[y as usize * width + x as usize];
                let keep = 1. - p[3];
                p[0] += color[0] * cast * keep;
                p[1] += color[1] * cast * keep;
                p[2] += color[2] * cast * keep;
                p[3] += cast * keep;
            }
        }
    }
    let mut bytes = Vec::with_capacity(width * height_px * 4);
    for [r, g, b, a] in pixels {
        let (r, g, b) = if a > 0. {
            (r / a, g / a, b / a)
        } else {
            (0., 0., 0.)
        };
        let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
        bytes.extend_from_slice(&[byte(b), byte(g), byte(r), byte(a)]);
    }
    let image = image::RgbaImage::from_raw(width as u32, height_px as u32, bytes)?;
    Some(Drawing {
        image: Arc::new(RenderImage::new(vec![image::Frame::new(image)])),
        offset: (left, top),
        width: width as u32,
        height: height_px as u32,
    })
}

/// Approximate Gaussian blur (three box passes) of a single channel in place.
fn blur(values: &mut [f32], width: usize, height: usize, sigma: f32) {
    if sigma < 0.5 || width == 0 || height == 0 {
        return;
    }
    // Box widths whose three passes approximate the requested deviation.
    let ideal = (12. * sigma * sigma / 3. + 1.).sqrt();
    let mut lower = ideal.floor() as usize;
    if lower.is_multiple_of(2) {
        lower = lower.saturating_sub(1).max(1);
    }
    let upper = lower + 2;
    let m = ((12. * sigma * sigma - 3. * (lower * lower) as f32 - 12. * lower as f32 - 9.)
        / (-4. * lower as f32 - 4.))
        .round()
        .max(0.) as usize;
    let mut scratch = vec![0f32; values.len()];
    for pass in 0..3 {
        let radius = (if pass < m { lower } else { upper }) / 2;
        box_pass(values, &mut scratch, width, height, radius, true);
        box_pass(&scratch, values, width, height, radius, false);
    }
}
fn box_pass(
    src: &[f32],
    dst: &mut [f32],
    width: usize,
    height: usize,
    radius: usize,
    horizontal: bool,
) {
    let (lines, length) = if horizontal {
        (height, width)
    } else {
        (width, height)
    };
    let at = |line: usize, i: usize| {
        if horizontal {
            line * width + i
        } else {
            i * width + line
        }
    };
    let span = (2 * radius + 1) as f32;
    for line in 0..lines {
        let mut sum = 0f32;
        for i in 0..=radius.min(length - 1) {
            sum += src[at(line, i)];
        }
        for i in 0..length {
            dst[at(line, i)] = sum / span;
            if i + radius + 1 < length {
                sum += src[at(line, i + radius + 1)];
            }
            if i >= radius {
                sum -= src[at(line, i - radius)];
            }
        }
    }
}
