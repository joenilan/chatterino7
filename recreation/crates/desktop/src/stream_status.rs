//! Broadcast status is distinct from chat connectivity. Reuse the existing Helix grant.
use crate::{auth::CLIENT_ID,live::Identity};
use std::{collections::HashMap,io::Read,sync::mpsc,time::{Duration,Instant}};
use serde_json::Value;
struct Job {epoch:u64,identity:Identity,channels:Vec<String>}
pub struct Streams {
    identity:Option<Identity>,channels:Vec<String>,epoch:u64,pending:bool,last:Instant,
    values:HashMap<String,(bool,Instant)>,tx:mpsc::SyncSender<Job>,rx:mpsc::Receiver<(u64,Option<HashMap<String,bool>>)>,
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
    pub fn get(&self,name:&str)->Option<bool>{self.values.get(name).filter(|(_,at)|at.elapsed()<Duration::from_secs(180)).map(|(live,_)|*live)}
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
fn load(client:&reqwest::blocking::Client,identity:&Identity,channels:&[String])->Option<HashMap<String,bool>> {
    let mut result=HashMap::new();
    for chunk in channels.chunks(100){
        let mut params=vec![("first","100")];params.extend(chunk.iter().map(|c|("user_login",c.as_str())));
        let response=client.get("https://api.twitch.tv/helix/streams").query(&params).header("Client-Id",CLIENT_ID).bearer_auth(&identity.access).send().ok()?;
        if !response.status().is_success(){return None;}
        let mut bytes=vec![];response.take(4*1024*1024+1).read_to_end(&mut bytes).ok()?;if bytes.len()>4*1024*1024{return None;}
        let value:Value=serde_json::from_slice(&bytes).ok()?;let data=value["data"].as_array()?;
        for name in chunk{result.insert(name.clone(),false);}
        for entry in data {let name=entry["user_login"].as_str()?.to_ascii_lowercase();if result.contains_key(&name){result.insert(name,entry["type"].as_str()==Some("live"));}}
    }
    Some(result)
}
