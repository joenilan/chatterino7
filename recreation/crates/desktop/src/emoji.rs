//! Emoji: `:shortcode:` completion and the emoji tabs of the emote picker.
//! Data: emoji-data (MIT, `assets/emoji-LICENSE.txt`), trimmed to
//! `assets/emoji.tsv` (emoji, short names, category) in emoji-data's order.
use crate::twitch_assets::Choice;
use std::sync::OnceLock;

pub struct Emoji {
    pub text: &'static str,
    pub names: Vec<&'static str>,
    pub category: &'static str,
}

const PROVIDER: &str = "Emoji";

pub fn all() -> &'static [Emoji] {
    static ALL: OnceLock<Vec<Emoji>> = OnceLock::new();
    ALL.get_or_init(|| {
        include_str!("../assets/emoji.tsv")
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let text = parts.next()?;
                let names = parts
                    .next()?
                    .split(',')
                    .filter(|n| !n.is_empty())
                    .collect::<Vec<_>>();
                let category = parts.next()?;
                (!names.is_empty()).then_some(Emoji {
                    text,
                    names,
                    category,
                })
            })
            .collect()
    })
}

pub fn is_emoji(choice: &Choice) -> bool {
    choice.provider == PROVIDER
}

fn choice(emoji: &Emoji) -> Choice {
    Choice {
        label: emoji.text.to_owned(),
        provider: PROVIDER,
        key: None,
    }
}

/// Emoji whose short name starts with `query` (then ones containing it), for `:` completion.
pub fn search(query: &str, limit: usize) -> Vec<Choice> {
    let query = query.trim_end_matches(':').to_ascii_lowercase();
    if query.len() < 2 || limit == 0 {
        return Vec::new();
    }
    let starts = all()
        .iter()
        .filter(|e| e.names.iter().any(|n| n.starts_with(&query)));
    let contains = all().iter().filter(|e| {
        !e.names.iter().any(|n| n.starts_with(&query)) && e.names.iter().any(|n| n.contains(&query))
    });
    starts.chain(contains).take(limit).map(choice).collect()
}

/// Whether an emoji's short names contain `query` (picker search).
pub fn name_matches(text: &str, query: &str) -> bool {
    all()
        .iter()
        .find(|e| e.text == text)
        .is_some_and(|e| e.names.iter().any(|n| n.contains(query)))
}
/// The `:name:` for an emoji (shown on hover in the picker and completion list).
pub fn name_of(text: &str) -> Option<&'static str> {
    all()
        .iter()
        .find(|e| e.text == text)
        .and_then(|e| e.names.first().copied())
}

