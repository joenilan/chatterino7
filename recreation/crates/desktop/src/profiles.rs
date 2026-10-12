//! On-demand public profile metadata, bounded and independent of the chat socket.
use crate::{auth::CLIENT_ID, live::Identity};
use std::{
    collections::HashMap,
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Profile {
    pub display_name: String,
    pub created: String,
    pub description: String,
    pub role: String,
    pub avatar: Option<crate::media::EmoteKey>,
}
/// Helix accepts up to 100 `id` parameters per users request.
const BATCH: usize = 100;
/// Pending lookups are bounded; the oldest interest is dropped first.
const PENDING: usize = 512;
/// Resolved or failed lookups are not repeated within this window.
const REFRESH: Duration = Duration::from_secs(600);
/// Failures retry sooner than successes refresh.
const RETRY: Duration = Duration::from_secs(60);
struct Job {
    epoch: u64,
    identity: Identity,
    ids: Vec<String>,
}
/// One batch result; `None` means the request itself failed.
type Batch = (u64, Vec<String>, Option<HashMap<String, Profile>>);
pub struct Profiles {
    identity: Option<Identity>,
    epoch: u64,
    requested: HashMap<String, Instant>,
    pending: std::collections::VecDeque<String>,
    in_flight: bool,
    values: HashMap<String, Profile>,
    tx: mpsc::SyncSender<Job>,
    rx: mpsc::Receiver<Batch>,
    /// Follower totals for user cards: user ID → Some(total) once known,
    /// None while loading or unavailable.
    followers: HashMap<String, Option<u64>>,
    follower_tx: mpsc::Sender<(u64, Identity, String)>,
    follower_rx: mpsc::Receiver<(u64, String, Option<u64>)>,
}
impl Profiles {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok(job) = jobs.recv() {
                let value = load(&client, &job.identity, &job.ids);
                if results.send((job.epoch, job.ids, value)).is_err() {
                    break;
                }
            }
        });
        let (follower_tx, follower_jobs) = mpsc::channel::<(u64, Identity, String)>();
        let (follower_results, follower_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok((epoch, identity, id)) = follower_jobs.recv() {
                let total = follower_total(&client, &identity, &id);
                if follower_results.send((epoch, id, total)).is_err() {
                    break;
                }
            }
        });
        Self {
            followers: HashMap::new(),
            follower_tx,
            follower_rx,
            identity: None,
            epoch: 0,
            requested: HashMap::new(),
            pending: Default::default(),
            in_flight: false,
            values: HashMap::new(),
            tx,
            rx,
        }
    }
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.values.get(id)
    }
    /// A user's follower count, looked up once per session when first asked.
    pub fn followers(&mut self, id: &str) -> Option<u64> {
        if let Some(known) = self.followers.get(id) {
            return *known;
        }
        let identity = self.identity.clone()?;
        if self.followers.len() > 1024 {
            self.followers.clear();
        }
        self.followers.insert(id.to_owned(), None);
        let _ = self.follower_tx.send((self.epoch, identity, id.to_owned()));
        None
    }
    /// Record interest; `pump` sends queued IDs in batches.
    pub fn request(&mut self, id: &str) {
        if id.is_empty()
            || id.len() > 255
            || id.chars().any(char::is_control)
            || self.identity.is_none()
            || self
                .requested
                .get(id)
                .is_some_and(|at| at.elapsed() < REFRESH)
            || self.pending.iter().any(|p| p == id)
        {
            return;
        }
        if self.pending.len() >= PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(id.into());
    }
    pub fn pump(&mut self, identity: Option<Identity>) -> bool {
        let mut changed = false;
        if self.identity != identity {
            self.identity = identity;
            self.epoch += 1;
            self.requested.clear();
            self.pending.clear();
            self.values.clear();
            self.followers.clear();
            changed = true;
        }
        while let Ok((epoch, id, total)) = self.follower_rx.try_recv() {
            if epoch == self.epoch && total.is_some() {
                self.followers.insert(id, total);
                changed = true;
            }
        }
        while let Ok((epoch, ids, value)) = self.rx.try_recv() {
            if epoch != self.epoch {
                continue;
            }
            self.in_flight = false;
            let failed = value.is_none();
            let mut found = value.unwrap_or_default();
            for id in ids {
                match found.remove(&id) {
                    Some(profile) => {
                        self.values.insert(id, profile);
                        changed = true;
                    }
                    // Unknown or failed IDs retry after RETRY instead of REFRESH.
                    None if failed => {
                        self.requested
                            .insert(id, Instant::now() - (REFRESH - RETRY));
                    }
                    None => {}
                }
            }
        }
        if let Some(identity) = &self.identity
            && !self.in_flight
            && !self.pending.is_empty()
        {
            let take = self.pending.len().min(BATCH);
            let ids: Vec<String> = self.pending.drain(..take).collect();
            if self.requested.len() + ids.len() > 1024 {
                // Bound memory: forget the oldest lookups and their values.
                let mut by_age: Vec<_> = self
                    .requested
                    .iter()
                    .map(|(k, at)| (*at, k.clone()))
                    .collect();
                by_age.sort();
                for (_, old) in by_age.into_iter().take(ids.len()) {
                    self.requested.remove(&old);
                    self.values.remove(&old);
                }
            }
            let now = Instant::now();
            for id in &ids {
                self.requested.insert(id.clone(), now);
            }
            let job = Job {
                epoch: self.epoch,
                identity: identity.clone(),
                ids,
            };
            match self.tx.try_send(job) {
                Ok(()) => self.in_flight = true,
                Err(mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job)) => {
                    for id in job.ids.into_iter().rev() {
                        self.requested.remove(&id);
                        self.pending.push_front(id);
                    }
                }
            }
        }
        changed
    }
}
fn load(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    ids: &[String],
) -> Option<HashMap<String, Profile>> {
    let query: Vec<(&str, &str)> = ids.iter().map(|id| ("id", id.as_str())).collect();
    let response = client
        .get("https://api.twitch.tv/helix/users")
        .query(&query)
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(&identity.access)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = vec![];
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 * 1024 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let mut found = HashMap::new();
    for user in value["data"].as_array()? {
        // Only accept identities that were actually requested.
        let Some(id) = user["id"].as_str().filter(|id| ids.iter().any(|r| r == id)) else {
            continue;
        };
        found.insert(id.to_owned(), parse(user));
    }
    Some(found)
}
/// Helix gives anyone the follower total (the list itself needs permission).
fn follower_total(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    id: &str,
) -> Option<u64> {
    let response = client
        .get("https://api.twitch.tv/helix/channels/followers")
        .query(&[("broadcaster_id", id), ("first", "1")])
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(&identity.access)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = vec![];
    response.take(64 * 1024).read_to_end(&mut bytes).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value["total"].as_u64()
}
fn parse(user: &serde_json::Value) -> Profile {
    Profile {
        display_name: user["display_name"]
            .as_str()
            .unwrap_or("")
            .trim()
            .chars()
            .take(80)
            .collect(),
        created: user["created_at"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(40)
            .collect(),
        description: user["description"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(500)
            .collect(),
        role: user["broadcaster_type"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(40)
            .collect(),
        avatar: user["profile_image_url"]
            .as_str()
            .and_then(crate::media::EmoteKey::avatar),
    }
}
