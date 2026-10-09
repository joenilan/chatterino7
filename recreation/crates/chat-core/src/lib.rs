//! Transport-independent, bounded chat state. No credentials or network access.
pub mod emotes;
pub mod selection;
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fragment {
    Text(String),
    Emote {
        provider: String,
        id: String,
        label: String,
        overlay: bool,
        animated: bool,
    },
}
impl Fragment {
    pub fn copy_text(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Emote { label, .. } => label,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub user_id: String,
    pub display_name: String,
    pub name_color: Option<u32>,
    pub fragments: Vec<Fragment>,
    pub deleted: bool,
}
impl Message {
    pub fn body(&self) -> String {
        if self.deleted {
            return "[message deleted]".into();
        }
        self.fragments.iter().map(Fragment::copy_text).collect()
    }
    pub fn copy_line(&self) -> String {
        format!("{}: {}", self.display_name, self.body())
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
}
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
    capacity: usize,
    messages: VecDeque<Message>,
    ids: HashSet<String>,
}
impl Timeline {
    pub fn new(channel_id: impl Into<String>, capacity: usize) -> Self {
        Self {
            channel_id: channel_id.into(),
            capacity: capacity.max(1),
            messages: VecDeque::new(),
            ids: HashSet::new(),
        }
    }
    pub fn messages(&self) -> &VecDeque<Message> {
        &self.messages
    }
    pub fn apply(&mut self, event: Event) -> Change {
        match event {
            Event::Message(message) => {
                if message.channel_id != self.channel_id
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
            } => self.redact(&channel_id, |m| m.id == message_id),
            Event::ClearUser {
                channel_id,
                user_id,
            } => self.redact(&channel_id, |m| m.user_id == user_id),
            Event::ClearChannel { channel_id } => self.redact(&channel_id, |_| true),
        }
    }
    fn redact(&mut self, channel: &str, matches: impl Fn(&Message) -> bool) -> Change {
        if channel != self.channel_id {
            return Change::Ignored;
        }
        let mut changed = false;
        for message in &mut self.messages {
            if !message.deleted && matches(message) {
                message.deleted = true;
                message.fragments.clear();
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
        id: format!("{channel}-{index}"),
        channel_id: channel.into(),
        user_id: format!("fixture-{}", index % 4),
        display_name: format!("viewer_{}", index % 4),
        name_color: None,
        fragments: vec![Fragment::Text(body.into())],
        deleted: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn deletion_redacts_copy_and_cannot_be_undone_by_duplicate() {
        let mut t = Timeline::new("a", 2);
        let m = fixture("a", 0);
        t.apply(Event::Message(m.clone()));
        t.apply(Event::DeleteMessage {
            channel_id: "a".into(),
            message_id: m.id.clone(),
        });
        assert!(t.messages()[0].fragments.is_empty());
        assert_eq!(t.messages()[0].body(), "[message deleted]");
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
