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
        let key = format!(
            "{:?}|{base}|{}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
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
            if cache.created >= 2 {
                window.request_animation_frame();
                return false;
            }
            cache.created += 1;
            let drawing = compose(&self.paint, line, origin, height, base, window);
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
    let (width, height_px) = ((right - left) as usize, (bottom - top) as usize);
    if width.checked_mul(height_px)? > 512 * 1024 {
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
    let mut pixels = Vec::with_capacity(width * height_px * 4);
    for y in 0..height_px {
        for x in 0..width {
            let local_x = (ax + left + x as i32) as f64 + 0.5 - ox as f64;
            let local_y = (ay + top + y as i32) as f64 + 0.5 - oy as f64;
            let rgb = paint.sample(
                local_x / scale as f64,
                local_y / scale as f64,
                f32::from(line.width()) as f64,
                f32::from(height) as f64,
                base,
            )?;
            pixels.extend_from_slice(&[
                (rgb & 255) as u8,
                ((rgb >> 8) & 255) as u8,
                ((rgb >> 16) & 255) as u8,
                coverage[y * width + x],
            ]);
        }
    }
    let image = image::RgbaImage::from_raw(width as u32, height_px as u32, pixels)?;
    Some(Drawing {
        image: Arc::new(RenderImage::new(vec![image::Frame::new(image)])),
        offset: (left, top),
        width: width as u32,
        height: height_px as u32,
    })
}
