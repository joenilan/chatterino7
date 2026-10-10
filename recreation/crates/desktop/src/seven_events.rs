//! Anonymous 7TV catalog invalidations. No presence publishing or account tokens.
use std::{collections::{BTreeSet,HashSet},net::{TcpStream,ToSocketAddrs},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use serde_json::{Value,json};
use tungstenite::{Message,stream::MaybeTlsStream};
#[derive(Clone,PartialEq,Eq,PartialOrd,Ord)]
pub struct Watch {pub channel:String,pub set:String,pub owner:String}
pub struct Events {wanted:Arc<Mutex<Vec<Watch>>>,dirty:Arc<Mutex<HashSet<String>>>,stop:Arc<AtomicBool>,connected:Arc<AtomicBool>}
impl Events {
    pub fn new()->Self{
        let wanted=Arc::new(Mutex::new(Vec::new()));let dirty=Arc::new(Mutex::new(HashSet::new()));let stop=Arc::new(AtomicBool::new(false));let connected=Arc::new(AtomicBool::new(false));
        let (w,d,s,c)=(wanted.clone(),dirty.clone(),stop.clone(),connected.clone());
        std::thread::spawn(move||run(w,d,s,c));Self{wanted,dirty,stop,connected}
    }
    pub fn pump(&self,mut watches:Vec<Watch>)->HashSet<String>{
        watches.sort();watches.dedup();watches.truncate(128);
        if let Ok(mut wanted)=self.wanted.lock(){*wanted=watches;}
        self.dirty.lock().map(|mut d|std::mem::take(&mut *d)).unwrap_or_default()
    }
    pub fn status(&self)->&'static str{if self.stop.load(Ordering::Relaxed){"Polling fallback · EventAPI rejected"}else if self.connected(){"Live catalog updates"}else{"Polling fallback · reconnecting"}}
    pub fn connected(&self)->bool{self.connected.load(Ordering::Relaxed)}
}
impl Drop for Events{fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);}}
type Socket=tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;
fn open()->Option<Socket>{
    let stream=("events.7tv.io",443).to_socket_addrs().ok()?.take(4).find_map(|addr|TcpStream::connect_timeout(&addr,Duration::from_secs(4)).ok())?;
    stream.set_read_timeout(Some(Duration::from_secs(8))).ok()?;stream.set_write_timeout(Some(Duration::from_secs(8))).ok()?;
    let config=tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(1024*1024)).max_frame_size(Some(1024*1024));
    let(mut socket,_)=tungstenite::client_tls_with_config("wss://events.7tv.io/v3",stream,Some(config),None).ok()?;
    match socket.get_mut(){MaybeTlsStream::Plain(s)=>s.set_read_timeout(Some(Duration::from_millis(200))),MaybeTlsStream::Rustls(s)=>s.sock.set_read_timeout(Some(Duration::from_millis(200))),_=>return None}.ok()?;Some(socket)
}
fn subscriptions(watches:&[Watch])->BTreeSet<(String,String)>{
    let mut set=BTreeSet::new();for w in watches {if !w.set.is_empty(){set.insert(("emote_set.update".into(),w.set.clone()));}if !w.owner.is_empty(){set.insert(("user.update".into(),w.owner.clone()));}}set
}
fn invalidate(dirty:&Mutex<HashSet<String>>,names:impl Iterator<Item=String>){if let Ok(mut d)=dirty.lock(){for name in names{if d.len()<128{d.insert(name);}}}}
fn permanent(code:u64)->bool{matches!(code,1002|1003|1007|1008|4001|4002|4003|4004|4005|4009|4010|4011)}
fn run(wanted:Arc<Mutex<Vec<Watch>>>,dirty:Arc<Mutex<HashSet<String>>>,stop:Arc<AtomicBool>,connected:Arc<AtomicBool>){
    let mut delay=1u64;
    while !stop.load(Ordering::Relaxed){
        let watches=wanted.lock().map(|w|w.clone()).unwrap_or_default();
        if watches.is_empty(){std::thread::sleep(Duration::from_millis(250));continue;}
        if let Some(mut socket)=open(){
            let mut maintenance=false;let started=Instant::now();let mut last=Instant::now();let mut timeout=Duration::from_secs(90);let mut limit=0usize;let mut hello=false;let mut active=BTreeSet::new();let mut acknowledged=BTreeSet::new();let mut routes=watches;
            while !stop.load(Ordering::Relaxed) && last.elapsed()<timeout {
                if hello {
                    routes=wanted.lock().map(|w|w.clone()).unwrap_or_default();if routes.is_empty(){break;}
                    let next=subscriptions(&routes).into_iter().take(limit).collect::<BTreeSet<_>>();
                    let mut failed=false;
                    for (op,entries) in [(36,active.difference(&next).cloned().collect::<Vec<_>>()),(35,next.difference(&active).cloned().collect())]{
                        for(kind,id)in entries{if socket.send(Message::Text(json!({"op":op,"d":{"type":kind,"condition":{"object_id":id}}}).to_string().into())).is_err(){failed=true;break;}}
                        if failed{break;}
                    }
                    if failed{break;}active=next;
                }
                match socket.read(){
                    Ok(Message::Text(text))=>{
                        let Ok(event)=serde_json::from_str::<Value>(&text)else{continue;};last=Instant::now();
                        match event["op"].as_u64(){
                            Some(1)=>{hello=true;limit=match event["d"]["subscription_limit"].as_i64(){Some(-1)=>256,Some(n)=>n.clamp(0,256)as usize,None=>100};timeout=Duration::from_millis(event["d"]["heartbeat_interval"].as_u64().unwrap_or(45000).clamp(1000,120000)*2+5000);invalidate(&dirty,routes.iter().map(|w|w.channel.clone()));},
                            Some(0)=>{let kind=event["d"]["type"].as_str().unwrap_or("");let id=event["d"]["body"]["id"].as_str().unwrap_or("");if !id.is_empty(){invalidate(&dirty,routes.iter().filter(|w|(kind=="emote_set.update"&&w.set==id)||(kind=="user.update"&&w.owner==id)).map(|w|w.channel.clone()));}},
                            Some(5)=>{let d=&event["d"];if let(Some(kind),Some(id))=(d["data"]["type"].as_str(),d["data"]["condition"]["object_id"].as_str()){
                                let key=(kind.to_owned(),id.to_owned());match d["command"].as_str(){Some("SUBSCRIBE")=>{if active.contains(&key)&&acknowledged.insert(key.clone()){invalidate(&dirty,routes.iter().filter(|w|(kind=="emote_set.update"&&w.set==id)||(kind=="user.update"&&w.owner==id)).map(|w|w.channel.clone()));}},Some("UNSUBSCRIBE")=>{acknowledged.remove(&key);},_=>{}}connected.store(!acknowledged.is_empty(),Ordering::Relaxed);
                            }},
                            Some(4)=>break,
                            Some(6)=>{connected.store(false,Ordering::Relaxed);stop.store(true,Ordering::Relaxed);return;},
                            Some(7)=>{let code=event["d"]["code"].as_u64().unwrap_or(0);if permanent(code){connected.store(false,Ordering::Relaxed);stop.store(true,Ordering::Relaxed);return;}maintenance=code==4007;break;},
                            _=>{}
                        }
                    },
                    Ok(Message::Ping(_))=>{if socket.flush().is_err(){break;}},
                    Ok(Message::Close(frame))=>{let code=frame.map(|f|u16::from(f.code)as u64).unwrap_or(0);if permanent(code){connected.store(false,Ordering::Relaxed);stop.store(true,Ordering::Relaxed);return;}maintenance=code==4007;break;},
                    Ok(_)=>{},
                    Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
                    Err(_)=>break,
                }
            }
            connected.store(false,Ordering::Relaxed);let _=socket.close(None);if maintenance{delay=330;}else if started.elapsed()>Duration::from_secs(30){delay=1;}
        }
        for _ in 0..delay*4{if stop.load(Ordering::Relaxed){return;}std::thread::sleep(Duration::from_millis(250));}delay=(delay*2).min(60);
    }
}
