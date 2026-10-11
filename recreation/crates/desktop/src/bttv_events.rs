//! BTTV live updates: an anonymous WebSocket that says when a channel's BTTV
//! emotes change (added, renamed, removed). The catalog then refetches that
//! channel's set, so emotes stay correct without a restart.
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, stream::MaybeTlsStream};

const HOST: &str = "sockets.betterttv.net";
/// Channels joined at most (visible and recent channels first).
const LIMIT: usize = 150;

pub struct BttvEvents {
    wanted: Arc<Mutex<Vec<String>>>,
    changed: Arc<Mutex<HashSet<String>>>,
    connected: Arc<AtomicBool>,
}
impl BttvEvents {
    pub fn new() -> Self {
        let wanted = Arc::new(Mutex::new(Vec::new()));
        let changed = Arc::new(Mutex::new(HashSet::new()));
        let connected = Arc::new(AtomicBool::new(false));
        let (w, c, k) = (wanted.clone(), changed.clone(), connected.clone());
        let _ = std::thread::Builder::new()
            .name("jawjack-bttv-events".into())
            .spawn(move || run(w, c, k));
        Self {
            wanted,
            changed,
            connected,
        }
    }
    /// Set the Twitch channel IDs to watch; returns IDs whose BTTV emotes changed.
    pub fn pump(&self, mut ids: Vec<String>) -> HashSet<String> {
        let mut seen = HashSet::new();
        ids.retain(|id| !id.is_empty() && seen.insert(id.clone()));
        ids.truncate(LIMIT);
        if let Ok(mut wanted) = self.wanted.lock() {
            *wanted = ids;
        }
        self.changed
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default()
    }
    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

/// The Twitch channel ID an emote event is about, if it is one.
pub fn changed_channel(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let name = v["name"].as_str()?;
    if !matches!(name, "emote_create" | "emote_update" | "emote_delete") {
        return None;
    }
    let id = v["data"]["channel"].as_str()?.strip_prefix("twitch:")?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then(|| id.to_owned())
}

type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;
fn open() -> Option<Socket> {
    let stream = (HOST, 443)
        .to_socket_addrs()
        .ok()?
        .take(4)
        .find_map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(4)).ok())?;
    stream.set_read_timeout(Some(Duration::from_secs(8))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(8)))
        .ok()?;
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(256 * 1024))
        .max_frame_size(Some(256 * 1024));
    let (mut socket, _) =
        tungstenite::client_tls_with_config(format!("wss://{HOST}/ws"), stream, Some(config), None)
            .ok()?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(Duration::from_millis(250))),
        MaybeTlsStream::Rustls(s) => s.sock.set_read_timeout(Some(Duration::from_millis(250))),
        _ => return None,
    }
    .ok()?;
    Some(socket)
}
fn send(socket: &mut Socket, name: &str, id: &str) -> bool {
    socket
        .send(Message::Text(
            json!({"name": name, "data": {"name": format!("twitch:{id}")}})
                .to_string()
                .into(),
        ))
        .is_ok()
}

fn run(
    wanted: Arc<Mutex<Vec<String>>>,
    changed: Arc<Mutex<HashSet<String>>>,
    connected: Arc<AtomicBool>,
) {
    let mut backoff = Duration::from_secs(2);
    loop {
        // Nothing to watch: don't hold a connection open.
        if wanted.lock().map(|w| w.is_empty()).unwrap_or(true) {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        let Some(mut socket) = open() else {
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_secs(120));
            continue;
        };
        connected.store(true, Ordering::Relaxed);
        let opened = Instant::now();
        let mut joined: Vec<String> = Vec::new();
        loop {
            let want = wanted.lock().map(|w| w.clone()).unwrap_or_default();
            let mut ok = true;
            for id in joined.iter().filter(|id| !want.contains(id)) {
                ok &= send(&mut socket, "part_channel", id);
            }
            joined.retain(|id| want.contains(id));
            let new: Vec<String> = want
                .iter()
                .filter(|id| !joined.contains(id))
                .cloned()
                .collect();
            for id in &new {
                ok &= send(&mut socket, "join_channel", id);
                joined.push(id.clone());
            }
            if !ok {
                break;
            }
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Some(id) = changed_channel(&text)
                        && let Ok(mut c) = changed.lock()
                    {
                        c.insert(id);
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => break,
            }
        }
        connected.store(false, Ordering::Relaxed);
        // A connection that lasted a while resets the backoff.
        backoff = if opened.elapsed() > Duration::from_secs(60) {
            Duration::from_secs(2)
        } else {
            (backoff * 2).min(Duration::from_secs(120))
        };
        std::thread::sleep(backoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_emote_events() {
        let create = r#"{"name":"emote_create","data":{"channel":"twitch:123","emote":{"id":"a","code":"KEKW"}}}"#;
        assert_eq!(changed_channel(create).as_deref(), Some("123"));
        let delete = r#"{"name":"emote_delete","data":{"channel":"twitch:9","emoteId":"a"}}"#;
        assert_eq!(changed_channel(delete).as_deref(), Some("9"));
        assert_eq!(changed_channel(r#"{"name":"lookup_user","data":{}}"#), None);
        assert_eq!(
            changed_channel(r#"{"name":"emote_create","data":{"channel":"twitch:../x"}}"#),
            None
        );
    }
}
