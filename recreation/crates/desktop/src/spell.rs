//! Spellcheck for the message box (English, SCOWL-derived Hunspell
//! dictionary; assets/dict/LICENSE-en_US.txt). Chat is full of names, emotes
//! and slang, so only plain lowercase-ish words are checked and common chat
//! words are always accepted.
use std::sync::OnceLock;

static DICTIONARY: OnceLock<Option<spellbook::Dictionary>> = OnceLock::new();

/// Load in the background on first use; None until ready (or if it failed).
fn dictionary() -> Option<&'static spellbook::Dictionary> {
    if DICTIONARY.get().is_none() {
        static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            let _ = std::thread::Builder::new()
                .name("jawjack-spell".into())
                .spawn(|| {
                    let _ = DICTIONARY.set(
                        spellbook::Dictionary::new(
                            include_str!("../assets/dict/en_US.aff"),
                            include_str!("../assets/dict/en_US.dic"),
                        )
                        .ok(),
                    );
                });
        }
        return None;
    }
    DICTIONARY.get().and_then(Option::as_ref)
}

/// Common chat words a dictionary doesn't know.
const CHAT_WORDS: &[&str] = &[
    "lol",
    "lmao",
    "lmfao",
    "rofl",
    "gg",
    "ggs",
    "wp",
    "glhf",
    "gl",
    "hf",
    "pog",
    "poggers",
    "kek",
    "kekw",
    "omg",
    "omegalul",
    "btw",
    "imo",
    "imho",
    "tbh",
    "idk",
    "idc",
    "irl",
    "afk",
    "brb",
    "gtg",
    "ty",
    "tyty",
    "thx",
    "np",
    "yw",
    "pls",
    "plz",
    "ok",
    "okay",
    "yeah",
    "yea",
    "yep",
    "nah",
    "nope",
    "hehe",
    "haha",
    "hahaha",
    "lul",
    "pepega",
    "sadge",
    "copium",
    "hopium",
    "chat",
    "streamer",
    "stream",
    "streams",
    "streaming",
    "vod",
    "vods",
    "clip",
    "clips",
    "emote",
    "emotes",
    "sub",
    "subs",
    "subbed",
    "resub",
    "bits",
    "mod",
    "mods",
    "modded",
    "vip",
    "vips",
    "raid",
    "raided",
    "lurk",
    "lurking",
    "lurker",
    "ez",
    "w",
    "l",
    "ngl",
    "fr",
    "ong",
    "rn",
    "smh",
    "wtf",
    "af",
    "tho",
    "gonna",
    "wanna",
    "gotta",
    "kinda",
    "sorta",
    "yall",
    "y'all",
    "dm",
    "dms",
    "ttv",
    "twitch",
    "youtube",
    "discord",
    "kick",
    "obs",
    "fps",
    "noob",
    "nerf",
    "buff",
    "op",
    "meta",
];

/// Words worth checking: letters (and inner apostrophes) only, not @mentions,
/// links, numbers, ALL-CAPS or MixedCase (usually emotes or names).
fn checkable(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    chars.len() >= 3
        && chars.iter().all(|c| c.is_alphabetic() || *c == '\'')
        && chars[0].is_alphabetic()
        && !chars[1..].iter().any(|c| c.is_uppercase())
}

/// Up to `limit` misspelled words in `text` with up to 3 suggestions each.
/// `known` (emote codes, chatter names, words you added) is never flagged.
pub fn check(text: &str, known: &dyn Fn(&str) -> bool, limit: usize) -> Vec<(String, Vec<String>)> {
    let Some(dictionary) = dictionary() else {
        return Vec::new();
    };
    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    for raw in text.split_whitespace() {
        if raw.starts_with(['@', '!', '/', ':'])
            || raw.contains("://")
            || raw.contains('.') && raw.len() > 4
        {
            continue;
        }
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'');
        let lower = word.to_lowercase();
        if !checkable(word)
            || CHAT_WORDS.contains(&lower.as_str())
            || known(word)
            || dictionary.check(word)
            || dictionary.check(&lower)
            || found.iter().any(|(w, _)| *w == word)
        {
            continue;
        }
        let mut suggestions = Vec::new();
        dictionary.suggest(word, &mut suggestions);
        suggestions.truncate(3);
        found.push((word.to_owned(), suggestions));
        if found.len() >= limit {
            break;
        }
    }
    found
}

/// Replace the first whole-word `word` in `text` with `with`.
pub fn replace_word(text: &str, word: &str, with: &str) -> String {
    let mut start = 0;
    while let Some(at) = text[start..].find(word).map(|i| i + start) {
        let before = text[..at].chars().last();
        let after = text[at + word.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if boundary(before) && boundary(after) {
            return format!("{}{}{}", &text[..at], with, &text[at + word.len()..]);
        }
        start = at + word.len();
    }
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spelling() {
        // Load synchronously for the test.
        let _ = DICTIONARY.set(
            spellbook::Dictionary::new(
                include_str!("../assets/dict/en_US.aff"),
                include_str!("../assets/dict/en_US.dic"),
            )
            .ok(),
        );
        let none = |_: &str| false;
        let found = check("teh stream was grate lol KEKW @bob https://x.y", &none, 5);
        let words: Vec<&str> = found.iter().map(|(w, _)| w.as_str()).collect();
        assert_eq!(words, vec!["teh"]);
        assert!(found[0].1.iter().any(|s| s == "the"), "{:?}", found[0].1);
        assert!(
            check("teh", &|w| w == "teh", 5).is_empty(),
            "known words are skipped"
        );
        assert_eq!(replace_word("teh tehx teh", "teh", "the"), "the tehx teh");
    }
}
