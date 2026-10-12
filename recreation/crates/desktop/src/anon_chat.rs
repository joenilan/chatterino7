//! Read-only Twitch chat while no account is signed in, over Twitch's public
//! IRC as an anonymous "justinfan" guest, as Chatterino does. There are no
//! credentials and nothing can be sent; signing in switches to EventSub.
use crate::live::Event;
use chat_core::Event as ChatEvent;
use std::{
    collections::HashSet,
    net::{TcpStream, ToSocketAddrs},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use tungstenite::{Message as Wire, stream::MaybeTlsStream};

const HOST: &str = "irc-ws.chat.twitch.tv";
/// Twitch allows 20 JOINs per 10 seconds for an anonymous connection.
const JOINS_PER_WINDOW: usize = 20;
const MAX_CHANNELS: usize = 100;
pub const STATUS: &str = "Reading as a guest · sign in to chat";

type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

pub struct AnonChat {
    wanted: Arc<Mutex<Vec<String>>>,
    pub events: mpsc::Receiver<Event>,
}
impl AnonChat {
    pub fn new() -> Self {
        let wanted: Arc<Mutex<Vec<String>>> = Default::default();
        let (tx, events) = mpsc::sync_channel(4096);
        let shared = wanted.clone();
        let _ = std::thread::Builder::new()
            .name("jawjack-guest-chat".into())
            .spawn(move || run(shared, tx));
        Self { wanted, events }
    }
    /// Channels to read, highest priority first; empty disconnects.
    pub fn configure(&self, mut channels: Vec<String>) {
        channels.truncate(MAX_CHANNELS);
        if let Ok(mut wanted) = self.wanted.lock()
            && *wanted != channels
        {
            *wanted = channels;
        }
    }
}

fn wanted(shared: &Mutex<Vec<String>>) -> Vec<String> {
    shared.lock().map(|w| w.clone()).unwrap_or_default()
}

fn run(shared: Arc<Mutex<Vec<String>>>, tx: mpsc::SyncSender<Event>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        if wanted(&shared).is_empty() {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let mut joined = HashSet::new();
        let started = Instant::now();
        let result = session(&shared, &tx, &mut joined);
        for channel in &joined {
            let _ = tx.send(Event::State(
                channel.clone(),
                "Reconnecting as a guest…".into(),
                false,
            ));
        }
        if result.is_err() {
            if started.elapsed() > Duration::from_secs(60) {
                backoff = Duration::from_secs(1);
            }
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }
}

fn open() -> Result<Socket, String> {
    let stream = (HOST, 443)
        .to_socket_addrs()
        .map_err(|_| "Twitch DNS lookup failed")?
        .filter_map(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).ok())
        .next()
        .ok_or("Could not reach Twitch chat")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| "Could not set socket timeout")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| "Could not set socket timeout")?;
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(1024 * 1024))
        .max_frame_size(Some(1024 * 1024));
    let (mut socket, _) =
        tungstenite::client_tls_with_config(format!("wss://{HOST}"), stream, Some(config), None)
            .map_err(|_| "Twitch secure connection failed")?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(Duration::from_millis(200))),
        MaybeTlsStream::Rustls(s) => s.sock.set_read_timeout(Some(Duration::from_millis(200))),
        _ => return Err("Unsupported secure transport".into()),
    }
    .map_err(|_| "Could not set read timeout")?;
    Ok(socket)
}

fn send(socket: &mut Socket, line: &str) -> Result<(), String> {
    socket
        .send(Wire::Text(line.to_owned().into()))
        .map_err(|_| "Twitch chat connection interrupted".to_owned())
}

