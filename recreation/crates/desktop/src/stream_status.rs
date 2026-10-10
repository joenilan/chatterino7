//! Broadcast status is distinct from chat connectivity. Reuse the existing Helix grant.
use crate::{auth::CLIENT_ID,live::Identity};
use std::{collections::HashMap,io::Read,sync::mpsc,time::{Duration,Instant}};
use serde_json::Value;
#[derive(Clone,Default)]
pub struct StreamInfo {pub live:bool,pub title:String,pub category:String,pub viewers:u64,pub started_at:String,pub language:String}
impl StreamInfo {
    pub fn uptime(&self) -> Option<String> {
        if !self.live { return None; }
        let started = chrono::DateTime::parse_from_rfc3339(&self.started_at).ok()?;
        let seconds = (chrono::Utc::now() - started.with_timezone(&chrono::Utc)).num_seconds();
        if seconds < 0 { return None; }
        let minutes = seconds / 60;
        Some(if minutes < 1 { "less than a minute".into() }
            else if minutes < 60 { format!("{minutes}m") }
            else if minutes < 1440 { format!("{}h {}m", minutes / 60, minutes % 60) }
            else { format!("{}d {}h {}m", minutes / 1440, minutes / 60 % 24, minutes % 60) })
    }
    pub fn local_start(&self) -> Option<String> {
        chrono::DateTime::parse_from_rfc3339(&self.started_at).ok()
            .map(|at| at.with_timezone(&chrono::Local).format("%b %d, %Y · %H:%M %:z").to_string())
    }
}
struct Job {epoch:u64,identity:Identity,channels:Vec<String>}
pub struct Streams {
    identity:Option<Identity>,channels:Vec<String>,epoch:u64,pending:bool,last:Instant,
    values:HashMap<String,(StreamInfo,Instant)>,tx:mpsc::SyncSender<Job>,rx:mpsc::Receiver<(u64,Option<HashMap<String,StreamInfo>>)>,
}
impl Streams {
    pub fn new()->Self {
        let(tx,jobs)=mpsc::sync_channel::<Job>(1);let(results,rx)=mpsc::sync_channel(2);
        std::thread::spawn(move||{
            let Ok(client)=reqwest::blocking::Client::builder().timeout(Duration::from_secs(12)).connect_timeout(Duration::from_secs(5)).redirect(reqwest::redirect::Policy::none()).build() else{return;};
            while let Ok(job)=jobs.recv(){let value=load(&client,&job.identity,&job.channels);if results.send((job.epoch,value)).is_err(){break;}}
        });
        Self{identity:None,channels:vec![],epoch:0,pending:false,last:Instant::now()-Duration::from_secs(60),values:HashMap::new(),tx,rx}
    }
    pub fn get(&self,name:&str)->Option<bool>{self.values.get(name).filter(|(_,at)|at.elapsed()<Duration::from_secs(180)).map(|(info,_)|info.live)}
    pub fn detail(&self,name:&str)->Option<(&StreamInfo,u64)>{self.values.get(name).filter(|(_,at)|at.elapsed()<Duration::from_secs(180)).map(|(info,at)|(info,at.elapsed().as_secs()))}
    pub fn summary(&self,name:&str)->String {
        match self.detail(name){Some((info,_)) if info.live=>format!("{} · {} · {} viewers",info.title,info.category,info.viewers),Some(_)=>"Stream offline · chat may still be connected".into(),None=>"Stream metadata unavailable or refreshing".into()}
    }
    pub fn pump(&mut self,identity:Option<Identity>,mut channels:Vec<String>)->bool {
        channels.sort();channels.dedup();let mut changed=false;
        if self.identity!=identity||self.channels!=channels {
            if self.identity.as_ref().map(|i|&i.user_id)!=identity.as_ref().map(|i|&i.user_id){self.values.clear();}
            self.identity=identity;self.channels=channels;self.epoch+=1;
            self.values.retain(|n,_|self.channels.contains(n));self.last=Instant::now()-Duration::from_secs(60);changed=true;
        }
        while let Ok((epoch,result))=self.rx.try_recv(){self.pending=false;if epoch!=self.epoch{continue;}if let Some(values)=result{for(n,live)in values{self.values.insert(n,(live,Instant::now()));}changed=true;}}
        let before=self.values.len();self.values.retain(|_,(_,at)|at.elapsed()<Duration::from_secs(180));changed|=before!=self.values.len();
        if !self.pending && !self.channels.is_empty() && self.last.elapsed()>=Duration::from_secs(60) {
            if let Some(identity)=&self.identity{if self.tx.try_send(Job{epoch:self.epoch,identity:identity.clone(),channels:self.channels.clone()}).is_ok(){self.pending=true;self.last=Instant::now();}}
        }
        changed
    }
}
fn load(client:&reqwest::blocking::Client,identity:&Identity,channels:&[String])->Option<HashMap<String,StreamInfo>> {
    let mut result=HashMap::new();
    for chunk in channels.chunks(100){
        let mut params=vec![("first","100")];params.extend(chunk.iter().map(|c|("user_login",c.as_str())));
        let response=client.get("https://api.twitch.tv/helix/streams").query(&params).header("Client-Id",CLIENT_ID).bearer_auth(&identity.access).send().ok()?;
        if !response.status().is_success(){return None;}
        let mut bytes=vec![];response.take(4*1024*1024+1).read_to_end(&mut bytes).ok()?;if bytes.len()>4*1024*1024{return None;}
        let value:Value=serde_json::from_slice(&bytes).ok()?;let data=value["data"].as_array()?;
        for name in chunk{result.insert(name.clone(),StreamInfo::default());}
        for entry in data {let name=entry["user_login"].as_str()?.to_ascii_lowercase();if result.contains_key(&name){result.insert(name,StreamInfo{live:entry["type"].as_str()==Some("live"),title:entry["title"].as_str().unwrap_or("").chars().take(300).collect(),category:entry["game_name"].as_str().unwrap_or("").chars().take(100).collect(),viewers:entry["viewer_count"].as_u64().unwrap_or(0),started_at:entry["started_at"].as_str().unwrap_or("").chars().take(40).collect(),language:entry["language"].as_str().unwrap_or("").chars().take(20).collect()});}}
    }
    Some(result)
}
