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
    favorites: Vec<[String; 3]>,
    global_state:crate::community::LoadState,
    last_manual_refresh:Option<Instant>,
    personal:Emotes,
    personal_user:Option<String>,
    personal_ready:bool,
    personal_epoch:u64,
    entitlements:crate::seven_entitlements::Entitlements,
    pub badges:crate::seven_badges::Badges,
    entitled_sets:HashMap<String,Emotes>,
    event_versions:HashMap<String,u64>,
    event_revision:u64,
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
                } else if channel.starts_with('#') {
                    format!("https://7tv.io/v3/emote-sets/{id}")
                } else {
                    format!("https://7tv.io/v3/users/twitch/{id}")
                };
                let result = if channel.starts_with('@'){load_personal(&client,&url)}else if channel.starts_with('#'){load_entitled(&client,&url,&id)}else{load(&client, &url, channel.is_empty())};
                if results.send((channel, generation, result)).is_err() {
                    return;
                }
            }
        });
        let _ = tx.try_send((String::new(), String::new(), 0));
        Self {
            global: HashMap::new(),
            favorites: Vec::new(),
            global_state:crate::community::LoadState::Loading,last_manual_refresh:None,
            personal:HashMap::new(),personal_user:None,personal_ready:false,personal_epoch:0,
            entitlements:Default::default(),badges:Default::default(),entitled_sets:HashMap::new(),event_versions:HashMap::new(),event_revision:0,
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
        serde_json::json!({"seventv_live_updates":self.events.connected(),"seventv_update_status":self.events.status(),"community":self.community.inspection(),"twitch_global_emotes":self.twitch.global.emotes.len(),"twitch_global_badges":self.twitch.global.badges.len(),"twitch_channel_badges":self.twitch.channels.iter().map(|(n,a)|(n.clone(),a.badges.len())).collect::<HashMap<_,_>>(),"personal_emotes":self.personal.len(),"personal_catalog_loaded":self.personal_ready,"personal_scope":"owned set plus passive session grants","seventv_badges":self.badges.inspection(),"seventv_entitled_senders":self.entitlements.len(),"seventv_entitled_sets":self.entitled_sets.len(),"seventv_entitled_emotes":self.entitled_sets.values().map(HashMap::len).sum::<usize>(),"global_emotes":self.global.len(),"channel_emotes":self.channels.iter().map(|(name,emotes)|(name.clone(),emotes.len())).collect::<HashMap<_,_>>()})
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
        let old_global_state=self.global_state;
        let user=identity.as_ref().map(|i|i.user_id.clone());
        let account_changed=self.personal_user!=user;
        if account_changed {self.personal_epoch=self.personal_epoch.wrapping_add(1);self.personal.clear();self.personal_ready=false;self.personal_user=user;}
        let personal_key=self.personal_user.as_ref().map(|id|format!("@{id}"));
        let channels=self.requested.iter().filter(|(name,_)|active.contains(*name)).map(|(_, (id,_))|id.clone()).collect();
        let (dirty,events)=self.events.pump(self.watches.values().cloned().collect(),channels);
        self.dirty.extend(dirty);
        let mut event_changed=self.entitlements.expire() | self.badges.expire();
        for event in events {
            event_changed |= self.entitlements.apply(&event) | self.badges.apply(&event);
            match event {
                crate::seven_entitlements::Change::Reset => {
                    self.event_revision=self.event_revision.wrapping_add(1);
                    event_changed |= !self.entitled_sets.is_empty();self.entitled_sets.clear();self.event_versions.clear();self.requested.retain(|k,_|!k.starts_with('#'));self.retry_after.retain(|k,_|!k.starts_with('#'));
                }
                crate::seven_entitlements::Change::Set{id,removed} => {
                    // Refetch complete authoritative sets instead of guessing sparse patch semantics.
                    if self.event_versions.contains_key(&id){
                        self.event_revision=self.event_revision.wrapping_add(1);self.event_versions.insert(id.clone(),self.event_revision);
                        event_changed |= self.entitled_sets.remove(&id).is_some();
                        if !removed{self.dirty.insert(format!("#{id}"));}
                    }
                }
                _=>{}
            }
        }
        let entitled=self.entitlements.sets();
        self.entitled_sets.retain(|id,_|entitled.contains(id));
        self.event_versions.retain(|id,_|entitled.contains(id));
        for id in &entitled {
            if !self.event_versions.contains_key(id){self.event_revision=self.event_revision.wrapping_add(1);self.event_versions.insert(id.clone(),self.event_revision);}
            let name=format!("#{id}");
            if !self.requested.contains_key(&name)&&!self.in_flight.contains(&name)&&self.tx.try_send((name.clone(),id.clone(),self.event_versions[id])).is_ok(){self.in_flight.insert(name.clone());self.requested.insert(name,(id.clone(),Instant::now()));}
        }
        let wanted=|k:&String|k.is_empty()||active.contains(k)||personal_key.as_ref()==Some(k)||k.strip_prefix('#').is_some_and(|id|entitled.contains(id));
        self.channels.retain(|k, _| active.contains(k));
        self.watches.retain(|k,_|wanted(k));
        self.dirty.retain(|k|wanted(k));
        self.retry_after.retain(|k,_|wanted(k));

        self.requested.retain(|k, _| wanted(k));
        if let Some(id)=self.personal_user.clone(){self.channel(&format!("@{id}"),&id);}
        if (self.global_requested.elapsed() >= Duration::from_secs(300) || (self.dirty.contains("") && self.global_requested.elapsed()>=Duration::from_secs(5)))
            && !self.in_flight.contains("") && self.retry_after.get("").is_none_or(|at|Instant::now()>=*at)
            && self.tx.try_send((String::new(), String::new(), 0)).is_ok()
        {
            self.global_state=crate::community::LoadState::Loading;
            self.global_requested = Instant::now();self.dirty.remove("");self.in_flight.insert(String::new());
        }
        for (name, (id, at)) in &mut self.requested {
            if (at.elapsed() >= Duration::from_secs(300) || (self.dirty.contains(name) && at.elapsed()>=Duration::from_secs(5)))
                && !self.in_flight.contains(name) && self.retry_after.get(name).is_none_or(|at|Instant::now()>=*at)
                && self.tx.try_send((name.clone(), id.clone(), if name.starts_with('@'){self.personal_epoch}else if let Some(id)=name.strip_prefix('#'){self.event_versions.get(id).copied().unwrap_or(0)}else{0})).is_ok()
            {
                *at = Instant::now();self.dirty.remove(name);self.in_flight.insert(name.clone());
            }
        }
        let channel_requests=self.requested.iter().filter(|(name,_)|!name.starts_with('@')&&!name.starts_with('#')).map(|(name,value)|(name.clone(),value.clone())).collect();
        let mut changed = event_changed | account_changed | self.twitch.pump(identity, &channel_requests);
        changed |= self.community.pump(&channel_requests);
        while let Ok((name, generation, result)) = self.rx.try_recv() {
            self.in_flight.remove(&name);
            if name.is_empty(){self.global_state=if result.is_some(){crate::community::LoadState::Ready}else{crate::community::LoadState::Failed};}
            if name.starts_with('@')&&generation!=self.personal_epoch{continue;}
            if let Some(id)=name.strip_prefix('#'){if self.event_versions.get(id)!=Some(&generation){if wanted(&name){self.dirty.insert(name.clone());}continue;}}
            if result.is_none() && wanted(&name) {self.retry_after.insert(name.clone(),Instant::now()+Duration::from_secs(60));self.dirty.insert(name.clone());}else{self.retry_after.remove(&name);}
            if let Some(loaded) = result {
                if wanted(&name)&&!name.starts_with('#'){self.watches.insert(name.clone(),crate::seven_events::Watch{channel:name.clone(),set:loaded.set,owner:loaded.owner});}
                let emotes=loaded.emotes;
                if let Some(id)=name.strip_prefix('#') {
                    if entitled.contains(id){
                        let remaining=10000usize.saturating_sub(self.entitled_sets.iter().filter(|(key,_)|key.as_str()!=id).map(|(_,v)|v.len()).sum());
                        let mut entries=emotes.into_iter().collect::<Vec<_>>();entries.sort_by(|a,b|a.0.cmp(&b.0));entries.truncate(remaining);
                        let emotes=entries.into_iter().collect();
                        if self.entitled_sets.get(id)!=Some(&emotes){self.entitled_sets.insert(id.into(),emotes);changed=true;}
                    }
                } else if personal_key.as_ref()==Some(&name){
                    self.personal_ready=true;
                    if self.personal!=emotes{self.personal=emotes;changed=true;}
                } else if name.is_empty() {
                    if self.global!=emotes{self.global = emotes;changed = true;}
                } else if active.contains(&name) {
                    if self.channels.get(&name)!=Some(&emotes){self.channels.insert(name, emotes);changed = true;}
                }
            }
        }
        changed || old_global_state!=self.global_state
    }
    fn lookup(&self, channel:&str, token:&str, sender:Option<&str>)->Option<Fragment> {
        let make_seven=|e:&Emote|Fragment::Emote{provider:"7tv".into(),id:e.id.clone(),label:token.into(),overlay:e.overlay,animated:e.animated,asset:Some(e.asset.clone())};
        let local=self.community.channels.get(channel);
        sender.filter(|id|Some(*id)==self.personal_user.as_deref()).and_then(|_|self.personal.get(token)).map(make_seven)
            .or_else(||sender.and_then(|id|self.entitlements.for_user(id).find_map(|set|self.entitled_sets.get(set).and_then(|s|s.get(token)))).map(make_seven))
            .or_else(||local.and_then(|s|s.ffz.get(token)).or_else(||local.and_then(|s|s.bttv.get(token))).cloned())
            .or_else(||self.channels.get(channel).and_then(|s|s.get(token)).map(make_seven))
            .or_else(||self.community.global.ffz.get(token).or_else(||self.community.global.bttv.get(token)).cloned())
            .or_else(||self.global.get(token).map(make_seven))
    }
    pub fn browser_status(&self,service:&str)->String {
        let states=[("7TV",self.global_state),("BTTV",self.community.global_bttv_state),("FFZ",self.community.global_ffz_state)];
        let mut labels=states.into_iter().filter(|(name,_)|service.is_empty()||service==*name).map(|(name,state)|format!("{name}: {}",state.label())).collect::<Vec<_>>();
        if service.is_empty()||service=="Twitch"{labels.push(if self.personal_user.is_none(){"Twitch: sign in to load globals".into()}else if self.twitch.global.emotes.is_empty(){"Twitch: globals not loaded yet".into()}else{"Twitch: globals loaded".into()});}
        labels.join(" · ")
    }
    pub fn can_retry_public(&self)->bool {
        use crate::community::LoadState;
        self.last_manual_refresh.is_none_or(|at|at.elapsed()>=Duration::from_secs(10)) &&
        [self.global_state,self.community.global_bttv_state,self.community.global_ffz_state].contains(&LoadState::Failed)
    }
    pub fn retry_public(&mut self)->bool {
        if !self.can_retry_public(){return false;}
        self.last_manual_refresh=Some(Instant::now());
        if !self.in_flight.contains(""){self.dirty.insert(String::new());self.retry_after.remove("");self.global_requested=Instant::now()-Duration::from_secs(6);self.global_state=crate::community::LoadState::Loading;}
        self.community.retry_global();true
    }
    fn favorite_key(choice: &crate::twitch_assets::Choice) -> Option<[String; 3]> {
        Some([choice.provider.into(), choice.key.as_ref()?.id.clone(), choice.label.clone()])
    }
    pub fn restore_favorites(&mut self, value: &Value) {
        self.favorites.clear();
        for value in value.as_array().into_iter().flatten().take(256) {
            if let Ok(key) = serde_json::from_value::<[String; 3]>(value.clone()) {
                if matches!(key[0].as_str(), "Twitch" | "7TV" | "BTTV" | "FFZ")
                    && !key[1].is_empty() && key[1].len() <= 512
                    && !key[2].is_empty() && key[2].len() <= 256
                    && !self.favorites.contains(&key) { self.favorites.push(key); }
            }
        }
    }
    pub fn favorites_json(&self) -> Value { serde_json::json!(self.favorites) }
    pub fn is_favorite(&self, choice: &crate::twitch_assets::Choice) -> bool {
        Self::favorite_key(choice).is_some_and(|key| self.favorites.contains(&key))
    }
    pub fn remove_saved_favorite(&mut self, key: &[String; 3]) { self.favorites.retain(|saved| saved != key); }
    pub fn toggle_favorite(&mut self, choice: &crate::twitch_assets::Choice) -> Result<bool, &'static str> {
        let key = Self::favorite_key(choice).ok_or("This emote has no stable asset identity.")?;
        if key[1].len() > 512 || key[2].len() > 256 {
            return Err("This emote identity is too long to save safely.");
        }
        if let Some(index) = self.favorites.iter().position(|saved| saved == &key) {
            self.favorites.remove(index);
            return Ok(false);
        }
        if self.favorites.len() >= 256 { return Err("Your 256-emote favorites collection is full. Remove a favorite first."); }
        self.favorites.push(key);
        Ok(true)
    }
    pub fn browser_collections(&mut self,current:&str)->Vec<crate::emote_picker::Collection>{
        use crate::emote_picker::{BrowserChoice,Collection};
        use crate::twitch_assets::Choice;
        let effective=self.choices(current,"",usize::MAX).into_iter().map(|c|(c.label,c.key)).collect::<HashMap<_,_>>();
        let seven=|set:&Emotes|set.iter().filter_map(|(name,e)|Some(Choice{label:name.clone(),provider:"7TV",key:Some(crate::media::EmoteKey::seven(&e.id,e.animated,&e.asset)?)})).collect::<Vec<_>>();
        let community=|set:&crate::community::Set|set.ffz.iter().chain(set.bttv.iter()).filter_map(|(name,f)|{
            if let Fragment::Emote{provider,id,animated,asset:Some(asset),..}=f{Some(Choice{label:name.clone(),provider:if provider=="ffz"{"FFZ"}else{"BTTV"},key:Some(crate::media::EmoteKey::community(provider,id,*animated,asset)?)})}else{None}
        }).collect::<Vec<_>>();
        let pack=|id:String,title:String,user:Option<String>,choices:Vec<Choice>|{
            let mut items=choices.into_iter().map(|choice|BrowserChoice{saved_key:None,available:effective.get(&choice.label)==Some(&choice.key),origin:title.clone(),choice}).collect::<Vec<_>>();
            items.sort_by(|a,b|(a.choice.provider,a.choice.label.to_lowercase()).cmp(&(b.choice.provider,b.choice.label.to_lowercase())));
            items.dedup_by(|a,b|a.choice.provider==b.choice.provider&&a.choice.label==b.choice.label&&a.choice.key==b.choice.key);
            Collection{id,title,user,items}
        };
        let mut global=seven(&self.global);global.extend(community(&self.community.global));global.extend(self.twitch.global.emotes.clone());
        let mut result=vec![pack("global".into(),"Global".into(),None,global)];
        let mut personal=seven(&self.personal);
        if let Some(user)=&self.personal_user{for id in self.entitlements.for_user(user){if let Some(set)=self.entitled_sets.get(id){personal.extend(seven(set));}}}
        if let Some(user)=self.personal_user.clone(){self.profiles.request(&user);}
        if !personal.is_empty(){result.push(pack("personal".into(),"Personal".into(),self.personal_user.clone(),personal));}
        let mut channels=self.requested.iter().filter(|(name,_)|!name.is_empty()&&!name.starts_with(['@','#'])).map(|(name,(id,_))|(name.clone(),id.clone())).collect::<Vec<_>>();
        channels.sort_by(|a,b|(a.0!=current,&a.0).cmp(&(b.0!=current,&b.0)));
        for(name,id)in channels{
            let mut choices=self.channels.get(&name).map(seven).unwrap_or_default();
            if let Some(set)=self.community.channels.get(&name){choices.extend(community(set));}
            self.profiles.request(&id);
            result.push(pack(format!("channel:{name}"),format!("#{name}"),Some(id),choices));
        }
        let all=result.iter().filter(|c|c.id=="global"||c.id=="personal"||c.id==format!("channel:{current}")).flat_map(|c|c.items.iter().filter(|i|i.available).cloned()).collect();
        let mut seen=HashSet::new();
        let mut favorites:Vec<BrowserChoice>=result.iter().flat_map(|c|c.items.iter()).filter(|item|self.is_favorite(&item.choice))
            .filter(|item|Self::favorite_key(&item.choice).is_some_and(|key|seen.insert(key)))
            .cloned().collect();
        for key in &self.favorites {
            if !seen.contains(key) {
                let provider=match key[0].as_str(){"Twitch"=>"Twitch","7TV"=>"7TV","BTTV"=>"BTTV","FFZ"=>"FFZ",_=>continue};
                favorites.push(BrowserChoice{choice:Choice{provider,label:key[2].clone(),key:None},
                    origin:"Unavailable source".into(),available:false,saved_key:Some(key.clone())});
            }
        }
        result.insert(0,Collection{id:"favorites".into(),title:"Favorites".into(),user:None,items:favorites});
        result.insert(0,Collection{id:"all".into(),title:"Available here".into(),user:None,items:all});
        result
    }
    pub fn choices(&self,channel:&str,query:&str,limit:usize)->Vec<crate::twitch_assets::Choice>{
        let query=query.to_lowercase();let mut names=HashSet::new();
        names.extend(self.global.keys().cloned());
        names.extend(self.personal.keys().cloned());
        if let Some(user)=&self.personal_user{for id in self.entitlements.for_user(user){if let Some(set)=self.entitled_sets.get(id){names.extend(set.keys().cloned());}}}
        if let Some(set)=self.channels.get(channel){names.extend(set.keys().cloned());}
        for set in std::iter::once(&self.community.global).chain(self.community.channels.get(channel)) {names.extend(set.ffz.keys().cloned());names.extend(set.bttv.keys().cloned());}
        let mut choices=HashMap::new();
        for name in names {if !name.to_lowercase().contains(&query){continue;}
            if let Some(Fragment::Emote{provider,id,animated,asset:Some(asset),..})=self.lookup(channel,&name,self.personal_user.as_deref()){
                if let Some(key)=crate::media::EmoteKey::community(&provider,&id,animated,&asset){
                    let provider=match provider.as_str(){"bttv"=>"BTTV","ffz"=>"FFZ",_ if self.personal.contains_key(&name)||self.personal_user.as_deref().is_some_and(|u|self.entitlements.for_user(u).any(|id|self.entitled_sets.get(id).is_some_and(|set|set.contains_key(&name))))=>"7TV Personal",_=>"7TV"};
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
fn load_entitled(client:&reqwest::blocking::Client,url:&str,id:&str)->Option<Loaded>{
    let data=fetch(client,url)?;
    if !data.is_null()&&data["id"].as_str()!=Some(id){return None;}
    if !data.is_null()&&data["flags"].as_u64().unwrap_or(0)&12==0{return Some(Loaded{emotes:HashMap::new(),set:String::new(),owner:String::new()});}
    parse(&data,true,true)
}
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
