//! Chatterino's own badges (Supporter, Contributor, Developer…), from its
//! public list at api.chatterino.com, by Twitch user ID. Cosmetic only.
use crate::{media::EmoteKey, twitch_assets::Badge};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};

const URL: &str = "https://api.chatterino.com/badges";
const WIRE_LIMIT: u64 = 4 * 1024 * 1024;
const USER_LIMIT: usize = 200_000;

/// User ID → their Chatterino badges.
type Grants = HashMap<String, Vec<Badge>>;

fn parse(value: &Value) -> Option<Grants> {
    let mut grants = Grants::new();
    let mut users = 0usize;
    for item in value["badges"].as_array()?.iter().take(64) {
        let title: String = item["tooltip"]
            .as_str()
            .unwrap_or("Chatterino badge")
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect();
        // The 2x image suits an 18 px badge on high-DPI screens.
        let Some(key) = ["image2", "image1", "image3"]
            .into_iter()
            .find_map(|size| EmoteKey::chatterino_badge(item[size].as_str()?))
        else {
            continue;
        };
        for user in item["users"].as_array().into_iter().flatten() {
            let Some(id) = user.as_str().filter(|id| {
                !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit())
            }) else {
                continue;
            };
            users += 1;
            if users > USER_LIMIT {
                return None;
            }
            grants.entry(id.to_owned()).or_default().push(Badge {
                title: title.clone(),
                key: key.clone(),
            });
        }
    }
    Some(grants)
}

fn load(client: &reqwest::blocking::Client) -> Option<Grants> {
    let response = client.get(URL).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = Vec::new();
    response.take(WIRE_LIMIT + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > WIRE_LIMIT {
        return None;
    }
    parse(&serde_json::from_slice(&bytes).ok()?)
}

pub struct Badges {
    grants: Grants,
    next_attempt: Instant,
    in_flight: bool,
    tx: mpsc::SyncSender<()>,
    rx: mpsc::Receiver<Option<Grants>>,
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
            grants: Grants::new(),
            next_attempt: Instant::now(),
            in_flight: false,
            tx,
            rx,
        }
    }
    /// Refresh every 30 minutes (5 after a failure); true when the list changed.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.rx.try_recv() {
            self.in_flight = false;
            self.next_attempt =
                Instant::now() + Duration::from_secs(if result.is_some() { 1800 } else { 300 });
            if let Some(grants) = result {
                changed = grants.len() != self.grants.len();
                self.grants = grants;
            }
        }
        if !self.in_flight && Instant::now() >= self.next_attempt && self.tx.try_send(()).is_ok() {
            self.in_flight = true;
        }
        changed
    }
    pub fn users(&self) -> usize {
        self.grants.len()
    }
    pub fn for_user(&self, user_id: &str) -> &[Badge] {
        self.grants.get(user_id).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn badges_by_user_id() {
        let value = serde_json::json!({"badges":[
            {"tooltip":"Chatterino Supporter","image1":"https://fourtf.com/chatterino/badges/supporter.png","image2":"https://fourtf.com/chatterino/badges/supporter2x.png","users":["123","bad id"]},
            {"tooltip":"Elsewhere","image1":"https://example.com/x.png","users":["123"]}
        ]});
        let grants = super::parse(&value).unwrap();
        assert_eq!(grants["123"].len(), 1, "foreign image hosts are skipped");
        assert_eq!(grants["123"][0].title, "Chatterino Supporter");
        assert!(!grants.contains_key("bad id"));
    }
}
