//! Mention styling: @mentions in the mentioned chatter's colour, and (optional)
//! chatter names written without the @ turned into mentions, like Chatterino.
//! Learns names and colours from messages as they arrive.
use chat_core::{Fragment, Message};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

/// Settings → Appearance → Mentions: colour @mentions like the chatter's name.
pub static COLOR_MENTIONS: AtomicBool = AtomicBool::new(true);
/// Settings → Appearance → Mentions: treat a chatter's bare name as a mention.
pub static FIND_NAMES: AtomicBool = AtomicBool::new(false);
/// Remember this many chatters; past it, start over.
const LIMIT: usize = 20_000;

#[derive(Default)]
struct Seen {
    colors: HashMap<String, u32>,
    logins: HashMap<String, (String, String)>,
}
thread_local! {
    static SEEN: RefCell<Seen> = RefCell::new(Seen::default());
}

/// Learn the author's name and the colour their name is drawn in.
pub fn note(message: &Message) {
    let (Some(login), Some(color)) = (message.login.as_ref(), message.author_color()) else {
        return;
    };
    if message.user_id.is_empty() {
        return;
    }
    SEEN.with_borrow_mut(|seen| {
        if seen.logins.len() > LIMIT {
            *seen = Seen::default();
        }
        seen.colors.insert(message.user_id.clone(), color);
        seen.logins.insert(
            login.to_lowercase(),
            (message.user_id.clone(), message.display_name.clone()),
        );
    });
}
/// The mentioned chatter's colour, when that's on and they've been seen.
pub fn color(user_id: &str) -> Option<u32> {
    if !COLOR_MENTIONS.load(Ordering::Relaxed) {
        return None;
    }
    SEEN.with_borrow(|seen| seen.colors.get(user_id).copied())
}
/// Known chatters' "@name" in plain text (history arrives without mention
/// data) and, with "Find names without @", their bare names become mentions.
/// The text is unchanged, so copying is too.
pub fn link_names(message: &mut Message) {
    let bare = FIND_NAMES.load(Ordering::Relaxed);
    let fragments = std::mem::take(&mut message.fragments);
    message.fragments = SEEN.with_borrow(|seen| split(fragments, &seen.logins, bare));
}
fn split(
    fragments: Vec<Fragment>,
    logins: &HashMap<String, (String, String)>,
    bare: bool,
) -> Vec<Fragment> {
    let mut out = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        let Fragment::Text(text) = fragment else {
            out.push(fragment);
            continue;
        };
        let mut start = 0;
        let mut word_start = None;
        let bytes: Vec<(usize, char)> = text.char_indices().collect();
        for (index, (at, ch)) in bytes.iter().enumerate() {
            let in_word = ch.is_alphanumeric() || *ch == '_';
            if in_word && word_start.is_none() {
                word_start = Some(*at);
            }
            let ends = !in_word || index + 1 == bytes.len();
            if let (true, Some(from)) = (ends, word_start) {
                let to = if in_word { at + ch.len_utf8() } else { *at };
                let word = &text[from..to];
                // "@name" always; a bare name only when asked; never short words.
                let at = text[..from].ends_with('@');
                if word.len() >= 3
                    && (at || bare)
                    && let Some((user_id, display_name)) = logins.get(&word.to_lowercase())
                {
                    let from = if at { from - 1 } else { from };
                    if from > start {
                        out.push(Fragment::Text(text[start..from].to_owned()));
                    }
                    out.push(Fragment::Mention {
                        user_id: user_id.clone(),
                        login: word.to_lowercase(),
                        display_name: display_name.clone(),
                        label: text[from..to].to_owned(),
                    });
                    start = to;
                }
                word_start = None;
            }
        }
        if start < text.len() {
            out.push(Fragment::Text(text[start..].to_owned()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Fragment, HashMap, split};
    #[test]
    fn names_without_at_become_mentions() {
        let mut logins = HashMap::new();
        logins.insert("alice".to_owned(), ("1".to_owned(), "Alice".to_owned()));
        let out = split(
            vec![Fragment::Text("hi Alice, and @alice too; malice".into())],
            &logins,
            true,
        );
        let shape: Vec<String> = out
            .iter()
            .map(|f| match f {
                Fragment::Mention { label, .. } => format!("[{label}]"),
                other => other.copy_text().to_owned(),
            })
            .collect();
        assert_eq!(
            shape,
            ["hi ", "[Alice]", ", and ", "[@alice]", " too; malice"]
        );
        let at_only = split(vec![Fragment::Text("Alice @Alice".into())], &logins, false);
        assert_eq!(at_only.len(), 2);
        let text: String = out.iter().map(|f| f.copy_text()).collect();
        assert_eq!(text, "hi Alice, and @alice too; malice");
    }
}
