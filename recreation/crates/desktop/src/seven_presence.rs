//! 7TV activity, as Chatterino7 sends it: after you chat in a channel, tell
//! 7TV you're present there so other 7TV users see your paint, badge and personal
//! emotes. At most once a minute per channel; nothing is sent while turned off.
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_secs(60);

struct Job {
    user: String,
    channel: String,
    /// With a session: a passive presence that asks 7TV to send this session the
    /// cosmetics of people present in the channel, without announcing the user.
    session: Option<String>,
}
pub struct Presence {
    pub enabled: bool,
    next: HashMap<String, Instant>,
    joined: HashMap<(String, String), Instant>,
    tx: mpsc::Sender<Job>,
    /// Outcome of the latest request, for control inspection.
    pub last: Arc<Mutex<String>>,
}
impl Presence {
    pub fn new(enabled: bool) -> Self {
        let (tx, jobs) = mpsc::channel::<Job>();
        let last = Arc::new(Mutex::new("Nothing sent yet".to_owned()));
        let report = last.clone();
        std::thread::spawn(move || {
            let Ok(client) = crate::live::client() else {
                return;
            };
            // Twitch user ID → 7TV user ID; `None` means no 7TV account.
            let mut accounts: HashMap<String, Option<String>> = HashMap::new();
            while let Ok(job) = jobs.recv() {
                let account = accounts
                    .entry(job.user.clone())
                    .or_insert_with(|| lookup(&client, &job.user))
                    .clone();
                let outcome = if let Some(id) = account {
                    client
                        .post(format!("https://7tv.io/v3/users/{id}/presences"))
                        .header("Content-Type", "application/json")
                        .body(
                            match &job.session {
                                Some(session) => json!({"kind": 1, "passive": true, "session_id": session, "data": {"id": job.channel, "platform": "TWITCH", "ctx": "channel"}}),
                                None => json!({"kind": 1, "data": {"id": job.channel, "platform": "TWITCH"}}),
                            }
                            .to_string(),
                        )
                        .send()
                        .map(|r| {
                            format!("HTTP {} for channel {}", r.status().as_u16(), job.channel)
                        })
                        .unwrap_or_else(|_| "7TV unreachable".into())
                } else {
                    "No 7TV account for this Twitch user".into()
                };
                if let Ok(mut last) = report.lock() {
                    *last = outcome;
                }
            }
        });
        Self {
            enabled,
            next: HashMap::new(),
            joined: HashMap::new(),
            tx,
            last,
        }
    }
    /// Call after a message was sent successfully as `user` in `channel` (Twitch IDs).
    pub fn activity(&mut self, user: &str, channel: &str) {
        let valid =
            |id: &str| !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit());
        if !self.enabled || !valid(user) || !valid(channel) {
            return;
        }
        let now = Instant::now();
        if self.next.get(channel).is_some_and(|at| now < *at) {
            return;
        }
        self.next.insert(channel.to_owned(), now + INTERVAL);
        let _ = self.tx.send(Job {
            user: user.to_owned(),
            channel: channel.to_owned(),
            session: None,
        });
    }
    /// On opening a channel (and every 5 minutes while open): passive presence for
    /// this 7TV session, so paints and badges of people already chatting show at once.
    pub fn join(&mut self, user: &str, channel: &str, session: &str) {
        let valid =
            |id: &str| !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit());
        if !self.enabled || !valid(user) || !valid(channel) {
            return;
        }
        let key = (session.to_owned(), channel.to_owned());
        if self
            .joined
            .get(&key)
            .is_some_and(|at| at.elapsed() < Duration::from_secs(300))
        {
            return;
        }
        if self.joined.len() > 512 {
            self.joined.retain(|(s, _), _| s == session);
        }
        self.joined.insert(key, Instant::now());
        let _ = self.tx.send(Job {
            user: user.to_owned(),
            channel: channel.to_owned(),
            session: Some(session.to_owned()),
        });
    }
}
fn lookup(client: &reqwest::blocking::Client, twitch_id: &str) -> Option<String> {
    let response = client
        .get(format!("https://7tv.io/v3/users/twitch/{twitch_id}"))
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    // The user object embeds whole emote sets, so it can be large.
    use std::io::Read;
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value["user"]["id"]
        .as_str()
        .map(str::to_owned)
        .filter(|id| id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric()))
}
