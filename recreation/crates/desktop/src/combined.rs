//! Combined feeds: one pane that shows several channels interleaved. Its
//! name is "+a,b,c" (never a valid Twitch login, so it can't leak into
//! Twitch requests); each message is tagged with its origin, and sending
//! goes to one explicitly chosen channel.

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
pub fn is_combined(name: &str) -> bool {
    name.starts_with('+')
}
/// "a + b + c" for headers and tabs.
pub fn title(name: &str) -> String {
    if is_combined(name) {
        sources(name).join(" + ")
    } else {
        name.to_owned()
    }
}
/// A saved combined name ("+a,b"), checked again on load.
pub fn from_saved(name: &str) -> Option<String> {
    parse(&name.strip_prefix('+')?.replace(',', "+"), |s| {
        chat_core::twitch_login(s).filter(|l| l == s)
    })
}
/// A combined pane name from user input like "a+b" or "a, b c" (2–8 channels).
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
    }
}
