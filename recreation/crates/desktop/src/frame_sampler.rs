//! Bounded animation sampling.
//!
//! Long or large animations are thinned instead of rejected: when the kept-frame
//! or decoded-byte budget would be exceeded, adjacent kept frames are merged and
//! the sampling stride doubles. A dropped frame's delay is added to the frame that
//! represents it, so the full loop duration is preserved at lower frame rate.
use image::{Delay, Frame, RgbaImage};
use std::time::Duration;

pub struct Limits {
    /// Source frames decoded before the remainder of the loop is ignored.
    pub source_frames: usize,
    /// Kept frames before thinning.
    pub kept_frames: usize,
    /// Kept decoded RGBA bytes before thinning.
    pub bytes: usize,
    /// Optional bounding box that kept frames are downscaled into.
    pub fit: Option<(u32, u32)>,
}

pub enum Push {
    Continue,
    /// The source budget is spent; stop decoding and keep what was sampled.
    Stop,
}

pub struct Frames {
    limits: Limits,
    frames: Vec<Frame>,
    seen: usize,
    stride: usize,
    bytes: usize,
}

fn milliseconds(frame: &Frame) -> u32 {
    Duration::from(frame.delay()).as_millis().clamp(20, 10_000) as u32
}

fn extend_delay(frame: &mut Frame, extra: u32) {
    let delay = Duration::from(frame.delay())
        .as_millis()
        .min(u32::MAX as u128) as u32;
    let old = std::mem::replace(frame, Frame::new(RgbaImage::new(0, 0)));
    let (x, y) = (old.left(), old.top());
    *frame = Frame::from_parts(
        old.into_buffer(),
        x,
        y,
        Delay::from_numer_denom_ms(delay.saturating_add(extra), 1),
    );
}

impl Frames {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            frames: Vec::new(),
            seen: 0,
            stride: 1,
            bytes: 0,
        }
    }

    pub fn push(&mut self, frame: Frame) -> Push {
        if self.seen >= self.limits.source_frames {
            return Push::Stop;
        }
        let index = self.seen;
        self.seen += 1;
        let delay = milliseconds(&frame);
        if !index.is_multiple_of(self.stride) {
            self.extend_last(delay);
            return Push::Continue;
        }
        let buffer = self.fit(frame.into_buffer());
        while !self.frames.is_empty()
            && (self.frames.len() >= self.limits.kept_frames
                || self.bytes + buffer.len() > self.limits.bytes)
        {
            self.thin();
            if !index.is_multiple_of(self.stride) {
                self.extend_last(delay);
                return Push::Continue;
            }
        }
        if self.bytes + buffer.len() > self.limits.bytes {
            // A single frame larger than the whole budget cannot be kept.
            return Push::Stop;
        }
        self.bytes += buffer.len();
        self.frames.push(Frame::from_parts(
            buffer,
            0,
            0,
            Delay::from_numer_denom_ms(delay, 1),
        ));
        Push::Continue
    }

    /// Kept frames and their decoded byte size.
    pub fn finish(self) -> (Vec<Frame>, usize) {
        (self.frames, self.bytes)
    }

    fn extend_last(&mut self, delay: u32) {
        if let Some(last) = self.frames.last_mut() {
            extend_delay(last, delay);
        }
    }

    /// Merge each kept pair into its first frame and double the stride, so the
    /// kept frames stay aligned to source indices that are multiples of it.
    fn thin(&mut self) {
        let mut kept = Vec::with_capacity(self.frames.len().div_ceil(2));
        let mut old = std::mem::take(&mut self.frames).into_iter();
        while let Some(mut first) = old.next() {
            if let Some(second) = old.next() {
                extend_delay(
                    &mut first,
                    Duration::from(second.delay()).as_millis() as u32,
                );
            }
            kept.push(first);
        }
        self.frames = kept;
        self.stride *= 2;
        self.bytes = self.frames.iter().map(|f| f.buffer().len()).sum();
    }

    fn fit(&self, buffer: RgbaImage) -> RgbaImage {
        let Some((max_w, max_h)) = self.limits.fit else {
            return buffer;
        };
        let (w, h) = buffer.dimensions();
        let scale = (max_w as f64 / w as f64)
            .min(max_h as f64 / h as f64)
            .min(1.);
        let (width, height) = (
            (w as f64 * scale).round().max(1.) as u32,
            (h as f64 * scale).round().max(1.) as u32,
        );
        if (width, height) == (w, h) {
            return buffer;
        }
        image::imageops::resize(
            &buffer,
            width,
            height,
            image::imageops::FilterType::Triangle,
        )
    }
}
