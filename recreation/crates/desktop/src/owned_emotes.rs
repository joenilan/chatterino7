//! Authenticated account-owned Twitch emotes. No catalog presence implies ownership.
use crate::{auth::CLIENT_ID, live::Identity, media::EmoteKey, twitch_assets::Choice};
use serde_json::Value;
use std::{collections::{HashMap, HashSet}, io::Read, sync::{mpsc, Arc, atomic::{AtomicU64, Ordering}}, time::{Duration, Instant}};
#[derive(Clone)]
pub struct Owned { pub owner: String, pub choice: Choice }
#[derive(Clone)]
struct Continuation { started: Instant, cursor: String, seen: HashSet<String>, items: HashMap<(String,String),Owned>, pages: usize }
impl Default for Continuation {fn default()->Self{Self{started:Instant::now(),cursor:String::new(),seen:HashSet::new(),items:HashMap::new(),pages:0}}}
struct Job { epoch: u64, identity: Identity, continuation: Continuation }
struct Loaded { started: Instant, items: Vec<Owned>, partial: bool, continuation: Option<Continuation> }
pub struct Inventory {
    identity: Option<Identity>, epoch: Arc<AtomicU64>, tx: mpsc::SyncSender<Job>,
    rx: mpsc::Receiver<(u64, Result<Loaded, String>)>, inflight: bool,
    complete: bool, continuation: Option<Continuation>, next: Instant, expires: Option<Instant>, pub items: Vec<Owned>, pub status: String,
}
impl Inventory {
    pub fn new() -> Self {
        let (tx, jobs)=mpsc::sync_channel::<Job>(1); let (results,rx)=mpsc::sync_channel(2);
        let epoch=Arc::new(AtomicU64::new(0)); let current=epoch.clone();
        std::thread::spawn(move || {
            let Ok(client)=reqwest::blocking::Client::builder().timeout(Duration::from_secs(12))
                .connect_timeout(Duration::from_secs(5)).redirect(reqwest::redirect::Policy::none()).build() else{return;};
            while let Ok(job)=jobs.recv() {
                if current.load(Ordering::Relaxed)!=job.epoch {continue;}
                let epoch=job.epoch;
                let loaded=load(&client,job,&current);
                if results.send((epoch,loaded)).is_err(){break;}
            }
        });
        Self{identity:None,epoch,tx,rx,inflight:false,complete:false,continuation:None,next:Instant::now(),expires:None,items:vec![],status:"Sign in to load subscription emotes".into()}
    }
    pub fn pump(&mut self, identity: Option<Identity>) -> bool {
        let mut changed=false;
        if self.identity!=identity {
            self.epoch.fetch_add(1,Ordering::Relaxed); self.identity=identity;
            self.inflight=false; self.complete=false; self.continuation=None; self.items.clear(); self.expires=None; self.next=Instant::now(); changed=true;
            self.status=match &self.identity {
                None=>"Sign in to load subscription emotes",
                Some(i) if !i.can_read_emotes=>"Sign in again once to update this older login; subscription emotes then load automatically",
                _=>"Loading subscription emotes…",
            }.into();
        }
        if self.expires.is_some_and(|at|Instant::now()>=at) {self.items.clear();self.complete=false;self.expires=None;self.continuation=None;self.epoch.fetch_add(1,Ordering::Relaxed);self.inflight=false;self.next=Instant::now();changed=true;}
        if !self.inflight && Instant::now()>=self.next {
            if let Some(identity)=self.identity.as_ref().filter(|i|i.can_read_emotes) {
                if self.tx.try_send(Job{epoch:self.epoch.load(Ordering::Relaxed),identity:identity.clone(),continuation:self.continuation.clone().unwrap_or_default()}).is_ok(){self.inflight=true;}
            }
        }
        while let Ok((epoch,result))=self.rx.try_recv(){
            if epoch!=self.epoch.load(Ordering::Relaxed){continue;}
            self.inflight=false; self.next=Instant::now()+Duration::from_secs(300);
            match result {
                Ok(loaded)=>{
                    self.continuation=loaded.continuation;
                    if self.continuation.is_some(){self.next=Instant::now();}
                    let refreshing = self.complete && self.continuation.is_some();
                    if !refreshing {
                        self.items=loaded.items;self.expires=Some(loaded.started+Duration::from_secs(600));
                        self.complete=!loaded.partial;
                    }
                    self.status=if refreshing {format!("{} Twitch account emotes · refreshing…",self.items.len())}
                        else if self.continuation.is_some(){format!("{} Twitch account emotes · loading more…",self.items.len())}
                        else if loaded.partial {format!("{} Twitch account emotes · partial inventory",self.items.len())}
                        else{format!("{} Twitch account emotes",self.items.len())};
                }
                Err(error)=>{
                    // Retain same-account inventory only within its existing TTL.
                    if error.contains("permission") {self.items.clear();self.complete=false;self.expires=None;self.continuation=None;}
                    self.status=error;
                }
            }
            changed=true;
        }
        changed
    }
}
impl Drop for Inventory {fn drop(&mut self){self.epoch.fetch_add(1,Ordering::Relaxed);}}
fn load(client:&reqwest::blocking::Client,job:Job,current:&AtomicU64)->Result<Loaded,String>{
    let mut state=job.continuation;
    let deadline=Instant::now()+Duration::from_secs(90);
    for _ in 0..16 {
        if current.load(Ordering::Relaxed)!=job.epoch{return Err("Account changed".into());}
        if Instant::now()>=deadline {break;}
        if state.pages>=512{return Ok(Loaded{started:state.started,items:state.items.into_values().collect(),partial:true,continuation:None});}
        let mut request=client.get("https://api.twitch.tv/helix/chat/emotes/user")
            .header("Client-Id",CLIENT_ID).bearer_auth(&job.identity.access).query(&[("user_id",job.identity.user_id.as_str())]);
        if !state.cursor.is_empty(){request=request.query(&[("after",state.cursor.as_str())]);}
        let response=request.send().map_err(|_|"Twitch account emotes unavailable; retrying shortly")?;
        if !response.status().is_success(){return Err(match response.status().as_u16(){
            401|403=>"Twitch emote permission needs reconnecting in Account".into(),
            429=>"Twitch emote rate limit reached; retrying later".into(),
            code=>format!("Twitch account emotes unavailable (HTTP {code})"),
        });}
        let mut bytes=Vec::new();response.take(4*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"Twitch emote response interrupted")?;
        if bytes.len()>4*1024*1024{return Err("Twitch emote response exceeded the size limit".into());}
        let value:Value=serde_json::from_slice(&bytes).map_err(|_|"Twitch emote response was malformed")?;
        let data=value["data"].as_array().ok_or("Twitch emote list was missing")?;
        for emote in data {
            if state.items.len()>=20_000{return Ok(Loaded{started:state.started,items:state.items.into_values().collect(),partial:true,continuation:None});}
            let Some(label)=emote["name"].as_str().filter(|s|!s.is_empty()&&s.len()<=128&&!s.chars().any(char::is_whitespace))else{continue;};
            let animated=emote["format"].as_array().is_some_and(|a|a.iter().any(|f|f=="animated"));
            let Some(key)=emote["id"].as_str().and_then(|id|EmoteKey::twitch(id,animated))else{continue;};
            let owner=emote["owner_id"].as_str().filter(|s|s.len()<=32&&s.bytes().all(|b|b.is_ascii_digit())).unwrap_or("").to_owned();
            // No broadcaster_id context is requested: these are account-wide usable emotes.
            // Twitch also returns follower emotes unlocked by a subscription here.
            // Preserve those; only extra broadcaster-context queries need local scoping.
            state.items.insert((key.id.clone(),label.to_owned()),Owned{owner,choice:Choice{label:label.into(),provider:"Twitch",key:Some(key)}});
        }
        state.pages+=1;
        let next=value["pagination"]["cursor"].as_str().unwrap_or("");
        if next.is_empty(){return Ok(Loaded{started:state.started,items:state.items.into_values().collect(),partial:false,continuation:None});}
        if next.len()>2048||!state.seen.insert(next.to_owned()){return Err("Twitch returned a repeated or invalid emote cursor".into());}
        state.cursor=next.into();
    }
    Ok(Loaded{started:state.started,items:state.items.values().cloned().collect(),partial:true,continuation:Some(state)})
}
