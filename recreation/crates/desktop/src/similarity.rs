//! Repeated-message suppression (Chatterino's "similar messages"): a chatter's
//! message that is nearly the same as one of their recent ones is dimmed or
//! hidden. Similarity is 1 − edit distance / longer length, on lowercase text.

/// How similar counts as a repeat (Chatterino's default is 0.9).
pub const THRESHOLD: f32 = 0.9;
/// Only messages this recent are compared.
pub const WINDOW_SECONDS: i64 = 60;
/// At most this many earlier messages from the same chatter are compared.
pub const CHECKED: usize = 5;
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
    if (a.len().min(b.len()) as f32) / (longest as f32) < THRESHOLD {
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

/// Whether `message` repeats one of its author's recent messages in `recent`
/// (newest last).
pub fn is_repeat<'a>(
    message: &chat_core::Message,
    recent: impl DoubleEndedIterator<Item = &'a chat_core::Message>,
) -> bool {
    let (Some(at), body) = (message.sent_at, message.body()) else {
        return false;
    };
    if body.trim().is_empty() || message.presentation.notice.is_some() {
        return false;
    }
    recent
        .rev()
        .take(200)
        .filter(|m| m.user_id == message.user_id && !m.deleted && m.id != message.id)
        .take_while(|m| m.sent_at.is_some_and(|t| at - t <= WINDOW_SECONDS))
        .take(CHECKED)
        .any(|m| similarity(&m.body(), &body) >= THRESHOLD)
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
}
