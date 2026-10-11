//! Local OBS connection through obs-websocket v5 (built into OBS 28+): stream and
//! recording state, scenes, mic mute and stream health. Off until enabled; plain
//! `ws://` to the owner's machine; reconnects with backoff.
use crate::streamer_bot::{Settings, auth_string};
use serde_json::{Value, json};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tungstenite::{Message as Wire, stream::MaybeTlsStream};

pub const DEFAULT_PORT: u16 = 4455;
/// Event categories: Scenes (4), Inputs (8), Outputs (64).
const EVENTS: u64 = 4 | 8 | 64;
const POLL: Duration = Duration::from_secs(2);
const MAX_SCENES: usize = 60;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Health {
    pub kbps: u64,
    /// Dropped (skipped) frames as a percentage of frames sent.
    pub dropped: f32,
    pub cpu: f32,
    pub fps: f32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub connected: bool,
    pub status: String,
    pub scenes: Vec<String>,
    pub scene: String,
    pub streaming: bool,
    /// How long the current stream has run, as OBS reported it at `sampled`.
    pub stream_time: Option<Duration>,
    pub recording: bool,
    /// The first mic/aux input and whether it's muted.
    pub mic: Option<(String, bool)>,
    pub health: Option<Health>,
}

enum Command {
    Configure(Settings),
    Scene(String),
    ToggleMute,
}

pub struct Obs {
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<State>,
    pub state: State,
    /// When `state.stream_time` was measured, to tick the uptime locally.
    pub sampled: Instant,
}
impl Obs {
    pub fn new() -> Self {
        let (tx, commands) = mpsc::channel();
        let (states, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("jawjack-obs".into())
            .spawn(move || worker(commands, states));
        Self {
            tx,
            rx,
            state: State {
                status: "Off".into(),
                ..Default::default()
            },
            sampled: Instant::now(),
        }
    }
    pub fn configure(&self, settings: Settings) {
        let _ = self.tx.send(Command::Configure(settings));
    }
    pub fn set_scene(&self, scene: String) {
        let _ = self.tx.send(Command::Scene(scene));
    }
    pub fn toggle_mute(&self) {
        let _ = self.tx.send(Command::ToggleMute);
    }
    /// Uptime now, ticking between OBS samples.
    pub fn uptime(&self) -> Option<Duration> {
        self.state
            .stream_time
            .filter(|_| self.state.streaming)
            .map(|t| t + self.sampled.elapsed())
    }
    /// Returns true when the state changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(state) = self.rx.try_recv() {
            if state.stream_time != self.state.stream_time {
                self.sampled = Instant::now();
            }
            changed |= state != self.state;
            self.state = state;
        }
        changed
    }
}

pub fn settings_default() -> Settings {
    Settings {
        port: DEFAULT_PORT,
        ..Settings::default()
    }
}

type Socket = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn send(socket: &mut Socket, op: u8, d: Value) -> Result<(), ()> {
    socket
        .send(Wire::Text(json!({"op": op, "d": d}).to_string().into()))
        .map_err(|_| ())
}
fn request(socket: &mut Socket, kind: &str, id: &str, data: Value) -> Result<(), ()> {
    send(
        socket,
        6,
        json!({"requestType": kind, "requestId": id, "requestData": data}),
    )
}

