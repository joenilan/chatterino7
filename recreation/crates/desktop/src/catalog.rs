//! Public 7TV catalogs; no Twitch bearer tokens or account credentials.
use chat_core::{EmoteAsset, Fragment};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone,PartialEq,Eq)]
struct Emote {
    id: String,
    asset: EmoteAsset,
    overlay: bool,
    animated: bool,
}
type Emotes = HashMap<String, Emote>;
struct Loaded {emotes:Emotes,set:String,owner:String}
pub struct Catalog {
    global: Emotes,
    personal:Emotes,
    personal_user:Option<String>,
    personal_ready:bool,
    personal_epoch:u64,
    watches:HashMap<String,crate::seven_events::Watch>,
    events:crate::seven_events::Events,
    dirty:HashSet<String>,
    in_flight:HashSet<String>,
    retry_after:HashMap<String,Instant>,
    pub twitch: crate::twitch_assets::TwitchAssets,
    pub profiles:crate::profiles::Profiles,
    pub community: crate::community::Community,
    channels: HashMap<String, Emotes>,
    requested: HashMap<String, (String, Instant)>,
    global_requested: Instant,
    tx: mpsc::SyncSender<(String, String, u64)>,
    rx: mpsc::Receiver<(String, u64, Option<Loaded>)>,
}
impl Catalog {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<(String, String, u64)>(32);
        let (results, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(12))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok((channel, id, generation)) = jobs.recv() {
                let url = if channel.is_empty() {
                    "https://7tv.io/v3/emote-sets/global".to_owned()
                } else {
                    format!("https://7tv.io/v3/users/twitch/{id}")
                };
                let result = if channel.starts_with('@'){load_personal(&client,&url)}else{load(&client, &url, channel.is_empty())};
                if results.send((channel, generation, result)).is_err() {
                    return;
                }
            }
        });
        let _ = tx.try_send((String::new(), String::new(), 0));
        Self {
            global: HashMap::new(),
            personal:HashMap::new(),personal_user:None,personal_ready:false,personal_epoch:0,
            watches:HashMap::new(),events:crate::seven_events::Events::new(),dirty:HashSet::new(),
            in_flight:HashSet::from([String::new()]),retry_after:HashMap::new(),
            twitch: crate::twitch_assets::TwitchAssets::new(),
            profiles:crate::profiles::Profiles::new(),
            community: crate::community::Community::new(),
            channels: HashMap::new(),
            requested: HashMap::new(),
            global_requested: Instant::now(),
            tx,
            rx,
        }
    }
    pub fn inspection(&self) -> Value {
        serde_json::json!({"seventv_live_updates":self.events.connected(),"seventv_update_status":self.events.status(),"community":self.community.inspection(),"twitch_global_emotes":self.twitch.global.emotes.len(),"twitch_global_badges":self.twitch.global.badges.len(),"twitch_channel_badges":self.twitch.channels.iter().map(|(n,a)|(n.clone(),a.badges.len())).collect::<HashMap<_,_>>(),"personal_emotes":self.personal.len(),"personal_catalog_loaded":self.personal_ready,"personal_scope":"signed-in owned set","global_emotes":self.global.len(),"channel_emotes":self.channels.iter().map(|(name,emotes)|(name.clone(),emotes.len())).collect::<HashMap<_,_>>()})
    }
    pub fn channel(&mut self, name: &str, id: &str) {
        if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return;
        }
        if self.retry_after.get(name).is_some_and(|at|Instant::now()<*at){return;}
        if self
            .requested
            .get(name)
            .is_some_and(|(_, t)| t.elapsed() < Duration::from_secs(300))
        {
            return;
        }
        if !self.in_flight.contains(name) && self.tx.try_send((name.into(), id.into(), if name.starts_with('@'){self.personal_epoch}else{0})).is_ok() {
            self.in_flight.insert(name.into());
            self.requested
                .insert(name.into(), (id.into(), Instant::now()));
        }
    }
    pub fn pump(&mut self, active: HashSet<String>, identity: Option<crate::live::Identity>) -> bool {
        let user=identity.as_ref().map(|i|i.user_id.clone());
        let account_changed=self.personal_user!=user;
        if account_changed {self.personal_epoch=self.personal_epoch.wrapping_add(1);self.personal.clear();self.personal_ready=false;self.personal_user=user;}
        let personal_key=self.personal_user.as_ref().map(|id|format!("@{id}"));
        let wanted=|k:&String|k.is_empty()||active.contains(k)||personal_key.as_ref()==Some(k);
        self.channels.retain(|k, _| active.contains(k));
        self.watches.retain(|k,_|wanted(k));
        self.dirty.retain(|k|wanted(k));
        self.retry_after.retain(|k,_|wanted(k));
        self.dirty.extend(self.events.pump(self.watches.values().cloned().collect()));
        self.requested.retain(|k, _| wanted(k));
        if let Some(id)=self.personal_user.clone(){self.channel(&format!("@{id}"),&id);}
        if (self.global_requested.elapsed() >= Duration::from_secs(300) || (self.dirty.contains("") && self.global_requested.elapsed()>=Duration::from_secs(5)))
            && !self.in_flight.contains("") && self.retry_after.get("").is_none_or(|at|Instant::now()>=*at)
            && self.tx.try_send((String::new(), String::new(), 0)).is_ok()
        {
            self.global_requested = Instant::now();self.dirty.remove("");self.in_flight.insert(String::new());
        }
        for (name, (id, at)) in &mut self.requested {
            if (at.elapsed() >= Duration::from_secs(300) || (self.dirty.contains(name) && at.elapsed()>=Duration::from_secs(5)))
                && !self.in_flight.contains(name) && self.retry_after.get(name).is_none_or(|at|Instant::now()>=*at)
                && self.tx.try_send((name.clone(), id.clone(), if name.starts_with('@'){self.personal_epoch}else{0})).is_ok()
            {
                *at = Instant::now();self.dirty.remove(name);self.in_flight.insert(name.clone());
            }
        }
        let channel_requests=self.requested.iter().filter(|(name,_)|!name.starts_with('@')).map(|(name,value)|(name.clone(),value.clone())).collect();
        let mut changed = account_changed | self.twitch.pump(identity, &channel_requests);
        changed |= self.community.pump(&channel_requests);
        while let Ok((name, generation, result)) = self.rx.try_recv() {
            self.in_flight.remove(&name);
            if name.starts_with('@')&&generation!=self.personal_epoch{continue;}
            if result.is_none() && wanted(&name) {self.retry_after.insert(name.clone(),Instant::now()+Duration::from_secs(60));self.dirty.insert(name.clone());}else{self.retry_after.remove(&name);}
            if let Some(loaded) = result {
                if wanted(&name){self.watches.insert(name.clone(),crate::seven_events::Watch{channel:name.clone(),set:loaded.set,owner:loaded.owner});}
                let emotes=loaded.emotes;
                if personal_key.as_ref()==Some(&name){
                    self.personal_ready=true;
                    if self.personal!=emotes{self.personal=emotes;changed=true;}
                } else if name.is_empty() {
                    if self.global!=emotes{self.global = emotes;changed = true;}
                } else if active.contains(&name) {
                    if self.channels.get(&name)!=Some(&emotes){self.channels.insert(name, emotes);changed = true;}
                }
            }
        }
        changed
    }
    fn lookup(&self, channel:&str, token:&str, sender:Option<&str>)->Option<Fragment> {
        let make_seven=|e:&Emote|Fragment::Emote{provider:"7tv".into(),id:e.id.clone(),label:token.into(),overlay:e.overlay,animated:e.animated,asset:Some(e.asset.clone())};
        let local=self.community.channels.get(channel);
        sender.filter(|id|Some(*id)==self.personal_user.as_deref()).and_then(|_|self.personal.get(token)).map(make_seven)
            .or_else(||local.and_then(|s|s.ffz.get(token)).or_else(||local.and_then(|s|s.bttv.get(token))).cloned())
            .or_else(||self.channels.get(channel).and_then(|s|s.get(token)).map(make_seven))
            .or_else(||self.community.global.ffz.get(token).or_else(||self.community.global.bttv.get(token)).cloned())
            .or_else(||self.global.get(token).map(make_seven))
    }
    pub fn choices(&self,channel:&str,query:&str,limit:usize)->Vec<crate::twitch_assets::Choice>{
        let query=query.to_lowercase();let mut names=HashSet::new();
        names.extend(self.global.keys().cloned());
        names.extend(self.personal.keys().cloned());
        if let Some(set)=self.channels.get(channel){names.extend(set.keys().cloned());}
        for set in std::iter::once(&self.community.global).chain(self.community.channels.get(channel)) {names.extend(set.ffz.keys().cloned());names.extend(set.bttv.keys().cloned());}
        let mut choices=HashMap::new();
        for name in names {if !name.to_lowercase().contains(&query){continue;}
            if let Some(Fragment::Emote{provider,id,animated,asset:Some(asset),..})=self.lookup(channel,&name,self.personal_user.as_deref()){
                if let Some(key)=crate::media::EmoteKey::community(&provider,&id,animated,&asset){
                    let provider=match provider.as_str(){"bttv"=>"BTTV","ffz"=>"FFZ",_ if self.personal.contains_key(&name)=>"7TV Personal",_=>"7TV"};
                    choices.insert(name.clone(),crate::twitch_assets::Choice{label:name,provider,key:Some(key)});
                }
            }
        }
        for emote in &self.twitch.global.emotes {if emote.label.to_lowercase().contains(&query){choices.insert(emote.label.clone(),emote.clone());}}
        let mut choices:Vec<_>=choices.into_values().collect();
        choices.sort_by(|a,b|{let al=a.label.to_lowercase();let bl=b.label.to_lowercase();(!al.starts_with(&query),al,&a.label).cmp(&(!bl.starts_with(&query),bl,&b.label))});
        choices.truncate(limit);choices
    }
    pub fn expand(&self, channel: &str, sender:&str, fragments: &[Fragment]) -> Vec<Fragment> {
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
                    if let Some(emote)=self.lookup(channel,token,Some(sender)) {
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
fn fetch(client: &reqwest::blocking::Client, url: &str) -> Option<Value> {
    let response = client.get(url).send().ok()?;
    if response.status() == 404 {
        return Some(Value::Null);
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
    serde_json::from_slice(&bytes).ok()
}
fn load(client:&reqwest::blocking::Client,url:&str,global:bool)->Option<Loaded>{parse(&fetch(client,url)?,global,false)}
fn load_personal(client:&reqwest::blocking::Client,url:&str)->Option<Loaded>{
    let user=fetch(client,url)?;
    if !user.is_null()&&(!user["user"].is_object()||user["id"].as_str()!=url.rsplit('/').next()||user["platform"].as_str()!=Some("TWITCH")){return None;}
    let owner=user["user"]["id"].as_str().filter(|s|!s.is_empty()&&s.len()<=128&&s.bytes().all(|b|b.is_ascii_alphanumeric())).unwrap_or("").to_owned();
    if !user.is_null()&&owner.is_empty(){return None;}
    let set=user["user"]["emote_sets"].as_array().into_iter().flatten().take(128).find(|set|set["flags"].as_u64().unwrap_or(0)&4!=0).and_then(|set|set["id"].as_str()).filter(|s|!s.is_empty()&&s.len()<=128&&s.bytes().all(|b|b.is_ascii_alphanumeric()));
    let Some(set)=set else{return Some(Loaded{emotes:HashMap::new(),set:String::new(),owner});};
    let data=fetch(client,&format!("https://7tv.io/v3/emote-sets/{set}"))?;
    if !data.is_null()&&data["id"].as_str()!=Some(set){return None;}
    if !data.is_null()&&(data["flags"].as_u64().unwrap_or(0)&4==0||data["owner"]["id"].as_str().is_some_and(|id|id!=owner)){
        return Some(Loaded{emotes:HashMap::new(),set:String::new(),owner});
    }
    let mut loaded=parse(&data,true,true)?;loaded.owner=owner;Some(loaded)
}
fn parse(json:&Value,global:bool,personal:bool)->Option<Loaded>{
    if json.is_null(){return Some(Loaded{emotes:HashMap::new(),set:String::new(),owner:String::new()});}
    let node=if global{json}else{&json["emote_set"]};
    if !node.is_null()&&(!node["id"].as_str().is_some_and(|id|!id.is_empty())||node.get("emotes").is_some_and(|v|!v.is_array())){return None;}
    if !global&&!json["user"].is_object(){return None;}
    let list = if global {
        json["emotes"].as_array()
    } else {
        json["emote_set"]["emotes"].as_array()
    };
    let mut emotes = HashMap::new();
    for entry in list.into_iter().flatten().take(2000) {
        if personal && (entry["data"]["listed"].as_bool()!=Some(true)||!entry["data"]["state"].as_array().is_some_and(|states|states.iter().any(|s|s=="PERSONAL"))||entry["data"]["flags"].as_u64().unwrap_or(0)&(1<<24)!=0){continue;}
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
    let valid_id=|v:&Value|v.as_str().filter(|s|!s.is_empty()&&s.len()<=128&&s.bytes().all(|b|b.is_ascii_alphanumeric())).unwrap_or("").to_owned();
    Some(Loaded{emotes,set:valid_id(if global{&json["id"]}else{&json["emote_set"]["id"]}),owner:if global{String::new()}else{valid_id(&json["user"]["id"])}})
}
fn valid_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
