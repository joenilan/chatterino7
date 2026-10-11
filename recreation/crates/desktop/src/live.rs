//! Twitch EventSub chat for one account, sharded over up to three WebSockets.
//! No credentials in logs/state.
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
    pub can_read_emotes: bool,
    /// Scopes Twitch reported for this token at its last validation.
    pub scopes: Arc<[String]>,
}
impl Identity {
    pub fn has(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}
fn user(identity: &Option<Identity>) -> Option<&str> {
    identity.as_ref().map(|i| i.user_id.as_str())
}
#[derive(Clone, PartialEq, Eq, Default)]
struct Config {
    identity: Option<Identity>,
    channels: Vec<String>,
}
pub enum Event {
    State(String, String, bool),
    Resolved(String, String),
    /// Follow or reward redemption on the signed-in broadcaster's own channel.
    Activity(crate::activity_feed::Item),
    /// A whisper received by the signed-in account.
    Whisper {
        id: String,
        from_login: String,
        from_name: String,
        text: String,
    },
    RoomSettings(String, crate::room_settings::RoomSettings, bool),
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
    /// Set the account and the channels to connect, highest priority first.
    /// Channel changes are applied incrementally; returns true only when the
    /// account changed and every channel restarts.
    pub fn configure(&self, identity: Option<Identity>, channels: Vec<String>) -> bool {
        let mut seen = HashSet::new();
        let channels = channels
            .into_iter()
            .filter(|c| seen.insert(c.clone()))
            .collect();
        let next = Config { identity, channels };
        if let Ok(mut c) = self.config.lock()
            && *c != next
        {
            // A refreshed token or newly granted scopes keep existing subscriptions,
            // which Twitch ties to the user's authorization, not the token string.
            let account_changed = user(&c.identity) != user(&next.identity);
            *c = next;
            if account_changed {
                self.epoch.fetch_add(1, Ordering::Relaxed);
            }
            return account_changed;
        }
        false
    }
    pub fn send(
        &self,
        request: u64,
        channel: String,
        text: String,
        reply_parent: Option<String>,
    ) -> Result<(), String> {
        if crate::commands::is_command(&text) {
            return Err(
                "Slash commands must be handled locally. Nothing was sent; draft kept.".into(),
            );
        }
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
                let mut request_body =
                    json!({"broadcaster_id":id,"sender_id":identity.user_id,"message":text});
                if let Some(parent) = reply_parent {
                    request_body["reply_parent_message_id"] = json!(parent);
                }
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
pub(crate) fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize Twitch networking".into())
}
pub(crate) fn response_json(response: reqwest::blocking::Response) -> Result<(u16, Value), String> {
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
        .ok_or_else(|| "Channel was not found on Twitch (renamed, banned or deleted)".into())
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
struct RoomSnapshot {
    version: u64,
    identity: Identity,
    channel: String,
    id: String,
}
fn room_snapshot_worker(
    tx: mpsc::SyncSender<(u64, Event)>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
) -> mpsc::SyncSender<RoomSnapshot> {
    let (sender, jobs) = mpsc::sync_channel::<RoomSnapshot>(32);
    std::thread::spawn(move || {
        let Ok(client) = client() else {
            return;
        };
        while let Ok(job) = jobs.recv() {
            if epoch.load(Ordering::Relaxed) != job.version {
                continue;
            }
            if let Ok(response) = client
                .get("https://api.twitch.tv/helix/chat/settings")
                .query(&[("broadcaster_id", job.id.as_str())])
                .header("Client-Id", CLIENT_ID)
                .bearer_auth(&job.identity.access)
                .send()
            {
                if epoch.load(Ordering::Relaxed) != job.version {
                    continue;
                }
                if let Ok((200, value)) = response_json(response)
                    && let Some(settings) =
                        crate::room_settings::RoomSettings::parse(&value["data"][0])
                {
                    let _ = tx.send((
                        job.version,
                        Event::RoomSettings(job.channel, settings, true),
                    ));
                }
            }
        }
    });
    sender
}
/// Every connected channel uses one subscription per kind.
const CHANNEL_KINDS: [&str; 6] = [
    "channel.chat.message",
    "channel.chat.message_delete",
    "channel.chat.clear_user_messages",
    "channel.chat.clear",
    "channel.chat.notification",
    "channel.chat_settings.update",
];
/// Notices and room settings are useful but not required for chat to work.
fn optional_kind(kind: &str) -> bool {
    matches!(
        kind,
        "channel.chat.notification" | "channel.chat_settings.update"
    )
}
/// Extra, optional subscriptions for the signed-in broadcaster's own channel,
/// feeding the Studio activity feed: (type, version).
const OWN_KINDS: [(&str, &str); 2] = [
    ("channel.follow", "2"),
    ("channel.channel_points_custom_reward_redemption.add", "1"),
];
/// EventSub allows 300 enabled subscriptions per WebSocket and three WebSockets
/// per user token. One shard also carries the own channel's extras.
const CHANNELS_PER_SOCKET: usize = (300 - OWN_KINDS.len()) / CHANNEL_KINDS.len();
const MAX_SOCKETS: usize = 3;
pub const MAX_CHANNELS: usize = CHANNELS_PER_SOCKET * MAX_SOCKETS;
/// Subscription requests stay well inside Helix's 800 points/minute budget,
/// leaving room for profiles, emotes and room settings.
const REQUEST_INTERVAL: Duration = Duration::from_millis(110);
/// A channel whose required subscription failed is retried after this delay.
const FAILURE_RETRY: Duration = Duration::from_secs(60);
/// A channel login that no longer exists is checked again much later.
const MISSING_RETRY: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
struct Dedup {
    seen: HashSet<String>,
    order: VecDeque<String>,
}
/// What one socket thread reports to the manager.
#[derive(Default)]
struct ShardStatus {
    session: Option<String>,
    /// Bumped on every fresh connection; reconnect handoffs keep subscriptions.
    generation: u64,
    /// Broadcaster IDs (and reasons) whose subscriptions Twitch revoked.
    revoked: Vec<(String, String)>,
}
struct Shard {
    status: Arc<Mutex<ShardStatus>>,
    stop: Arc<AtomicBool>,
    /// Session state the manager last reported to the UI.
    seen_generation: u64,
    online: bool,
}
/// Per-channel subscription progress on its shard.
struct Assignment {
    shard: usize,
    generation: u64,
    broadcaster: Option<String>,
    ids: Vec<String>,
    step: usize,
    notices_unavailable: bool,
    ready: bool,
    failed_until: Option<Instant>,
}
impl Assignment {
    fn new(shard: usize) -> Self {
        Self {
            shard,
            generation: 0,
            broadcaster: None,
            ids: Vec::new(),
            step: 0,
            notices_unavailable: false,
            ready: false,
            failed_until: None,
        }
    }
    fn restart(&mut self, generation: u64) {
        self.generation = generation;
        self.ids.clear();
        self.step = 0;
        self.notices_unavailable = false;
        self.ready = false;
    }
}
fn state(
    tx: &mpsc::SyncSender<(u64, Event)>,
    version: u64,
    channel: &str,
    text: &str,
    ready: bool,
) {
    let _ = tx.send((version, Event::State(channel.into(), text.into(), ready)));
}
/// Owns channel-to-socket assignment and all subscription requests. Only an
/// account change resets everything; channel changes are incremental.
fn run(
    config: Arc<Mutex<Config>>,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<(u64, Event)>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
) {
    let snapshots = room_snapshot_worker(tx.clone(), epoch.clone());
    let names: Arc<Mutex<HashMap<String, String>>> = Default::default();
    let dedup: Arc<Mutex<Dedup>> = Default::default();
    let Ok(client) = client() else { return };
    let mut identity: Option<Identity> = None;
    let mut shards: Vec<Option<Shard>> = (0..MAX_SOCKETS).map(|_| None).collect();
    let mut assigned: HashMap<String, Assignment> = HashMap::new();
    let mut deletions: VecDeque<String> = VecDeque::new();
    let mut over_cap: HashSet<String> = HashSet::new();
    let mut next_request = Instant::now();
    let mut turn = 0usize;
    // Account-wide whisper subscription: (shard, generation) it was made on, and retry time.
    let mut whispers: Option<(usize, u64)> = None;
    let mut whispers_retry = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let c = config.lock().map(|c| c.clone()).unwrap_or_default();
        let version = epoch.load(Ordering::Relaxed);
        if user(&c.identity) != user(&identity) {
            // Account change: tear everything down; the UI already reset its panes.
            for shard in shards.iter_mut().filter_map(Option::take) {
                shard.stop.store(true, Ordering::Relaxed);
            }
            assigned.clear();
            deletions.clear();
            over_cap.clear();
            whispers = None;
            if let Ok(mut names) = names.lock() {
                names.clear();
            }
        }
        // Always use the newest token for further requests.
        identity = c.identity.clone();
        let Some(id) = identity.clone() else {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };
        // Desired channels in priority order, capped by what EventSub allows.
        let wanted: Vec<&String> = c.channels.iter().take(MAX_CHANNELS).collect();
        for channel in c.channels.iter().skip(MAX_CHANNELS) {
            if over_cap.insert(channel.clone()) {
                state(
                    &tx,
                    version,
                    channel,
                    &format!(
                        "Not connected · Jawjack connects at most {MAX_CHANNELS} channels at once. Close others to connect this one."
                    ),
                    false,
                );
            }
        }
        over_cap.retain(|ch| c.channels.iter().skip(MAX_CHANNELS).any(|c| c == ch));
        // Removed channels release their subscriptions.
        let removed: Vec<String> = assigned
            .keys()
            .filter(|ch| !wanted.contains(ch))
            .cloned()
            .collect();
        for channel in removed {
            if let Some(a) = assigned.remove(&channel) {
                deletions.extend(a.ids);
                if let (Some(b), Ok(mut names)) = (a.broadcaster, names.lock()) {
                    names.remove(&b);
                }
            }
        }
        // New channels go to the least-loaded shard with room, opening up to three sockets.
        for channel in &wanted {
            if assigned.contains_key(*channel) {
                continue;
            }
            let load = |i: usize| assigned.values().filter(|a| a.shard == i).count();
            let Some(index) = (0..MAX_SOCKETS)
                .filter(|i| load(*i) < CHANNELS_PER_SOCKET)
                .min_by_key(|i| (shards[*i].is_none(), load(*i)))
            else {
                break;
            };
            if shards[index].is_none() {
                let status: Arc<Mutex<ShardStatus>> = Default::default();
                let shard_stop = Arc::new(AtomicBool::new(false));
                let (s, st, t, e, n, d) = (
                    status.clone(),
                    shard_stop.clone(),
                    tx.clone(),
                    epoch.clone(),
                    names.clone(),
                    dedup.clone(),
                );
                std::thread::spawn(move || socket_loop(s, st, t, e, version, n, d));
                shards[index] = Some(Shard {
                    status,
                    stop: shard_stop,
                    seen_generation: 0,
                    online: false,
                });
            }
            let mut assignment = Assignment::new(index);
            // On an already-online shard, start against its current session.
            if let Some(shard) = &shards[index]
                && shard.online
            {
                assignment.generation = shard.seen_generation;
            }
            assigned.insert((*channel).clone(), assignment);
            state(&tx, version, channel, "Connecting…", false);
        }
        // Idle sockets close; Twitch would drop an empty session anyway.
        for (index, slot) in shards.iter_mut().enumerate() {
            if slot.is_some()
                && !assigned.values().any(|a| a.shard == index)
                && let Some(shard) = slot.take()
            {
                shard.stop.store(true, Ordering::Relaxed);
            }
        }
        // Reflect socket sessions: a fresh connection resubscribes its channels.
        for (index, slot) in shards.iter_mut().enumerate() {
            let Some(shard) = slot else { continue };
            let (session, generation, revoked) = shard
                .status
                .lock()
                .map(|mut s| {
                    (
                        s.session.clone(),
                        s.generation,
                        std::mem::take(&mut s.revoked),
                    )
                })
                .unwrap_or_default();
            for (broadcaster, reason) in revoked {
                if let Some((channel, a)) = assigned
                    .iter_mut()
                    .find(|(_, a)| a.broadcaster.as_deref() == Some(broadcaster.as_str()))
                {
                    a.restart(a.generation);
                    a.failed_until = Some(Instant::now() + FAILURE_RETRY);
                    state(
                        &tx,
                        version,
                        channel,
                        &format!(
                            "Twitch revoked this chat subscription ({reason}). Retrying in a minute."
                        ),
                        false,
                    );
                }
            }
            let online = session.is_some();
            if online != shard.online || generation != shard.seen_generation {
                for (channel, a) in assigned.iter_mut().filter(|(_, a)| a.shard == index) {
                    if online {
                        if a.generation != generation {
                            a.restart(generation);
                            state(&tx, version, channel, "Connecting…", false);
                        }
                    } else {
                        a.ready = false;
                        state(
                            &tx,
                            version,
                            channel,
                            "Reconnecting; messages during the gap are unavailable.",
                            false,
                        );
                    }
                }
                shard.online = online;
                shard.seen_generation = generation;
            }
        }
        if Instant::now() < next_request {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        // One paced request. A fresh session with no subscription goes first
        // (Twitch closes it after ten seconds); otherwise shards take turns, each
        // serving its highest-priority waiting channel.
        let sessions: Vec<Option<(String, u64)>> = shards
            .iter()
            .map(|s| {
                s.as_ref().and_then(|s| {
                    s.status
                        .lock()
                        .ok()
                        .and_then(|st| st.session.clone().map(|id| (id, st.generation)))
                })
            })
            .collect();
        let now = Instant::now();
        let pending = |index: usize, a: &Assignment| {
            a.shard == index
                && !a.ready
                && a.failed_until.is_none_or(|t| now >= t)
                && sessions[index]
                    .as_ref()
                    .is_some_and(|(_, g)| *g == a.generation)
        };
        let starving = (0..MAX_SOCKETS).find(|i| {
            sessions[*i].is_some()
                && !assigned
                    .values()
                    .any(|a| a.shard == *i && !a.ids.is_empty())
                && assigned.values().any(|a| pending(*i, a))
        });
        let mut job = None;
        for index in starving
            .into_iter()
            .chain((0..MAX_SOCKETS).map(|k| (turn + k) % MAX_SOCKETS))
        {
            if let Some(channel) = wanted
                .iter()
                .find(|ch| assigned.get(**ch).is_some_and(|a| pending(index, a)))
            {
                job = Some(((*channel).clone(), index));
                break;
            }
        }
        turn = (turn + 1) % MAX_SOCKETS;
        if let Some((channel, index)) = job {
            let session = sessions[index]
                .as_ref()
                .map(|(s, _)| s.clone())
                .unwrap_or_default();
            if let Some(a) = assigned.get_mut(&channel) {
                next_request = Instant::now() + REQUEST_INTERVAL;
                let step = Step {
                    client: &client,
                    identity: &id,
                    session: &session,
                    names: &names,
                    tx: &tx,
                    version,
                    snapshots: &snapshots,
                };
                step.run(&channel, a, &mut next_request);
            }
        } else if id.has("user:read:whispers")
            && now >= whispers_retry
            && !whispers.is_some_and(|(i, g)| {
                sessions[i]
                    .as_ref()
                    .is_some_and(|(_, current)| *current == g)
            })
            && let Some((index, (session, generation))) = sessions
                .iter()
                .enumerate()
                .find_map(|(i, s)| s.clone().map(|s| (i, s)))
        {
            // Whispers aren't tied to a channel; one subscription on any live socket.
            next_request = Instant::now() + REQUEST_INTERVAL;
            let body = json!({"type":"user.whisper.message","version":"1","condition":{"user_id":id.user_id},"transport":{"method":"websocket","session_id":session}});
            let ok = client
                .post("https://api.twitch.tv/helix/eventsub/subscriptions")
                .header("Client-Id", CLIENT_ID)
                .bearer_auth(&id.access)
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .send()
                .ok()
                .and_then(|r| response_json(r).ok())
                .is_some_and(|(status, _)| status == 202 || status == 409);
            if ok {
                whispers = Some((index, generation));
            } else {
                whispers_retry = Instant::now() + FAILURE_RETRY;
            }
        } else if let Some(sub) = deletions.pop_front() {
            next_request = Instant::now() + REQUEST_INTERVAL;
            // Best effort: an ended session removes its subscriptions anyway.
            let _ = client
                .delete("https://api.twitch.tv/helix/eventsub/subscriptions")
                .query(&[("id", sub.as_str())])
                .header("Client-Id", CLIENT_ID)
                .bearer_auth(&id.access)
                .send();
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    for shard in shards.iter_mut().filter_map(Option::take) {
        shard.stop.store(true, Ordering::Relaxed);
    }
}
/// Context for one subscription request on one shard session.
struct Step<'a> {
    client: &'a reqwest::blocking::Client,
    identity: &'a Identity,
    session: &'a str,
    names: &'a Arc<Mutex<HashMap<String, String>>>,
    tx: &'a mpsc::SyncSender<(u64, Event)>,
    version: u64,
    snapshots: &'a mpsc::SyncSender<RoomSnapshot>,
}
impl Step<'_> {
    /// Perform the channel's next request: resolve its ID, then one subscription.
    fn run(&self, channel: &str, a: &mut Assignment, next_request: &mut Instant) {
        let (tx, version) = (self.tx, self.version);
        let Some(broadcaster) = a.broadcaster.clone() else {
            match resolve(self.client, self.identity, channel) {
                Ok(id) => {
                    if let Ok(mut names) = self.names.lock() {
                        names.insert(id.clone(), channel.to_owned());
                    }
                    let _ = tx.send((version, Event::Resolved(channel.into(), id.clone())));
                    a.broadcaster = Some(id);
                }
                Err(error) => {
                    // A missing account (renamed, banned or deleted) rarely returns soon.
                    let retry = if error.contains("not found") {
                        MISSING_RETRY
                    } else {
                        FAILURE_RETRY
                    };
                    a.failed_until = Some(Instant::now() + retry);
                    state(tx, version, channel, &error, false);
                }
            }
            return;
        };
        let own = broadcaster == self.identity.user_id;
        let total = CHANNEL_KINDS.len() + if own { OWN_KINDS.len() } else { 0 };
        let extra = a.step >= CHANNEL_KINDS.len();
        let (kind, kind_version, condition) = if extra {
            let (kind, kind_version) = OWN_KINDS[a.step - CHANNEL_KINDS.len()];
            let condition = if kind == "channel.follow" {
                json!({"broadcaster_user_id":broadcaster,"moderator_user_id":self.identity.user_id})
            } else {
                json!({"broadcaster_user_id":broadcaster})
            };
            (kind, kind_version, condition)
        } else {
            (
                CHANNEL_KINDS[a.step],
                "1",
                json!({"broadcaster_user_id":broadcaster,"user_id":self.identity.user_id}),
            )
        };
        let body = json!({"type":kind,"version":kind_version,"condition":condition,"transport":{"method":"websocket","session_id":self.session}});
        let response = match self
            .client
            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&self.identity.access)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
        {
            Ok(response) => response,
            // Transient network failure: retry the same step shortly.
            Err(_) => {
                *next_request = Instant::now() + Duration::from_secs(2);
                return;
            }
        };
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
        };
        let (reset, remaining) = (header("ratelimit-reset"), header("ratelimit-remaining"));
        let until_reset = || {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Duration::from_secs(reset.map_or(5, |r| r.saturating_sub(now).clamp(1, 60)))
        };
        // Leave headroom for the rest of the app when the shared budget runs low.
        if remaining.is_some_and(|r| r < 80) {
            *next_request = Instant::now() + until_reset();
        }
        let Ok((status, value)) = response_json(response) else {
            *next_request = Instant::now() + Duration::from_secs(2);
            return;
        };
        if status == 429 {
            *next_request = Instant::now() + until_reset();
            return;
        }
        if status == 202 && value["data"][0]["status"].as_str() == Some("enabled") {
            if let Some(id) = value["data"][0]["id"].as_str() {
                a.ids.push(id.to_owned());
            }
        } else if extra {
            // Activity-feed extras are best effort (missing scope or role).
        } else if optional_kind(kind) {
            a.notices_unavailable = true;
        } else {
            a.restart(a.generation);
            a.failed_until = Some(Instant::now() + FAILURE_RETRY);
            state(
                tx,
                version,
                channel,
                &format!("Chat subscription failed (HTTP {status}). Retrying in a minute."),
                false,
            );
            return;
        }
        a.step += 1;
        if a.step == total {
            a.ready = true;
            a.failed_until = None;
            let _ = self.snapshots.try_send(RoomSnapshot {
                version,
                identity: self.identity.clone(),
                channel: channel.into(),
                id: broadcaster,
            });
            state(
                tx,
                version,
                channel,
                if a.notices_unavailable {
                    "Connected · Some room events unavailable until reconnect"
                } else {
                    "Connected"
                },
                true,
            );
        }
    }
}
/// One EventSub WebSocket. Reports its session to the manager and delivers events.
fn socket_loop(
    status: Arc<Mutex<ShardStatus>>,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<(u64, Event)>,
    epoch: Arc<std::sync::atomic::AtomicU64>,
    version: u64,
    names: Arc<Mutex<HashMap<String, String>>>,
    dedup: Arc<Mutex<Dedup>>,
) {
    let live = || !stop.load(Ordering::Relaxed) && epoch.load(Ordering::Relaxed) == version;
    let deliver_one = |v: &Value| {
        if let Ok(mut d) = dedup.lock() {
            let d = &mut *d;
            deliver(v, &names, &tx, &mut d.seen, &mut d.order, version);
        }
    };
    let mut retry = 1;
    while live() {
        let connected_at = Instant::now();
        let result: Result<(), String> = (|| {
            let mut socket = open("wss://eventsub.wss.twitch.tv/ws?keepalive_timeout_seconds=60")?;
            let mut last = Instant::now();
            let mut timeout = Duration::from_secs(10);
            while live() {
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
                match v["metadata"]["message_type"].as_str().unwrap_or("") {
                    "session_welcome" => {
                        timeout = Duration::from_secs(
                            v["payload"]["session"]["keepalive_timeout_seconds"]
                                .as_u64()
                                .unwrap_or(60)
                                + 5,
                        );
                        let id = v["payload"]["session"]["id"]
                            .as_str()
                            .ok_or("Missing Twitch session")?;
                        if let Ok(mut s) = status.lock() {
                            s.session = Some(id.into());
                            s.generation += 1;
                        }
                    }
                    "session_reconnect" => {
                        let url = v["payload"]["session"]["reconnect_url"]
                            .as_str()
                            .ok_or("Missing reconnect URL")?;
                        let mut replacement = open(url)?;
                        let deadline = Instant::now() + Duration::from_secs(10);
                        let session = loop {
                            if !live() {
                                return Ok(());
                            }
                            if Instant::now() > deadline {
                                return Err("Twitch reconnect welcome timed out".into());
                            }
                            if let Some(old) = frame(&mut socket)? {
                                deliver_one(&old);
                            }
                            if let Some(next) = frame(&mut replacement)?
                                && next["metadata"]["message_type"] == "session_welcome"
                            {
                                break next["payload"]["session"]["id"].as_str().map(str::to_owned);
                            }
                        };
                        // Subscriptions carry over: keep the generation, update the session.
                        if let (Some(session), Ok(mut s)) = (session, status.lock()) {
                            s.session = Some(session);
                        }
                        // Read queued old-socket events before closing after the replacement welcome.
                        for _ in 0..1024 {
                            match frame(&mut socket) {
                                Ok(Some(old)) => deliver_one(&old),
                                _ => break,
                            }
                        }
                        let _ = socket.close(None);
                        socket = replacement;
                        last = Instant::now();
                    }
                    "revocation" => {
                        let sub = &v["payload"]["subscription"];
                        if let (Some(b), Ok(mut s)) = (
                            sub["condition"]["broadcaster_user_id"].as_str(),
                            status.lock(),
                        ) {
                            let reason = sub["status"].as_str().unwrap_or("revoked");
                            s.revoked
                                .push((b.into(), reason.chars().take(40).collect()));
                        }
                    }
                    _ => deliver_one(&v),
                }
            }
            let _ = socket.close(None);
            Ok(())
        })();
        if let Ok(mut s) = status.lock() {
            s.session = None;
        }
        if result.is_err() {
            let deadline = Instant::now() + Duration::from_secs(retry);
            while Instant::now() < deadline && live() {
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
    if order.len() > 8192
        && let Some(id) = order.pop_front()
    {
        seen.remove(&id);
    }
    let e = &v["payload"]["event"];
    if v["payload"]["subscription"]["type"] == "user.whisper.message" {
        let text = |key: &str| {
            e[key]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(64)
                .collect::<String>()
        };
        let body: String = e["whisper"]["text"]
            .as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(500)
            .collect();
        if let Some(login) = chat_core::twitch_login(&text("from_user_login")) {
            let _ = tx.send((
                version,
                Event::Whisper {
                    id: text("whisper_id"),
                    from_login: login,
                    from_name: text("from_user_name"),
                    text: body,
                },
            ));
        }
        return;
    }
    let id = e["broadcaster_user_id"].as_str().unwrap_or("");
    let Some(channel) = map.lock().ok().and_then(|m| m.get(id).cloned()) else {
        return;
    };
    if v["payload"]["subscription"]["type"] == "channel.chat_settings.update" {
        if let Some(settings) = crate::room_settings::RoomSettings::parse(e) {
            let _ = tx.send((version, Event::RoomSettings(channel, settings, false)));
        }
        return;
    }
    let kind = v["payload"]["subscription"]["type"].as_str().unwrap_or("");
    if let Some(item) = crate::activity_feed::Feed::from_eventsub(kind, e) {
        let _ = tx.send((version, Event::Activity(item)));
        return;
    }
    let text = |key: &str| e[key].as_str().unwrap_or("").to_owned();
    let event = match v["payload"]["subscription"]["type"].as_str().unwrap_or("") {
        kind @ ("channel.chat.message" | "channel.chat.notification") => {
            ChatEvent::Message(chat_message(
                e,
                envelope,
                &channel,
                id,
                v["metadata"]["message_timestamp"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|time| time.timestamp()),
                kind == "channel.chat.message",
            ))
        }
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

/// Build a message from an EventSub chat event (also used for recent history,
/// which is converted into the same shape).
pub(crate) fn chat_message(
    e: &Value,
    envelope: &str,
    channel: &str,
    id: &str,
    sent_at: Option<i64>,
    chat: bool,
) -> Message {
    let text = |key: &str| e[key].as_str().unwrap_or("").to_owned();
    Message {
        sent_at,
        id: if text("message_id").is_empty() {
            envelope.to_owned()
        } else {
            text("message_id")
        },
        channel_id: channel.to_owned(),
        user_id: text("chatter_user_id"),
        display_name: if e["chatter_is_anonymous"].as_bool() == Some(true) {
            "Anonymous".into()
        } else {
            // Twitch can send display names with stray whitespace; Chatterino trims too.
            let name = text("chatter_user_name");
            match name.trim() {
                "" => "Twitch".into(),
                trimmed => trimmed.to_owned(),
            }
        },
        login: chat_core::twitch_login(&text("chatter_user_login")),
        replyable: chat
            && e["source_broadcaster_user_id"]
                .as_str()
                .is_none_or(|source| source == id),
        reply: parse_reply(&e["reply"]),
        mentions: e["message"]["fragments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|f| f["type"] == "mention")
            .filter_map(|f| f["mention"]["user_id"].as_str())
            .filter(|s| !s.is_empty() && s.len() <= 255 && !s.chars().any(char::is_control))
            .take(64)
            .map(str::to_owned)
            .collect(),
        name_color: e["color"]
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .filter(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|s| u32::from_str_radix(s, 16).ok()),
        badges: e["badges"]
            .as_array()
            .into_iter()
            .flatten()
            .take(8)
            .filter_map(|b| {
                Some(chat_core::Badge {
                    set_id: b["set_id"].as_str()?.chars().take(128).collect(),
                    id: b["id"].as_str()?.chars().take(128).collect(),
                })
            })
            .collect(),
        fragments: twitch_fragments(&e["message"]),
        presentation: chat_core::MessagePresentation {
            local_alias: None,
            local_color: None,
            kind: e["message_type"]
                .as_str()
                .unwrap_or("text")
                .chars()
                .take(80)
                .collect(),
            bits: e["cheer"]["bits"].as_u64().unwrap_or(0),
            reward_id: e["channel_points_custom_reward_id"]
                .as_str()
                .map(|s| s.chars().take(128).collect()),
            notice_type: e["notice_type"]
                .as_str()
                .map(|s| s.chars().take(80).collect()),
            notice: e["system_message"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().take(1200).collect()),
            // Provenance follows the source ID even when its optional display
            // name is unavailable, so destination-room cosmetics stay suppressed.
            source: e["source_broadcaster_user_id"]
                .as_str()
                .filter(|source| !source.is_empty() && *source != id)
                .map(|_| {
                    e["source_broadcaster_user_name"]
                        .as_str()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or("Shared Chat")
                        .chars()
                        .take(80)
                        .collect()
                }),
            historical: false,
        },
        deleted: false,
    }
}

fn twitch_fragments(message: &Value) -> Vec<Fragment> {
    let original = message["text"].as_str().unwrap_or("");
    let raw: Vec<_> = message["fragments"]
        .as_array()
        .into_iter()
        .flatten()
        .take(256)
        .collect();
    let joined: String = raw.iter().filter_map(|f| f["text"].as_str()).collect();
    let mut fragments: Vec<_> = raw
        .into_iter()
        .map(|fragment| {
            let text = fragment["text"].as_str().unwrap_or("").to_owned();
            match fragment["type"].as_str().unwrap_or("text") {
                "emote" => {
                    if let Some(id) = fragment["emote"]["id"]
                        .as_str()
                        .filter(|id| crate::media::EmoteKey::twitch(id, false).is_some())
                    {
                        return Fragment::Emote {
                            provider: "twitch".into(),
                            id: id.into(),
                            label: text,
                            overlay: false,
                            asset: None,
                            animated: fragment["emote"]["format"]
                                .as_array()
                                .is_some_and(|formats| formats.iter().any(|f| f == "animated")),
                        };
                    }
                }
                "gif" => {
                    let gif = &fragment["gif"];
                    return Fragment::Gif {
                        id: gif["id"]
                            .as_str()
                            .or_else(|| gif["gif_id"].as_str())
                            .unwrap_or("")
                            .chars()
                            .take(128)
                            .collect(),
                        url: gif["url"]
                            .as_str()
                            .filter(|u| u.len() <= 4096)
                            .unwrap_or("")
                            .into(),
                        label: if text.is_empty() {
                            "[GIF]".into()
                        } else {
                            text
                        },
                    };
                }
                "cheermote" => {
                    let c = &fragment["cheermote"];
                    return Fragment::Cheer {
                        prefix: c["prefix"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(64)
                            .collect(),
                        bits: c["bits"].as_u64().unwrap_or(0),
                        tier: c["tier"].as_u64().unwrap_or(0),
                        label: if text.is_empty() {
                            format!("{} Bits", c["bits"].as_u64().unwrap_or(0))
                        } else {
                            text
                        },
                        asset: None,
                    };
                }
                "mention" => {
                    let mention = &fragment["mention"];
                    let id = mention["user_id"].as_str().filter(|id| {
                        !id.is_empty() && id.len() <= 255 && !id.chars().any(char::is_control)
                    });
                    let login = mention["user_login"]
                        .as_str()
                        .and_then(chat_core::twitch_login);
                    let name = mention["user_name"].as_str().map(str::trim).filter(|name| {
                        !name.is_empty() && name.len() <= 400 && !name.chars().any(char::is_control)
                    });
                    if let (Some(id), Some(login), Some(name)) = (id, login, name) {
                        return Fragment::Mention {
                            user_id: id.into(),
                            login,
                            display_name: name.into(),
                            label: text,
                        };
                    }
                }
                "text" => return Fragment::Text(text),
                kind => {
                    return Fragment::Unknown {
                        kind: kind.chars().take(64).collect(),
                        label: if text.is_empty() {
                            "[Unsupported content]".into()
                        } else {
                            text
                        },
                    };
                }
            }
            Fragment::Text(text)
        })
        .collect();
    if joined != original || fragments.is_empty() {
        // Keep rich content even if Twitch's fallback text differs from fragment text.
        fragments.retain(|f| matches!(f, Fragment::Gif { .. } | Fragment::Unknown { .. }));
        if !original.is_empty() {
            for fragment in &mut fragments {
                match fragment {
                    Fragment::Gif { label, .. } | Fragment::Unknown { label, .. } => label.clear(),
                    _ => {}
                }
            }
            fragments.insert(0, Fragment::Text(original.into()));
        }
    }
    fragments
}

fn parse_reply(value: &Value) -> Option<chat_core::Reply> {
    let id = value["parent_message_id"].as_str()?.to_owned();
    if id.is_empty() || id.len() > 128 {
        return None;
    }
    Some(chat_core::Reply {
        parent_id: id.clone(),
        parent_user_id: value["parent_user_id"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(128)
            .collect(),
        parent_name: value["parent_user_name"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Chatter")
            .chars()
            .take(100)
            .collect(),
        thread_id: value["thread_message_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .unwrap_or(&id)
            .to_owned(),
        parent_deleted: false,
    })
}
