//! Combined feeds: one pane that shows several channels interleaved, across
//! platforms (Twitch channels and YouTube streams). Its name is "+a,yt:id,c"
//! (never a valid Twitch login, so it can't leak into Twitch requests); each
//! message is tagged with its origin, the pane can show or hide each platform,
//! and sending goes to one explicitly chosen source.

/// A chat service a source belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Twitch,
    YouTube,
}
impl Platform {
    pub fn of(source: &str) -> Self {
        if crate::youtube::is_youtube(source) {
            Self::YouTube
        } else {
            Self::Twitch
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Twitch => "Twitch",
            Self::YouTube => "YouTube",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Twitch => "twitch",
            Self::YouTube => "youtube",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        [Self::Twitch, Self::YouTube]
            .into_iter()
            .find(|p| p.key() == key)
    }
}
/// The platforms among some sources, Twitch first.
pub fn platforms_of(sources: &[String]) -> Vec<Platform> {
    [Platform::Twitch, Platform::YouTube]
        .into_iter()
        .filter(|p| sources.iter().any(|s| Platform::of(s) == *p))
        .collect()
}
/// How a source reads in tags and menus: "#channel" or "YouTube".
pub fn label(source: &str) -> String {
    if crate::youtube::is_youtube(source) {
        "YouTube".into()
    } else {
        format!("#{source}")
    }
}

/// The real channels behind a pane name: itself, or a combined feed's sources.
pub fn sources(name: &str) -> Vec<String> {
    match name.strip_prefix('+') {
        Some(list) => list
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        None => vec![name.to_owned()],
    }
}
/// Special tabs that collect from everywhere instead of reading one chat
/// (Chatterino's /mentions and /whispers). They never connect to Twitch.
pub const MENTIONS: &str = "/mentions";
pub const WHISPERS: &str = "/whispers";
pub const LIVE: &str = "/live";
pub fn is_special(name: &str) -> bool {
    name.starts_with('/')
}
pub fn special_title(name: &str) -> Option<&'static str> {
    match name {
        MENTIONS => Some("Mentions"),
        WHISPERS => Some("Whispers"),
        LIVE => Some("Live"),
        _ => None,
    }
}
pub fn is_combined(name: &str) -> bool {
    name.starts_with('+')
}
/// "a + b + c" for headers and tabs.
pub fn title(name: &str) -> String {
    if is_combined(name) {
        sources(name)
            .iter()
            .map(|s| {
                if crate::youtube::is_youtube(s) {
                    "YouTube".to_owned()
                } else {
                    s.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" + ")
    } else if crate::youtube::is_youtube(name) {
        "YouTube stream".into()
    } else {
        name.to_owned()
    }
}
/// A saved combined name ("+a,b"), checked again on load.
pub fn from_saved(name: &str) -> Option<String> {
    parse(&name.strip_prefix('+')?.replace(',', "+"), |s| {
        chat_core::twitch_login(s)
            .filter(|l| l == s)
            .or_else(|| crate::youtube::parse(s).filter(|v| v == s))
    })
}
/// A combined pane name from user input like "a+b" or "a, b c" (2–8 sources).
pub fn parse(input: &str, valid: impl Fn(&str) -> Option<String>) -> Option<String> {
    if !input.contains(['+', ',']) {
        return None;
    }
    let mut channels: Vec<String> = Vec::new();
    for part in input
        .split(['+', ',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let channel = valid(part)?;
        if !channels.contains(&channel) {
            channels.push(channel);
        }
    }
    (2..=8)
        .contains(&channels.len())
        .then(|| format!("+{}", channels.join(",")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        let valid = |s: &str| chat_core::twitch_login(s);
        assert_eq!(
            parse("bessvibes + 3putt_", valid).as_deref(),
            Some("+bessvibes,3putt_")
        );
        assert_eq!(parse("a, b, a", valid).as_deref(), Some("+a,b"));
        assert_eq!(parse("solo", valid), None);
        assert_eq!(parse("a+", valid), None, "needs two channels");
        assert_eq!(sources("+a,b"), vec!["a", "b"]);
        assert_eq!(sources("a"), vec!["a"]);
        assert_eq!(title("+a,b"), "a + b");
        // Twitch and YouTube in one feed.
        let mixed = |s: &str| chat_core::twitch_login(s).or_else(|| crate::youtube::parse(s));
        let name = parse("dreadedzombie + https://youtu.be/dQw4w9WgXcQ", mixed).unwrap();
        assert_eq!(name, "+dreadedzombie,yt:dQw4w9WgXcQ");
        assert_eq!(from_saved(&name).as_deref(), Some(name.as_str()));
        assert_eq!(title(&name), "dreadedzombie + YouTube");
        assert_eq!(
            platforms_of(&sources(&name)),
            vec![Platform::Twitch, Platform::YouTube]
        );
        assert_eq!(platforms_of(&sources("+a,b")), vec![Platform::Twitch]);
        assert_eq!(label("yt:dQw4w9WgXcQ"), "YouTube");
        assert_eq!(Platform::from_key("youtube"), Some(Platform::YouTube));
    }
}
