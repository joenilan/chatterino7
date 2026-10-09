//! Bounded Helix metadata, separate from the chat socket and anonymous image loader.
use crate::{auth::CLIENT_ID, live::Identity, media::EmoteKey};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Choice {
    pub label: String,
    pub provider: &'static str,
    pub key: EmoteKey,
}
#[derive(Clone)]
pub struct Badge {
    pub title: String,
    pub key: EmoteKey,
}
#[derive(Default)]
pub struct Assets {
    pub emotes: Vec<Choice>,
    pub badges: HashMap<(String, String), Badge>,
}
struct Job {
    epoch: u64,
    identity: Identity,
    channel: String,
    id: String,
}
pub struct TwitchAssets {
    identity: Option<Identity>,
    epoch: u64,
    pub global: Assets,
    pub channels: HashMap<String, Assets>,
    requested: HashMap<String, Instant>,
    tx: mpsc::SyncSender<Job>,
    rx: mpsc::Receiver<(u64, String, Option<Assets>)>,
}
impl TwitchAssets {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(32);
        let (results, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(12))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            while let Ok(job) = jobs.recv() {
                let data = load(&client, &job.identity, &job.id);
                if results.send((job.epoch, job.channel, data)).is_err() {
                    break;
                }
            }
        });
        Self {
            identity: None,
            epoch: 0,
            global: Assets::default(),
            channels: HashMap::new(),
            requested: HashMap::new(),
            tx,
            rx,
        }
    }
    pub fn pump(
        &mut self,
        identity: Option<Identity>,
        channels: &HashMap<String, (String, Instant)>,
    ) -> bool {
        let mut changed = false;
        if self.identity != identity {
            let new_user =
                self.identity.as_ref().map(|i| &i.user_id) != identity.as_ref().map(|i| &i.user_id);
            self.identity = identity;
            self.epoch += 1;
            self.requested.clear();
            if new_user {
                self.global = Assets::default();
                self.channels.clear();
                changed = true;
            }
        }
        self.channels.retain(|k, _| channels.contains_key(k));
        self.requested
            .retain(|k, _| k.is_empty() || channels.contains_key(k));
        if let Some(identity) = &self.identity {
            for (channel, id) in std::iter::once(("", "")).chain(
                channels
                    .iter()
                    .map(|(k, (id, _))| (k.as_str(), id.as_str())),
            ) {
                if self
                    .requested
                    .get(channel)
                    .is_some_and(|at| at.elapsed() < Duration::from_secs(600))
                {
                    continue;
                }
                if self
                    .tx
                    .try_send(Job {
                        epoch: self.epoch,
                        identity: identity.clone(),
                        channel: channel.into(),
                        id: id.into(),
                    })
                    .is_ok()
                {
                    self.requested.insert(channel.into(), Instant::now());
                }
            }
        }
        while let Ok((epoch, channel, data)) = self.rx.try_recv() {
            if epoch != self.epoch {
                continue;
            }
            if let Some(data) = data {
                if channel.is_empty() {
                    self.global = data;
                    changed = true;
                } else if channels.contains_key(&channel) {
                    self.channels.insert(channel, data);
                    changed = true;
                }
            } else {
                self.requested
                    .insert(channel, Instant::now() - Duration::from_secs(540));
            }
        }
        changed
    }
    pub fn badge(&self, channel: &str, set: &str, id: &str) -> Option<&Badge> {
        let k = (set.to_owned(), id.to_owned());
        self.channels
            .get(channel)
            .and_then(|a| a.badges.get(&k))
            .or_else(|| self.global.badges.get(&k))
    }
}
fn get(client: &reqwest::blocking::Client, identity: &Identity, path: &str) -> Option<Value> {
    let response = client
        .get(format!("https://api.twitch.tv/helix/{path}"))
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(&identity.access)
        .send()
        .ok()?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|n| n > 4 * 1024 * 1024)
    {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
fn load(client: &reqwest::blocking::Client, identity: &Identity, id: &str) -> Option<Assets> {
    let suffix = if id.is_empty() {
        "/global".into()
    } else {
        format!("?broadcaster_id={id}")
    };
    let badges = get(client, identity, &format!("chat/badges{suffix}"))?;
    let mut result = Assets::default();
    for set in badges["data"].as_array().into_iter().flatten().take(1024) {
        let Some(set_id) = set["set_id"].as_str().filter(|s| s.len() <= 128) else {
            continue;
        };
        for v in set["versions"].as_array().into_iter().flatten().take(512) {
            let Some(id) = v["id"].as_str().filter(|s| s.len() <= 128) else {
                continue;
            };
            let Some(key) = v["image_url_2x"].as_str().and_then(EmoteKey::badge) else {
                continue;
            };
            let title = v["title"]
                .as_str()
                .unwrap_or(set_id)
                .chars()
                .take(160)
                .collect();
            result
                .badges
                .insert((set_id.into(), id.into()), Badge { title, key });
        }
    }
    // Only Twitch global emotes are universally available. Channel subscriber emotes
    // need ownership information before inclusion; never imply an entitlement.
    if id.is_empty() {
        if let Some(emotes) = get(client, identity, "chat/emotes/global") {
            for v in emotes["data"].as_array().into_iter().flatten().take(2000) {
                let Some(label) = v["name"].as_str().filter(|s| {
                    !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_whitespace)
                }) else {
                    continue;
                };
                let animated = v["format"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v == "animated"));
                if let Some(key) = v["id"]
                    .as_str()
                    .and_then(|id| EmoteKey::twitch(id, animated))
                {
                    result.emotes.push(Choice {
                        label: label.into(),
                        provider: "Twitch",
                        key,
                    });
                }
            }
        }
    }
    Some(result)
}
