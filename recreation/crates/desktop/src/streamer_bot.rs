//! Local Streamer.bot connection (STREAMER_BOT.md): list actions and run the
//! ones the streamer pinned. Off until enabled; plain `ws://` to the owner's
//! machine; reconnects with backoff. Payloads are treated as untrusted data.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tungstenite::{Message as Wire, stream::MaybeTlsStream};

const MAX_ACTIONS: usize = 1000;
pub const MAX_PINNED: usize = 12;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub group: String,
    pub enabled: bool,
}
#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".into(),
            port: 8080,
            password: None,
        }
    }
}
enum Command {
    Configure(Settings),
    Run {
        request: u64,
        action: Action,
        args: Value,
    },
    Refresh,
}
pub enum Event {
    Status(String, bool),
    Actions(Vec<Action>),
    Ran(u64, Result<String, String>),
}

pub struct StreamerBot {
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<Event>,
    pub status: String,
    pub connected: bool,
    pub actions: Vec<Action>,
    next_request: u64,
}
impl StreamerBot {
    pub fn new() -> Self {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        std::thread::spawn(move || worker(commands, events));
        Self {
            tx,
            rx,
            status: "Off".into(),
            connected: false,
            actions: Vec::new(),
            next_request: 0,
        }
    }
    pub fn configure(&self, settings: Settings) {
        let _ = self.tx.send(Command::Configure(settings));
    }
    pub fn refresh(&self) {
        let _ = self.tx.send(Command::Refresh);
    }
    /// Run an action; the result arrives as `Event::Ran(request, ..)` via `poll`.
    pub fn run(&mut self, action: Action, args: Value) -> u64 {
        self.next_request += 1;
        let _ = self.tx.send(Command::Run {
            request: self.next_request,
            action,
            args,
        });
        self.next_request
    }
    /// Apply worker events; returns completed runs for feedback.
    pub fn poll(&mut self) -> (bool, Vec<(u64, Result<String, String>)>) {
        let mut changed = false;
        let mut ran = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            changed = true;
            match event {
                Event::Status(status, connected) => {
                    self.status = status;
                    self.connected = connected;
                    if !connected {
                        self.actions.clear();
                    }
                }
                Event::Actions(actions) => self.actions = actions,
                Event::Ran(request, result) => ran.push((request, result)),
            }
        }
        (changed, ran)
    }
}

