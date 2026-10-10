//! Anonymous, bounded emote downloads. Never uses the Twitch credentialed client.
use gpui_kit::*;
use image::{AnimationDecoder, ImageDecoder, ImageFormat};
use std::{
    collections::{HashMap, HashSet},
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
    pub fn seven_badge(id:&str,animated:bool,asset:&chat_core::EmoteAsset)->Option<Self>{
        for value in [&asset.url,&asset.static_url]{
            let url=reqwest::Url::parse(value).ok()?;
            if value.len()>2048||url.scheme()!="https"||url.host_str()!=Some("cdn.7tv.app")||!url.username().is_empty()||url.password().is_some()||url.port().is_some_and(|p|p!=443)||url.query().is_some()||url.fragment().is_some()||!url.path().starts_with("/badge/")||url.path().contains("..")||!url.path().bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'/'|b'_'|b'-'|b'.')){return None;}
        }
        if asset.width==0||asset.height==0||asset.width>256||asset.height>256{return None;}
        Some(Self{id:format!("7tv-badge:{id}"),animated,external:Some(asset.clone())})
    }
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
    pub fn community(provider:&str,id:&str,animated:bool,asset:&chat_core::EmoteAsset)->Option<Self> {
        if provider=="7tv" {return Self::seven(id,animated,asset);}
        let(host,prefixes)=match provider {"bttv"=>("cdn.betterttv.net",vec!["/emote/"]),"ffz"=>("cdn.frankerfacez.com",vec!["/emote/","/emoticon/"]),_=>return None};
        for value in [&asset.url,&asset.static_url] {
            let url=reqwest::Url::parse(value).ok()?;
            if url.scheme()!="https"||url.host_str()!=Some(host)||!url.username().is_empty()||url.password().is_some()||url.port().is_some_and(|p|p!=443)||url.query().is_some()||url.fragment().is_some()||!prefixes.iter().any(|p|url.path().starts_with(p))||url.path().contains("..")||!url.path().bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'/'|b'_'|b'-'|b'.')){return None;}
        }
        if asset.width==0||asset.height==0||asset.width>256||asset.height>256{return None;}
        Some(Self{id:id.into(),animated,external:Some(asset.clone())})
    }
    pub fn avatar(value:&str)->Option<Self>{
        let url=reqwest::Url::parse(value).ok()?;
        if value.len()>2048||url.scheme()!="https"||url.host_str()!=Some("static-cdn.jtvnw.net")||!url.path().starts_with("/jtv_user_pictures/")||!url.username().is_empty()||url.password().is_some()||url.port().is_some_and(|p|p!=443)||url.query().is_some()||url.fragment().is_some(){return None;}
        Some(Self{id:format!("avatar:{}",url.path()),animated:false,external:Some(chat_core::EmoteAsset{url:value.into(),static_url:value.into(),width:48,height:48})})
    }
    pub fn gif(id: &str, value: &str, animated: bool) -> Option<Self> {
        let url=reqwest::Url::parse(value).ok()?;
        // Known GIPHY delivery hosts from Twitch's documented integration. Unknown
        // origins retain their readable message fallback; never fetch arbitrary URLs.
        if value.len()>4096 || url.scheme()!="https" || !matches!(url.host_str(),Some("media.giphy.com"|"media0.giphy.com"|"media1.giphy.com"|"media2.giphy.com"|"media3.giphy.com"|"media4.giphy.com")) || !url.username().is_empty() || url.password().is_some() || url.port().is_some_and(|p|p!=443) || url.fragment().is_some() || !url.path().starts_with("/media/") {return None;}
        Some(Self{id:format!("gif:{id}"),animated,external:Some(chat_core::EmoteAsset{url:value.into(),static_url:value.into(),width:160,height:100})})
    }
    pub fn cheer(id:&str,animated:bool,asset:&chat_core::EmoteAsset)->Option<Self>{
        for value in [&asset.url,&asset.static_url] {
            let url=reqwest::Url::parse(value).ok()?;
            if url.scheme()!="https" || !matches!(url.host_str(),Some("d3aqoihi2n8ty8.cloudfront.net"|"static-cdn.jtvnw.net")) || !url.path().starts_with("/actions/") || !url.username().is_empty() || url.password().is_some() || url.port().is_some_and(|p|p!=443) || url.fragment().is_some() || url.query().is_some() {return None;}
        }
        Some(Self{id:format!("cheer:{id}"),animated,external:Some(asset.clone())})
    }
    pub fn provider(&self)->&'static str {
        if self.id.starts_with("7tv-badge:"){return "7tv_badge";}
        if self.id.starts_with("avatar:"){return "twitch_avatar";}
        if self.id.starts_with("gif:"){return "twitch_gif";}
        if self.id.starts_with("cheer:"){return "twitch_cheer";}
        if self.id.starts_with("badge:"){return "twitch_badge";}
        match self.external.as_ref().and_then(|a|reqwest::Url::parse(&a.url).ok()).and_then(|u|u.host_str().map(str::to_owned)).as_deref(){Some("cdn.betterttv.net")=>"bttv",Some("cdn.frankerfacez.com")=>"ffz",Some("cdn.7tv.app")=>"7tv",_=>"twitch"}
    }
    pub fn badge(value: &str) -> Option<Self> {
        let url=reqwest::Url::parse(value).ok()?;
        let path=url.path();
        if url.scheme()!="https" || url.host_str()!=Some("static-cdn.jtvnw.net") || !url.username().is_empty() || url.password().is_some() || url.port().is_some_and(|p|p!=443) || url.query().is_some() || url.fragment().is_some() || !path.starts_with("/badges/v1/") || path.contains("..") || !path.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'/'|b'-')) {return None;}
        Some(Self{id:format!("badge:{path}"),animated:false,external:Some(chat_core::EmoteAsset{url:value.into(),static_url:value.into(),width:18,height:18})})
    }
    pub fn width(&self) -> f32 {
        if self.id.starts_with("badge:") {return 20.;}
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
    Pending,
    Ready {
        image: Arc<DecodedMedia>,
        bytes: usize,
        used: u64,
    },
    Failed(Instant, MediaFailure),
}
#[derive(Clone, Copy)]
enum MediaFailure { Transport, Timeout, Http(u16), WireLimit, Format, Decode, Dimensions, FrameLimit, DecodedLimit, CacheBudget }
impl MediaFailure {
    fn label(self) -> String { match self {
        Self::Transport => "transport".into(), Self::Timeout => "timeout".into(),
        Self::Http(status) => format!("http_{status}"), Self::WireLimit => "wire_limit".into(),
        Self::Format => "unsupported_format".into(), Self::Decode => "decode".into(),
        Self::Dimensions => "dimensions".into(), Self::FrameLimit => "frame_limit".into(),
        Self::DecodedLimit => "decoded_limit".into(), Self::CacheBudget => "cache_budget".into(),
    }}
}
type Download = (EmoteKey, Result<(Arc<DecodedMedia>, usize), MediaFailure>);
pub struct MediaCache {
    entries: HashMap<EmoteKey, Entry>,
    tx: mpsc::SyncSender<EmoteKey>,
    rx: mpsc::Receiver<Download>,
    used: u64,
    bytes: usize,
    layout_keys: HashSet<EmoteKey>,
    layout_dirty: bool,
    conservative_layout: bool,
}
impl MediaCache {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<EmoteKey>(32);
        let jobs = Arc::new(Mutex::new(jobs));
        let (results, rx) = mpsc::sync_channel(8);
        for _ in 0..4 {
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
            layout_keys: HashSet::new(),
            layout_dirty: false,
            conservative_layout: false,
        }
    }
    pub fn inspection(&self) -> serde_json::Value {
        // Aggregate fixed categories only: no credentials, request URLs or raw errors.
        let mut failure_reasons = std::collections::BTreeMap::<String, usize>::new();
        for entry in self.entries.values() { if let Entry::Failed(_, reason) = entry {
            *failure_reasons.entry(reason.label()).or_default() += 1;
        }}
        serde_json::json!({"failure_reasons":failure_reasons,"decoded_bytes":self.bytes,"entries":self.entries.len(),"pending":self.entries.values().filter(|e|matches!(e,Entry::Pending)).count(),"failed":self.entries.values().filter(|e|matches!(e,Entry::Failed(..))).count(),"assets":self.entries.iter().filter_map(|(key,entry)| {
            if let Entry::Ready{image,..}=entry { Some(serde_json::json!({"id":key.id,"provider":key.provider(),"animated_requested":key.animated,"frames":image.image.frame_count(),"frame_now":image.frame(false)})) } else {None}
        }).collect::<Vec<_>>()})
    }
    pub fn peek(&self,key:&EmoteKey)->Option<Arc<DecodedMedia>> {
        match self.entries.get(key){Some(Entry::Ready{image,..})=>Some(image.clone()),_=>None}
    }
    pub fn touch(&mut self,key:&EmoteKey){
        self.used=self.used.wrapping_add(1);
        if let Some(Entry::Ready{used,..})=self.entries.get_mut(key){*used=self.used;}
    }
    pub fn failed(&self, key: &EmoteKey) -> bool {
        matches!(self.entries.get(key), Some(Entry::Failed(at, _)) if at.elapsed() < Duration::from_secs(60))
    }
    pub fn get(&mut self, key: &EmoteKey, cx: &mut App) -> Option<Arc<DecodedMedia>> {
        self.used = self.used.wrapping_add(1);
        match self.entries.get_mut(key) {
            Some(Entry::Ready { image, used, .. }) => {
                *used = self.used;
                return Some(image.clone());
            }
            Some(Entry::Pending) => return None,
            Some(Entry::Failed(at, _)) if at.elapsed() < Duration::from_secs(60) => return None,
            _ => {}
        }
        // Expired failure changes the inline fallback from text back to an image
        // reservation, even if a full queue defers the actual retry.
        if (self.layout_keys.contains(key) || self.conservative_layout) && matches!(self.entries.get(key), Some(Entry::Failed(..))) {
            self.layout_dirty = true;
        }
        if self.tx.try_send(key.clone()).is_ok() {
            // A full queue must not evict usable images on every paint.
            if self.entries.len() >= 512 { self.evict(cx); }
            self.entries
                .insert(key.clone(), Entry::Pending);
        }
        None
    }
    /// Only base inline assets can change transcript geometry. This set
    /// includes queue-rejected interest and is pruned on eviction, capped at 1024.
    /// Overflow falls back conservatively to invalidating every completion.
    /// Overlay/picker/avatar reads otherwise only need repaint.
    pub fn get_layout(&mut self, key: &EmoteKey, cx: &mut App) -> Option<Arc<DecodedMedia>> {
        let image = self.get(key, cx);
        if self.layout_keys.len() < 1024 || self.layout_keys.contains(key) {
            self.layout_keys.insert(key.clone());
        } else { self.conservative_layout = true; }
        image
    }
    pub fn take_layout_dirty(&mut self) -> bool { std::mem::take(&mut self.layout_dirty) }
    /// Low-priority look-ahead never fills the queue ahead of visible chat.
    pub fn prefetch(&mut self, key: &EmoteKey, cx: &mut App) {
        if matches!(self.entries.get(key),Some(Entry::Ready{..}|Entry::Pending))
            || matches!(self.entries.get(key),Some(Entry::Failed(at, _)) if at.elapsed()<Duration::from_secs(60))
            || self.entries.values().filter(|e| matches!(e, Entry::Pending)).count() >= 12 { return; }
        self.get(key, cx);
    }
    fn evict(&mut self, cx: &mut App) {
        let key = self
            .entries
            .iter()
            .min_by_key(|(_, e)| match e {
                Entry::Ready { used, .. } => *used,
                Entry::Failed(..) => 0,
                Entry::Pending => u64::MAX,
            })
            .map(|(key, _)| key.clone());
        if let Some(key) = key {
            self.remove(&key, cx);
        }
    }
    fn remove(&mut self, key: &EmoteKey, cx: &mut App) {
        self.layout_dirty |= self.layout_keys.remove(key) || self.conservative_layout;
        if let Some(Entry::Ready { image, bytes, .. }) = self.entries.remove(key) {
            self.bytes = self.bytes.saturating_sub(bytes);
            cx.drop_image(image.image.clone(), None);
        }
    }
    pub fn pump(&mut self, cx: &mut App) -> bool {
        let mut changed = false;
        while let Ok((key, result)) = self.rx.try_recv() {
            if !matches!(self.entries.get(&key), Some(Entry::Pending)) {
                continue;
            }
            self.layout_dirty |= self.layout_keys.contains(&key) || self.conservative_layout;
            match result {
                Ok((mut image, bytes)) => {
                    if let Some(decoded) = Arc::get_mut(&mut image) { decoded.epoch = Instant::now(); }
                    // Large visible GIFs must not evict and redownload one another
                    // continuously. Reject excess animation for one cooldown; the
                    // renderer requests the bounded static variant instead.
                    if key.id.starts_with("gif:") && key.animated && self.bytes + bytes > 48 * 1024 * 1024 {
                        self.entries.insert(key,Entry::Failed(Instant::now(), MediaFailure::CacheBudget));
                        changed=true;
                        continue;
                    }
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
                Err(reason) => {
                    self.entries.insert(key, Entry::Failed(Instant::now(), reason));
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
) -> Result<(Arc<DecodedMedia>, usize), MediaFailure> {
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
    let response = client.get(url).send().map_err(|e| if e.is_timeout() {MediaFailure::Timeout} else {MediaFailure::Transport})?;
    if !response.status().is_success() {
        return Err(MediaFailure::Http(response.status().as_u16()));
    }
    let rich_gif=key.id.starts_with("gif:");
    let max_wire: usize = if rich_gif { 12 * 1024 * 1024 } else { 2 * 1024 * 1024 };
    let max_dimension = if rich_gif || key.id.starts_with("avatar:") { 512 } else { 256 };
    let max_decoded = if rich_gif { 24 * 1024 * 1024 } else { 8 * 1024 * 1024 };
    if response
        .content_length()
        .is_some_and(|n| n > max_wire as u64)
    {
        return Err(MediaFailure::WireLimit);
    }
    let mut bytes = Vec::new();
    response
        .take(max_wire as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| MediaFailure::Transport)?;
    if bytes.len() > max_wire {
        return Err(MediaFailure::WireLimit);
    }
    let format = image::guess_format(&bytes).map_err(|_| MediaFailure::Format)?;
    let mut frames = Vec::new();
    let mut decoded = 0usize;
    let mut push = |mut frame: image::Frame| -> Result<(), MediaFailure> {
        let (w, h) = frame.buffer().dimensions();
        if w == 0 || h == 0 || w > max_dimension || h > max_dimension {
            return Err(MediaFailure::Dimensions);
        }
        if frames.len() >= 120 { return Err(MediaFailure::FrameLimit); }
        decoded = decoded.checked_add(w as usize * h as usize * 4).ok_or(MediaFailure::DecodedLimit)?;
        if decoded > max_decoded {
            return Err(MediaFailure::DecodedLimit);
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
                image::codecs::gif::GifDecoder::new(Cursor::new(&bytes)).map_err(|_| MediaFailure::Decode)?;
            let (w, h) = decoder.dimensions();
            if w > max_dimension || h > max_dimension {
                return Err(MediaFailure::Dimensions);
            }
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(max_decoded as u64);
            decoder.set_limits(limits).map_err(|_| MediaFailure::Decode)?;
            for frame in decoder.into_frames() {
                push(frame.map_err(|_| MediaFailure::Decode)?)?;
                if !key.animated {
                    break;
                }
            }
        }
        ImageFormat::WebP if key.animated => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(Cursor::new(&bytes)).map_err(|_| MediaFailure::Decode)?;
            let (w, h) = decoder.dimensions();
            if w > max_dimension || h > max_dimension {
                return Err(MediaFailure::Dimensions);
            }
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(max_decoded as u64);
            decoder.set_limits(limits).map_err(|_| MediaFailure::Decode)?;
            for frame in decoder.into_frames() {
                push(frame.map_err(|_| MediaFailure::Decode)?)?;
            }
        }
        ImageFormat::Png | ImageFormat::WebP | ImageFormat::Jpeg => {
            let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(max_dimension);
            limits.max_image_height = Some(max_dimension);
            limits.max_alloc = Some(max_decoded as u64);
            reader.limits(limits);
            push(image::Frame::new(
                reader.decode().map_err(|_| MediaFailure::Decode)?.into_rgba8(),
            ))?;
        }
        _ => return Err(MediaFailure::Format),
    }
    if frames.is_empty() {
        return Err(MediaFailure::Decode);
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
