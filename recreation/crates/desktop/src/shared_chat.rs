//! Twitch Shared Chat sessions: which channels share a chat with an open one.
//! Looked up with Helix `GET /shared_chat/session` when a shared message shows up
//! (or while a session is known), so tabs can show "+N" and who it's shared with.
use crate::{auth::CLIENT_ID, live::Identity};
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};

/// While a session is known, refresh it this often (people join and leave).
const REFRESH: Duration = Duration::from_secs(180);
/// A channel without a session is looked up again at most this often.
const RECHECK: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// Broadcaster IDs of every channel in the session, host first.
    pub participants: Vec<String>,
}

pub struct SharedChat {
    sessions: HashMap<String, Session>,
    checked: HashMap<String, Instant>,
    tx: mpsc::Sender<(String, Option<Session>)>,
    rx: mpsc::Receiver<(String, Option<Session>)>,
}
impl SharedChat {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            sessions: HashMap::new(),
            checked: HashMap::new(),
            tx,
            rx,
        }
    }
    pub fn get(&self, channel: &str) -> Option<&Session> {
        self.sessions.get(channel)
    }
    /// Look the channel's session up if it's due. `seen_shared`: a message from
    /// another channel just arrived here, so a session probably exists.
    pub fn want(
        &mut self,
        channel: &str,
        broadcaster_id: &str,
        identity: &Identity,
        seen_shared: bool,
    ) {
        let known = self.sessions.contains_key(channel);
        if !known && !seen_shared {
            return;
        }
        let every = if known { REFRESH } else { RECHECK };
        if self
            .checked
            .get(channel)
            .is_some_and(|t| t.elapsed() < every)
        {
            return;
        }
        self.checked.insert(channel.to_owned(), Instant::now());
        let (tx, channel, id, identity) = (
            self.tx.clone(),
            channel.to_owned(),
            broadcaster_id.to_owned(),
            identity.clone(),
        );
        let _ = std::thread::Builder::new()
            .name("jawjack-shared-chat".into())
            .spawn(move || {
                let _ = tx.send((channel, fetch(&id, &identity)));
            });
    }
    /// Apply finished lookups; true when something changed.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        while let Ok((channel, session)) = self.rx.try_recv() {
            match session {
                Some(session) if session.participants.len() > 1 => {
                    changed |= self.sessions.get(&channel) != Some(&session);
                    self.sessions.insert(channel, session);
                }
                // No session (or a lookup failure that says so): drop it.
                _ => changed |= self.sessions.remove(&channel).is_some(),
            }
        }
        changed
    }
}

fn fetch(broadcaster_id: &str, identity: &Identity) -> Option<Session> {
    let client = crate::live::client().ok()?;
    let response = client
        .get("https://api.twitch.tv/helix/shared_chat/session")
        .query(&[("broadcaster_id", broadcaster_id)])
        .header("Client-Id", CLIENT_ID)
        .bearer_auth(identity.access.as_str())
        .send()
        .ok()?;
    let (status, value) = crate::live::response_json(response).ok()?;
    if status != 200 {
        return None;
    }
    let session = &value["data"][0];
    let host = session["host_broadcaster_id"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let mut participants: Vec<String> = session["participants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["broadcaster_id"].as_str())
        .filter(|id| !id.is_empty() && id.len() <= 64)
        .map(str::to_owned)
        .collect();
    // Host first, so "+N" and the tooltip read in the session's order.
    if let Some(at) = participants.iter().position(|p| *p == host) {
        let host = participants.remove(at);
        participants.insert(0, host);
    }
    Some(Session { participants })
}