type Socket = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn worker(commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) {
    let mut settings = Settings::default();
    let mut socket: Option<Socket> = None;
    let mut retry_at = Instant::now();
    let mut backoff = Duration::from_secs(2);
    let mut pending: Vec<(String, u64)> = Vec::new();
    loop {
        // Commands first, so turning the integration off takes effect at once.
        loop {
            match commands.try_recv() {
                Ok(Command::Configure(next)) => {
                    if next != settings {
                        settings = next;
                        socket = None;
                        retry_at = Instant::now();
                        backoff = Duration::from_secs(2);
                        let _ = events.send(Event::Status(
                            if settings.enabled {
                                "Connecting…"
                            } else {
                                "Off"
                            }
                            .into(),
                            false,
                        ));
                    }
                }
                Ok(Command::Refresh) => {
                    if let Some(s) = socket.as_mut() {
                        let _ = send(s, json!({"request":"GetActions","id":"actions"}));
                    }
                }
                Ok(Command::Run {
                    request,
                    action,
                    args,
                }) => match socket.as_mut() {
                    Some(s) => {
                        let id = format!("run-{request}");
                        let body = json!({"request":"DoAction","id":id,"action":{"id":action.id,"name":action.name},"args":args});
                        if send(s, body).is_ok() {
                            pending.push((id, request));
                        } else {
                            let _ = events.send(Event::Ran(
                                request,
                                Err("Streamer.bot connection lost".into()),
                            ));
                        }
                    }
                    None => {
                        let _ = events.send(Event::Ran(
                            request,
                            Err("Streamer.bot isn't connected".into()),
                        ));
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        if !settings.enabled {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        if socket.is_none() {
            if Instant::now() < retry_at {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            match connect(&settings) {
                Ok(mut s) => {
                    backoff = Duration::from_secs(2);
                    let _ = events.send(Event::Status("Connected".into(), true));
                    let _ = send(&mut s, json!({"request":"GetActions","id":"actions"}));
                    socket = Some(s);
                }
                Err(error) => {
                    let _ = events.send(Event::Status(
                        format!("{error} · retrying in {}s", backoff.as_secs()),
                        false,
                    ));
                    retry_at = Instant::now() + backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                    continue;
                }
            }
        }
        let Some(s) = socket.as_mut() else { continue };
        match s.read() {
            Ok(Wire::Text(text)) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let id = value["id"].as_str().unwrap_or("");
                if id == "actions" {
                    let mut actions: Vec<Action> = value["actions"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .take(MAX_ACTIONS)
                        .filter_map(|a| {
                            Some(Action {
                                id: a["id"].as_str()?.chars().take(64).collect(),
                                name: a["name"].as_str()?.chars().take(120).collect(),
                                group: a["group"].as_str().unwrap_or("").chars().take(80).collect(),
                                enabled: a["enabled"].as_bool().unwrap_or(true),
                            })
                        })
                        .collect();
                    actions.sort_by(|a, b| {
                        (a.group.to_lowercase(), a.name.to_lowercase())
                            .cmp(&(b.group.to_lowercase(), b.name.to_lowercase()))
                    });
                    let _ = events.send(Event::Actions(actions));
                } else if let Some(at) = pending.iter().position(|(p, _)| p == id) {
                    let (_, request) = pending.remove(at);
                    let result = if value["status"] == "ok" {
                        Ok("Streamer.bot ran the action".into())
                    } else {
                        Err(value["error"]
                            .as_str()
                            .unwrap_or("Streamer.bot refused the action")
                            .chars()
                            .take(200)
                            .collect())
                    };
                    let _ = events.send(Event::Ran(request, result));
                }
            }
            Ok(Wire::Ping(_)) => {
                let _ = s.flush();
            }
            Ok(Wire::Close(_)) => {
                socket = None;
                let _ = events.send(Event::Status(
                    "Streamer.bot closed the connection".into(),
                    false,
                ));
                retry_at = Instant::now() + backoff;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                socket = None;
                for (_, request) in pending.drain(..) {
                    let _ = events.send(Event::Ran(
                        request,
                        Err("Streamer.bot connection lost".into()),
                    ));
                }
                let _ = events.send(Event::Status(
                    "Connection lost · reconnecting".into(),
                    false,
                ));
                retry_at = Instant::now() + backoff;
            }
        }
    }
}

fn send(socket: &mut Socket, value: Value) -> Result<(), ()> {
    socket
        .send(Wire::Text(value.to_string().into()))
        .map_err(|_| ())
}

fn connect(settings: &Settings) -> Result<Socket, String> {
    let url = format!("ws://{}:{}/", settings.host, settings.port);
    let (mut socket, _) = tungstenite::connect(url.as_str())
        .map_err(|_| "Streamer.bot isn't reachable".to_owned())?;
    if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    }
    // Wait briefly for Hello, which says whether a password is required.
    let deadline = Instant::now() + Duration::from_secs(5);
    let hello = loop {
        if Instant::now() > deadline {
            return Err("Streamer.bot didn't say hello".into());
        }
        match socket.read() {
            Ok(Wire::Text(text)) => {
                let value: Value = serde_json::from_str(&text).unwrap_or_default();
                if value["request"] == "Hello" {
                    break value;
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err("Streamer.bot connection failed".into()),
        }
    };
    let auth = &hello["authentication"];
    if let (Some(salt), Some(challenge)) = (auth["salt"].as_str(), auth["challenge"].as_str()) {
        let password = settings
            .password
            .as_deref()
            .ok_or("Streamer.bot needs its WebSocket password")?;
        send(&mut socket, json!({"request":"Authenticate","id":"auth","authentication":auth_string(password, salt, challenge)}))
            .map_err(|_| "Streamer.bot connection failed")?;
    }
    Ok(socket)
}

/// `b64(sha256(b64(sha256(password + salt)) + challenge))`, per Streamer.bot's docs.
pub fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let secret = STANDARD.encode(Sha256::digest(format!("{password}{salt}")));
    STANDARD.encode(Sha256::digest(format!("{secret}{challenge}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn auth_string_is_deterministic_base64() {
        let a = super::auth_string("pw", "salt", "challenge");
        assert_eq!(a, super::auth_string("pw", "salt", "challenge"));
        assert_eq!(a.len(), 44);
        assert_ne!(a, super::auth_string("pw2", "salt", "challenge"));
    }
}
