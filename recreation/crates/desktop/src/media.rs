//! Anonymous, bounded emote downloads. Never uses the Twitch credentialed client.
use gpui_kit::*;
use image::{AnimationDecoder, ImageDecoder, ImageFormat};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EmoteKey {
    pub id: String,
    pub animated: bool,
    pub external: Option<chat_core::EmoteAsset>,
}
pub fn valid_seven_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("cdn.7tv.app")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|p| p == 443)
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path().starts_with("/emote/")
        && !url.path().contains("..")
        && url
            .path()
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.'))
}
impl EmoteKey {
    pub fn seven(id: &str, animated: bool, asset: &chat_core::EmoteAsset) -> Option<Self> {
        if !valid_seven_url(&asset.url)
            || !valid_seven_url(&asset.static_url)
            || asset.width == 0
            || asset.height == 0
            || asset.width > 256
            || asset.height > 256
        {
            return None;
        }
        Some(Self {
            id: id.into(),
            animated,
            external: Some(asset.clone()),
        })
    }
    pub fn width(&self) -> f32 {
        self.external.as_ref().map_or(28., |a| {
            (28. * a.width as f32 / a.height as f32).clamp(8., 112.)
        })
    }

    pub fn twitch(id: &str, animated: bool) -> Option<Self> {
        if id.is_empty()
            || id.len() > 160
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return None;
        }
        Some(Self {
            id: id.into(),
            animated,
            external: None,
        })
    }
}
pub struct DecodedMedia {
    pub image: Arc<RenderImage>,
    ends_ms: Vec<u64>,
    cycle_ms: u64,
    epoch: Instant,
}
impl DecodedMedia {
    pub fn frame(&self, reduced: bool) -> usize {
        if reduced || self.ends_ms.len() <= 1 {
            return 0;
        }
        let phase = (self.epoch.elapsed().as_millis() % self.cycle_ms as u128) as u64;
        self.ends_ms
            .partition_point(|end| *end <= phase)
            .min(self.ends_ms.len() - 1)
    }
}
enum Entry {
    Pending(Instant),
    Ready {
        image: Arc<DecodedMedia>,
        bytes: usize,
        used: u64,
    },
    Failed(Instant),
}
type Download = (EmoteKey, Result<(Arc<DecodedMedia>, usize), ()>);
pub struct MediaCache {
    entries: HashMap<EmoteKey, Entry>,
    tx: mpsc::SyncSender<EmoteKey>,
    rx: mpsc::Receiver<Download>,
    used: u64,
    bytes: usize,
}
impl MediaCache {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<EmoteKey>(32);
        let jobs = Arc::new(Mutex::new(jobs));
        let (results, rx) = mpsc::sync_channel(2);
        for _ in 0..2 {
            let jobs = jobs.clone();
            let results = results.clone();
            std::thread::spawn(move || {
                let Ok(client) = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(12))
                    .connect_timeout(Duration::from_secs(5))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                else {
                    return;
                };
                loop {
                    let job = match jobs.lock() {
                        Ok(rx) => rx.recv(),
                        Err(_) => return,
                    };
                    let Ok(key) = job else {
                        return;
                    };
                    let result = download(&client, &key);
                    if results.send((key, result)).is_err() {
                        return;
                    }
                }
            });
        }
        Self {
            entries: HashMap::new(),
            tx,
            rx,
            used: 0,
            bytes: 0,
        }
    }
    pub fn inspection(&self) -> serde_json::Value {
        serde_json::json!({"decoded_bytes":self.bytes,"entries":self.entries.len(),"assets":self.entries.iter().filter_map(|(key,entry)| {
            if let Entry::Ready{image,..}=entry { Some(serde_json::json!({"id":key.id,"provider":if key.external.is_some(){"7tv"}else{"twitch"},"animated_requested":key.animated,"frames":image.image.frame_count(),"frame_now":image.frame(false)})) } else {None}
        }).collect::<Vec<_>>()})
    }
    pub fn failed(&self, key: &EmoteKey) -> bool {
        matches!(self.entries.get(key), Some(Entry::Failed(at)) if at.elapsed() < Duration::from_secs(60))
    }
    pub fn get(&mut self, key: &EmoteKey, cx: &mut App) -> Option<Arc<DecodedMedia>> {
        self.used = self.used.wrapping_add(1);
        match self.entries.get_mut(key) {
            Some(Entry::Ready { image, used, .. }) => {
                *used = self.used;
                return Some(image.clone());
            }
            Some(Entry::Pending(at)) if at.elapsed() < Duration::from_secs(30) => return None,
            Some(Entry::Failed(at)) if at.elapsed() < Duration::from_secs(60) => return None,
            _ => {}
        }
        if self.entries.len() >= 128 {
            self.evict(cx);
        }
        if self.tx.try_send(key.clone()).is_ok() {
            self.entries
                .insert(key.clone(), Entry::Pending(Instant::now()));
        }
        None
    }
    fn evict(&mut self, cx: &mut App) {
        let key = self
            .entries
            .iter()
            .min_by_key(|(_, e)| match e {
                Entry::Ready { used, .. } => *used,
                Entry::Failed(_) => 0,
                Entry::Pending(_) => u64::MAX,
            })
            .map(|(key, _)| key.clone());
        if let Some(key) = key {
            self.remove(&key, cx);
        }
    }
    fn remove(&mut self, key: &EmoteKey, cx: &mut App) {
        if let Some(Entry::Ready { image, bytes, .. }) = self.entries.remove(key) {
            self.bytes = self.bytes.saturating_sub(bytes);
            cx.drop_image(image.image.clone(), None);
        }
    }
    pub fn pump(&mut self, cx: &mut App) -> bool {
        let mut changed = false;
        while let Ok((key, result)) = self.rx.try_recv() {
            if !matches!(self.entries.get(&key), Some(Entry::Pending(_))) {
                continue;
            }
            match result {
                Ok((image, bytes)) => {
                    while self.bytes + bytes > 48 * 1024 * 1024 && self.bytes > 0 {
                        self.evict(cx);
                    }
                    self.used = self.used.wrapping_add(1);
                    self.bytes += bytes;
                    self.entries.insert(
                        key,
                        Entry::Ready {
                            image,
                            bytes,
                            used: self.used,
                        },
                    );
                }
                Err(()) => {
                    self.entries.insert(key, Entry::Failed(Instant::now()));
                }
            }
            changed = true;
        }
        changed
    }
}
fn download(
    client: &reqwest::blocking::Client,
    key: &EmoteKey,
) -> Result<(Arc<DecodedMedia>, usize), ()> {
    let format = if key.animated { "animated" } else { "static" };
    let url = key
        .external
        .as_ref()
        .map(|asset| {
            if key.animated {
                asset.url.clone()
            } else {
                asset.static_url.clone()
            }
        })
        .unwrap_or_else(|| {
            format!(
                "https://static-cdn.jtvnw.net/emoticons/v2/{}/{format}/dark/2.0",
                key.id
            )
        });
    let response = client.get(url).send().map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    const MAX_WIRE: usize = 2 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_WIRE as u64)
    {
        return Err(());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_WIRE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() > MAX_WIRE {
        return Err(());
    }
    let format = image::guess_format(&bytes).map_err(|_| ())?;
    let mut frames = Vec::new();
    let mut decoded = 0usize;
    let mut push = |mut frame: image::Frame| -> Result<(), ()> {
        let (w, h) = frame.buffer().dimensions();
        if w == 0 || h == 0 || w > 256 || h > 256 || frames.len() >= 120 {
            return Err(());
        }
        decoded = decoded.checked_add(w as usize * h as usize * 4).ok_or(())?;
        if decoded > 8 * 1024 * 1024 {
            return Err(());
        }
        for pixel in frame.buffer_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        frames.push(frame);
        Ok(())
    };
    match format {
        ImageFormat::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(Cursor::new(&bytes)).map_err(|_| ())?;
            let (w, h) = decoder.dimensions();
            if w > 256 || h > 256 {
                return Err(());
            }
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(8 * 1024 * 1024);
            decoder.set_limits(limits).map_err(|_| ())?;
            for frame in decoder.into_frames() {
                push(frame.map_err(|_| ())?)?;
                if !key.animated {
                    break;
                }
            }
        }
        ImageFormat::WebP if key.animated => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(Cursor::new(&bytes)).map_err(|_| ())?;
            let (w, h) = decoder.dimensions();
            if w > 256 || h > 256 {
                return Err(());
            }
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(8 * 1024 * 1024);
            decoder.set_limits(limits).map_err(|_| ())?;
            for frame in decoder.into_frames() {
                push(frame.map_err(|_| ())?)?;
            }
        }
        ImageFormat::Png | ImageFormat::WebP => {
            let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(256);
            limits.max_image_height = Some(256);
            limits.max_alloc = Some(8 * 1024 * 1024);
            reader.limits(limits);
            push(image::Frame::new(
                reader.decode().map_err(|_| ())?.into_rgba8(),
            ))?;
        }
        _ => return Err(()),
    }
    if frames.is_empty() {
        return Err(());
    }
    let image = Arc::new(RenderImage::new(frames));
    let mut cycle_ms = 0;
    let mut ends_ms = Vec::new();
    for index in 0..image.frame_count() {
        let delay = Duration::from(image.delay(index))
            .as_millis()
            .clamp(20, 10_000) as u64;
        cycle_ms += delay;
        ends_ms.push(cycle_ms);
    }
    Ok((
        Arc::new(DecodedMedia {
            image,
            ends_ms,
            cycle_ms,
            epoch: Instant::now(),
        }),
        decoded,
    ))
}
