//! Observed 7TV paints: definitions never imply an active sender entitlement.
//!
//! A paint is one or more layers (linear/radial gradients or an image, the first
//! layer on top, like CSS background layers) plus chained drop-shadows that
//! give 7TV names their glow. Rendering lives in name_paint.rs.
use crate::seven_entitlements::Change;
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub at: f64,
    pub rgba: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Brush {
    Linear {
        angle: f64,
    },
    Radial {
        ellipse: bool,
    },
    Solid,
    /// Static or animated image from cdn.7tv.app, clipped to the text.
    Image {
        url: String,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub brush: Brush,
    pub stops: Vec<Stop>,
    pub repeat: bool,
}
/// CSS `drop-shadow(x y radius color)`, applied in order to the result so far.
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub rgba: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Paint {
    pub id: String,
    pub name: String,
    pub color: Option<u32>,
    pub layers: Vec<Layer>,
    pub shadows: Vec<Shadow>,
    /// Why the paint cannot be drawn at all; the name falls back to its color.
    pub limitation: Option<String>,
    /// Drawn, but part of the 7TV effect is omitted.
    pub approximation: Option<String>,
}
/// One decoded image frame (BGRA) for image layers.
pub struct Frame<'a> {
    pub bytes: &'a [u8],
    pub width: u32,
    pub height: u32,
}
fn packed(value: &Value) -> Option<u32> {
    value
        .as_i64()
        .filter(|n| *n >= i32::MIN as i64 && *n <= u32::MAX as i64)
        .map(|n| n as u32)
}
fn image_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("cdn.7tv.app")
        && url.path().starts_with("/paint/")
        && !url.path().contains("..")
        && url.query().is_none()
        && value.len() <= 512)
        .then(|| value.to_owned())
}
fn layer(data: &Value, color: Option<u32>) -> Result<Layer, &'static str> {
    let brush = match data["function"].as_str().unwrap_or("") {
        "LINEAR_GRADIENT" | "linear-gradient" => {
            let angle = data["angle"].as_f64().unwrap_or(0.);
            if !angle.is_finite() {
                return Err("Invalid gradient angle");
            }
            Brush::Linear {
                angle: angle.rem_euclid(360.),
            }
        }
        "RADIAL_GRADIENT" | "radial-gradient" => Brush::Radial {
            ellipse: data["shape"].as_str() == Some("ellipse"),
        },
        "URL" | "url" => Brush::Image {
            url: image_url(data["image_url"].as_str().unwrap_or(""))
                .ok_or("Invalid paint image")?,
        },
        _ => return Err("Unknown paint function"),
    };
    let mut stops = Vec::new();
    if let Some(list) = data["stops"].as_array() {
        if list.len() > 32 {
            return Err("More than 32 gradient stops");
        }
        for stop in list {
            let at = stop["at"]
                .as_f64()
                .filter(|at| at.is_finite() && (0.0..=1.0).contains(at))
                .ok_or("Invalid gradient stop")?;
            let rgba = packed(&stop["color"]).ok_or("Invalid gradient color")?;
            if stops.last().is_some_and(|last: &Stop| last.at > at) {
                return Err("Unordered gradient stops");
            }
            stops.push(Stop { at, rgba });
        }
    }
    let repeat = data["repeat"].as_bool().unwrap_or(false);
    let brush = match brush {
        Brush::Image { .. } => brush,
        _ if stops.is_empty() && color.is_some() && matches!(brush, Brush::Linear { .. }) => {
            Brush::Solid
        }
        _ if stops.is_empty() => return Err("Missing gradient stops"),
        _ if repeat && stops.last().unwrap().at - stops.first().unwrap().at <= f64::EPSILON => {
            return Err("Zero-width repeating gradient");
        }
        brush => brush,
    };
    Ok(Layer {
        brush,
        stops,
        repeat,
    })
}
pub fn definition(data: &Value) -> Option<Paint> {
    let id = data["id"]
        .as_str()
        .filter(|id| {
            !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric())
        })?
        .to_owned();
    let name = data["name"]
        .as_str()
        .unwrap_or("Unnamed paint")
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    let color = packed(&data["color"]);
    let mut paint = Paint {
        id,
        name,
        color,
        layers: Vec::new(),
        shadows: Vec::new(),
        limitation: None,
        approximation: None,
    };
    // The classic format puts one brush at the top level; layered paints list
    // theirs in `gradients` (first on top). Accept both.
    if data["function"].as_str().is_some_and(|f| !f.is_empty()) {
        match layer(data, color) {
            Ok(layer) => paint.layers.push(layer),
            Err(reason) => paint.limitation = Some(reason.into()),
        }
    }
    let mut skipped = 0;
    for data in data["gradients"].as_array().into_iter().flatten().take(8) {
        match layer(data, color) {
            Ok(layer) => paint.layers.push(layer),
            Err(_) => skipped += 1,
        }
    }
    if !paint.layers.is_empty() {
        paint.limitation = None;
    } else if paint.limitation.is_none() {
        paint.limitation = Some("No drawable paint layer".into());
    }
    for shadow in data["shadows"].as_array().into_iter().flatten().take(8) {
        let number = |key: &str| {
            shadow[key]
                .as_f64()
                .filter(|n| n.is_finite() && n.abs() <= 64.)
        };
        if let (Some(x), Some(y), Some(radius), Some(rgba)) = (
            number("x_offset"),
            number("y_offset"),
            number("radius"),
            packed(&shadow["color"]),
        ) {
            paint.shadows.push(Shadow {
                x,
                y,
                radius: radius.max(0.),
                rgba,
            });
        }
    }
    let mut omitted = Vec::new();
    if skipped > 0 {
        omitted.push("some layers");
    }
    if !data["text"].is_null() {
        omitted.push("text styling");
    }
    if !omitted.is_empty() {
        paint.approximation = Some(format!("Approximate: {} not drawn", omitted.join(", ")));
    }
    Some(paint)
}
#[derive(Default)]
pub struct Paints {
    previews: HashMap<String, std::sync::Arc<gpui_kit::RenderImage>>,
    definitions: HashMap<String, (Paint, Instant)>,
    users: HashMap<String, (String, Instant)>,
    last_expiry: Option<Instant>,
}
impl Paints {
    pub fn apply(&mut self, event: &Change) -> bool {
        match event {
            Change::Reset => {
                let changed = !self.users.is_empty();
                self.users.clear();
                self.definitions.clear();
                self.previews.clear();
                changed
            }
            Change::PaintDefinition(paint) => {
                if !self.definitions.contains_key(&paint.id) && self.definitions.len() >= 256 {
                    let victim = self
                        .definitions
                        .iter()
                        .filter(|(id, _)| !self.users.values().any(|(current, _)| current == *id))
                        .min_by_key(|(_, (_, at))| *at)
                        .map(|(id, _)| id.clone());
                    let Some(victim) = victim else {
                        return false;
                    };
                    self.definitions.remove(&victim);
                    self.previews.remove(&victim);
                }
                let changed = self
                    .definitions
                    .get(&paint.id)
                    .is_none_or(|(old, _)| old != paint);
                if changed {
                    self.previews.remove(&paint.id);
                    if let Some(preview) = paint.preview() {
                        self.previews.insert(paint.id.clone(), preview);
                    }
                }
                self.definitions
                    .insert(paint.id.clone(), (paint.clone(), Instant::now()));
                changed && self.users.values().any(|(id, _)| id == &paint.id)
            }
            Change::PaintGrant {
                users,
                paint,
                remove,
            } => {
                let mut changed = false;
                for user in users {
                    if *remove {
                        if self.users.get(user).is_some_and(|(id, _)| id == paint) {
                            self.users.remove(user);
                            changed = true;
                        }
                    } else if self.users.contains_key(user) || self.users.len() < 2048 {
                        changed |= self.users.get(user).is_none_or(|(id, _)| id != paint);
                        self.users
                            .insert(user.clone(), (paint.clone(), Instant::now()));
                    }
                }
                changed
            }
            _ => false,
        }
    }
    pub fn expire(&mut self) -> bool {
        if self
            .last_expiry
            .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
        {
            return false;
        }
        self.last_expiry = Some(Instant::now());
        let before = self.users.len();
        self.users
            .retain(|_, (_, at)| at.elapsed() < Duration::from_secs(1800));
        before != self.users.len()
    }
    pub fn for_user(&self, user: &str) -> Option<&Paint> {
        self.users
            .get(user)
            .and_then(|(id, _)| self.definitions.get(id))
            .map(|(paint, _)| paint)
    }
    pub fn preview_for_user(&self, user: &str) -> Option<std::sync::Arc<gpui_kit::RenderImage>> {
        self.users
            .get(user)
            .and_then(|(id, _)| self.previews.get(id))
            .cloned()
    }
    pub fn pending_for_user(&self, user: &str) -> bool {
        self.users.contains_key(user) && self.for_user(user).is_none()
    }
    pub fn inspection(&self) -> Value {
        serde_json::json!({
            "definitions":self.definitions.len(),"sender_grants":self.users.len(),
            "unresolved_grants":self.users.values().filter(|(id,_)|!self.definitions.contains_key(id)).count(),
            "unsupported_definitions":self.definitions.values().filter(|(p,_)|p.limitation.is_some()).count(),"approximated_definitions":self.definitions.values().filter(|(p,_)|p.limitation.is_none()&&p.approximation.is_some()).count(),
            "name_rendering":"glyph-mask layers, chained drop-shadows, animated image layers","scope":"observed session entitlements only"
        })
    }
}

