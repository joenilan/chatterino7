//! Direct 7TV cosmetic lookups for chatters, so paints and badges show on a
//! person's first message instead of only after 7TV broadcasts their presence.
//! Batched (up to 25 users per request, one request a second), cached for 30
//! minutes, and fed through the same changes as the 7TV event stream.
use crate::seven_entitlements::Change;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::mpsc,
    time::{Duration, Instant},
};

const BATCH: usize = 25;
const PACE: Duration = Duration::from_secs(1);
const FRESH: Duration = Duration::from_secs(30 * 60);
const QUEUE: usize = 2000;

pub struct Lookup {
    seen: HashMap<String, Instant>,
    queue: VecDeque<String>,
    pending: bool,
    next: Instant,
    tx: mpsc::Sender<Vec<String>>,
    rx: mpsc::Receiver<Vec<Change>>,
}
impl Lookup {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::channel::<Vec<String>>();
        let (results, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(client) = crate::live::client() else {
                return;
            };
            // Definitions this worker already delivered, so only new ones are fetched.
            let mut defined: HashSet<String> = HashSet::new();
            while let Ok(users) = jobs.recv() {
                // An empty job means the event stream reset: definitions are gone too.
                if users.is_empty() {
                    defined.clear();
                    continue;
                }
                let changes = lookup(&client, &users, &mut defined);
                if results.send(changes).is_err() {
                    break;
                }
            }
        });
        Self {
            seen: HashMap::new(),
            queue: VecDeque::new(),
            pending: false,
            next: Instant::now(),
            tx,
            rx,
        }
    }
    /// Queue a Twitch user ID seen in chat (ignored if checked recently).
    pub fn want(&mut self, user: &str) {
        if user.is_empty() || user.len() > 32 || !user.bytes().all(|b| b.is_ascii_digit()) {
            return;
        }
        if self.seen.get(user).is_some_and(|at| at.elapsed() < FRESH) || self.queue.len() >= QUEUE {
            return;
        }
        self.seen.insert(user.to_owned(), Instant::now());
        self.queue.push_back(user.to_owned());
    }
    /// After the event stream reset, cosmetics must be looked up again.
    pub fn reset(&mut self) {
        self.seen.clear();
        let _ = self.tx.send(Vec::new());
    }
    pub fn pump(&mut self) -> Vec<Change> {
        let mut out = Vec::new();
        while let Ok(changes) = self.rx.try_recv() {
            self.pending = false;
            out.extend(changes);
        }
        if !self.pending && !self.queue.is_empty() && Instant::now() >= self.next {
            let batch: Vec<String> = self.queue.drain(..self.queue.len().min(BATCH)).collect();
            if self.tx.send(batch).is_ok() {
                self.pending = true;
                self.next = Instant::now() + PACE;
            }
        }
        if self.seen.len() > 20_000 {
            self.seen.retain(|_, at| at.elapsed() < FRESH);
        }
        out
    }
}

fn gql(client: &reqwest::blocking::Client, query: String) -> Option<Value> {
    use std::io::Read;
    let response = client
        .post("https://7tv.io/v3/gql")
        .header("Content-Type", "application/json")
        .body(json!({ "query": query }).to_string())
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn valid(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn lookup(
    client: &reqwest::blocking::Client,
    users: &[String],
    defined: &mut HashSet<String>,
) -> Vec<Change> {
    let query = users
        .iter()
        .enumerate()
        .map(|(i, id)| format!("u{i}: userByConnection(platform: TWITCH, id: \"{id}\") {{ style {{ paint_id badge_id }} }}"))
        .collect::<Vec<_>>()
        .join(" ");
    let Some(value) = gql(client, format!("{{ {query} }}")) else {
        return Vec::new();
    };
    let mut paints: HashMap<String, Vec<String>> = HashMap::new();
    let mut badges: HashMap<String, Vec<String>> = HashMap::new();
    for (i, user) in users.iter().enumerate() {
        let style = &value["data"][format!("u{i}")]["style"];
        if let Some(paint) = style["paint_id"].as_str().filter(|s| valid(s)) {
            paints
                .entry(paint.to_owned())
                .or_default()
                .push(user.clone());
        }
        if let Some(badge) = style["badge_id"].as_str().filter(|s| valid(s)) {
            badges
                .entry(badge.to_owned())
                .or_default()
                .push(user.clone());
        }
    }
    let mut changes = Vec::new();
    let unknown: Vec<&String> = paints
        .keys()
        .chain(badges.keys())
        .filter(|id| !defined.contains(*id))
        .collect();
    if !unknown.is_empty() {
        let list = unknown
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "{{ cosmetics(list: [{list}]) {{ paints {{ id name function color angle shape image_url repeat stops {{ at color }} shadows {{ x_offset y_offset radius color }} }} badges {{ id name tooltip host {{ url files {{ name static_name width height format size }} }} }} }} }}"
        );
        if let Some(value) = gql(client, query) {
            let cosmetics = &value["data"]["cosmetics"];
            for paint in cosmetics["paints"].as_array().into_iter().flatten() {
                if let Some(paint) = crate::seven_paints::definition(paint) {
                    defined.insert(paint.id.clone());
                    changes.push(Change::PaintDefinition(paint));
                }
            }
            for badge in cosmetics["badges"].as_array().into_iter().flatten() {
                if let Some((id, badge)) = crate::seven_badges::definition(badge) {
                    defined.insert(id.clone());
                    changes.push(Change::BadgeDefinition { id, badge });
                }
            }
        }
    }
    for (paint, users) in paints {
        changes.push(Change::PaintGrant {
            users,
            paint,
            remove: false,
        });
    }
    for (badge, users) in badges {
        changes.push(Change::BadgeGrant {
            users,
            badge,
            remove: false,
        });
    }
    changes
}