fn session(
    shared: &Mutex<Vec<String>>,
    tx: &mpsc::SyncSender<Event>,
    joined: &mut HashSet<String>,
) -> Result<(), String> {
    let mut socket = open()?;
    let guest = 10_000
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() % 80_000)
            .unwrap_or(0);
    send(&mut socket, "CAP REQ :twitch.tv/tags twitch.tv/commands")?;
    send(&mut socket, "PASS SCHMOOPIIE")?;
    send(&mut socket, &format!("NICK justinfan{guest}"))?;
    let mut joins: Vec<Instant> = Vec::new();
    let mut last_read = Instant::now();
    loop {
        let want = wanted(shared);
        if want.is_empty() {
            let _ = socket.close(None);
            return Ok(());
        }
        // Leave closed channels; join new ones within Twitch's join rate.
        let leaving: Vec<String> = joined
            .iter()
            .filter(|c| !want.contains(c))
            .cloned()
            .collect();
        for channel in leaving {
            send(&mut socket, &format!("PART #{channel}"))?;
            joined.remove(&channel);
        }
        joins.retain(|at| at.elapsed() < Duration::from_secs(10));
        let room = JOINS_PER_WINDOW.saturating_sub(joins.len());
        let new: Vec<String> = want
            .iter()
            .filter(|c| !joined.contains(*c) && chat_core::twitch_login(c).is_some())
            .take(room)
            .cloned()
            .collect();
        if !new.is_empty() {
            let list = new
                .iter()
                .map(|c| format!("#{c}"))
                .collect::<Vec<_>>()
                .join(",");
            send(&mut socket, &format!("JOIN {list}"))?;
            for channel in new {
                joins.push(Instant::now());
                let _ = tx.send(Event::State(channel.clone(), STATUS.into(), true));
                joined.insert(channel);
            }
        }
        match socket.read() {
            Ok(Wire::Text(text)) => {
                last_read = Instant::now();
                for line in text.split("\r\n").filter(|l| !l.is_empty()) {
                    if let Some(rest) = line.strip_prefix("PING") {
                        send(&mut socket, &format!("PONG{rest}"))?;
                        continue;
                    }
                    if line.contains(" RECONNECT") && line.starts_with(":tmi.twitch.tv") {
                        return Err("Twitch asked to reconnect".into());
                    }
                    deliver(line, joined, tx);
                }
            }
            Ok(Wire::Close(_)) => return Err("Twitch closed the connection".into()),
            Ok(_) => last_read = Instant::now(),
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                // Twitch pings every ~5 minutes; silence well past that is a dead link.
                if last_read.elapsed() > Duration::from_secs(400) {
                    return Err("Twitch chat went quiet".into());
                }
            }
            Err(_) => return Err("Twitch chat connection interrupted".into()),
        }
    }
}

fn deliver(line: &str, joined: &HashSet<String>, tx: &mpsc::SyncSender<Event>) {
    use crate::recent_messages::Live;
    let Some((channel, live)) = crate::recent_messages::live_line(line) else {
        return;
    };
    if !joined.contains(&channel) {
        return;
    }
    let event = match live {
        Live::Room(id) => Event::Resolved(channel, id),
        Live::Message(message) => Event::Chat(channel, ChatEvent::Message(message)),
        Live::Delete(message_id) => Event::Chat(
            channel.clone(),
            ChatEvent::DeleteMessage {
                channel_id: channel,
                message_id,
            },
        ),
        Live::ClearUser(user_id) => Event::Chat(
            channel.clone(),
            ChatEvent::ClearUser {
                channel_id: channel,
                user_id,
            },
        ),
        Live::Clear => Event::Chat(
            channel.clone(),
            ChatEvent::ClearChannel {
                channel_id: channel,
            },
        ),
    };
    let _ = tx.send(event);
}

#[cfg(test)]
mod tests {
    /// Network: joins a public channel as a guest and waits for its ROOMSTATE.
    #[test]
    #[ignore]
    fn guest_join_receives_room_id() {
        let chat = super::AnonChat::new();
        chat.configure(vec!["twitch".into()]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            if let Ok(crate::live::Event::Resolved(channel, id)) =
                chat.events.recv_timeout(std::time::Duration::from_secs(1))
            {
                assert_eq!(channel, "twitch");
                assert!(!id.is_empty());
                return;
            }
        }
        panic!("no ROOMSTATE within 20 s");
    }
}