impl Paint {
    /// The image URL of the first image layer, if any (loaded through the media cache).
    pub fn image(&self) -> Option<&str> {
        self.layers.iter().find_map(|l| match &l.brush {
            Brush::Image { url } => Some(url.as_str()),
            _ => None,
        })
    }
    /// Straight RGBA fill at a point: layers composited top-first over `base`
    /// (the name's own color). `None` while an image layer has no frame yet.
    pub fn sample(
        &self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        base: u32,
        frame: Option<&Frame>,
    ) -> Option<u32> {
        if width <= 0. || height <= 0. || ![x, y, width, height].iter().all(|n| n.is_finite()) {
            return None;
        }
        // Composite bottom-up: base, then the last layer, ..., the first layer on top.
        let (mut r, mut g, mut b) = (
            ((base >> 16) & 255) as f64,
            ((base >> 8) & 255) as f64,
            (base & 255) as f64,
        );
        for layer in self.layers.iter().rev() {
            let rgba = layer.sample(x, y, width, height, self.color, frame)?;
            let alpha = (rgba & 255) as f64 / 255.;
            r = ((rgba >> 24) & 255) as f64 * alpha + r * (1. - alpha);
            g = ((rgba >> 16) & 255) as f64 * alpha + g * (1. - alpha);
            b = ((rgba >> 8) & 255) as f64 * alpha + b * (1. - alpha);
        }
        Some(((r.round() as u32) << 16) | ((g.round() as u32) << 8) | b.round() as u32)
    }
    fn preview(&self) -> Option<std::sync::Arc<gpui_kit::RenderImage>> {
        if self.image().is_some() {
            return None;
        }
        let (width, height) = (192, 32);
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let rgb = self.sample(
                    x as f64 + 0.5,
                    y as f64 + 0.5,
                    width as f64,
                    height as f64,
                    crate::theme::CANVAS,
                    None,
                )?;
                pixels.extend_from_slice(&[
                    (rgb & 255) as u8,
                    ((rgb >> 8) & 255) as u8,
                    ((rgb >> 16) & 255) as u8,
                    255,
                ]);
            }
        }
        let buffer = image::RgbaImage::from_raw(width as u32, height as u32, pixels)?;
        Some(std::sync::Arc::new(gpui_kit::RenderImage::new(vec![
            image::Frame::new(buffer),
        ])))
    }
}
impl Layer {
    /// RGBA (0xRRGGBBAA) at a point.
    fn sample(
        &self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Option<u32>,
        frame: Option<&Frame>,
    ) -> Option<u32> {
        let mut t = match &self.brush {
            Brush::Solid => return color,
            Brush::Image { .. } => {
                // `background-size: cover`, centered.
                let frame = frame?;
                let (fw, fh) = (frame.width as f64, frame.height as f64);
                if fw <= 0. || fh <= 0. {
                    return None;
                }
                let scale = (width / fw).max(height / fh);
                let ix = ((x - (width - fw * scale) / 2.) / scale).clamp(0., fw - 1.) as usize;
                let iy = ((y - (height - fh * scale) / 2.) / scale).clamp(0., fh - 1.) as usize;
                let at = (iy * frame.width as usize + ix) * 4;
                let p = frame.bytes.get(at..at + 4)?;
                // Frames are BGRA.
                return Some(
                    ((p[2] as u32) << 24)
                        | ((p[1] as u32) << 16)
                        | ((p[0] as u32) << 8)
                        | p[3] as u32,
                );
            }
            Brush::Linear { angle } => {
                let a = angle.to_radians();
                let dx = a.sin();
                let dy = -a.cos();
                let span = width * dx.abs() + height * dy.abs();
                ((x - width / 2.) * dx + (y - height / 2.) * dy) / span + 0.5
            }
            Brush::Radial { ellipse } => {
                let (rx, ry) = if *ellipse {
                    (width / 2., height / 2.)
                } else {
                    let r = width.max(height) / 2.;
                    (r, r)
                };
                (((x - width / 2.) / rx).powi(2) + ((y - height / 2.) / ry).powi(2)).sqrt()
            }
        };
        let first = self.stops.first()?;
        let last = self.stops.last()?;
        if self.repeat {
            let span = last.at - first.at;
            if span <= f64::EPSILON {
                return None;
            }
            t = (t - first.at).rem_euclid(span) + first.at;
        }
        if t < first.at {
            return Some(first.rgba);
        }
        let right = self.stops.partition_point(|stop| stop.at <= t);
        if right == self.stops.len() {
            return Some(last.rgba);
        }
        let (a, b) = (&self.stops[right - 1], &self.stops[right]);
        let ratio = (t - a.at) / (b.at - a.at);
        let mut out = 0;
        for shift in [24, 16, 8, 0] {
            let a = ((a.rgba >> shift) & 255) as f64;
            let b = ((b.rgba >> shift) & 255) as f64;
            out |= ((a + (b - a) * ratio).round() as u32) << shift;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn north_star_parses_with_glow() {
        let data: Value = serde_json::from_str(r#"{"id":"01HGHA0FA0000A0RAV1AFV8A9Q","name":"North Star","function":"LINEAR_GRADIENT","color":null,"angle":135,"shape":"circle","image_url":"","repeat":true,"stops":[{"at":0.0,"color":-20366849},{"at":0.15,"color":-20366849},{"at":0.15,"color":-1207041},{"at":0.3,"color":-1207041}],"shadows":[{"x_offset":0.0,"y_offset":0.0,"radius":0.1,"color":-7012097},{"x_offset":0.0,"y_offset":0.0,"radius":4.0,"color":-7012097}]}"#).unwrap();
        let paint = definition(&data).unwrap();
        assert!(paint.limitation.is_none());
        assert_eq!(paint.layers.len(), 1);
        assert_eq!(paint.shadows.len(), 2);
        assert!(paint.sample(1., 1., 100., 20., 0xFFFFFF, None).is_some());
    }
    #[test]
    fn image_paint_is_drawable() {
        let data: Value = serde_json::from_str(r#"{"id":"A1","name":"Img","function":"URL","image_url":"https://cdn.7tv.app/paint/01FQB6K5T0000BDD0YMN21KEXX/layer/01JAMR1DWJ14HBYADTC6Q634WR/1x.webp","stops":[],"shadows":[]}"#).unwrap();
        let paint = definition(&data).unwrap();
        assert!(paint.limitation.is_none());
        assert!(paint.image().is_some());
        let pixels = [10u8, 20, 30, 255];
        let frame = Frame {
            bytes: &pixels,
            width: 1,
            height: 1,
        };
        assert_eq!(
            paint.sample(0., 0., 10., 10., 0, Some(&frame)),
            Some(0x1E140A)
        );
    }
}
