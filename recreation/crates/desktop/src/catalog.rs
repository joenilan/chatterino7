//! Public 7TV catalogs; no Twitch bearer tokens or account credentials.
use chat_core::{EmoteAsset, Fragment};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone)]
struct Emote {
    id: String,
    asset: EmoteAsset,
    overlay: bool,
    animated: bool,
}
type Emotes = HashMap<String, Emote>;
pub struct Catalog {
    global: Emotes,
    pub twitch: crate::twitch_assets::TwitchAssets,
    pub community: crate::community::Community,
    channels: HashMap<String, Emotes>,
    requested: HashMap<String, (String, Instant)>,
    global_requested: Instant,
    tx: mpsc::SyncSender<(String, String)>,
    rx: mpsc::Receiver<(String, Option<Emotes>)>,
}
impl Catalog {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<(String, String)>(32);
        let (results, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(12))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok((channel, id)) = jobs.recv() {
                let url = if channel.is_empty() {
                    "https://7tv.io/v3/emote-sets/global".to_owned()
                } else {
                    format!("https://7tv.io/v3/users/twitch/{id}")
                };
                let result = load(&client, &url, channel.is_empty());
                if results.send((channel, result)).is_err() {
                    return;
                }
            }
        });
        let _ = tx.try_send((String::new(), String::new()));
        Self {
            global: HashMap::new(),
            twitch: crate::twitch_assets::TwitchAssets::new(),
            community: crate::community::Community::new(),
            channels: HashMap::new(),
            requested: HashMap::new(),
            global_requested: Instant::now(),
            tx,
            rx,
        }
    }
    pub fn inspection(&self) -> Value {
        serde_json::json!({"community":self.community.inspection(),"twitch_global_emotes":self.twitch.global.emotes.len(),"twitch_global_badges":self.twitch.global.badges.len(),"twitch_channel_badges":self.twitch.channels.iter().map(|(n,a)|(n.clone(),a.badges.len())).collect::<HashMap<_,_>>(),"global_emotes":self.global.len(),"channel_emotes":self.channels.iter().map(|(name,emotes)|(name.clone(),emotes.len())).collect::<HashMap<_,_>>()})
    }
    pub fn channel(&mut self, name: &str, id: &str) {
        if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return;
        }
        if self
            .requested
            .get(name)
            .is_some_and(|(_, t)| t.elapsed() < Duration::from_secs(300))
        {
            return;
        }
        if self.tx.try_send((name.into(), id.into())).is_ok() {
            self.requested
                .insert(name.into(), (id.into(), Instant::now()));
        }
    }
    pub fn pump(&mut self, active: HashSet<String>, identity: Option<crate::live::Identity>) -> bool {
        self.channels.retain(|k, _| active.contains(k));
        self.requested.retain(|k, _| active.contains(k));
        if self.global_requested.elapsed() >= Duration::from_secs(300)
            && self.tx.try_send((String::new(), String::new())).is_ok()
        {
            self.global_requested = Instant::now();
        }
        for (name, (id, at)) in &mut self.requested {
            if at.elapsed() >= Duration::from_secs(300)
                && self.tx.try_send((name.clone(), id.clone())).is_ok()
            {
                *at = Instant::now();
            }
        }
        let mut changed = self.twitch.pump(identity, &self.requested);
        changed |= self.community.pump(&self.requested);
        while let Ok((name, result)) = self.rx.try_recv() {
            if let Some(emotes) = result {
                if name.is_empty() {
                    self.global = emotes;
                    changed = true;
                } else if active.contains(&name) {
                    self.channels.insert(name, emotes);
                    changed = true;
                }
            }
        }
        changed
    }
    fn lookup(&self, channel:&str, token:&str)->Option<Fragment> {
        let make_seven=|e:&Emote|Fragment::Emote{provider:"7tv".into(),id:e.id.clone(),label:token.into(),overlay:e.overlay,animated:e.animated,asset:Some(e.asset.clone())};
        let local=self.community.channels.get(channel);
        local.and_then(|s|s.ffz.get(token)).or_else(||local.and_then(|s|s.bttv.get(token))).cloned()
            .or_else(||self.channels.get(channel).and_then(|s|s.get(token)).map(make_seven))
            .or_else(||self.community.global.ffz.get(token).or_else(||self.community.global.bttv.get(token)).cloned())
            .or_else(||self.global.get(token).map(make_seven))
    }
    pub fn choices(&self,channel:&str,query:&str,limit:usize)->Vec<crate::twitch_assets::Choice>{
        let query=query.to_lowercase();let mut names=HashSet::new();
        names.extend(self.global.keys().cloned());
        if let Some(set)=self.channels.get(channel){names.extend(set.keys().cloned());}
        for set in std::iter::once(&self.community.global).chain(self.community.channels.get(channel)) {names.extend(set.ffz.keys().cloned());names.extend(set.bttv.keys().cloned());}
        let mut choices=HashMap::new();
        for name in names {if !name.to_lowercase().contains(&query){continue;}
            if let Some(Fragment::Emote{provider,id,animated,asset:Some(asset),..})=self.lookup(channel,&name){
                if let Some(key)=crate::media::EmoteKey::community(&provider,&id,animated,&asset){
                    let provider=match provider.as_str(){"bttv"=>"BTTV","ffz"=>"FFZ",_=>"7TV"};
                    choices.insert(name.clone(),crate::twitch_assets::Choice{label:name,provider,key});
                }
            }
        }
        for emote in &self.twitch.global.emotes {if emote.label.to_lowercase().contains(&query){choices.insert(emote.label.clone(),emote.clone());}}
        let mut choices:Vec<_>=choices.into_values().collect();
        choices.sort_by(|a,b|{let al=a.label.to_lowercase();let bl=b.label.to_lowercase();(!al.starts_with(&query),al,&a.label).cmp(&(!bl.starts_with(&query),bl,&b.label))});
        choices.truncate(limit);choices
    }
    pub fn expand(&self, channel: &str, fragments: &[Fragment]) -> Vec<Fragment> {
        let mut result = Vec::new();
        for fragment in fragments {
            if let Fragment::Cheer{prefix,bits,tier,label,..}=fragment {
                result.push(Fragment::Cheer{prefix:prefix.clone(),bits:*bits,tier:*tier,label:label.clone(),asset:self.twitch.cheer(channel,prefix,*tier).cloned()});continue;
            }
            let text = match fragment {
                Fragment::Text(text) => text.as_str(),
                Fragment::Emote {
                    provider, label, ..
                } if matches!(provider.as_str(),"7tv"|"bttv"|"ffz") => label.as_str(),
                _ => {
                    result.push(fragment.clone());
                    continue;
                }
            };
            for part in text.split_inclusive(char::is_whitespace) {
                let token = part.trim_end_matches(char::is_whitespace);
                if !token.is_empty() {
                    if let Some(emote)=self.lookup(channel,token) {
                        result.push(emote);
                    } else {
                        result.push(Fragment::Text(token.into()));
                    }
                }
                if token.len() < part.len() {
                    result.push(Fragment::Text(part[token.len()..].into()));
                }
            }
        }
        result
    }
}
fn load(client: &reqwest::blocking::Client, url: &str, global: bool) -> Option<Emotes> {
    let response = client.get(url).send().ok()?;
    if response.status() == 404 {
        return Some(HashMap::new());
    }
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|n| n > 4 * 1024 * 1024)
    {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    let json: Value = serde_json::from_slice(&bytes).ok()?;
    let list = if global {
        json["emotes"].as_array()
    } else {
        json["emote_set"]["emotes"].as_array()
    };
    let mut emotes = HashMap::new();
    for entry in list.into_iter().flatten().take(2000) {
        let Some(name) = entry["name"]
            .as_str()
            .filter(|n| !n.is_empty() && n.len() <= 128 && !n.chars().any(char::is_whitespace))
        else {
            continue;
        };
        let Some(id) = entry["id"]
            .as_str()
            .filter(|s| s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        else {
            continue;
        };
        let host = &entry["data"]["host"];
        let Some(base) = host["url"].as_str() else {
            continue;
        };
        let base = if base.starts_with("//") {
            format!("https:{base}")
        } else {
            base.into()
        };
        let Some(files) = host["files"].as_array() else {
            continue;
        };
        let selected = files
            .iter()
            .filter(|f| {
                f["format"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("WEBP"))
                    && f["width"].as_u64().is_some_and(|n| n > 0 && n <= 256)
                    && f["height"].as_u64().is_some_and(|n| n > 0 && n <= 256)
                    && f["size"].as_u64().is_none_or(|n| n <= 2 * 1024 * 1024)
            })
            .min_by_key(|f| f["height"].as_u64().unwrap_or(0).abs_diff(56));
        let Some(file) = selected else {
            continue;
        };
        let Some(filename) = file["name"].as_str().filter(|s| valid_file(s)) else {
            continue;
        };
        let static_name = file["static_name"]
            .as_str()
            .filter(|s| valid_file(s))
            .unwrap_or(filename);
        let asset = EmoteAsset {
            url: format!("{}/{filename}", base.trim_end_matches('/')),
            static_url: format!("{}/{static_name}", base.trim_end_matches('/')),
            width: file["width"].as_u64().unwrap_or(28) as u16,
            height: file["height"].as_u64().unwrap_or(28) as u16,
        };
        if !crate::media::valid_seven_url(&asset.url)
            || !crate::media::valid_seven_url(&asset.static_url)
        {
            continue;
        }
        emotes.insert(
            name.into(),
            Emote {
                id: id.into(),
                asset,
                overlay: entry["flags"].as_u64().unwrap_or(0) & 1 != 0,
                animated: file["frame_count"].as_u64().unwrap_or(1) > 1,
            },
        );
    }
    Some(emotes)
}
fn valid_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
