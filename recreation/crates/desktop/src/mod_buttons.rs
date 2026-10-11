//! Your own moderator buttons (Chatterino's moderation buttons): a short label
//! and a moderation command template. `{user}`, `{msg-id}` and `{channel}` are
//! filled from the message. Only Twitch moderation commands are accepted, so
//! a button can never post plain chat.
use serde_json::{Value, json};

pub const MAX: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct ModButton {
    pub label: String,
    pub command: String,
}

/// Commands a button may run.
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

pub fn validate(label: &str, command: &str) -> Result<ModButton, String> {
    let label = label.trim();
    let command = command.trim();
    if label.is_empty() || label.chars().count() > 8 {
        return Err("Give the button a label of 1–8 characters.".into());
    }
    let name = command
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !ALLOWED.contains(&name.as_str()) {
        return Err(format!("Use a moderation command: {}.", ALLOWED.join(", ")));
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
