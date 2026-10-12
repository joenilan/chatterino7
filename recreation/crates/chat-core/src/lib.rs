//! Transport-independent, bounded chat state. No credentials or network access.
pub mod emotes;
pub mod selection;
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EmoteAsset {
    pub url: String,
    pub static_url: String,
    pub width: u16,
    pub height: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fragment {
    Text(String),
    /// Provider-authored identity; label retains the exact source text.
    Mention {
        user_id: String,
        login: String,
        display_name: String,
        label: String,
    },
    Gif {
        id: String,
        url: String,
        label: String,
    },
    Cheer {
        prefix: String,
        bits: u64,
        tier: u64,
        label: String,
        asset: Option<EmoteAsset>,
    },
    Unknown {
        kind: String,
        label: String,
    },
    Emote {
        provider: String,
        id: String,
        label: String,
        overlay: bool,
        animated: bool,
        asset: Option<EmoteAsset>,
    },
}
impl Fragment {
    pub fn copy_text(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Mention { label, .. }
            | Self::Emote { label, .. }
            | Self::Gif { label, .. }
            | Self::Cheer { label, .. }
            | Self::Unknown { label, .. } => label,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub set_id: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub parent_id: String,
    pub parent_user_id: String,
    pub parent_name: String,
    pub thread_id: String,
    pub parent_deleted: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessagePresentation {
    /// Private UI overlay; never supplied to provider APIs.
    pub local_alias: Option<String>,
    pub local_color: Option<u32>,
    pub kind: String,
    pub bits: u64,
    pub reward_id: Option<String>,
    pub notice: Option<String>,
    pub notice_type: Option<String>,
    pub source: Option<String>,
    /// Loaded from recent history rather than received live.
    pub historical: bool,
    /// Nearly repeats the same chatter's recent message (shown dimmed).
    pub similar: bool,
    /// Moderator-only suspicious-user label, e.g. "Suspicious · Monitored".
    pub suspicious: Option<String>,
    /// In a combined feed: the channel this message came from.
    pub origin: Option<String>,
    /// Moderation lines: how many times this action was stacked into one line.
    pub repeats: u32,
    /// Shared Chat: the broadcaster ID of the channel this message was sent in.
    pub source_id: Option<String>,
    /// "表示名 (login)" when the display name is more than a recased login.
    pub name_both: Option<String>,
}
/// How chatters' names show: 0 display name, 1 login, 2 both when they differ
/// (Chatterino's "Username and localized name", the default).
pub static NAME_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub sent_at: Option<i64>,
    pub id: String,
    pub channel_id: String,
    pub user_id: String,
    pub display_name: String,
    pub login: Option<String>,
    pub reply: Option<Reply>,
    pub mentions: Vec<String>,
    pub replyable: bool,
    pub name_color: Option<u32>,
    pub badges: Vec<Badge>,
    pub fragments: Vec<Fragment>,
    pub presentation: MessagePresentation,
    pub deleted: bool,
}
impl Message {
    pub fn author_label(&self) -> &str {
        if let Some(alias) = self
            .presentation
            .local_alias
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            return alias;
        }
        match NAME_MODE.load(std::sync::atomic::Ordering::Relaxed) {
            1 => self
                .login
                .as_deref()
                .filter(|l| !l.is_empty())
                .unwrap_or(&self.display_name),
            2 => self
                .presentation
                .name_both
                .as_deref()
                .unwrap_or(&self.display_name),
            _ => &self.display_name,
        }
    }
    pub fn author_color(&self) -> Option<u32> {
        self.presentation.local_color.or(self.name_color)
    }
    /// The text (deleted messages keep theirs; they are shown greyed out).
    pub fn body(&self) -> String {
        if self.fragments.is_empty()
            && let Some(notice) = &self.presentation.notice
        {
            return notice.clone();
        }
        self.fragments.iter().map(Fragment::copy_text).collect()
    }
    pub fn copy_line(&self) -> String {
        format!("{}: {}", self.author_label(), self.body())
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Message(Message),
    DeleteMessage {
        channel_id: String,
        message_id: String,
    },
    ClearUser {
        channel_id: String,
        user_id: String,
    },
    ClearChannel {
        channel_id: String,
    },
    /// A moderation line in chat ("X was timed out or banned"). `precise` lines
    /// (from the moderator's own event, with length and reason) replace the most
    /// recent vague line for the same user instead of adding another.
    Moderation {
        channel_id: String,
        user_id: String,
        text: String,
        sent_at: Option<i64>,
        precise: bool,
    },
}
/// Moderation lines are written as `presentation.kind == MODERATION`.
pub const MODERATION: &str = "moderation";
/// How far back a repeat or a precise update looks for the same user's line.
const MODERATION_WINDOW: usize = 40;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Ignored,
    Appended { evicted: bool },
    Updated,
}

/// Retention and duplicate detection are bounded together. Deduplication covers
/// retained messages only; transports must additionally deduplicate envelopes.
#[derive(Debug)]
pub struct Timeline {
    channel_id: String,
    /// Further channels a combined feed accepts.
    also: Vec<String>,
    capacity: usize,
    messages: VecDeque<Message>,
    ids: HashSet<String>,
}
impl Timeline {
    /// Restyle authors without touching identity, body or deletion state.
    pub fn update_author_appearance(
        &mut self,
        mut lookup: impl FnMut(&str, Option<u32>) -> (Option<String>, Option<u32>),
    ) {
        for message in &mut self.messages {
            let (alias, color) = lookup(&message.user_id, message.name_color);
            message.presentation.local_alias = alias;
            message.presentation.local_color = color;
        }
    }

    pub fn enrich(&mut self, mut apply: impl FnMut(&mut Message)) {
        for message in &mut self.messages {
            if !message.deleted {
                apply(message);
            }
        }
    }

    pub fn new(channel_id: impl Into<String>, capacity: usize) -> Self {
        Self {
            channel_id: channel_id.into(),
            also: Vec::new(),
            capacity: capacity.max(1),
            messages: VecDeque::new(),
            ids: HashSet::new(),
        }
    }
    /// Accept messages from these channels too (a combined feed).
    pub fn accept_channels(&mut self, channels: Vec<String>) {
        self.also = channels;
    }
    fn accepts(&self, channel: &str) -> bool {
        channel == self.channel_id || self.also.iter().any(|c| c == channel)
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Return the removed front-row count so the viewport owner can retain its anchor.
    pub fn set_capacity(&mut self, capacity: usize) -> usize {
        self.capacity = capacity.max(1);
        let removed = self.messages.len().saturating_sub(self.capacity);
        for _ in 0..removed {
            if let Some(message) = self.messages.pop_front() {
                self.ids.remove(&message.id);
            }
        }
        self.messages.shrink_to_fit();
        self.ids.shrink_to_fit();
        removed
    }
    /// Remove every message from this view only (Chatterino's "Clear messages").
    /// Returns how many rows were removed.
    pub fn clear(&mut self) -> usize {
        let removed = self.messages.len();
        self.messages.clear();
        self.ids.clear();
        removed
    }
    pub fn messages(&self) -> &VecDeque<Message> {
        &self.messages
    }
    pub fn apply(&mut self, event: Event) -> Change {
        match event {
            Event::Message(mut message) => {
                if let Some(login) = message.login.as_deref()
                    && !login.is_empty()
                    && !message.display_name.eq_ignore_ascii_case(login)
                {
                    message.presentation.name_both =
                        Some(format!("{} ({login})", message.display_name));
                }
                if !self.accepts(&message.channel_id)
                    || message.id.is_empty()
                    || self.ids.contains(&message.id)
                {
                    return Change::Ignored;
                }
                let evicted = self.messages.len() == self.capacity;
                if evicted && let Some(old) = self.messages.pop_front() {
                    self.ids.remove(&old.id);
                }
                self.ids.insert(message.id.clone());
                self.messages.push_back(message);
                Change::Appended { evicted }
            }
            Event::DeleteMessage {
                channel_id,
                message_id,
            } => {
                let quotes = self.redact_quotes(&channel_id, |r| r.parent_id == message_id);
                let body = self.redact(&channel_id, |m| m.id == message_id);
                if quotes { Change::Updated } else { body }
            }
            Event::ClearUser {
                channel_id,
                user_id,
            } => {
                let quotes = self.redact_quotes(&channel_id, |r| r.parent_user_id == user_id);
                let body = self.redact(&channel_id, |m| m.user_id == user_id);
                if quotes { Change::Updated } else { body }
            }
            Event::ClearChannel { channel_id } => self.redact(&channel_id, |_| true),
            Event::Moderation {
                channel_id,
                user_id,
                text,
                sent_at,
                precise,
            } => self.moderation(channel_id, user_id, text, sent_at, precise),
        }
    }
    fn moderation(
        &mut self,
        channel_id: String,
        user_id: String,
        text: String,
        sent_at: Option<i64>,
        precise: bool,
    ) -> Change {
        if !self.accepts(&channel_id) {
            return Change::Ignored;
        }
        // Twitch sends a vague "messages cleared" event to everyone and, in channels
        // you moderate, the precise action too, in either order (seconds apart).
        // Rules: a precise action upgrades a vague line from the last few seconds;
        // a vague echo of a fresh precise line is dropped; repeats of the same
        // action stack into one line ("(3 times)").
        const ECHO_SECONDS: i64 = 10;
        let close = |a: Option<i64>, b: Option<i64>| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() <= ECHO_SECONDS,
            _ => true,
        };
        let recent = self
            .messages
            .iter_mut()
            .rev()
            .take(MODERATION_WINDOW)
            .find(|m| {
                m.presentation.kind == MODERATION
                    && m.user_id == user_id
                    && m.channel_id == channel_id
            });
        if let Some(line) = recent {
            let vague = line.presentation.source.as_deref() == Some("vague");
            match (precise, vague) {
                (true, true) if close(sent_at, line.sent_at) => {
                    line.presentation.notice = Some(text);
                    line.presentation.source = None;
                    return Change::Updated;
                }
                (false, false) if close(sent_at, line.sent_at) => return Change::Ignored,
                // The same precise action again, or another vague clear: stack.
                (true, false) | (false, true)
                    if !precise || line.presentation.notice.as_deref() == Some(text.as_str()) =>
                {
                    line.presentation.repeats += 1;
                    line.sent_at = sent_at.or(line.sent_at);
                    return Change::Updated;
                }
                _ => {}
            }
        }
        let id = format!(
            "mod:{user_id}:{}:{}",
            sent_at.unwrap_or(0),
            self.messages.len()
        );
        let message = Message {
            sent_at,
            id,
            channel_id,
            user_id,
            display_name: String::new(),
            login: None,
            reply: None,
            mentions: vec![],
            replyable: false,
            name_color: None,
            badges: vec![],
            fragments: vec![],
            presentation: MessagePresentation {
                kind: MODERATION.into(),
                notice: Some(text),
                source: (!precise).then(|| "vague".into()),
                ..Default::default()
            },
            deleted: false,
        };
        self.apply(Event::Message(message))
    }
    fn redact_quotes(&mut self, channel: &str, matches: impl Fn(&Reply) -> bool) -> bool {
        if !self.accepts(channel) {
            return false;
        }
        let mut changed = false;
        for message in &mut self.messages {
            if let Some(reply) = &mut message.reply
                && matches(reply)
                && !reply.parent_deleted
            {
                reply.parent_deleted = true;
                changed = true;
            }
        }
        changed
    }
    fn redact(&mut self, channel: &str, matches: impl Fn(&Message) -> bool) -> Change {
        if !self.accepts(channel) {
            return Change::Ignored;
        }
        let mut changed = false;
        for message in &mut self.messages {
            if !message.deleted && message.presentation.kind != MODERATION && matches(message) {
                // Keep the text: Jawjack shows deleted messages greyed out, like Chatterino.
                message.deleted = true;
                message.presentation = MessagePresentation {
                    local_alias: message.presentation.local_alias.take(),
                    local_color: message.presentation.local_color,
                    historical: message.presentation.historical,
                    ..MessagePresentation::default()
                };
                if let Some(reply) = &mut message.reply {
                    reply.parent_deleted = true;
                }
                changed = true;
            }
        }
        if changed {
            Change::Updated
        } else {
            Change::Ignored
        }
    }
}

/// Synthetic replay data only. IDs and names do not represent real accounts.
pub fn fixture(channel: &str, index: usize) -> Message {
    let body = match index % 5 {
        0 => "A dense chat client should stay readable while messages arrive.",
        1 => "Unicode check: 日本語 · 한국어 · Português · 👩🏽‍💻",
        2 => "Scroll upward, add a burst, then jump back to the latest message.",
        3 => "<script>plain text</script> **not Markdown** [not a link](https://example.com)",
        _ => "Replay fixture. Twitch authentication and emotes are not connected yet.",
    };
    Message {
        sent_at: None,
        id: format!("{channel}-{index}"),
        channel_id: channel.into(),
        login: None,
        reply: None,
        mentions: Vec::new(),
        replyable: false,
        user_id: format!("fixture-{}", index % 4),
        display_name: format!("viewer_{}", index % 4),
        name_color: None,
        badges: Vec::new(),
        fragments: vec![Fragment::Text(body.into())],
        presentation: MessagePresentation::default(),
        deleted: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn moderation(user: &str, text: &str, at: i64, precise: bool) -> Event {
        Event::Moderation {
            channel_id: "a".into(),
            user_id: user.into(),
            text: text.into(),
            sent_at: Some(at),
            precise,
        }
    }
    fn lines(t: &Timeline) -> Vec<(String, u32)> {
        t.messages()
            .iter()
            .filter(|m| m.presentation.kind == MODERATION)
            .map(|m| (m.body(), m.presentation.repeats))
            .collect()
    }
    #[test]
    fn moderation_lines_upgrade_dedupe_and_stack() {
        let mut t = Timeline::new("a", 50);
        // Vague first, then the moderator's precise event: one precise line.
        t.apply(moderation("u1", "bob was timed out or banned.", 100, false));
        t.apply(moderation("u1", "mod timed out bob for 10m", 101, true));
        assert_eq!(lines(&t), vec![("mod timed out bob for 10m".into(), 0)]);
        // Precise first, then the vague echo: still one line.
        t.apply(moderation("u2", "mod banned amy", 200, true));
        t.apply(moderation("u2", "amy was timed out or banned.", 201, false));
        assert_eq!(lines(&t).len(), 2);
        // The same action again later stacks.
        t.apply(moderation("u1", "mod timed out bob for 10m", 400, true));
        t.apply(moderation("u1", "bob was timed out or banned.", 401, false));
        assert_eq!(lines(&t)[0], ("mod timed out bob for 10m".into(), 1));
        // Viewers only see vague clears: repeats stack.
        t.apply(moderation("u3", "cat was timed out or banned.", 500, false));
        t.apply(moderation("u3", "cat was timed out or banned.", 560, false));
        assert_eq!(lines(&t)[2], ("cat was timed out or banned.".into(), 1));
        // A different action (unban) gets its own line.
        t.apply(moderation("u2", "mod unbanned amy", 600, true));
        assert_eq!(lines(&t).len(), 4);
        // Clearing a user's messages never hides the moderation line itself.
        t.apply(Event::ClearUser {
            channel_id: "a".into(),
            user_id: "u1".into(),
        });
        assert!(t.messages().iter().all(|m| !m.deleted));
    }
    #[test]
    fn duplicate_is_ignored() {
        let mut t = Timeline::new("a", 10);
        assert_eq!(
            t.apply(Event::Message(fixture("a", 0))),
            Change::Appended { evicted: false }
        );
        assert_eq!(t.apply(Event::Message(fixture("a", 0))), Change::Ignored);
        assert_eq!(t.messages().len(), 1);
    }
    #[test]
    fn channels_are_isolated() {
        let mut t = Timeline::new("a", 10);
        assert_eq!(t.apply(Event::Message(fixture("b", 0))), Change::Ignored);
        t.apply(Event::Message(fixture("a", 0)));
        assert_eq!(
            t.apply(Event::ClearChannel {
                channel_id: "b".into()
            }),
            Change::Ignored
        );
        assert!(!t.messages()[0].deleted);
    }
    #[test]
    fn retention_is_bounded_and_evicts_oldest() {
        let mut t = Timeline::new("a", 2);
        for i in 0..100 {
            t.apply(Event::Message(fixture("a", i)));
        }
        assert_eq!(t.messages().len(), 2);
        assert_eq!(t.ids.len(), 2);
        assert_eq!(t.messages()[0].id, "a-98");
    }
    #[test]
    fn deletion_keeps_text_greyed_and_cannot_be_undone_by_duplicate() {
        let mut t = Timeline::new("a", 2);
        let m = fixture("a", 0);
        t.apply(Event::Message(m.clone()));
        t.apply(Event::DeleteMessage {
            channel_id: "a".into(),
            message_id: m.id.clone(),
        });
        assert_eq!(
            t.messages()[0].body(),
            m.body(),
            "deleted text stays readable"
        );
        assert_eq!(t.apply(Event::Message(m)), Change::Ignored);
        assert!(t.messages()[0].deleted);
    }
    #[test]
    fn timeout_only_redacts_target_user() {
        let mut t = Timeline::new("a", 10);
        for i in 0..4 {
            t.apply(Event::Message(fixture("a", i)));
        }
        t.apply(Event::ClearUser {
            channel_id: "a".into(),
            user_id: "fixture-1".into(),
        });
        assert_eq!(t.messages().iter().filter(|m| m.deleted).count(), 1);
        assert!(t.messages()[1].deleted);
    }
    #[test]
    fn fragments_preserve_unicode_and_emote_copy_labels() {
        let mut m = fixture("a", 0);
        m.fragments = vec![
            Fragment::Text("日本語 👩🏽‍💻 ".into()),
            Fragment::Emote {
                provider: "7tv".into(),
                id: "synthetic".into(),
                label: "Wave".into(),
                overlay: true,
                animated: false,
                asset: None,
            },
        ];
        assert_eq!(m.body(), "日本語 👩🏽‍💻 Wave");
    }
    #[test]
    fn clear_is_idempotent() {
        let mut t = Timeline::new("a", 2);
        t.apply(Event::Message(fixture("a", 0)));
        assert_eq!(
            t.apply(Event::ClearChannel {
                channel_id: "a".into()
            }),
            Change::Updated
        );
        assert_eq!(
            t.apply(Event::ClearChannel {
                channel_id: "a".into()
            }),
            Change::Ignored
        );
    }
    #[test]
    fn zero_capacity_is_safe() {
        let mut t = Timeline::new("a", 0);
        t.apply(Event::Message(fixture("a", 0)));
        t.apply(Event::Message(fixture("a", 1)));
        assert_eq!(t.messages().len(), 1);
    }
}

/// Only server-provided Twitch logins may become profile or mention targets.
pub fn twitch_login(raw: &str) -> Option<String> {
    (!raw.is_empty()
        && raw.len() <= 25
        && raw.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'))
    .then(|| raw.to_ascii_lowercase())
}
