//! Public FFZ definitions and ID-based grants. Cosmetics never confer permissions.
use crate::{media::EmoteKey, twitch_assets::Badge};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};

const WIRE_LIMIT: usize = 16 * 1024 * 1024;
const GRANT_LIMIT: usize = 1_000_000;
const ID_BYTES_LIMIT: usize = 16 * 1024 * 1024;
pub type Grants = BTreeMap<u64, Vec<String>>;

#[derive(Clone)]
pub struct Definition {
    pub badge: Badge,
    pub background: Option<u32>,
}
impl PartialEq for Definition {
    fn eq(&self, other: &Self) -> bool {
        self.background == other.background
            && self.badge.title == other.badge.title
            && self.badge.key == other.badge.key
    }
}
#[derive(Default, PartialEq)]
struct Snapshot {
    definitions: BTreeMap<u64, Definition>,
    grants: Grants,
}

/// Parse complete ID maps; reject oversized/malformed snapshots instead of
/// silently choosing a subset of supporters. Never consult username maps.
pub fn grants(value: &Value) -> Option<Grants> {
    if value.is_null() {
        return Some(Grants::new());
    }
    let object = value.as_object()?;
    if object.len() > 256 {
        return None;
    }
    let mut count = 0usize;
    let mut bytes = 0usize;
    let mut output = Grants::new();
    for (badge, users) in object {
        let id = badge.parse::<u64>().ok().filter(|id| *id > 0)?;
        let mut ids = Vec::new();
        for user in users.as_array()? {
            count += 1;
            if count > GRANT_LIMIT {
                return None;
            }
            let user = if let Some(value) = user.as_str() {
                if value.is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
                    return None;
                }
                value.to_owned()
            } else {
                user.as_u64()?.to_string()
            };
            bytes += user.len();
            if bytes > ID_BYTES_LIMIT {
                return None;
            }
            ids.push(user);
        }
        ids.sort_unstable();
        ids.dedup();
        if output.insert(id, ids).is_some() {
            return None;
        }
    }
    Some(output)
}

fn parse(value: &Value) -> Option<Snapshot> {
    let list = value["badges"].as_array()?;
    if list.len() > 256 || !value["users"].is_object() {
        return None;
    }
    let grants = grants(&value["users"])?;
    let mut definitions = BTreeMap::new();
    for item in list {
        let Some(id) = item["id"].as_u64().filter(|id| *id > 0) else {
            continue;
        };
        let title = item["title"]
            .as_str()
            .or_else(|| item["name"].as_str())
            .unwrap_or("Badge")
            .chars()
            .filter(|c| !c.is_control())
            .take(160)
            .collect::<String>();
        let key = ["2", "1", "4"].into_iter().find_map(|scale| {
            let raw = item["urls"][scale].as_str()?;
            let url = if raw.starts_with("//") {
                format!("https:{raw}")
            } else {
                raw.into()
            };
            EmoteKey::ffz_badge(id, &url)
        });
        let Some(key) = key else {
            continue;
        };
        // Only the simple documented color is supported; never evaluate CSS.
        let background = item["color"]
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .filter(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|s| u32::from_str_radix(s, 16).ok());
        definitions.insert(
            id,
            Definition {
                badge: Badge {
                    title: format!("FFZ · {title}"),
                    key,
                },
                background,
            },
        );
    }
    Some(Snapshot {
        definitions,
        grants,
    })
}

fn load(client: &reqwest::blocking::Client) -> Option<Snapshot> {
    let response = client
        .get("https://api.frankerfacez.com/v1/badges/ids")
        .send()
        .ok()?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|n| n > WIRE_LIMIT as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .take(WIRE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > WIRE_LIMIT {
        return None;
    }
    parse(&serde_json::from_slice::<Value>(&bytes).ok()?)
}

pub struct Badges {
    snapshot: Snapshot,
    last_good: Option<Instant>,
    next_attempt: Instant,
    in_flight: bool,
    failed: bool,
    tx: mpsc::SyncSender<()>,
    rx: mpsc::Receiver<Option<Snapshot>>,
}
impl Badges {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .ok();
            while jobs.recv().is_ok() {
                if results.send(client.as_ref().and_then(load)).is_err() {
                    break;
                }
            }
        });
        Self {
            snapshot: Snapshot::default(),
            last_good: None,
            next_attempt: Instant::now(),
            in_flight: false,
            failed: false,
            tx,
            rx,
        }
    }
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.rx.try_recv() {
            self.in_flight = false;
            self.failed = result.is_none();
            self.next_attempt =
                Instant::now() + Duration::from_secs(if self.failed { 60 } else { 900 });
            if let Some(snapshot) = result {
                changed |= self.snapshot != snapshot;
                self.snapshot = snapshot;
                self.last_good = Some(Instant::now());
            }
        }
        if self
            .last_good
            .is_some_and(|at| at.elapsed() >= Duration::from_secs(1800))
        {
            changed |= !self.snapshot.definitions.is_empty() || !self.snapshot.grants.is_empty();
            self.snapshot = Snapshot::default();
            self.last_good = None;
        }
        if !self.in_flight && Instant::now() >= self.next_attempt {
            match self.tx.try_send(()) {
                Ok(()) => {
                    self.in_flight = true;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.failed = true;
                    self.next_attempt = Instant::now() + Duration::from_secs(60);
                }
                Err(mpsc::TrySendError::Full(_)) => {}
            }
        }
        changed
    }
    pub fn retry(&mut self) {
        if !self.in_flight {
            self.next_attempt = Instant::now();
        }
    }
    pub fn for_user(&self, user: &str, channel: Option<&Grants>) -> Vec<Definition> {
        let mut seen = Vec::new();
        let mut result = Vec::new();
        for grants in std::iter::once(&self.snapshot.grants).chain(channel) {
            for (id, users) in grants {
                if seen.contains(id)
                    || users
                        .binary_search_by(|candidate| candidate.as_str().cmp(user))
                        .is_err()
                {
                    continue;
                }
                if let Some(badge) = self.snapshot.definitions.get(id) {
                    seen.push(*id);
                    result.push(badge.clone());
                    if result.len() == 8 {
                        return result;
                    }
                }
            }
        }
        result
    }
    pub fn inspection(&self) -> Value {
        serde_json::json!({"definitions":self.snapshot.definitions.len(),"grant_count":self.snapshot.grants.values().map(Vec::len).sum::<usize>(),"loading":self.in_flight,"unavailable":self.failed,"last_good_age_seconds":self.last_good.map(|at|at.elapsed().as_secs())})
    }
}
