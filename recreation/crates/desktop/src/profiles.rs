//! On-demand public profile metadata, bounded and independent of the chat socket.
use crate::{auth::CLIENT_ID,live::Identity};
use std::{collections::HashMap,io::Read,sync::mpsc,time::{Duration,Instant}};
#[derive(Clone)]
pub struct Profile {pub display_name:String,pub created:String,pub description:String,pub role:String,pub avatar:Option<crate::media::EmoteKey>}
struct Job {epoch:u64,identity:Identity,id:String}
pub struct Profiles {identity:Option<Identity>,epoch:u64,requested:HashMap<String,Instant>,values:HashMap<String,Profile>,tx:mpsc::SyncSender<Job>,rx:mpsc::Receiver<(u64,String,Option<Profile>)>}
impl Profiles {
    pub fn new()->Self{
        let(tx,jobs)=mpsc::sync_channel::<Job>(16);let(results,rx)=mpsc::sync_channel(2);
        std::thread::spawn(move||{let Ok(client)=reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).redirect(reqwest::redirect::Policy::none()).build()else{return;};while let Ok(job)=jobs.recv(){let value=load(&client,&job.identity,&job.id);if results.send((job.epoch,job.id,value)).is_err(){break;}}});
        Self{identity:None,epoch:0,requested:HashMap::new(),values:HashMap::new(),tx,rx}
    }
    pub fn get(&self,id:&str)->Option<&Profile>{self.values.get(id)}
    pub fn request(&mut self,id:&str){
        if id.is_empty()||id.len()>32||!id.bytes().all(|b|b.is_ascii_digit())||self.requested.get(id).is_some_and(|at|at.elapsed()<Duration::from_secs(600)){return;}
        let Some(identity)=&self.identity else{return;};
        if self.requested.len()>=128 {if let Some(old)=self.requested.iter().min_by_key(|(_,at)|*at).map(|(id,_)|id.clone()){self.requested.remove(&old);self.values.remove(&old);}}
        if self.tx.try_send(Job{epoch:self.epoch,identity:identity.clone(),id:id.into()}).is_ok(){self.requested.insert(id.into(),Instant::now());}
    }
    pub fn pump(&mut self,identity:Option<Identity>)->bool{
        let mut changed=false;
        if self.identity!=identity{self.identity=identity;self.epoch+=1;self.requested.clear();self.values.clear();changed=true;}
        while let Ok((epoch,id,value))=self.rx.try_recv(){if epoch!=self.epoch||!self.requested.contains_key(&id){continue;}if let Some(value)=value{self.values.insert(id,value);}else{self.requested.insert(id,Instant::now()-Duration::from_secs(540));}changed=true;}
        changed
    }
}
fn load(client:&reqwest::blocking::Client,identity:&Identity,id:&str)->Option<Profile>{
    let response=client.get("https://api.twitch.tv/helix/users").query(&[("id",id)]).header("Client-Id",CLIENT_ID).bearer_auth(&identity.access).send().ok()?;
    if !response.status().is_success(){return None;}
    let mut bytes=vec![];response.take(128*1024+1).read_to_end(&mut bytes).ok()?;if bytes.len()>128*1024{return None;}
    let value:serde_json::Value=serde_json::from_slice(&bytes).ok()?;let user=value["data"].as_array()?.first()?;
    if user["id"].as_str()!=Some(id){return None;}
    Some(Profile{display_name:user["display_name"].as_str().unwrap_or("").chars().take(80).collect(),created:user["created_at"].as_str().unwrap_or("").chars().take(40).collect(),description:user["description"].as_str().unwrap_or("").chars().take(500).collect(),role:user["broadcaster_type"].as_str().unwrap_or("").chars().take(40).collect(),avatar:user["profile_image_url"].as_str().and_then(crate::media::EmoteKey::avatar)})
}
