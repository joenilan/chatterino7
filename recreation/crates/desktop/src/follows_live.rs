//! Followed channels that are live (Helix Get Followed Streams,
//! `user:read:follows`), refreshed every two minutes for the sidebar.
use crate::{auth::CLIENT_ID, live::Identity};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(120);
const MAX_PAGES: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct LiveFollow {
    pub login: String,
    pub name: String,
    pub game: String,
    pub title: String,
    pub viewers: u64,
}

pub struct FollowsLive {
    /// Live followed channels, most viewers first; None until loaded.
    pub list: Option<Vec<LiveFollow>>,
    user: Option<String>,
    last: Option<Instant>,
    pending: bool,
    tx: mpsc::SyncSender<Identity>,
    rx: mpsc::Receiver<(String, Option<Vec<LiveFollow>>)>,
}
impl FollowsLive {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Identity>(1);
        let (results, rx) = mpsc::sync_channel(1);
        let _ = std::thread::Builder::new()
            .name("jawjack-follows-live".into())
            .spawn(move || {
                let Ok(client) = crate::live::client() else {
                    return;
                };
                while let Ok(identity) = jobs.recv() {
                    let list = fetch(&client, &identity);
                    if results.send((identity.user_id.clone(), list)).is_err() {
                        break;
                    }
                }
            });
        Self {
            list: None,
            user: None,
            last: None,
            pending: false,
            tx,
            rx,
        }
    }
    /// Returns true when the list changed.
    pub fn pump(&mut self, identity: Option<&Identity>) -> bool {
        let user = identity.map(|i| i.user_id.clone());
        let mut changed = false;
        if user != self.user {
            self.user = user;
            self.last = None;
            changed = self.list.take().is_some();
        }
        while let Ok((user, list)) = self.rx.try_recv() {
            self.pending = false;
            if Some(&user) == self.user.as_ref()
                && let Some(list) = list
                && self.list.as_ref() != Some(&list)
            {
                self.list = Some(list);
                changed = true;
            }
        }
        if let Some(identity) = identity.filter(|i| i.has("user:read:follows"))
            && !self.pending
            && self.last.is_none_or(|t| t.elapsed() >= REFRESH)
            && self.tx.try_send(identity.clone()).is_ok()
        {
            self.pending = true;
            self.last = Some(Instant::now());
        }
        changed
    }
}

fn fetch(client: &reqwest::blocking::Client, identity: &Identity) -> Option<Vec<LiveFollow>> {
    let text = |v: &serde_json::Value, max: usize| {
        v.as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(max)
            .collect::<String>()
    };
    let mut list = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![("user_id", identity.user_id.as_str()), ("first", "100")];
        if let Some(after) = &cursor {
            query.push(("after", after.as_str()));
        }
        let response = client
            .get("https://api.twitch.tv/helix/streams/followed")
            .query(&query)
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .ok()?;
        let Ok((200, page)) = crate::live::response_json(response) else {
            return None;
        };
        for s in page["data"].as_array().into_iter().flatten() {
            let Some(login) = chat_core::twitch_login(&text(&s["user_login"], 64)) else {
                continue;
            };
            list.push(LiveFollow {
                name: text(&s["user_name"], 64),
                login,
                game: text(&s["game_name"], 80),
                title: text(&s["title"], 140),
                viewers: s["viewer_count"].as_u64().unwrap_or(0),
            });
        }
        cursor = page["pagination"]["cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    list.sort_by(|a, b| {
        b.viewers
            .cmp(&a.viewers)
            .then_with(|| a.login.cmp(&b.login))
    });
    Some(list)
}

/// "842", "12.4k", "1.2M".
pub fn viewers(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1_000.).replace(".0k", "k"),
        _ => format!("{:.1}M", n as f64 / 1_000_000.).replace(".0M", "M"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn viewer_counts() {
        assert_eq!(super::viewers(842), "842");
        assert_eq!(super::viewers(12_400), "12.4k");
        assert_eq!(super::viewers(3_000), "3k");
        assert_eq!(super::viewers(1_250_000), "1.2M");
    }
}
