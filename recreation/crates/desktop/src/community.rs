//! Anonymous BTTV / FFZ catalogs. Provider failures do not stop live chat.
use chat_core::{EmoteAsset, Fragment};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone,Copy,PartialEq,Eq,Default)]
pub enum LoadState { #[default] Loading, Ready, Failed }
impl LoadState { pub fn label(self)->&'static str {match self {Self::Loading=>"loading",Self::Ready=>"loaded",Self::Failed=>"unavailable"}} }
pub type Emotes = HashMap<String, Fragment>;
#[derive(Default)]
pub struct Set {
    pub bttv: Emotes,
    pub ffz: Emotes,
}
pub struct Community {
    pub global: Set,
    pub global_bttv_state: LoadState,
    pub global_ffz_state: LoadState,
    pub channels: HashMap<String, Set>,
    requested: HashMap<String, Instant>,
    tx: mpsc::SyncSender<(String, String)>,
    rx: mpsc::Receiver<(String, Option<Emotes>, Option<Emotes>)>,
}
impl Community {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<(String, String)>(32);
        let (results, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(12))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok((channel, id)) = jobs.recv() {
                let bttv = get(
                    &client,
                    &if id.is_empty() {
                        "https://api.betterttv.net/3/cached/emotes/global".into()
                    } else {
                        format!("https://api.betterttv.net/3/cached/users/twitch/{id}")
                    },
                )
                .map(|v| parse_bttv(&v, id.is_empty()));
                let ffz = get(
                    &client,
                    &if id.is_empty() {
                        "https://api.frankerfacez.com/v1/set/global".into()
                    } else {
                        format!("https://api.frankerfacez.com/v1/room/id/{id}")
                    },
                )
                .map(|v| parse_ffz(&v, id.is_empty()));
                if results.send((channel, bttv, ffz)).is_err() {
                    break;
                }
            }
        });
        Self {
            global: Set::default(),
            global_bttv_state:LoadState::Loading,global_ffz_state:LoadState::Loading,
            channels: HashMap::new(),
            requested: HashMap::new(),
            tx,
            rx,
        }
    }
    pub fn pump(&mut self, channels: &HashMap<String, (String, Instant)>) -> bool {
        self.channels.retain(|k, _| channels.contains_key(k));
        self.requested
            .retain(|k, _| k.is_empty() || channels.contains_key(k));
        for (channel, id) in std::iter::once(("", "")).chain(
            channels
                .iter()
                .map(|(k, (id, _))| (k.as_str(), id.as_str())),
        ) {
            if self
                .requested
                .get(channel)
                .is_some_and(|at| at.elapsed() < Duration::from_secs(300))
            {
                continue;
            }
            if self.tx.try_send((channel.into(), id.into())).is_ok() {
                self.requested.insert(channel.into(), Instant::now());
            }
        }
        let mut changed = false;
        while let Ok((channel, bttv, ffz)) = self.rx.try_recv() {
            if !channel.is_empty() && !channels.contains_key(&channel) {
                continue;
            }
            if channel.is_empty(){
                let bs=if bttv.is_some(){LoadState::Ready}else{LoadState::Failed};
                let fs=if ffz.is_some(){LoadState::Ready}else{LoadState::Failed};
                changed|=self.global_bttv_state!=bs||self.global_ffz_state!=fs;
                self.global_bttv_state=bs;self.global_ffz_state=fs;
            }
            let set = if channel.is_empty() {
                &mut self.global
            } else {
                self.channels.entry(channel).or_default()
            };
            if let Some(emotes) = bttv {
                set.bttv = emotes;
                changed = true;
            }
            if let Some(emotes) = ffz {
                set.ffz = emotes;
                changed = true;
            }
        }
        changed
    }
    pub fn retry_global(&mut self){
        if self.global_bttv_state!=LoadState::Loading && self.global_ffz_state!=LoadState::Loading {
            self.requested.remove("");self.global_bttv_state=LoadState::Loading;self.global_ffz_state=LoadState::Loading;
        }
    }
    pub fn inspection(&self) -> Value {
        serde_json::json!({"global_bttv":self.global.bttv.len(),"global_ffz":self.global.ffz.len(),"channels":self.channels.iter().map(|(name,set)|(name.clone(),serde_json::json!({"bttv":set.bttv.len(),"ffz":set.ffz.len()}))).collect::<HashMap<_,_>>()})
    }
}
fn get(client: &reqwest::blocking::Client, url: &str) -> Option<Value> {
    let r = client.get(url).send().ok()?;
    if r.status() == 404 {
        return Some(Value::Null);
    }
    if !r.status().is_success() || r.content_length().is_some_and(|n| n > 4 * 1024 * 1024) {
        return None;
    }
    let mut bytes = Vec::new();
    r.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
fn label(v: &Value) -> Option<&str> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_whitespace))
}
fn insert(
    out: &mut Emotes,
    provider: &str,
    id: String,
    name: &str,
    asset: EmoteAsset,
    animated: bool,
) {
    if crate::media::EmoteKey::community(provider, &id, animated, &asset).is_none() {
        return;
    }
    out.insert(
        name.into(),
        Fragment::Emote {
            provider: provider.into(),
            id,
            label: name.into(),
            overlay: false,
            animated,
            asset: Some(asset),
        },
    );
}
fn parse_bttv(v: &Value, global: bool) -> Emotes {
    let mut out = HashMap::new();
    let entries: Vec<_> = if global {
        v.as_array().into_iter().flatten().collect()
    } else {
        v["channelEmotes"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(v["sharedEmotes"].as_array().into_iter().flatten())
            .collect()
    };
    for e in entries.into_iter().take(2000) {
        let Some(name) = label(&e["code"]) else {
            continue;
        };
        let Some(id) = e["id"].as_str().filter(|s| {
            !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric())
        }) else {
            continue;
        };
        // Prefix effects and legacy positioned overlays require their own compositor.
        if e["modifier"].as_bool() == Some(true)
            || matches!(
                id,
                "5849c9a4f52be01a7ee5f79d"
                    | "567b5b520e984428652809b6"
                    | "58487cc6f52be01a7ee5f205"
                    | "5849c9c8f52be01a7ee5f79e"
                    | "567b5c080e984428652809ba"
                    | "567b5dc00e984428652809bd"
                    | "5e76d338d6581c3724c0f0b2"
                    | "5e76d399d6581c3724c0f0b8"
            )
        {
            continue;
        }
        let animated = e["animated"].as_bool().unwrap_or(e["imageType"] == "gif");
        let width = e["width"].as_u64().unwrap_or(28);
        let height = e["height"].as_u64().unwrap_or(28);
        if width == 0 || height == 0 || width > 256 || height > 256 {
            continue;
        }
        // BTTV exposes /static only for still frames of animated emotes.
        // Nonanimated assets live at the ordinary size URL.
        let still = if animated {
            format!("https://cdn.betterttv.net/emote/{id}/static/2x.webp")
        } else {
            format!("https://cdn.betterttv.net/emote/{id}/2x.webp")
        };
        let url = if animated {
            format!("https://cdn.betterttv.net/emote/{id}/2x.gif")
        } else {
            still.clone()
        };
        insert(
            &mut out,
            "bttv",
            id.into(),
            name,
            EmoteAsset {
                url,
                static_url: still,
                width: width as u16,
                height: height as u16,
            },
            animated,
        );
    }
    out
}
fn ffz_url(v: &Value, animated: bool) -> Option<String> {
    let raw = v["2"].as_str().or_else(|| v["1"].as_str())?;
    let mut value = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.into()
    };
    if animated && !value.ends_with(".webp") && !value.ends_with(".gif") {
        value.push_str(".webp");
    }
    Some(value)
}
fn parse_ffz(v: &Value, global: bool) -> Emotes {
    let mut out = HashMap::new();
    let ids: Vec<String> = if global {
        v["default_sets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|n| n.to_string()))
            .collect()
    } else {
        v["room"]["set"]
            .as_u64()
            .map(|n| vec![n.to_string()])
            .unwrap_or_default()
    };
    for id in ids.into_iter().take(32) {
        for e in v["sets"][&id]["emoticons"]
            .as_array()
            .into_iter()
            .flatten()
            .take(2000)
        {
            if out.len() >= 2000 {
                break;
            }
            if e["hidden"].as_bool() == Some(true)
                || e["modifier"].as_bool() == Some(true)
                || e["modifier_flags"].as_u64().unwrap_or(0) != 0
            {
                continue;
            }
            let Some(name) = label(&e["name"]) else {
                continue;
            };
            let Some(id) = e["id"].as_u64().map(|n| n.to_string()) else {
                continue;
            };
            let Some(still) = ffz_url(&e["urls"], false) else {
                continue;
            };
            let motion = ffz_url(&e["animated"], true);
            let width = e["width"].as_u64().unwrap_or(28);
            let height = e["height"].as_u64().unwrap_or(28);
            if width == 0 || height == 0 || width > 256 || height > 256 {
                continue;
            }
            let animated = motion.is_some();
            insert(
                &mut out,
                "ffz",
                id,
                name,
                EmoteAsset {
                    url: motion.unwrap_or_else(|| still.clone()),
                    static_url: still,
                    width: width as u16,
                    height: height as u16,
                },
                animated,
            );
        }
    }
    out
}