/// One picker tab per emoji category, in emoji-data's order.
pub fn collections() -> Vec<crate::emote_picker::Collection> {
    let mut out: Vec<crate::emote_picker::Collection> = Vec::new();
    for emoji in all() {
        let item = crate::emote_picker::BrowserChoice {
            choice: choice(emoji),
            origin: emoji.category.to_owned(),
            available: true,
            saved_key: None,
        };
        match out.last_mut() {
            Some(last) if last.title == emoji.category => last.items.push(item),
            _ => out.push(crate::emote_picker::Collection {
                id: format!(
                    "emoji-{}",
                    emoji.category.to_ascii_lowercase().replace([' ', '&'], "-")
                ),
                title: emoji.category.to_owned(),
                user: None,
                items: vec![item],
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_and_search() {
        assert!(all().len() > 1800);
        let found = search("joy", 5);
        assert_eq!(found[0].label, "😂");
        assert!(search("thumbsup", 3).iter().any(|c| c.label == "👍"));
        assert!(search("x", 5).is_empty(), "single letters don't complete");
        assert_eq!(name_of("🔥"), Some("fire"));
        let tabs = collections();
        assert!(tabs.iter().any(|c| c.title == "Smileys & Emotion"));
    }
}

/// Settings › Appearance › Emoji style (Chatterino's): draw emoji with
/// Windows' font, or as images from the emoji-datasource sets.
pub const STYLES: [&str; 5] = ["System", "Twitter", "Google", "Apple", "Facebook"];
pub static STYLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const SETS: [&str; 5] = ["", "twitter", "google", "apple", "facebook"];
/// The emoji-datasource version the image URLs point at.
const DATASOURCE: &str = "15.1.2";
/// Shown at about the height of chat text.
const SIZE: u16 = 20;

/// emoji-datasource's file name: code points as hex, joined by '-'.
fn file_stem(text: &str) -> String {
    text.chars()
        .map(|c| format!("{:04x}", c as u32))
        .collect::<Vec<_>>()
        .join("-")
}
fn is_skin_tone(c: char) -> bool {
    ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}
/// Every emoji text Jawjack knows, and also each without its U+FE0F
/// (chat often leaves it out) for symbols from U+2600 up, so (c), (r) and
/// TM stay text. Values are the file stems; the second is the longest key.
fn lookup() -> &'static (std::collections::HashMap<String, String>, usize) {
    static LOOKUP: OnceLock<(std::collections::HashMap<String, String>, usize)> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for emoji in all() {
            let stem = file_stem(emoji.text);
            let bare: String = emoji.text.chars().filter(|c| *c != '\u{FE0F}').collect();
            if bare != emoji.text
                && !bare.is_empty()
                && bare.chars().next().is_some_and(|c| c as u32 >= 0x2600)
            {
                map.entry(bare).or_insert_with(|| stem.clone());
            }
            map.insert(emoji.text.to_owned(), stem);
        }
        let longest = map.keys().map(String::len).max().unwrap_or(0);
        (map, longest)
    })
}
/// The image for an emoji file stem in the chosen style.
fn image(stem: &str, style: usize) -> chat_core::EmoteAsset {
    let set = SETS[style];
    let url = format!(
        "https://cdn.jsdelivr.net/npm/emoji-datasource-{set}@{DATASOURCE}/img/{set}/64/{stem}.png"
    );
    chat_core::EmoteAsset {
        static_url: url.clone(),
        url,
        width: SIZE,
        height: SIZE,
    }
}
/// Split emoji in a message's text into image pieces (when a style other
/// than System is chosen). Copying still gives the emoji characters.
pub fn to_images(message: &mut chat_core::Message) {
    let style = STYLE
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(SETS.len() - 1);
    if style == 0 {
        return;
    }
    let (map, longest) = lookup();
    let mut fragments = Vec::with_capacity(message.fragments.len());
    for fragment in std::mem::take(&mut message.fragments) {
        let chat_core::Fragment::Text(text) = fragment else {
            fragments.push(fragment);
            continue;
        };
        let mut plain = String::new();
        let mut at = 0;
        while at < text.len() {
            // The longest known emoji starting here.
            let mut found = None;
            let mut end = (at + longest).min(text.len());
            while end > at {
                if text.is_char_boundary(end)
                    && let Some(stem) = map.get(&text[at..end])
                {
                    found = Some((end, stem.clone()));
                    break;
                }
                end -= 1;
            }
            let Some((mut end, mut stem)) = found else {
                let c = text[at..].chars().next().expect("in bounds");
                plain.push(c);
                at += c.len_utf8();
                continue;
            };
            if let Some(tone) = text[end..].chars().next().filter(|c| is_skin_tone(*c)) {
                stem = format!("{stem}-{:04x}", tone as u32);
                end += tone.len_utf8();
            }
            if !plain.is_empty() {
                fragments.push(chat_core::Fragment::Text(std::mem::take(&mut plain)));
            }
            fragments.push(chat_core::Fragment::Emote {
                provider: "emoji".into(),
                id: stem.clone(),
                label: text[at..end].to_owned(),
                overlay: false,
                animated: false,
                asset: Some(image(&stem, style)),
            });
            at = end;
        }
        if !plain.is_empty() {
            fragments.push(chat_core::Fragment::Text(plain));
        }
    }
    message.fragments = fragments;
}

#[cfg(test)]
mod image_tests {
    use super::{STYLE, to_images};
    #[test]
    fn emoji_become_images_and_copy_back() {
        let mut message = chat_core::fixture("c", 0);
        message.fragments = vec![chat_core::Fragment::Text(
            "hi \u{2764} and \u{1F44D}\u{1F3FD} (c) \u{00A9} #1".into(),
        )];
        let before = message.body();
        STYLE.store(1, std::sync::atomic::Ordering::Relaxed);
        to_images(&mut message);
        STYLE.store(0, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(message.body(), before, "copying gives the same text");
        let stems: Vec<_> = message
            .fragments
            .iter()
            .filter_map(|f| match f {
                chat_core::Fragment::Emote { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            stems,
            vec!["2764-fe0f".to_owned(), "1f44d-1f3fd".to_owned()]
        );
    }
}
