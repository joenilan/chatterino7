//! Twitch's chatter list, for @-completion in channels the account moderates or
//! owns (Get Chatters needs `moderator:read:chatters` and that role). Lists are
//! refreshed every two minutes for visible channels only, one request at a time.
use crate::{auth::CLIENT_ID, live::Identity};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(120);
const MAX_PAGES: usize = 5;

struct Job {
    identity: Identity,
    channel: String,
}
pub struct Chatters {
    lists: HashMap<String, (Rc<Vec<String>>, Instant)>,
    pending: bool,
    user: Option<String>,
    tx: mpsc::SyncSender<Job>,
    rx: mpsc::Receiver<(String, Option<Vec<String>>)>,
}
impl Chatters {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let Ok(client) = crate::live::client() else {
                return;
            };
            while let Ok(job) = jobs.recv() {
                let list = fetch(&client, &job.identity, &job.channel);
                if results.send((job.channel, list)).is_err() {
                    break;
                }
            }
        });
        Self {
            lists: HashMap::new(),
            pending: false,
            user: None,
            tx,
            rx,
        }
    }
    pub fn get(&self, channel: &str) -> Option<Rc<Vec<String>>> {
        self.lists.get(channel).map(|(list, _)| list.clone())
    }
    /// Refresh lists for `channels` (visible, moderated); returns channels that changed.
    pub fn pump(&mut self, identity: Option<&Identity>, channels: &[String]) -> Vec<String> {
        let user = identity.map(|i| i.user_id.clone());
        if user != self.user {
            self.user = user;
            self.lists.clear();
        }
        let mut changed = Vec::new();
        while let Ok((channel, list)) = self.rx.try_recv() {
            self.pending = false;
            let at = Instant::now();
            match list {
                Some(list) => {
                    self.lists.insert(channel.clone(), (Rc::new(list), at));
                    changed.push(channel);
                }
                // Keep any older list; retry on the next refresh.
                None => {
                    if let Some(entry) = self.lists.get_mut(&channel) {
                        entry.1 = at;
                    } else {
                        self.lists.insert(channel, (Rc::new(Vec::new()), at));
                    }
                }
            }
        }
        self.lists.retain(|c, _| channels.contains(c));
        let (Some(identity), false) = (identity, self.pending) else {
            return changed;
        };
        let due = channels
            .iter()
            .filter(|c| {
                self.lists
                    .get(*c)
                    .is_none_or(|(_, at)| at.elapsed() >= REFRESH)
            })
            .min_by_key(|c| self.lists.get(*c).map(|(_, at)| *at));
        if let Some(channel) = due
            && self
                .tx
                .try_send(Job {
                    identity: identity.clone(),
                    channel: channel.clone(),
                })
                .is_ok()
        {
            self.pending = true;
        }
        changed
    }
}

fn fetch(
    client: &reqwest::blocking::Client,
    identity: &Identity,
    channel: &str,
) -> Option<Vec<String>> {
    let get = |url: &str, query: &[(&str, &str)]| {
        let response = client
            .get(url)
            .query(query)
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .ok()?;
        match crate::live::response_json(response) {
            Ok((200, value)) => Some(value),
            _ => None,
        }
    };
    let users = get("https://api.twitch.tv/helix/users", &[("login", channel)])?;
    let broadcaster = users["data"][0]["id"].as_str()?.to_owned();
    let mut logins = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            ("broadcaster_id", broadcaster.as_str()),
            ("moderator_id", identity.user_id.as_str()),
            ("first", "1000"),
        ];
        if let Some(after) = &cursor {
            query.push(("after", after.as_str()));
        }
        let page = get("https://api.twitch.tv/helix/chat/chatters", &query)?;
        logins.extend(
            page["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["user_login"].as_str().map(str::to_ascii_lowercase)),
        );
        cursor = page["pagination"]["cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    logins.sort();
    logins.dedup();
    Some(logins)
}
