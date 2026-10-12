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

/// Settings → Chat → Shared chat: 0 stripe + tint, 1 stripe only, 2 stripe and
/// dim other channels, 3 off. Your own channel's messages always stay plain.
pub static STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
pub const STYLES: [&str; 4] = ["Stripe + tint", "Stripe", "Dim others", "Off"];
pub fn style() -> u8 {
    STYLE.load(std::sync::atomic::Ordering::Relaxed).min(3)
}

/// Guest-channel colours: distinct from each other and from highlight colours.
const PALETTE: [u32; 6] = [0x5CC8A8, 0x6AA8F0, 0xF08A5D, 0xE36FA8, 0xD9C35A, 0x9C7CF0];

fn hash(id: &str) -> usize {
    id.bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619)) as usize
}
/// A streamer's colour on its own (stable across sessions).
pub fn color_of(id: &str) -> u32 {
    PALETTE[hash(id) % PALETTE.len()]
}
/// Colours for a session's guests (not `own`): each keeps its usual colour
/// unless another guest already has it, then takes the next free one.
pub fn colors(participants: &[String], own: Option<&str>) -> HashMap<String, u32> {
    let mut used = Vec::new();
    let mut out = HashMap::new();
    for id in participants.iter().filter(|id| Some(id.as_str()) != own) {
        let mut at = hash(id) % PALETTE.len();
        for _ in 0..PALETTE.len() {
            if !used.contains(&at) {
                break;
            }
            at = (at + 1) % PALETTE.len();
        }
        used.push(at);
        out.insert(id.clone(), PALETTE[at]);
    }
    out
}

use crate::{ChannelPane, theme};
use gpui_kit::component::StyledExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl ChannelPane {
    /// Header: who the chat is shared with, as overlapping avatar bubbles ringed in
    /// each channel's colour (yours neutral). Click one to hide or show its messages.
    pub fn render_shared_avatars(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session = self.shared_session.clone().filter(|s| s.len() > 1)?;
        let own = self.shared_own.clone();
        let colors = colors(&session, own.as_deref());
        let mut bubbles = Vec::new();
        for (index, id) in session.iter().enumerate() {
            let (avatar, name) = {
                let mut catalog = self.catalog.borrow_mut();
                catalog.profiles.request(id);
                let profile = catalog.profiles.get(id);
                (
                    profile.and_then(|p| p.avatar.clone()),
                    profile
                        .map(|p| p.display_name.clone())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "…".into()),
                )
            };
            let image = avatar.and_then(|key| self.media.borrow_mut().get(&key, cx));
            let mine = own.as_deref() == Some(id.as_str());
            let ring = if mine {
                0x6B6F78
            } else {
                colors.get(id).copied().unwrap_or(0x6B6F78)
            };
            let hidden = self.hidden_sources.contains(id);
            let tip = match (mine, hidden) {
                (true, false) => format!("{name} (this channel) · click to hide its messages"),
                (true, true) => format!("{name} (this channel) hidden · click to show"),
                (false, false) => format!("{name}'s chat · click to hide it"),
                (false, true) => format!("{name}'s chat hidden · click to show it"),
            };
            let initial = name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            let id = id.clone();
            bubbles.push(
                div()
                    .id(("shared-bubble", index))
                    .size(px(22.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(ring))
                    .bg(rgb(theme::PANEL))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(index > 0, |el| el.ml(px(-7.)))
                    .when(hidden, |el| el.opacity(0.3))
                    .child(match image {
                        Some(image) => {
                            crate::emote_picker::icon(Some(image), String::new(), 18., 18.)
                                .into_any_element()
                        }
                        None => div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::TEXT))
                            .child(initial)
                            .into_any_element(),
                    })
                    .tooltip(move |w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_shared_source(&id, cx)))
                    .into_any_element(),
            );
        }
        Some(
            div()
                .id("shared-chat-bubbles")
                .h_flex()
                .items_center()
                .flex_shrink_0()
                .px_1()
                .children(bubbles)
                .into_any_element(),
        )
    }
    /// Show or hide one channel's messages in a shared chat (one always stays shown).
    pub fn toggle_shared_source(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(at) = self.hidden_sources.iter().position(|h| h == id) {
            self.hidden_sources.remove(at);
        } else {
            let total = self.shared_session.as_ref().map_or(0, Vec::len);
            if self.hidden_sources.len() + 1 >= total {
                return;
            }
            self.hidden_sources.push(id.to_owned());
        }
        self.search.dirty = true;
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
        cx.notify();
    }
    /// The workspace's latest view of this channel's session.
    pub fn set_shared_session(
        &mut self,
        session: Option<Vec<String>>,
        own: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.shared_session == session && self.shared_own == own {
            return;
        }
        self.hidden_sources
            .retain(|id| session.as_ref().is_some_and(|s| s.contains(id)));
        self.shared_session = session;
        self.shared_own = own;
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn guest_colours_are_stable_and_distinct() {
        let session: Vec<String> = ["1", "2", "3", "4", "5", "6"].map(String::from).to_vec();
        let colors = super::colors(&session, Some("1"));
        assert_eq!(colors.len(), 5);
        assert!(!colors.contains_key("1"));
        let mut unique: Vec<u32> = colors.values().copied().collect();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 5);
        assert_eq!(super::colors(&session, Some("1")), colors);
    }
}
