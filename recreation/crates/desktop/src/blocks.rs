//! Your Twitch block list (Helix Get User Block List, `user:read:blocked_users`),
//! so blocked users' messages can be hidden as on twitch.tv. Refreshed every
//! hour, and right after /block or /unblock.
use crate::{auth::CLIENT_ID, live::Identity};
use std::{
    collections::BTreeSet,
    sync::mpsc,
    time::{Duration, Instant},
};

/// Hourly: block lists can be very large (bot-blocking tools), so stay light.
const REFRESH: Duration = Duration::from_secs(3600);
/// Up to 10,000 blocks (some streamers block large bot lists).
const MAX_PAGES: usize = 100;
pub const MAX_IDS: usize = MAX_PAGES * 100;

pub struct Blocks {
    /// Blocked user IDs, once loaded.
    pub ids: Option<BTreeSet<String>>,
    user: Option<String>,
    last: Option<Instant>,
    pending: bool,
    tx: mpsc::SyncSender<Identity>,
    rx: mpsc::Receiver<(String, Option<BTreeSet<String>>)>,
}
impl Blocks {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Identity>(1);
        let (results, rx) = mpsc::sync_channel(1);
        let _ = std::thread::Builder::new()
            .name("jawjack-blocks".into())
            .spawn(move || {
                let Ok(client) = crate::live::client() else {
                    return;
                };
                while let Ok(identity) = jobs.recv() {
                    let ids = fetch(&client, &identity);
                    if results.send((identity.user_id.clone(), ids)).is_err() {
                        break;
                    }
                }
            });
        Self {
            ids: None,
            user: None,
            last: None,
            pending: false,
            tx,
            rx,
        }
    }
    /// Fetch again soon (after /block or /unblock).
    pub fn refresh(&mut self) {
        self.last = None;
    }
    /// Returns true when the list changed.
    pub fn pump(&mut self, identity: Option<&Identity>) -> bool {
        let user = identity.map(|i| i.user_id.clone());
        let mut changed = false;
        if user != self.user {
            self.user = user;
            self.last = None;
            changed = self.ids.take().is_some();
        }
        while let Ok((user, ids)) = self.rx.try_recv() {
            self.pending = false;
            if Some(&user) == self.user.as_ref()
                && let Some(ids) = ids
                && self.ids.as_ref() != Some(&ids)
            {
                self.ids = Some(ids);
                changed = true;
            }
        }
        if let Some(identity) = identity.filter(|i| i.has("user:read:blocked_users"))
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

fn fetch(client: &reqwest::blocking::Client, identity: &Identity) -> Option<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            ("broadcaster_id", identity.user_id.as_str()),
            ("first", "100"),
        ];
        if let Some(after) = &cursor {
            query.push(("after", after.as_str()));
        }
        let response = client
            .get("https://api.twitch.tv/helix/users/blocks")
            .query(&query)
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .ok()?;
        let Ok((200, page)) = crate::live::response_json(response) else {
            return None;
        };
        ids.extend(
            page["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|b| b["user_id"].as_str().map(str::to_owned)),
        );
        cursor = page["pagination"]["cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Some(ids)
}
