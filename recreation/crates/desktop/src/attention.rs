//! Session-local, retention-bounded attention. References never own message bodies.
use crate::event_highlights::Kind;
use chat_core::{Fragment, Message, Timeline};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    time::Instant,
};
#[derive(Default)]
pub struct Attention {
    pub unread: BTreeSet<u64>,
    pub read_boundary: Option<u64>,
    pub highlights: Rc<BTreeSet<u64>>,
    /// Rows that mention you or match your highlights (what the "@" counts and
    /// Mentions collects). Event colours such as subs and raids aren't in it.
    pinged: BTreeSet<u64>,
    pub colors: Rc<BTreeMap<u64, usize>>,
    pub arrivals: BTreeMap<u64, Instant>,
    identity: Option<(String, String)>,
    terms: Vec<String>,
    rules: Vec<crate::highlight_rules::Rule>,
    channel: String,
    since: u64,
    events: crate::event_highlights::EventHighlights,
    raiders: crate::event_highlights::Raiders,
    /// Reply threads you follow (thread root IDs): new replies highlight and ping.
    pub followed: std::collections::BTreeSet<String>,
}
/// Why a new message deserves a sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ping {
    Mention,
    Highlight,
    /// A highlight rule matched: its sound choice (highlight_rules::SOUNDS
    /// index), whether it goes to Mentions, and whether it flashes the taskbar.
    Rule {
        sound: u8,
        mentions: bool,
        flash: bool,
        /// Which rule (its place in the list), for its own sound file.
        index: u8,
    },
    /// A kind of event with a sound chosen in Event highlights.
    Event(crate::event_highlights::Kind),
}
pub fn terms(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let item = item.chars().take(40).collect::<String>().to_lowercase();
        if !result.contains(&item) {
            result.push(item);
        }
        // Highlight words (8) plus "!name" exclusions (up to 24 more).
        if result.iter().filter(|t| !t.starts_with('!')).count() > 8 || result.len() >= 32 {
            result.pop();
            break;
        }
    }
    result
}
impl Attention {
    /// The thread a message belongs to (a reply's thread, or itself as a root).
    pub fn thread_of(message: &Message) -> String {
        message
            .reply
            .as_ref()
            .map(|r| r.thread_id.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| message.id.clone())
    }
    fn in_followed_thread(&self, message: &Message) -> bool {
        message.reply.as_ref().is_some_and(|r| {
            self.followed.contains(&r.thread_id) || self.followed.contains(&r.parent_id)
        })
    }
    /// Follow or unfollow a thread; returns true if now following.
    pub fn toggle_follow(&mut self, thread: &str) -> bool {
        if self.followed.remove(thread) {
            false
        } else {
            if self.followed.len() >= 100 {
                self.followed.pop_first();
            }
            self.followed.insert(thread.to_owned());
            true
        }
    }
    /// "!name" in highlight words: never highlight or ping for that chatter (bots).
    fn excluded(&self, message: &Message) -> bool {
        let login = message.login.as_deref().unwrap_or("");
        let name = message.display_name.to_lowercase();
        self.terms
            .iter()
            .filter_map(|t| t.strip_prefix('!'))
            .any(|x| x == login || x == name)
    }
    fn color(&self, message: &Message) -> Option<usize> {
        let whisper = message.presentation.notice_type.as_deref() == Some("whisper");
        if message.deleted
            || message.presentation.kind == chat_core::MODERATION
            || (message.presentation.kind == chat_core::SYSTEM && !whisper)
            || self.excluded(message)
            || (message.presentation.similar && !crate::similarity::flag(2))
        {
            return None;
        }
        // Replies in a followed thread stand out like mentions (not your own).
        if self.in_followed_thread(message)
            && self
                .identity
                .as_ref()
                .is_none_or(|(user, _)| &message.user_id != user)
        {
            return self.events.get(Kind::Mention).or(Some(5));
        }
        if let Some((user, _)) = &self.identity
            && &message.user_id == user
        {
            return self.events.get(Kind::OwnMessage);
        }
        if let Some(color) =
            crate::highlight_rules::first_color(&self.rules, message, &self.channel)
        {
            return Some(color);
        }
        if self.identity.as_ref().is_some_and(|(user, _)| {
            message.mentions.iter().any(|id| id == user)
                || message
                    .reply
                    .as_ref()
                    .is_some_and(|r| &r.parent_user_id == user)
        }) {
            return self.events.get(Kind::Mention);
        }
        if let Some(kind) = crate::event_highlights::EventHighlights::kind(message) {
            return self.events.get(kind);
        }
        if self.raiders.recent(message)
            && let Some(color) = self.events.get(Kind::Raid)
        {
            return Some(color);
        }
        // Your login in plain text counts as a mention; custom words use the first colour.
        let matches = |text: &str| {
            let folded = text.to_lowercase();
            if self.identity.as_ref().is_some_and(|(_, login)| {
                crate::highlight_rules::normalized_match(&folded, login, true)
            }) {
                return Some(self.events.get(Kind::Mention));
            }
            self.terms
                .iter()
                .filter(|term| !term.starts_with('!'))
                .any(|term| crate::highlight_rules::normalized_match(&folded, term, true))
                .then_some(Some(0))
        };
        let mut text = String::new();
        for fragment in &message.fragments {
            match fragment {
                Fragment::Text(part) | Fragment::Mention { label: part, .. } => text.push_str(part),
                _ => {
                    if let Some(color) = matches(&text) {
                        return color;
                    }
                    text.clear();
                }
            }
        }
        matches(&text).flatten()
    }
    /// Apply new event colours; re-highlights retained rows when they changed.
    pub fn set_events(
        &mut self,
        events: &crate::event_highlights::EventHighlights,
        timeline: &Timeline,
        first: u64,
    ) -> bool {
        if &self.events == events {
            return false;
        }
        self.events = events.clone();
        self.reconcile(timeline, first);
        true
    }
    /// The signed-in account here: (user ID, login).
    pub fn identity(&self) -> Option<&(String, String)> {
        self.identity.as_ref()
    }
    pub fn configure(
        &mut self,
        identity: Option<(String, String)>,
        terms: &[String],
        rules: &[crate::highlight_rules::Rule],
        channel: &str,
        timeline: &Timeline,
        first: u64,
        next: u64,
    ) -> bool {
        let changed = self.identity != identity
            || self.terms != terms
            || self.rules != rules
            || self.channel != channel;
        if self.identity != identity {
            self.identity = identity;
            self.since = next;
            self.read_boundary = None;
            self.unread.clear();
            Rc::make_mut(&mut self.highlights).clear();
            Rc::make_mut(&mut self.colors).clear();
            self.pinged.clear();
        }
        if changed {
            self.terms = terms.to_vec();
            self.rules = rules.to_vec();
            self.channel = channel.into();
            self.reconcile(timeline, first);
        }
        changed
    }
    /// Your own live message that tags you (@yourname). Never a ping, but the
    /// Mentions tab shows it.
    pub fn mentions_self(&self, message: &Message) -> bool {
        let Some((user, login)) = self.identity.as_ref() else {
            return false;
        };
        if &message.user_id != user || message.presentation.historical || message.deleted {
            return false;
        }
        let text: String = message
            .fragments
            .iter()
            .filter_map(|f| match f {
                Fragment::Text(part) | Fragment::Mention { label: part, .. } => Some(part.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        message.mentions.iter().any(|id| id == user)
            || crate::highlight_rules::normalized_match(&text, login, true)
    }
    /// Mention (your name, a reply to you) or a highlight rule/word; event kinds
    /// such as subs and raids are colour only.
    fn ping(&self, message: &Message) -> Option<Ping> {
        let (user, login) = self.identity.as_ref()?;
        if message.deleted || &message.user_id == user || self.excluded(message) {
            return None;
        }
        if self.in_followed_thread(message) {
            return Some(Ping::Mention);
        }
        let text: String = message
            .fragments
            .iter()
            .filter_map(|f| match f {
                Fragment::Text(part) | Fragment::Mention { label: part, .. } => Some(part.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if message.mentions.iter().any(|id| id == user)
            || message
                .reply
                .as_ref()
                .is_some_and(|r| &r.parent_user_id == user)
            || crate::highlight_rules::normalized_match(&text, login, true)
        {
            return Some(Ping::Mention);
        }
        if let Some(rule) = crate::highlight_rules::first_match(&self.rules, message, &self.channel)
        {
            return Some(Ping::Rule {
                index: self
                    .rules
                    .iter()
                    .position(|r| std::ptr::eq(r, rule))
                    .unwrap_or(0) as u8,
                sound: rule.sound,
                mentions: rule.mentions,
                flash: rule.flash,
            });
        }
        if self
            .terms
            .iter()
            .filter(|term| !term.starts_with('!'))
            .any(|term| crate::highlight_rules::normalized_match(&text, term, true))
        {
            return Some(Ping::Highlight);
        }
        self.event_ping(message)
    }
    /// An event kind with a sound (whispers play theirs when they arrive).
    fn event_ping(&self, message: &Message) -> Option<Ping> {
        let kind = crate::event_highlights::EventHighlights::kind(message)?;
        (kind != Kind::Whisper && kind != Kind::OwnMessage && self.events.sound(kind) != 1)
            .then_some(Ping::Event(kind))
    }
    /// Records a new row; returns a ping for a fresh (not history) mention or highlight.
    pub fn appended(&mut self, message: &Message, row: u64, first: u64) -> Option<Ping> {
        // Your own messages never need catching up on.
        let own = self
            .identity
            .as_ref()
            .is_some_and(|(user, _)| &message.user_id == user);
        // Replying to a thread follows it, as on Twitch.
        if own
            && !message.presentation.historical
            && let Some(reply) = &message.reply
        {
            let thread = if reply.thread_id.is_empty() {
                reply.parent_id.clone()
            } else {
                reply.thread_id.clone()
            };
            if !thread.is_empty() {
                self.followed.insert(thread);
            }
        }
        // History loaded on open is context, not something new to catch up on.
        if !own
            && !message.presentation.historical
            && message.presentation.kind != chat_core::SYSTEM
        {
            self.unread.insert(row);
        }
        self.arrivals.insert(row, Instant::now());
        self.raiders.note(message);
        let mut ping = None;
        if row >= self.since
            && let Some(color) = self.color(message)
        {
            Rc::make_mut(&mut self.highlights).insert(row);
            Rc::make_mut(&mut self.colors).insert(row, color);
            if !message.presentation.historical {
                ping = self.ping(message);
                if matches!(ping, Some(p) if !matches!(p, Ping::Event(_))) {
                    self.pinged.insert(row);
                }
            }
        } else if row >= self.since
            && !message.presentation.historical
            && !message.deleted
            && !self.excluded(message)
            && self
                .identity
                .as_ref()
                .is_some_and(|(user, _)| &message.user_id != user)
        {
            // A sound for an event kind whose colour is off.
            ping = self.event_ping(message);
        }
        self.prune(first);
        ping
    }
    fn prune(&mut self, first: u64) {
        if self.read_boundary.is_some_and(|row| row < first) {
            self.read_boundary = None;
        }
        while self
            .arrivals
            .first_key_value()
            .is_some_and(|(row, _)| *row < first)
        {
            self.arrivals.pop_first();
        }
        while self.unread.first().is_some_and(|row| *row < first) {
            self.unread.pop_first();
        }
        let colors = Rc::make_mut(&mut self.colors);
        while colors
            .first_key_value()
            .is_some_and(|(row, _)| *row < first)
        {
            colors.pop_first();
        }
        let h = Rc::make_mut(&mut self.highlights);
        while h.first().is_some_and(|row| *row < first) {
            h.pop_first();
        }
        while self.pinged.first().is_some_and(|row| *row < first) {
            self.pinged.pop_first();
        }
    }
    pub fn reconcile(&mut self, timeline: &Timeline, first: u64) {
        self.prune(first);
        let mut highlights = BTreeSet::new();
        let mut colors = BTreeMap::new();
        for (index, message) in timeline.messages().iter().enumerate() {
            let row = first + index as u64;
            if message.deleted {
                self.unread.remove(&row);
                self.pinged.remove(&row);
            } else if row >= self.since
                && let Some(color) = self.color(message)
            {
                highlights.insert(row);
                colors.insert(row, color);
                if self.ping(message).is_none() {
                    self.pinged.remove(&row);
                }
            } else {
                self.pinged.remove(&row);
            }
        }
        self.highlights = Rc::new(highlights);
        self.colors = Rc::new(colors);
    }
    pub fn mark_through(&mut self, row: u64) -> bool {
        let before = self.unread.len();
        self.unread.retain(|r| *r > row);
        before != self.unread.len()
    }
    pub fn reading_transition(&mut self, was: bool, reading: bool, next: u64) {
        if was && !reading {
            self.read_boundary = Some(self.unread.first().copied().unwrap_or(next));
        } else if !was && reading && self.read_boundary.is_none() {
            self.read_boundary = self.unread.first().copied();
        }
    }
    pub fn mark_all(&mut self) -> bool {
        let changed = !self.unread.is_empty() || self.read_boundary.is_some();
        self.unread.clear();
        self.read_boundary = None;
        changed
    }
    /// The signed-in user's ID, if known.
    pub fn own_user(&self) -> Option<&str> {
        self.identity.as_ref().map(|(user, _)| user.as_str())
    }
    pub fn counts(&self) -> (usize, usize) {
        (
            self.unread.len(),
            self.unread.intersection(&self.pinged).count(),
        )
    }
}
pub fn count(value: usize) -> String {
    if value > 99 {
        "99+".into()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod follow_tests {
    use super::Attention;
    fn message(id: &str, thread: Option<&str>) -> chat_core::Message {
        chat_core::Message {
            sent_at: Some(0),
            id: id.into(),
            channel_id: "c".into(),
            user_id: "other".into(),
            display_name: "Bob".into(),
            login: Some("bob".into()),
            reply: thread.map(|t| chat_core::Reply {
                parent_id: t.into(),
                parent_user_id: "x".into(),
                parent_name: "X".into(),
                thread_id: t.into(),
                parent_deleted: false,
            }),
            mentions: vec![],
            replyable: true,
            name_color: None,
            badges: vec![],
            fragments: vec![chat_core::Fragment::Text("hi".into())],
            presentation: Default::default(),
            deleted: false,
        }
    }
    #[test]
    fn event_colours_are_not_mentions() {
        let mut a = Attention {
            identity: Some(("me".into(), "me".into())),
            ..Attention::default()
        };
        let mut sub = message("s", None);
        sub.presentation.notice_type = Some("resub".into());
        assert_eq!(a.appended(&sub, 0, 0), None);
        let mut mention = message("m", None);
        mention.mentions = vec!["me".into()];
        assert_eq!(a.appended(&mention, 1, 0), Some(super::Ping::Mention));
        // Both are coloured, but only the mention counts toward "@".
        assert_eq!(a.highlights.len(), 2);
        assert_eq!(a.counts(), (2, 1));
    }
    #[test]
    fn event_sounds_ping_without_counting_as_mentions() {
        use crate::event_highlights::Kind;
        let mut a = Attention {
            identity: Some(("me".into(), "me".into())),
            ..Attention::default()
        };
        let mut sub = message("s1", None);
        sub.presentation.notice_type = Some("resub".into());
        assert_eq!(a.appended(&sub, 0, 0), None, "no sound chosen");
        a.events.set_sound(Kind::Subscription, 2);
        let mut sub = message("s2", None);
        sub.presentation.notice_type = Some("resub".into());
        assert_eq!(
            a.appended(&sub, 1, 0),
            Some(super::Ping::Event(Kind::Subscription))
        );
        // Still a sound even with the colour off; never part of "@".
        a.events.set(Kind::Subscription, None);
        let mut sub = message("s3", None);
        sub.presentation.notice_type = Some("resub".into());
        assert_eq!(
            a.appended(&sub, 2, 0),
            Some(super::Ping::Event(Kind::Subscription))
        );
        assert_eq!(a.counts().1, 0);
    }
    #[test]
    fn following_threads() {
        let mut a = Attention::default();
        assert_eq!(Attention::thread_of(&message("root", None)), "root");
        assert_eq!(Attention::thread_of(&message("r1", Some("root"))), "root");
        assert!(a.toggle_follow("root"));
        assert!(a.in_followed_thread(&message("r2", Some("root"))));
        assert!(!a.in_followed_thread(&message("r3", Some("other"))));
        assert!(!a.toggle_follow("root"));
        assert!(!a.in_followed_thread(&message("r2", Some("root"))));
    }
}
