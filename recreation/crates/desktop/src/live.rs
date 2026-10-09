//! One shared Twitch EventSub connection per account. No credentials in logs/state.
use crate::auth::CLIENT_ID;
use chat_core::{Event as ChatEvent, Fragment, Message};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Read,
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tungstenite::{Message as Wire, stream::MaybeTlsStream};

#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    pub user_id: String,
    pub access: String,
}
#[derive(Clone, PartialEq, Eq, Default)]
struct Config {
    identity: Option<Identity>,
    channels: Vec<String>,
}
pub enum Event {
    State(String, String, bool),
    Resolved(String, String),
    Chat(String, ChatEvent),
    Sent(u64, Result<(), String>),
}
pub struct LiveChat {
    config: Arc<Mutex<Config>>,
    cancel: Arc<AtomicBool>,
    pub events: mpsc::Receiver<(u64, Event)>,
    output: mpsc::SyncSender<(u64, Event)>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
    sends: Mutex<VecDeque<(Instant, String)>>,
}
impl Drop for LiveChat {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl LiveChat {
    pub fn new() -> Self {
        let config = Arc::new(Mutex::new(Config::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let (output, events) = mpsc::sync_channel(4096);
        let epoch = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (c, stop, tx, e) = (
            config.clone(),
            cancel.clone(),
            output.clone(),
            epoch.clone(),
        );
        std::thread::spawn(move || run(c, stop, tx, e));
        Self {
            config,
            cancel,
            events,
            output,
            epoch,
            sends: Mutex::new(VecDeque::new()),
        }
    }
    pub fn version(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }
    pub fn configure(&self, identity: Option<Identity>, mut channels: Vec<String>) -> bool {
        channels.sort();
        channels.dedup();
        let next = Config { identity, channels };
        if let Ok(mut c) = self.config.lock() {
            if *c != next {
                *c = next;
                self.epoch.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }
        false
    }
    pub fn send(&self, request: u64, channel: String, text: String) -> Result<(), String> {
        let Some(identity) = self.config.lock().ok().and_then(|c| c.identity.clone()) else {
            return Err("Account disconnected. Draft kept.".into());
        };
        {
            let mut sends = self.sends.lock().map_err(|_| "Send limiter unavailable")?;
            let now = Instant::now();
            while sends
                .front()
                .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(30))
            {
                sends.pop_front();
            }
            if sends.len() >= 20 {
                return Err("Chat rate limit: wait for the 30-second window. Draft kept.".into());
            }
            if sends
                .iter()
                .any(|(at, c)| *c == channel && now.duration_since(*at) < Duration::from_secs(1))
            {
                return Err("Wait one second between messages to this channel. Draft kept.".into());
            }
            sends.push_back((now, channel.clone()));
        }
        let (tx, epoch) = (self.output.clone(), self.epoch.clone());
        let version = epoch.load(Ordering::Relaxed);
        std::thread::spawn(move || {
            let outcome = (|| {
                if text.trim().is_empty()
                    || text.chars().count() > 500
                    || text.contains(['\r', '\n'])
                {
                    return Err("Message must contain 1–500 characters without newlines.".into());
                }
                let client = client()?;
                let id = resolve(&client, &identity, &channel)?;
                if epoch.load(Ordering::Relaxed) != version {
                    return Err("Account or channels changed. Nothing was sent.".into());
                }
                let request_body =
                    json!({"broadcaster_id":id,"sender_id":identity.user_id,"message":text});
                // Never retry a POST: a timeout may have happened after delivery.
                let response = client
                    .post("https://api.twitch.tv/helix/chat/messages")
                    .header("Client-Id", CLIENT_ID)
                    .bearer_auth(&identity.access)
                    .body(request_body.to_string())
                    .header("Content-Type", "application/json")
                    .send()
                    .map_err(|_| {
                        "Delivery is uncertain. Draft kept; check chat before retrying.".to_string()
                    })?;
                let (status, value) = response_json(response).map_err(|_| {
                    "Delivery response could not be read. Check chat before retrying.".to_string()
                })?;
                if status != 200 {
                    return Err(format!(
                        "Twitch rejected the send (HTTP {status}). Draft kept."
                    ));
                }
                if value["data"][0]["is_sent"].as_bool() == Some(true) {
                    Ok(())
                } else {
                    Err(format!(
                        "Not sent: {}",
                        value["data"][0]["drop_reason"]["message"]
                            .as_str()
                            .unwrap_or("Twitch did not confirm delivery")
                    ))
                }
            })();
            let _ = tx.send((version, Event::Sent(request, outcome)));
        });
        Ok(())
    }
}
fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize Twitch networking".into())
}
fn response_json(response: reqwest::blocking::Response) -> Result<(u16, Value), String> {
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .take(262145)
        .read_to_end(&mut bytes)
        .map_err(|_| "Twitch response interrupted")?;
    if bytes.len() > 262144 {
        return Err("Twitch response too large".into());
    }
    Ok((
        status,
        serde_json::from_slice(&bytes).map_err(|_| "Unexpected Twitch response")?,
    ))
}
fn resolve(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    channel: &str,
) -> Result<String, String> {
    let (status, v) = response_json(
        client
            .get("https://api.twitch.tv/helix/users")
            .query(&[("login", channel)])
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .map_err(|_| "Could not look up Twitch channel")?,
    )?;
    if status != 200 {
        return Err(format!("Channel lookup failed (HTTP {status})"));
    }
    v["data"][0]["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "Channel was not found on Twitch".into())
}
type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;
fn open(url: &str) -> Result<Socket, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid reconnect URL")?;
    // Reconnect destinations must remain Twitch's EventSub service.
    if parsed.scheme() != "wss"
        || parsed.host_str() != Some("eventsub.wss.twitch.tv")
        || parsed.port().is_some_and(|p| p != 443)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Unexpected EventSub reconnect destination".into());
    }
    let addresses = ("eventsub.wss.twitch.tv", 443)
        .to_socket_addrs()
        .map_err(|_| "Twitch DNS lookup failed")?;
    let stream = addresses
        .filter_map(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).ok())
        .next()
        .ok_or("Could not reach Twitch EventSub")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| "Could not set socket timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| "Could not set socket timeout")?;
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(1024 * 1024))
        .max_frame_size(Some(1024 * 1024));
    let (mut socket, _) = tungstenite::client_tls_with_config(url, stream, Some(config), None)
        .map_err(|_| "Twitch secure connection failed")?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(Duration::from_millis(200))),
        MaybeTlsStream::Rustls(s) => s.sock.set_read_timeout(Some(Duration::from_millis(200))),
        _ => return Err("Unsupported secure transport".into()),
    }
    .map_err(|_| "Could not set read timeout")?;
    Ok(socket)
}
fn frame(socket: &mut Socket) -> Result<Option<Value>, String> {
    match socket.read() {
        Ok(Wire::Text(t)) => serde_json::from_str(&t)
            .map(Some)
            .map_err(|_| "Invalid Twitch event".into()),
        Ok(Wire::Close(_)) => Err("Twitch closed the connection".into()),
        Ok(Wire::Ping(_)) => {
            socket.flush().map_err(|_| "Twitch ping response failed")?;
            Ok(None)
        }
        Ok(_) => Ok(None),
        Err(tungstenite::Error::Io(e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Ok(None)
        }
        Err(_) => Err("Twitch connection interrupted".into()),
    }
}
fn subscribe(
    c: Config,
    session: String,
    tx: mpsc::SyncSender<(u64, Event)>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
    version: u64,
    map: Arc<Mutex<HashMap<String, String>>>,
) {
    std::thread::spawn(move || {
        let Some(identity) = c.identity else { return };
        let Ok(client) = client() else { return };
        for (index, channel) in c.channels.into_iter().enumerate() {
            if index > 0 {
                std::thread::sleep(Duration::from_millis(550));
            }
            if epoch.load(Ordering::Relaxed) != version {
                break;
            }
            let result: Result<(), String> = (|| {
                let id = resolve(&client, &identity, &channel)?;
                if let Ok(mut names) = map.lock() {
                    names.insert(id.clone(), channel.clone());
                }
                let _ = tx.send((version, Event::Resolved(channel.clone(), id.clone())));
                for kind in [
                    "channel.chat.message",
                    "channel.chat.message_delete",
                    "channel.chat.clear_user_messages",
                    "channel.chat.clear",
                ] {
                    if epoch.load(Ordering::Relaxed) != version {
                        return Err("Channel subscription cancelled".into());
                    }
                    let body = json!({"type":kind,"version":"1","condition":{"broadcaster_user_id":id,"user_id":identity.user_id},"transport":{"method":"websocket","session_id":session}});
                    let (status, response) = response_json(
                        client
                            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
                            .header("Client-Id", CLIENT_ID)
                            .bearer_auth(&identity.access)
                            .header("Content-Type", "application/json")
                            .body(body.to_string())
                            .send()
                            .map_err(|_| "Twitch subscription request interrupted")?,
                    )?;
                    if status != 202 || response["data"][0]["status"].as_str() != Some("enabled") {
                        return Err(format!("Chat subscription failed (HTTP {status})"));
                    }
                }
                Ok(())
            })();
            if epoch.load(Ordering::Relaxed) != version {
                break;
            }
            let (message, ready) = match result {
                Ok(()) => ("Connected".into(), true),
                Err(e) => (e, false),
            };
            if tx
                .send((version, Event::State(channel, message, ready)))
                .is_err()
            {
                break;
            }
        }
    });
}
fn all(tx: &mpsc::SyncSender<(u64, Event)>, c: &Config, version: u64, message: &str) {
    for channel in &c.channels {
        if tx
            .send((
                version,
                Event::State(channel.clone(), message.into(), false),
            ))
            .is_err()
        {
            break;
        }
    }
}
fn run(
    config: Arc<Mutex<Config>>,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<(u64, Event)>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
) {
    let mut dedup = HashSet::new();
    let mut order = VecDeque::new();
    let mut retry = 1;
    while !stop.load(Ordering::Relaxed) {
        let (c, version) = config
            .lock()
            .map(|c| (c.clone(), epoch.load(Ordering::Relaxed)))
            .unwrap_or_default();
        if c.identity.is_none() || c.channels.is_empty() {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        all(&tx, &c, version, "Connecting…");
        let mut revoked = false;
        let connected_at = Instant::now();
        let result: Result<(), String> = (|| {
            let mut socket = open("wss://eventsub.wss.twitch.tv/ws?keepalive_timeout_seconds=60")?;
            let map = Arc::new(Mutex::new(HashMap::new()));
            let mut welcome = false;
            let mut last = Instant::now();
            let mut timeout = Duration::from_secs(10);
            while !stop.load(Ordering::Relaxed) && epoch.load(Ordering::Relaxed) == version {
                if last.elapsed() > timeout {
                    return Err("Twitch keepalive expired".into());
                }
                let Some(v) = frame(&mut socket)? else {
                    continue;
                };
                last = Instant::now();
                if connected_at.elapsed() > Duration::from_secs(60) {
                    retry = 1;
                }
                let kind = v["metadata"]["message_type"].as_str().unwrap_or("");
                if kind == "session_welcome" {
                    timeout = Duration::from_secs(
                        v["payload"]["session"]["keepalive_timeout_seconds"]
                            .as_u64()
                            .unwrap_or(60)
                            + 5,
                    );
                    if !welcome {
                        let id = v["payload"]["session"]["id"]
                            .as_str()
                            .ok_or("Missing Twitch session")?;
                        subscribe(
                            c.clone(),
                            id.into(),
                            tx.clone(),
                            epoch.clone(),
                            version,
                            map.clone(),
                        );
                        welcome = true;
                    }
                    continue;
                }
                if kind == "session_reconnect" {
                    let url = v["payload"]["session"]["reconnect_url"]
                        .as_str()
                        .ok_or("Missing reconnect URL")?;
                    let mut replacement = open(url)?;
                    let deadline = Instant::now() + Duration::from_secs(10);
                    loop {
                        if stop.load(Ordering::Relaxed) || epoch.load(Ordering::Relaxed) != version
                        {
                            return Ok(());
                        }
                        if Instant::now() > deadline {
                            return Err("Twitch reconnect welcome timed out".into());
                        }
                        if let Some(old) = frame(&mut socket)? {
                            deliver(&old, &map, &tx, &mut dedup, &mut order, version);
                        }
                        if let Some(next) = frame(&mut replacement)? {
                            if next["metadata"]["message_type"] == "session_welcome" {
                                break;
                            }
                        }
                    }
                    // Read queued old-socket events before closing after the replacement welcome.
                    for _ in 0..1024 {
                        match frame(&mut socket) {
                            Ok(Some(old)) => {
                                deliver(&old, &map, &tx, &mut dedup, &mut order, version)
                            }
                            _ => break,
                        }
                    }
                    let _ = socket.close(None);
                    socket = replacement;
                    last = Instant::now();
                    continue;
                }
                if kind == "revocation" {
                    revoked = true;
                    return Err(
                        "Twitch revoked a chat subscription. Reconnect or sign in again.".into(),
                    );
                }
                deliver(&v, &map, &tx, &mut dedup, &mut order, version);
            }
            let _ = socket.close(None);
            Ok(())
        })();
        // A newer UI configuration owns the next status; do not overwrite it.
        if epoch.load(Ordering::Relaxed) != version {
            continue;
        }
        // Stop subscription workers from reporting stale connection success.
        if epoch.load(Ordering::Relaxed) == version {
            epoch.fetch_add(1, Ordering::Relaxed);
        }
        if let Err(error) = result {
            all(
                &tx,
                &c,
                epoch.load(Ordering::Relaxed),
                &if revoked {
                    error.clone()
                } else {
                    format!("{error}. Reconnecting; messages during the gap are unavailable.")
                },
            );
            if revoked {
                let expected = epoch.load(Ordering::Relaxed);
                while !stop.load(Ordering::Relaxed) && epoch.load(Ordering::Relaxed) == expected {
                    std::thread::sleep(Duration::from_millis(200));
                }
                continue;
            }
            let deadline = Instant::now() + Duration::from_secs(retry);
            let expected = epoch.load(Ordering::Relaxed);
            while Instant::now() < deadline
                && !stop.load(Ordering::Relaxed)
                && epoch.load(Ordering::Relaxed) == expected
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            retry = (retry * 2).min(30);
        }
    }
}
fn deliver(
    v: &Value,
    map: &Arc<Mutex<HashMap<String, String>>>,
    tx: &mpsc::SyncSender<(u64, Event)>,
    seen: &mut HashSet<String>,
    order: &mut VecDeque<String>,
    version: u64,
) {
    if v["metadata"]["message_type"] != "notification" {
        return;
    }
    let envelope = v["metadata"]["message_id"].as_str().unwrap_or("");
    if envelope.is_empty() || !seen.insert(envelope.into()) {
        return;
    }
    order.push_back(envelope.to_owned());
    if order.len() > 8192 {
        if let Some(id) = order.pop_front() {
            seen.remove(&id);
        }
    }
    let e = &v["payload"]["event"];
    let id = e["broadcaster_user_id"].as_str().unwrap_or("");
    let Some(channel) = map.lock().ok().and_then(|m| m.get(id).cloned()) else {
        return;
    };
    let text = |key: &str| e[key].as_str().unwrap_or("").to_owned();
    let event = match v["payload"]["subscription"]["type"].as_str().unwrap_or("") {
        "channel.chat.message" => ChatEvent::Message(Message {
            id: text("message_id"),
            channel_id: channel.clone(),
            user_id: text("chatter_user_id"),
            display_name: text("chatter_user_name"),
            name_color: e["color"].as_str().and_then(|s| s.strip_prefix('#'))
                .filter(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .and_then(|s| u32::from_str_radix(s, 16).ok()),
            fragments: twitch_fragments(&e["message"]),
            deleted: false,
        }),
        "channel.chat.message_delete" => ChatEvent::DeleteMessage {
            channel_id: channel.clone(),
            message_id: text("message_id"),
        },
        "channel.chat.clear_user_messages" => ChatEvent::ClearUser {
            channel_id: channel.clone(),
            user_id: text("target_user_id"),
        },
        "channel.chat.clear" => ChatEvent::ClearChannel {
            channel_id: channel.clone(),
        },
        _ => return,
    };
    let _ = tx.send((version, Event::Chat(channel, event)));
}

fn twitch_fragments(message: &Value) -> Vec<Fragment> {
    let original = message["text"].as_str().unwrap_or("");
    let fragments: Vec<_> = message["fragments"].as_array().into_iter().flatten().map(|fragment| {
        let text = fragment["text"].as_str().unwrap_or("").to_owned();
        if fragment["type"] == "emote" {
            if let Some(id) = fragment["emote"]["id"].as_str() {
                if crate::media::EmoteKey::twitch(id, false).is_some() {
                    return Fragment::Emote { provider: "twitch".into(), id: id.into(), label: text, overlay: false, asset: None,
                        animated: fragment["emote"]["format"].as_array().is_some_and(|formats| formats.iter().any(|f| f == "animated")) };
                }
            }
        }
        Fragment::Text(text)
    }).collect();
    if fragments.iter().map(Fragment::copy_text).collect::<String>() == original && !fragments.is_empty() { fragments }
    else { vec![Fragment::Text(original.into())] }
}
