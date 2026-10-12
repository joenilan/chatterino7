//! Repeated-message suppression (Chatterino's "similar messages"): a chatter's
//! message that is nearly the same as one of their recent ones is dimmed or
//! hidden. Similarity is 1 − edit distance / longer length, on lowercase text.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

/// How similar counts as a repeat by default (Chatterino's is 0.9).
pub const THRESHOLD: f32 = 0.9;
/// Settings → Chat → Repeated messages, each an index into its choices.
pub const PERCENTS: [u8; 4] = [70, 80, 90, 95];
pub const WINDOWS: [i64; 4] = [10, 30, 60, 300];
pub const COUNTS: [usize; 4] = [1, 3, 5, 10];
pub static PERCENT: AtomicUsize = AtomicUsize::new(2);
pub static WINDOW: AtomicUsize = AtomicUsize::new(2);
pub static COUNT: AtomicUsize = AtomicUsize::new(2);
/// [same chatter only, include your own messages, repeats can highlight].
pub const FLAG_NAMES: [&str; 3] = ["same_user", "own", "highlight"];
pub static FLAGS: [AtomicBool; 3] = [
    AtomicBool::new(true),
    AtomicBool::new(false),
    AtomicBool::new(false),
];
pub fn flag(index: usize) -> bool {
    FLAGS[index].load(Relaxed)
}
fn pick<T: Copy>(choices: &[T], index: &AtomicUsize) -> T {
    choices[index.load(Relaxed).min(choices.len() - 1)]
}
fn threshold() -> f32 {
    pick(&PERCENTS, &PERCENT) as f32 / 100.
}
/// Longer messages are compared by their first part only (bounded cost).
const MAX_CHARS: usize = 300;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Show,
    Dim,
    Hide,
}
impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Show, Mode::Dim, Mode::Hide];
    pub fn key(self) -> &'static str {
        match self {
            Mode::Show => "show",
            Mode::Dim => "dim",
            Mode::Hide => "hide",
        }
    }
    pub fn from_key(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|m| m.key() == key)
            .unwrap_or_default()
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Show => "Show",
            Mode::Dim => "Dim",
            Mode::Hide => "Hide",
        }
    }
}

pub fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.to_lowercase().chars().take(MAX_CHARS).collect();
    let b: Vec<char> = b.to_lowercase().chars().take(MAX_CHARS).collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.;
    }
    // Lengths alone can rule a pair out before the quadratic distance.
    if (a.len().min(b.len()) as f32) / (longest as f32) < threshold().min(THRESHOLD) {
        return a.len().min(b.len()) as f32 / longest as f32;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    1. - previous[b.len()] as f32 / longest as f32
}

/// Whether `message` repeats a recent message in `recent` (newest last): its
/// author's own, or anyone's when "same chatter only" is off. `own` is the
/// signed-in user ID, whose messages are left alone unless asked.
pub fn is_repeat<'a>(
    message: &chat_core::Message,
    recent: impl DoubleEndedIterator<Item = &'a chat_core::Message>,
    own: Option<&str>,
) -> bool {
    let (Some(at), body) = (message.sent_at, message.body()) else {
        return false;
    };
    if body.trim().is_empty() || message.presentation.notice.is_some() {
        return false;
    }
    if !flag(1) && own.is_some_and(|id| id == message.user_id) {
        return false;
    }
    let same_user = flag(0);
    let window = pick(&WINDOWS, &WINDOW);
    let threshold = threshold();
    recent
        .rev()
        .take(200)
        .filter(|m| {
            (!same_user || m.user_id == message.user_id)
                && !m.user_id.is_empty()
                && !m.deleted
                && m.id != message.id
                && m.presentation.notice.is_none()
        })
        .take_while(|m| m.sent_at.is_some_and(|t| at - t <= window))
        .take(pick(&COUNTS, &COUNT))
        .any(|m| similarity(&m.body(), &body) >= threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn similar_texts() {
        assert!(
            similarity(
                "buy followers at example dot com",
                "buy followers at example dot co"
            ) >= THRESHOLD
        );
        assert!(similarity("hello there", "hello there") == 1.);
        assert!(similarity("gg", "this was a great game") < THRESHOLD);
        assert!(similarity("LUL LUL LUL", "lul lul lul") == 1.);
        assert_eq!(Mode::from_key("dim"), Mode::Dim);
    }
    #[test]
    fn repeats_by_chatter_and_own() {
        let say = |id: &str, user: &str, at: i64, text: &str| {
            let mut m = chat_core::fixture("a", 0);
            m.id = id.into();
            m.user_id = user.into();
            m.sent_at = Some(at);
            m.fragments = vec![chat_core::Fragment::Text(text.into())];
            m
        };
        let earlier = [say("1", "bob", 100, "buy cheap followers now")];
        // Same chatter only (the default): another chatter's copy is fine.
        assert!(!is_repeat(
            &say("2", "amy", 110, "buy cheap followers now"),
            earlier.iter(),
            None
        ));
        assert!(is_repeat(
            &say("3", "bob", 110, "buy cheap followers now!"),
            earlier.iter(),
            None
        ));
        // Your own repeats are left alone unless asked.
        assert!(!is_repeat(
            &say("4", "bob", 110, "buy cheap followers now"),
            earlier.iter(),
            Some("bob")
        ));
        // Outside the window: not a repeat.
        assert!(!is_repeat(
            &say("5", "bob", 200, "buy cheap followers now"),
            earlier.iter(),
            None
        ));
    }
}
