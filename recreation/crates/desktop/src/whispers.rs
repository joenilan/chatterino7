//! Twitch whispers: session-only conversations (nothing is written to disk),
//! received through EventSub and sent through Helix.
use std::collections::VecDeque;

const CONVERSATIONS: usize = 50;
const MESSAGES: usize = 200;

#[derive(Clone)]
pub struct Line {
    pub at: chrono::DateTime<chrono::Local>,
    pub mine: bool,
    pub text: String,
}
#[derive(Clone)]
pub struct Conversation {
    pub login: String,
    pub name: String,
    pub lines: VecDeque<Line>,
    pub unread: usize,
}
#[derive(Default)]
pub struct Whispers {
    /// Most recent conversation first.
    pub conversations: Vec<Conversation>,
    pub selected: Option<String>,
    seen_ids: VecDeque<String>,
}
impl Whispers {
    fn conversation(&mut self, login: &str, name: &str) -> &mut Conversation {
        let index = match self.conversations.iter().position(|c| c.login == login) {
            Some(index) => index,
            None => {
                if self.conversations.len() >= CONVERSATIONS {
                    self.conversations.pop();
                }
                self.conversations.push(Conversation {
                    login: login.to_owned(),
                    name: if name.is_empty() {
                        login.to_owned()
                    } else {
                        name.to_owned()
                    },
                    lines: VecDeque::new(),
                    unread: 0,
                });
                self.conversations.len() - 1
            }
        };
        // Move to the top.
        let conversation = self.conversations.remove(index);
        self.conversations.insert(0, conversation);
        &mut self.conversations[0]
    }
    /// Returns false for a duplicate delivery.
    pub fn incoming(
        &mut self,
        id: &str,
        login: &str,
        name: &str,
        text: &str,
        reading: bool,
    ) -> bool {
        if !id.is_empty() {
            if self.seen_ids.iter().any(|s| s == id) {
                return false;
            }
            if self.seen_ids.len() >= 256 {
                self.seen_ids.pop_front();
            }
            self.seen_ids.push_back(id.to_owned());
        }
        let selected = self.selected.as_deref() == Some(login);
        let conversation = self.conversation(login, name);
        if !name.is_empty() {
            conversation.name = name.to_owned();
        }
        push(conversation, false, text);
        if !(reading && selected) {
            conversation.unread += 1;
        }
        true
    }
    /// Start an empty conversation (the user typed a login).
    pub fn outgoing_start(&mut self, login: &str) {
        self.conversation(login, "");
    }
    pub fn outgoing(&mut self, login: &str, text: &str) {
        let conversation = self.conversation(login, "");
        push(conversation, true, text);
    }
    pub fn unread(&self) -> usize {
        self.conversations.iter().map(|c| c.unread).sum()
    }
    pub fn select(&mut self, login: &str) {
        self.selected = Some(login.to_owned());
        if let Some(c) = self.conversations.iter_mut().find(|c| c.login == login) {
            c.unread = 0;
        }
    }
}
fn push(conversation: &mut Conversation, mine: bool, text: &str) {
    if conversation.lines.len() >= MESSAGES {
        conversation.lines.pop_front();
    }
    conversation.lines.push_back(Line {
        at: chrono::Local::now(),
        mine,
        text: text.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversations_order_dedup_and_unread() {
        let mut w = Whispers::default();
        assert!(w.incoming("1", "alice", "Alice", "hi", false));
        assert!(!w.incoming("1", "alice", "Alice", "hi", false));
        w.incoming("2", "bob", "Bob", "yo", false);
        assert_eq!(w.conversations[0].login, "bob");
        assert_eq!(w.unread(), 2);
        w.select("alice");
        assert_eq!(w.unread(), 1);
        w.outgoing("alice", "hey");
        assert_eq!(w.conversations[0].login, "alice");
        assert!(w.conversations[0].lines.back().unwrap().mine);
    }
}
