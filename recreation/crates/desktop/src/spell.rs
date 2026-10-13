//! Spellcheck for the message box. English is built in (SCOWL-derived
//! Hunspell dictionary; assets/dict/LICENSE-en_US.txt); other languages are
//! Hunspell dictionaries from the `dictionary-*` npm packages, downloaded from
//! jsDelivr on first use and kept in the settings folder. Chat is full of
//! names, emotes and slang, so only plain lowercase-ish words are checked and
//! common chat words are always accepted.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Settings › Spellcheck language: (name, npm package suffix, version).
pub const LANGUAGES: [(&str, &str, &str); 10] = [
    ("English", "", ""),
    ("German", "de", "3.0.0"),
    ("Spanish", "es", "4.0.0"),
    ("French", "fr", "3.0.0"),
    ("Portuguese (Brazil)", "pt", "4.0.0"),
    ("Portuguese (Portugal)", "pt-pt", "2.0.0"),
    ("Italian", "it", "2.0.0"),
    ("Dutch", "nl", "2.0.0"),
    ("Polish", "pl", "2.0.0"),
    ("Swedish", "sv", "4.0.0"),
];
pub static LANGUAGE: AtomicUsize = AtomicUsize::new(0);
/// The loaded dictionary: (language index, dictionary or None if it failed).
static CURRENT: Mutex<Option<(usize, Option<Arc<spellbook::Dictionary>>)>> = Mutex::new(None);
/// The language being loaded (usize::MAX when none).
static LOADING: AtomicUsize = AtomicUsize::new(usize::MAX);
const MAX_FILE: usize = 16 * 1024 * 1024;

/// The chosen language's dictionary, loaded in the background; None until
/// ready (or if it couldn't be loaded).
fn dictionary() -> Option<Arc<spellbook::Dictionary>> {
    let want = LANGUAGE.load(Ordering::Relaxed).min(LANGUAGES.len() - 1);
    if let Ok(current) = CURRENT.lock()
        && let Some((index, dictionary)) = current.as_ref()
        && *index == want
    {
        return dictionary.clone();
    }
    if LOADING.swap(want, Ordering::Relaxed) != want {
        let _ = std::thread::Builder::new()
            .name("jawjack-spell".into())
            .spawn(move || {
                let loaded = load(want).ok().map(Arc::new);
                if let Ok(mut current) = CURRENT.lock() {
                    *current = Some((want, loaded));
                }
                let _ = LOADING.compare_exchange(
                    want,
                    usize::MAX,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
            });
    }
    None
}
fn load(index: usize) -> Result<spellbook::Dictionary, String> {
    let (_, code, version) = LANGUAGES[index];
    if code.is_empty() {
        return spellbook::Dictionary::new(
            include_str!("../assets/dict/en_US.aff"),
            include_str!("../assets/dict/en_US.dic"),
        )
        .map_err(|e| e.to_string());
    }
    let aff = fetch(code, version, "aff")?;
    let dic = fetch(code, version, "dic")?;
    spellbook::Dictionary::new(&aff, &dic).map_err(|e| e.to_string())
}
/// A dictionary file from the cache, or downloaded into it.
fn fetch(code: &str, version: &str, ext: &str) -> Result<String, String> {
    let cache = crate::profile::data_root().map(|root| {
        root.join("dictionaries")
            .join(format!("{code}-{version}.{ext}"))
    });
    if let Some(path) = &cache
        && let Ok(text) = std::fs::read_to_string(path)
    {
        return Ok(text);
    }
    let url = format!("https://cdn.jsdelivr.net/npm/dictionary-{code}@{version}/index.{ext}");
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("dictionary too large".into());
    }
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| "not UTF-8".to_string())?;
    if let Some(path) = &cache {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, &text);
    }
    Ok(text)
}

/// Your own words (Spelling → Add), lowercase; saved with the workspace.
pub static PERSONAL: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());
pub fn add_personal(word: &str) {
    if let Ok(mut words) = PERSONAL.lock()
        && words.len() < 2000
    {
        words.insert(word.to_lowercase());
    }
}
fn personal(word: &str) -> bool {
    PERSONAL
        .lock()
        .is_ok_and(|w| w.contains(&word.to_lowercase()))
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
            || personal(word)
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
        *CURRENT.lock().unwrap() = Some((0, load(0).ok().map(Arc::new)));
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

    /// Every downloadable language parses and knows a common word (network).
    #[test]
    #[ignore]
    fn downloaded_languages() {
        let words = [
            "", "Haus", "casa", "maison", "casa", "casa", "casa", "huis", "dom", "hus",
        ];
        for (index, word) in words.iter().enumerate().skip(1) {
            let (_, code, version) = LANGUAGES[index];
            let aff = fetch_uncached(code, version, "aff");
            let dic = fetch_uncached(code, version, "dic");
            let dictionary = spellbook::Dictionary::new(&aff, &dic).expect(code);
            assert!(dictionary.check(word), "{code}: {word}");
            assert!(!dictionary.check("qzxqzxq"), "{code}");
        }
    }
    fn fetch_uncached(code: &str, version: &str, ext: &str) -> String {
        reqwest::blocking::get(format!(
            "https://cdn.jsdelivr.net/npm/dictionary-{code}@{version}/index.{ext}"
        ))
        .unwrap()
        .text()
        .unwrap()
    }
}
