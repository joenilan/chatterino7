//! Your own moderator buttons (Chatterino's moderation buttons): a short label
//! and a moderation command template. `{user}`, `{msg-id}` and `{channel}` are
//! filled from the message. Any /command is accepted (Twitch's, Jawjack's or
//! your custom commands), never plain text, so a button can't post chat by
//! itself.
use serde_json::{Value, json};

pub const MAX: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct ModButton {
    pub label: String,
    pub command: String,
}

/// Suggested commands (any /command works).
const ALLOWED: [&str; 10] = [
    "/timeout",
    "/untimeout",
    "/ban",
    "/unban",
    "/delete",
    "/warn",
    "/vip",
    "/unvip",
    "/shoutout",
    "/clear",
];

/// Icons a button can show instead of text: its label is `:name`
/// (Chatterino lets mod buttons use images).
pub const ICONS: [(&str, gpui_kit::assets::IconName); 14] = {
    use gpui_kit::assets::IconName as I;
    [
        ("clock", I::Clock),
        ("timer", I::Timer),
        ("ban", I::Ban),
        ("gavel", I::Gavel),
        ("trash", I::Trash),
        ("user-x", I::UserX),
        ("eye-off", I::EyeOff),
        ("shield", I::Shield),
        ("swords", I::Swords),
        ("lock", I::Lock),
        ("pin", I::Pin),
        ("check", I::Check),
        ("sparkles", I::Sparkles),
        ("mic-off", I::MicOff),
    ]
};
/// The icon for a label like `:ban`.
pub fn icon(label: &str) -> Option<gpui_kit::assets::IconName> {
    let name = label.strip_prefix(':')?;
    ICONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, icon)| icon.clone())
}

pub fn validate(label: &str, command: &str) -> Result<ModButton, String> {
    let label = label.trim();
    let command = command.trim();
    if label.starts_with(':') && icon(label).is_none() {
        return Err(format!(
            "Icons: {}.",
            ICONS
                .iter()
                .map(|(n, _)| format!(":{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if label.is_empty() || (icon(label).is_none() && label.chars().count() > 8) {
        return Err("Give the button a label of 1–8 characters, or an icon like :ban.".into());
    }
    let name = command
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !name.starts_with('/') || name.len() < 2 {
        return Err(format!(
            "Use a /command, such as {}, or one of your custom commands.",
            ALLOWED.join(", ")
        ));
    }
    if command.chars().count() > 300 {
        return Err("Keep the command under 300 characters.".into());
    }
    Ok(ModButton {
        label: label.to_owned(),
        command: command.to_owned(),
    })
}

/// The command with placeholders filled, split into (name, arguments).
pub fn expand(command: &str, login: &str, message_id: &str, channel: &str) -> (String, String) {
    let filled = command
        .replace("{user}", login)
        .replace("{msg-id}", message_id)
        .replace("{channel}", channel);
    let mut parts = filled.trim().splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").to_ascii_lowercase();
    (name, parts.next().unwrap_or("").trim().to_owned())
}

pub fn load(value: &Value) -> Vec<ModButton> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| validate(b["label"].as_str()?, b["command"].as_str()?).ok())
        .take(MAX)
        .collect()
}
pub fn json(buttons: &[ModButton]) -> Value {
    Value::Array(
        buttons
            .iter()
            .map(|b| json!({"label": b.label, "command": b.command}))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validate_and_expand() {
        assert!(validate("1h", "/timeout {user} 3600 spam").is_ok());
        assert!(
            validate("hi", "hello chat").is_err(),
            "plain chat is refused"
        );
        assert!(validate("", "/ban {user}").is_err());
        assert_eq!(
            expand("/timeout {user} 3600 spam", "bob", "m1", "c"),
            ("/timeout".to_owned(), "bob 3600 spam".to_owned())
        );
        assert_eq!(expand("/delete {msg-id}", "bob", "m1", "c").1, "m1");
    }
}

#[cfg(test)]
mod icon_tests {
    use super::{icon, validate};
    #[test]
    fn icon_labels() {
        assert!(icon(":gavel").is_some());
        assert!(icon("gavel").is_none());
        assert!(validate(":gavel", "/ban {user}").is_ok());
        assert!(
            validate(":sparkles", "/vip {user}").is_ok(),
            "icons may be longer than 8"
        );
        assert!(validate(":nope", "/ban {user}").is_err());
    }
}