fn worker(commands: mpsc::Receiver<Command>, states: mpsc::Sender<State>) {
    let mut settings = settings_default();
    let mut socket: Option<Socket> = None;
    let mut state = State {
        status: "Off".into(),
        ..Default::default()
    };
    let mut retry_at = Instant::now();
    let mut backoff = Duration::from_secs(2);
    let mut next_poll = Instant::now();
    // Previous (bytes, time) for the bitrate.
    let mut last_bytes: Option<(u64, Instant)> = None;
    let publish = |state: &State| {
        let _ = states.send(state.clone());
    };
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Configure(next)) => {
                    if next != settings {
                        settings = next;
                        socket = None;
                        retry_at = Instant::now();
                        backoff = Duration::from_secs(2);
                        state = State {
                            status: if settings.enabled {
                                "Connecting…"
                            } else {
                                "Off"
                            }
                            .into(),
                            ..Default::default()
                        };
                        publish(&state);
                    }
                }
                Ok(Command::Scene(scene)) => {
                    if let Some(s) = socket.as_mut() {
                        let _ = request(
                            s,
                            "SetCurrentProgramScene",
                            "set-scene",
                            json!({"sceneName": scene}),
                        );
                    }
                }
                Ok(Command::ToggleMute) => {
                    if let (Some(s), Some((input, _))) = (socket.as_mut(), state.mic.as_ref()) {
                        let _ = request(s, "ToggleInputMute", "mute", json!({"inputName": input}));
                    }
                }
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
                    state = State {
                        connected: true,
                        status: "Connected".into(),
                        ..Default::default()
                    };
                    let _ = request(&mut s, "GetSceneList", "scenes", json!({}));
                    let _ = request(&mut s, "GetRecordStatus", "record", json!({}));
                    let _ = request(&mut s, "GetSpecialInputs", "special", json!({}));
                    publish(&state);
                    last_bytes = None;
                    next_poll = Instant::now();
                    socket = Some(s);
                }
                Err(error) => {
                    state = State {
                        status: format!("{error} · retrying in {}s", backoff.as_secs()),
                        ..Default::default()
                    };
                    publish(&state);
                    retry_at = Instant::now() + backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                    continue;
                }
            }
        }
        let Some(s) = socket.as_mut() else { continue };
        if Instant::now() >= next_poll {
            next_poll = Instant::now() + POLL;
            let _ = request(s, "GetStreamStatus", "stream", json!({}));
            let _ = request(s, "GetStats", "stats", json!({}));
        }
        match s.read() {
            Ok(Wire::Text(text)) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let d = &value["d"];
                let before = state.clone();
                match value["op"].as_u64() {
                    // Event.
                    Some(5) => {
                        let data = &d["eventData"];
                        match d["eventType"].as_str().unwrap_or("") {
                            "CurrentProgramSceneChanged" => {
                                state.scene = data["sceneName"].as_str().unwrap_or("").to_owned();
                            }
                            "SceneListChanged" => {
                                let _ = request(s, "GetSceneList", "scenes", json!({}));
                            }
                            "StreamStateChanged" => {
                                state.streaming = data["outputActive"].as_bool().unwrap_or(false);
                                if !state.streaming {
                                    state.stream_time = None;
                                    state.health = None;
                                }
                            }
                            "RecordStateChanged" => {
                                state.recording = data["outputActive"].as_bool().unwrap_or(false);
                            }
                            "InputMuteStateChanged" => {
                                if let Some((name, muted)) = state.mic.as_mut()
                                    && data["inputName"].as_str() == Some(name.as_str())
                                {
                                    *muted = data["inputMuted"].as_bool().unwrap_or(false);
                                }
                            }
                            _ => {}
                        }
                    }
                    // Request response.
                    Some(7) => {
                        let data = &d["responseData"];
                        let ok = d["requestStatus"]["result"].as_bool().unwrap_or(false);
                        match d["requestId"].as_str().unwrap_or("") {
                            "scenes" if ok => {
                                // OBS lists scenes bottom-up; show them top-down as in its dock.
                                let mut scenes: Vec<String> = data["scenes"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|s| {
                                        s["sceneName"]
                                            .as_str()
                                            .map(|n| n.chars().take(80).collect())
                                    })
                                    .collect();
                                scenes.reverse();
                                scenes.truncate(MAX_SCENES);
                                state.scenes = scenes;
                                state.scene = data["currentProgramSceneName"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_owned();
                            }
                            "record" if ok => {
                                state.recording = data["outputActive"].as_bool().unwrap_or(false)
                            }
                            "special" if ok => {
                                if let Some(mic) = ["mic1", "mic2", "mic3", "mic4"]
                                    .iter()
                                    .find_map(|k| data[*k].as_str().filter(|n| !n.is_empty()))
                                {
                                    state.mic = Some((mic.to_owned(), false));
                                    let _ = request(
                                        s,
                                        "GetInputMute",
                                        "mic",
                                        json!({"inputName": mic}),
                                    );
                                }
                            }
                            "mic" if ok => {
                                if let Some((_, muted)) = state.mic.as_mut() {
                                    *muted = data["inputMuted"].as_bool().unwrap_or(false);
                                }
                            }
                            "stream" if ok => {
                                state.streaming = data["outputActive"].as_bool().unwrap_or(false);
                                state.stream_time = state.streaming.then(|| {
                                    Duration::from_millis(
                                        data["outputDuration"].as_u64().unwrap_or(0),
                                    )
                                });
                                if state.streaming {
                                    let bytes = data["outputBytes"].as_u64().unwrap_or(0);
                                    let total = data["outputTotalFrames"].as_u64().unwrap_or(0);
                                    let skipped = data["outputSkippedFrames"].as_u64().unwrap_or(0);
                                    let kbps = last_bytes
                                        .filter(|(b, _)| *b <= bytes)
                                        .map(|(b, at)| {
                                            let seconds = at.elapsed().as_secs_f64().max(0.1);
                                            ((bytes - b) as f64 * 8. / 1000. / seconds) as u64
                                        })
                                        .unwrap_or(0);
                                    last_bytes = Some((bytes, Instant::now()));
                                    let health = state.health.get_or_insert_default();
                                    health.kbps = kbps;
                                    health.dropped = if total > 0 {
                                        skipped as f32 * 100. / total as f32
                                    } else {
                                        0.
                                    };
                                } else {
                                    last_bytes = None;
                                    state.health = None;
                                }
                            }
                            "stats" if ok => {
                                if let Some(health) = state.health.as_mut() {
                                    health.cpu = data["cpuUsage"].as_f64().unwrap_or(0.) as f32;
                                    health.fps = data["activeFps"].as_f64().unwrap_or(0.) as f32;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if state != before {
                    publish(&state);
                }
            }
            Ok(Wire::Close(frame)) => {
                socket = None;
                let auth = frame.is_some_and(|f| u16::from(f.code) == 4009);
                state = State {
                    status: if auth {
                        "OBS refused the password"
                    } else {
                        "OBS closed the connection"
                    }
                    .into(),
                    ..Default::default()
                };
                publish(&state);
                retry_at = Instant::now() + backoff;
                backoff = (backoff * 2).min(Duration::from_secs(60));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                socket = None;
                state = State {
                    status: "Connection lost · reconnecting".into(),
                    ..Default::default()
                };
                publish(&state);
                retry_at = Instant::now() + backoff;
            }
        }
    }
}

fn connect(settings: &Settings) -> Result<Socket, String> {
    let url = format!("ws://{}:{}/", settings.host, settings.port);
    let (mut socket, _) = tungstenite::connect(url.as_str())
        .map_err(|_| "OBS isn't running, or its WebSocket server is off".to_owned())?;
    if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut identified = false;
    while !identified {
        if Instant::now() > deadline {
            return Err("OBS didn't answer".into());
        }
        match socket.read() {
            Ok(Wire::Text(text)) => {
                let value: Value = serde_json::from_str(&text).unwrap_or_default();
                match value["op"].as_u64() {
                    // Hello: identify, with authentication when OBS asks for it.
                    Some(0) => {
                        let auth = &value["d"]["authentication"];
                        let mut identify = json!({"rpcVersion": 1, "eventSubscriptions": EVENTS});
                        if let (Some(salt), Some(challenge)) =
                            (auth["salt"].as_str(), auth["challenge"].as_str())
                        {
                            let password = settings
                                .password
                                .as_deref()
                                .ok_or("OBS needs its WebSocket password")?;
                            identify["authentication"] =
                                json!(auth_string(password, salt, challenge));
                        }
                        send(&mut socket, 1, identify).map_err(|_| "OBS connection failed")?;
                    }
                    Some(2) => identified = true,
                    _ => {}
                }
            }
            Ok(Wire::Close(frame)) => {
                return Err(if frame.is_some_and(|f| u16::from(f.code) == 4009) {
                    "OBS refused the password".into()
                } else {
                    "OBS closed the connection".into()
                });
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err("OBS connection failed".into()),
        }
    }
    Ok(socket)
}

/// What a network scan found at one address.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub host: String,
    pub port: u16,
    /// False for an old obs-websocket 4.x server (OBS 27 and earlier plugin).
    pub supported: bool,
}
/// The legacy obs-websocket 4.x port, recognised so we can say "update OBS".
const LEGACY_PORT: u16 = 4444;

/// OBS instances on this machine's local network (/24), for "Find OBS".
/// obs-websocket has no announcement, so each address is tried on the given
/// port, the default 4455 and the legacy 4444 with a short timeout, and
/// confirmed by its greeting. Private ranges only.
pub fn discover(port: u16) -> Vec<Found> {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
    // The LAN address the system would route from (a UDP connect sends nothing).
    let local = UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").map(|_| s))
        .and_then(|s| s.local_addr())
        .ok()
        .and_then(|a| match a.ip() {
            IpAddr::V4(ip) if ip.is_private() => Some(ip),
            _ => None,
        });
    let mut hosts: Vec<Ipv4Addr> = vec![Ipv4Addr::LOCALHOST];
    if let Some(ip) = local {
        let [a, b, c, _] = ip.octets();
        hosts.extend((1..=254).map(|d| Ipv4Addr::new(a, b, c, d)));
    }
    let mut ports = vec![port, DEFAULT_PORT, LEGACY_PORT];
    ports.dedup();
    ports.sort_unstable();
    ports.dedup();
    let found = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for chunk in hosts.chunks(8) {
            let (found, ports) = (&found, &ports);
            scope.spawn(move || {
                for ip in chunk {
                    for port in ports {
                        let addr = SocketAddr::new(IpAddr::V4(*ip), *port);
                        if TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_err() {
                            continue;
                        }
                        if let Some(supported) = probe(&addr.to_string())
                            && let Ok(mut f) = found.lock()
                        {
                            f.push(Found {
                                host: ip.to_string(),
                                port: *port,
                                supported,
                            });
                        }
                    }
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap_or_default();
    found.sort_by(|a, b| (!a.supported, &a.host, a.port).cmp(&(!b.supported, &b.host, b.port)));
    found
}
/// Some(true) for an obs-websocket 5 Hello; Some(false) for a WebSocket on the
/// legacy port that stays silent (4.x waits for the client); None otherwise.
fn probe(address: &str) -> Option<bool> {
    let (mut socket, _) = tungstenite::connect(format!("ws://{address}/")).ok()?;
    if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
    }
    let result = match socket.read() {
        Ok(Wire::Text(text)) => serde_json::from_str::<Value>(&text)
            .is_ok_and(|v| v["op"] == 0 && v["d"]["obsWebSocketVersion"].is_string())
            .then_some(true),
        Err(tungstenite::Error::Io(e))
            if address.ends_with(&format!(":{LEGACY_PORT}"))
                && matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
        {
            Some(false)
        }
        _ => None,
    };
    let _ = socket.close(None);
    result
}
