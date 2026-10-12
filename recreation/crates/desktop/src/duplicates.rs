//! Sending the same message twice (Twitch rejects an exact repeat within 30 s).
use std::time::{Duration, Instant};

/// Twitch drops a message identical to your previous one within 30 seconds.
/// Like Chatterino, add an invisible character so the repeat still goes through.
pub fn duplicate_safe(last: &mut Option<(String, Instant)>, text: String, youtube: bool) -> String {
    const MAGIC: &str = " \u{E0000}";
    let base = text.trim_end_matches(MAGIC).to_owned();
    let repeat = !youtube
        && last.as_ref().is_some_and(|(previous, at)| {
            previous.trim_end_matches(MAGIC) == base && at.elapsed() < Duration::from_secs(30)
        });
    let sent = if repeat && !last.as_ref().is_some_and(|(p, _)| p.ends_with(MAGIC)) {
        format!("{base}{MAGIC}")
    } else {
        base
    };
    *last = Some((sent.clone(), Instant::now()));
    sent
}
#[cfg(test)]
mod duplicate_tests {
    use super::*;
    #[test]
    fn repeats_alternate_an_invisible_suffix() {
        let mut last = None;
        assert_eq!(duplicate_safe(&mut last, "gg".into(), false), "gg");
        assert_eq!(
            duplicate_safe(&mut last, "gg".into(), false),
            "gg \u{E0000}"
        );
        assert_eq!(duplicate_safe(&mut last, "gg".into(), false), "gg");
        assert_eq!(duplicate_safe(&mut last, "hi".into(), false), "hi");
        assert_eq!(
            duplicate_safe(&mut last, "hi".into(), true),
            "hi",
            "YouTube allows repeats"
        );
    }
}
