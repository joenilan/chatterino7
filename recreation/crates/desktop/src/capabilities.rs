//! What the signed-in account may do in each channel.
//!
//! Authority needs both the channel role (broadcaster, or moderator from Twitch's
//! moderated-channel list) and the granted scope. Tools check here when they are
//! shown and again immediately before they write.
use crate::{auth::CLIENT_ID, live::Identity};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::mpsc,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(600);
const RETRY: Duration = Duration::from_secs(60);
const MAX_PAGES: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Broadcaster,
    Moderator,
    Viewer,
}
impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Broadcaster => "Broadcaster",
            Role::Moderator => "Moderator",
            Role::Viewer => "Viewer",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    // Moderator or broadcaster in the target channel.
    Ban,
    Warn,
    DeleteMessages,
    ManageRoom,
    Announce,
    Shoutout,
    // Broadcaster of the target channel.
    ManageModerators,
    ManageVips,
    // Always the signed-in account's own channel.
    EditStreamInfo,
    Marker,
    Commercial,
    Raid,
    // Account-level, any channel.
    ChatColor,
    Whisper,
    Block,
    /// Create a clip of a live channel (anyone signed in).
    Clip,
}
enum Needs {
    Moderator,
    Broadcaster,
    OwnChannel,
    Account,
}
impl Action {
    fn scopes(self) -> &'static [&'static str] {
        use Action::*;
        match self {
            Ban => &["moderator:manage:banned_users"],
            Warn => &["moderator:manage:warnings"],
            DeleteMessages => &["moderator:manage:chat_messages"],
            ManageRoom => &["moderator:manage:chat_settings"],
            Announce => &["moderator:manage:announcements"],
            Shoutout => &["moderator:manage:shoutouts"],
            ManageModerators => &["channel:manage:moderators"],
            ManageVips => &["channel:manage:vips"],
            EditStreamInfo | Marker => &["channel:manage:broadcast"],
            Commercial => &["channel:edit:commercial"],
            Raid => &["channel:manage:raids"],
            ChatColor => &["user:manage:chat_color"],
            Whisper => &["user:manage:whispers"],
            Block => &["user:manage:blocked_users"],
            Clip => &["clips:edit"],
        }
    }
    fn needs(self) -> Needs {
        use Action::*;
        match self {
            Ban | Warn | DeleteMessages | ManageRoom | Announce | Shoutout => Needs::Moderator,
            ManageModerators | ManageVips => Needs::Broadcaster,
            EditStreamInfo | Marker | Commercial | Raid => Needs::OwnChannel,
            ChatColor | Whisper | Block | Clip => Needs::Account,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Check {
    Available,
    SignedOut,
    /// The role is not known yet (moderated channels still loading).
    Checking,
    NeedsRole(&'static str),
    NeedsPermission,
}
impl Check {
    pub fn available(&self) -> bool {
        *self == Check::Available
    }
    pub fn reason(&self) -> &'static str {
        match self {
            Check::Available => "Available",
            Check::SignedOut => "Sign in with Twitch first.",
            Check::Checking => "Checking your role in this channel…",
            Check::NeedsRole(why) => why,
            Check::NeedsPermission => {
                "Twitch has not granted this permission yet. Update Twitch permissions from the account menu."
            }
        }
    }
}

struct Job {
    epoch: u64,
    identity: Identity,
}
pub struct Capabilities {
    identity: Option<Identity>,
    login: Option<String>,
    /// Broadcaster logins where the account is a moderator; `None` until known.
    moderated: Option<HashSet<String>>,
    status: String,
    epoch: u64,
    pending: bool,
    next: Instant,
    tx: mpsc::SyncSender<Job>,
    rx: mpsc::Receiver<(u64, Result<HashSet<String>, String>)>,
}
impl Capabilities {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let Ok(client) = crate::live::client() else {
                return;
            };
            while let Ok(job) = jobs.recv() {
                let value = moderated_channels(&client, &job.identity);
                if results.send((job.epoch, value)).is_err() {
                    break;
                }
            }
        });
        Self {
            identity: None,
            login: None,
            moderated: None,
            status: "Signed out".into(),
            epoch: 0,
            pending: false,
            next: Instant::now(),
            tx,
            rx,
        }
    }
    /// Track the account and refresh moderated channels periodically.
    pub fn pump(&mut self, identity: Option<Identity>, login: Option<String>) -> bool {
        let mut changed = false;
        let user = |i: &Option<Identity>| i.as_ref().map(|i| i.user_id.clone());
        let scope_changed = self.identity.as_ref().map(|i| i.scopes.clone())
            != identity.as_ref().map(|i| i.scopes.clone());
        if user(&self.identity) != user(&identity) || scope_changed {
            self.epoch += 1;
            self.moderated = None;
            self.pending = false;
            self.next = Instant::now();
            changed = true;
        }
        self.identity = identity;
        self.login = login;
        while let Ok((epoch, result)) = self.rx.try_recv() {
            if epoch != self.epoch {
                continue;
            }
            self.pending = false;
            match result {
                Ok(channels) => {
                    self.status = format!("Moderator in {} channels", channels.len());
                    self.moderated = Some(channels);
                    self.next = Instant::now() + REFRESH;
                }
                Err(e) => {
                    self.status = e;
                    self.next = Instant::now() + RETRY;
                }
            }
            changed = true;
        }
        let Some(identity) = &self.identity else {
            if self.status != "Signed out" {
                self.status = "Signed out".into();
                changed = true;
            }
            return changed;
        };
        if !identity.has("user:read:moderated_channels") {
            let text = "Moderated channels need updated Twitch permissions";
            if self.status != text {
                self.status = text.into();
                changed = true;
            }
            return changed;
        }
        if !self.pending
            && Instant::now() >= self.next
            && self
                .tx
                .try_send(Job {
                    epoch: self.epoch,
                    identity: identity.clone(),
                })
                .is_ok()
        {
            self.pending = true;
            if self.moderated.is_none() {
                self.status = "Checking moderated channels…".into();
                changed = true;
            }
        }
        changed
    }
    /// The account's role in `channel`, if known.
    pub fn role(&self, channel: &str) -> Option<Role> {
        let login = self.login.as_deref()?;
        self.identity.as_ref()?;
        if channel.eq_ignore_ascii_case(login) {
            return Some(Role::Broadcaster);
        }
        let moderated = self.moderated.as_ref()?;
        Some(if moderated.contains(&channel.to_ascii_lowercase()) {
            Role::Moderator
        } else {
            Role::Viewer
        })
    }
    pub fn check(&self, channel: &str, action: Action) -> Check {
        let Some(identity) = &self.identity else {
            return Check::SignedOut;
        };
        match action.needs() {
            // These act on the signed-in account itself, whatever channel is open.
            Needs::OwnChannel | Needs::Account => {}
            needs => {
                // The broadcaster role is known locally even while the list loads.
                let role = match self.role(channel) {
                    Some(role) => role,
                    None if !identity.has("user:read:moderated_channels") => Role::Viewer,
                    None => return Check::Checking,
                };
                match (needs, role) {
                    (_, Role::Broadcaster) | (Needs::Moderator, Role::Moderator) => {}
                    (Needs::Moderator, _) => {
                        return Check::NeedsRole("You are not a moderator in this channel.");
                    }
                    _ => {
                        return Check::NeedsRole(
                            "Only the broadcaster can do this in this channel.",
                        );
                    }
                }
            }
        }
        if action.scopes().iter().all(|s| identity.has(s)) {
            Check::Available
        } else {
            Check::NeedsPermission
        }
    }
    /// Channels you moderate (lowercase logins), once known.
    pub fn moderated(&self) -> Option<Vec<String>> {
        self.moderated.as_ref().map(|m| m.iter().cloned().collect())
    }
    pub fn inspection(&self) -> Value {
        let mut moderated: Vec<_> = self.moderated.iter().flatten().cloned().collect();
        moderated.sort();
        json!({
            "status": self.status,
            "login": self.login,
            "granted_scopes": self.identity.as_ref().map(|i| i.scopes.len()),
            "moderated_channels": moderated,
        })
    }
}

fn moderated_channels(
    client: &reqwest::blocking::Client,
    identity: &Identity,
) -> Result<HashSet<String>, String> {
    let mut channels = HashSet::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            ("user_id", identity.user_id.clone()),
            ("first", "100".into()),
        ];
        if let Some(after) = &cursor {
            query.push(("after", after.clone()));
        }
        let response = client
            .get("https://api.twitch.tv/helix/moderation/channels")
            .query(&query)
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&identity.access)
            .send()
            .map_err(|_| "Could not reach Twitch to check moderated channels")?;
        let (status, value) = crate::live::response_json(response)?;
        if status != 200 {
            return Err(format!(
                "Could not check moderated channels (HTTP {status}); retrying in a minute"
            ));
        }
        for entry in value["data"].as_array().into_iter().flatten() {
            if let Some(login) = entry["broadcaster_login"].as_str() {
                channels.insert(login.to_ascii_lowercase());
            }
        }
        cursor = value["pagination"]["cursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Ok(channels)
}
