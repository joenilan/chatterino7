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
/// A chatter's 7TV account, for the user card.
#[derive(Clone, Debug, PartialEq)]
pub struct Seven {
    /// The 7TV user ID (for 7tv.app/users/<id>).
    pub id: String,
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
    /// 7TV accounts for user cards: Some(None) means no 7TV account (or the
    /// lookup failed); absent means not asked yet; a pending entry is in `seven_asked`.
    seven: HashMap<String, Option<Seven>>,
    seven_asked: std::collections::HashSet<String>,
    seven_tx: mpsc::Sender<String>,
    seven_rx: mpsc::Receiver<(String, Option<Seven>)>,
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
        let (seven_tx, seven_jobs) = mpsc::channel::<String>();
        let (seven_results, seven_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok(id) = seven_jobs.recv() {
                let account = seven_account(&client, &id);
                if seven_results.send((id, account)).is_err() {
                    break;
                }
            }
        });
        Self {
            seven: HashMap::new(),
            seven_asked: Default::default(),
            seven_tx,
            seven_rx,
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
    /// A user's 7TV account, looked up once per session when first asked.
    pub fn seven(&mut self, id: &str) -> Option<Seven> {
        if let Some(known) = self.seven.get(id) {
            return known.clone();
        }
        if id.is_empty()
            || !id.bytes().all(|b| b.is_ascii_digit())
            || !self.seven_asked.insert(id.to_owned())
        {
            return None;
        }
        if self.seven.len() > 1024 {
            self.seven.clear();
            self.seven_asked.clear();
        }
        let _ = self.seven_tx.send(id.to_owned());
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
            // Public Twitch profiles are keyed by user ID and stay valid when
            // the *same* account renews its token. Clearing them here flashed
            // every avatar and interrupted the pending batch until restart.
            let changed_user = self.identity.as_ref().map(|i| i.user_id.as_str())
                != identity.as_ref().map(|i| i.user_id.as_str());
            self.identity = identity;
            self.epoch += 1;
            self.in_flight = false; // Retire any response from the prior epoch.
            self.pending.clear();
            if changed_user {
                self.requested.clear();
                self.values.clear();
                self.followers.clear();
            } else {
                // Old-epoch responses cannot be used. Resubmit the unresolved
                // profiles with the renewed token instead of waiting ten minutes.
                let retry: Vec<String> = self
                    .requested
                    .keys()
                    .filter(|id| !self.values.contains_key(*id))
                    .take(PENDING)
                    .cloned()
                    .collect();
                for id in retry {
                    self.requested.remove(&id);
                    self.pending.push_back(id);
                }
            }
            changed = true;
        }
        while let Ok((id, account)) = self.seven_rx.try_recv() {
            self.seven_asked.remove(&id);
            self.seven.insert(id, account);
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

/// 7TV's account for a Twitch user ID (public, no sign-in); None when they
/// have none or 7TV doesn't answer.
fn seven_account(client: &reqwest::blocking::Client, twitch_id: &str) -> Option<Seven> {
    let response = client
        .get(format!("https://7tv.io/v3/users/twitch/{twitch_id}"))
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut body = Vec::new();
    response.take(256 * 1024).read_to_end(&mut body).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&body).ok()?;
    parse_seven(&value)
}
fn parse_seven(value: &serde_json::Value) -> Option<Seven> {
    let user = &value["user"];
    let id = user["id"].as_str()?;
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let avatar = user["avatar_url"].as_str().and_then(|url| {
        // 7TV sometimes sends protocol-relative URLs.
        let url = if let Some(rest) = url.strip_prefix("//") {
            format!("https://{rest}")
        } else {
            url.to_owned()
        };
        crate::media::EmoteKey::seven_avatar(&url)
    });
    Some(Seven {
        id: id.to_owned(),
        avatar,
    })
}

#[cfg(test)]
mod seven_tests {
    use super::parse_seven;
    #[test]
    fn reads_the_7tv_account_and_avatar() {
        let value = serde_json::json!({"id":"123","user":{"id":"01ABC","avatar_url":"//cdn.7tv.app/user/01ABC/av_01X/3x.webp"}});
        let seven = parse_seven(&value).unwrap();
        assert_eq!(seven.id, "01ABC");
        assert!(seven.avatar.is_some());
        let none = serde_json::json!({"id":"123","user":{"id":"01ABC","avatar_url":"https://evil.example/x.webp"}});
        assert!(parse_seven(&none).unwrap().avatar.is_none());
        assert!(parse_seven(&serde_json::json!({"error":"not found"})).is_none());
    }
}

#[cfg(test)]
mod profile_cache_lifecycle_tests {
    use super::*;

    #[test]
    fn renewal_preserves_public_portraits_and_does_not_deadlock_lookup_queue() {
        use std::sync::Arc;
        let old = Identity {
            user_id: "123".into(),
            access: "old-token".into(),
            can_read_emotes: true,
            scopes: Arc::from(vec!["user:read:chat".to_owned()]),
        };
        let mut new = old.clone();
        new.access = "renewed-token".into();
        let mut profiles = Profiles::new();
        profiles.identity = Some(old);
        profiles.epoch = 5;
        profiles.in_flight = true;
        profiles.requested.insert("456".into(), Instant::now());
        profiles.values.insert(
            "456".into(),
            Profile {
                display_name: "Viewer".into(),
                created: String::new(),
                description: String::new(),
                role: String::new(),
                avatar: crate::media::EmoteKey::avatar(
                    "https://static-cdn.jtvnw.net/jtv_user_pictures/viewer-profile_image-70x70.png",
                ),
            },
        );
        assert!(profiles.pump(Some(new)));
        assert_eq!(profiles.epoch, 6);
        assert!(!profiles.in_flight);
        assert!(
            profiles
                .get("456")
                .and_then(|p| p.avatar.as_ref())
                .is_some()
        );
        assert!(profiles.requested.contains_key("456"));
    }

    #[test]
    fn token_renewal_reschedules_profile_requests_from_old_epoch() {
        use std::sync::Arc;
        let account = |access: &str| Identity {
            user_id: "123".into(),
            access: access.into(),
            can_read_emotes: false,
            scopes: Arc::from(Vec::<String>::new()),
        };
        let mut profiles = Profiles::new();
        profiles.identity = Some(account("old"));
        profiles.epoch = 1;
        profiles.in_flight = true;
        profiles.requested.insert("987".into(), Instant::now());
        // Close the synthetic sender, so this test cannot make any network
        // request. Failed enqueue must keep the user queued for later.
        let (tx, rx) = mpsc::sync_channel(1);
        drop(rx);
        profiles.tx = tx;
        assert!(profiles.pump(Some(account("new"))));
        assert_eq!(profiles.epoch, 2);
        assert!(!profiles.in_flight);
        assert!(profiles.pending.contains(&"987".to_owned()));
    }

    #[test]
    fn changing_accounts_retains_no_prior_account_public_profile_cache() {
        use std::sync::Arc;
        let identity = |user: &str| Identity {
            user_id: user.into(),
            access: String::new(),
            can_read_emotes: false,
            scopes: Arc::from(Vec::<String>::new()),
        };
        let mut profiles = Profiles::new();
        profiles.identity = Some(identity("123"));
        profiles.values.insert(
            "456".into(),
            Profile {
                display_name: "Viewer".into(),
                created: String::new(),
                description: String::new(),
                role: String::new(),
                avatar: None,
            },
        );
        assert!(profiles.pump(Some(identity("other"))));
        assert!(profiles.values.is_empty());
    }
}
