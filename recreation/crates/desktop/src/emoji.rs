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
