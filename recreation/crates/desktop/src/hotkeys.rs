//! Customisable keyboard shortcuts (Chatterino's hotkey settings). Built-in
//! bindings stay registered; a custom key is bound after them (later bindings
//! win) and the key it replaces is disabled with NoAction, so text input
//! bindings from the UI kit are never cleared.
use gpui_kit::{App, KeyBinding, NoAction};
use std::{collections::BTreeMap, sync::Mutex};

pub struct Hotkey {
    pub id: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    context: &'static str,
    bind: fn(&str, Option<&str>) -> KeyBinding,
}

macro_rules! hotkey {
    ($id:literal, $label:literal, $default:literal, $context:literal, $action:expr) => {
        Hotkey {
            id: $id,
            label: $label,
            default: $default,
            context: $context,
            bind: |keys, context| KeyBinding::new(keys, $action, context),
        }
    };
}

pub const HOTKEYS: &[Hotkey] = {
    use crate::workspace::*;
    &[
        hotkey!(
            "switch-channel",
            "Switch channel",
            "ctrl-p",
            "ChatWorkspace",
            SwitchChannel
        ),
        hotkey!(
            "last-channel",
            "Back to the last channel",
            "alt-`",
            "ChatWorkspace",
            LastChannel
        ),
        hotkey!(
            "create-clip",
            "Create a clip of the channel you're in",
            "alt-x",
            "ChatWorkspace",
            CreateClip
        ),
        hotkey!("quit", "Save and quit", "ctrl-q", "ChatWorkspace", Quit),
        hotkey!(
            "run-command-1",
            "Run command 1",
            "",
            "ChatWorkspace",
            RunCommand1
        ),
        hotkey!(
            "run-command-2",
            "Run command 2",
            "",
            "ChatWorkspace",
            RunCommand2
        ),
        hotkey!(
            "run-command-3",
            "Run command 3",
            "",
            "ChatWorkspace",
            RunCommand3
        ),
        hotkey!(
            "run-command-4",
            "Run command 4",
            "",
            "ChatWorkspace",
            RunCommand4
        ),
        hotkey!(
            "add-channel",
            "Add a channel",
            "ctrl-k",
            "ChatWorkspace",
            AddChannel
        ),
        hotkey!(
            "next-workspace",
            "Next workspace",
            "ctrl-tab",
            "ChatWorkspace",
            NextTab
        ),
        hotkey!(
            "previous-workspace",
            "Previous workspace",
            "ctrl-shift-tab",
            "ChatWorkspace",
            PreviousTab
        ),
        hotkey!(
            "next-channel",
            "Next channel",
            "ctrl-pagedown",
            "ChatWorkspace",
            NextChannel
        ),
        hotkey!(
            "previous-channel",
            "Previous channel",
            "ctrl-pageup",
            "ChatWorkspace",
            PreviousChannel
        ),
        hotkey!(
            "channel-details",
            "Channel details",
            "ctrl-i",
            "ChatWorkspace",
            ChannelDetails
        ),
        hotkey!(
            "search-channel",
            "Search this channel",
            "ctrl-f",
            "JawjackChannel",
            crate::FindChat
        ),
        hotkey!(
            "search-open",
            "Search open channels",
            "ctrl-shift-f",
            "ChatWorkspace",
            SearchOpenChannels
        ),
        hotkey!(
            "activity",
            "Highlights & unread",
            "ctrl-shift-m",
            "ChatWorkspace",
            OpenActivity
        ),
        hotkey!(
            "mark-all-read",
            "Mark all read",
            "ctrl-shift-r",
            "ChatWorkspace",
            MarkAllRead
        ),
        hotkey!(
            "studio",
            "Show or hide the Studio panel",
            "ctrl-shift-s",
            "ChatWorkspace",
            ToggleStudio
        ),
        hotkey!(
            "sent-history",
            "Sent message history",
            "ctrl-shift-h",
            "JawjackChannel",
            crate::SentHistory
        ),
        hotkey!(
            "new-workspace",
            "New workspace",
            "ctrl-t",
            "ChatWorkspace",
            NewTab
        ),
        hotkey!(
            "close-workspace",
            "Close workspace",
            "ctrl-w",
            "ChatWorkspace",
            CloseTab
        ),
        hotkey!(
            "reopen-workspace",
            "Reopen workspace",
            "ctrl-shift-t",
            "ChatWorkspace",
            ReopenTab
        ),
        hotkey!(
            "pause-chat",
            "Pause or resume scrolling",
            "ctrl-space",
            "JawjackChannel",
            crate::ToggleChatPause
        ),
        hotkey!(
            "next-pane",
            "Focus next split",
            "f6",
            "ChatWorkspace",
            FocusNextPane
        ),
        hotkey!(
            "previous-highlight",
            "Previous highlight",
            "alt-pageup",
            "JawjackChannel",
            crate::PreviousHighlight
        ),
        hotkey!(
            "next-highlight",
            "Next highlight",
            "alt-pagedown",
            "JawjackChannel",
            crate::NextHighlight
        ),
        hotkey!(
            "emote-picker",
            "Open the emote picker",
            "",
            "JawjackChannel",
            crate::OpenEmotePicker
        ),
        hotkey!(
            "settings",
            "Open settings",
            "",
            "ChatWorkspace",
            ToggleAppearance
        ),
        hotkey!(
            "streamer-mode",
            "Toggle streamer mode",
            "",
            "ChatWorkspace",
            ToggleStreamerMode
        ),
        hotkey!(
            "sidebar",
            "Show or hide the sidebar",
            "",
            "ChatWorkspace",
            ToggleSidebar
        ),
        hotkey!("whispers", "Whispers", "", "ChatWorkspace", OpenWhispers),
        hotkey!(
            "hide-tabs",
            "Hide or show channel tabs",
            "ctrl-u",
            "ChatWorkspace",
            ToggleTabStrip
        ),
        hotkey!(
            "chatter-list",
            "Chatter list",
            "",
            "JawjackChannel",
            crate::OpenChatterList
        ),
        hotkey!(
            "streamlink",
            "Watch this channel in Streamlink",
            "",
            "JawjackChannel",
            crate::WatchInStreamlink
        ),
        hotkey!(
            "open-browser",
            "Open this stream in the browser",
            "",
            "JawjackChannel",
            crate::OpenStreamInBrowser
        ),
        hotkey!(
            "player-popout",
            "Open the player pop-out",
            "",
            "JawjackChannel",
            crate::OpenPlayerPopout
        ),
        hotkey!(
            "live-tabs",
            "Only show live channel tabs",
            "",
            "ChatWorkspace",
            ToggleLiveChannels
        ),
        hotkey!(
            "move-tab-left",
            "Move tab left",
            "",
            "ChatWorkspace",
            MoveTabLeft
        ),
        hotkey!(
            "move-tab-right",
            "Move tab right",
            "",
            "ChatWorkspace",
            MoveTabRight
        ),
        hotkey!(
            "equalize-split",
            "Make the splits equal",
            "",
            "ChatWorkspace",
            EqualizeSplit
        ),
        hotkey!(
            "custom-commands",
            "Edit custom commands",
            "",
            "ChatWorkspace",
            EditCustomCommands
        ),
        hotkey!(
            "rename-workspace",
            "Rename workspace",
            "",
            "ChatWorkspace",
            RenameWorkspace
        ),
    ]
};

/// Custom keys by hotkey ID (saved with the workspace).
static OVERRIDES: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

pub fn overrides() -> BTreeMap<String, String> {
    OVERRIDES.lock().map(|o| o.clone()).unwrap_or_default()
}
/// The keys currently in effect for a hotkey.
pub fn keys(hotkey: &Hotkey) -> String {
    overrides()
        .get(hotkey.id)
        .cloned()
        .unwrap_or_else(|| hotkey.default.to_owned())
}

/// "ctrl-shift-k" → "Ctrl+Shift+K".
pub fn label(keys: &str) -> String {
    if keys.is_empty() {
        return "Not set".into();
    }
    let Ok(keystroke) = gpui_kit::Keystroke::parse(keys) else {
        return keys.to_owned();
    };
    let m = &keystroke.modifiers;
    let mut parts: Vec<String> = [
        (m.control, "Ctrl"),
        (m.alt, "Alt"),
        (m.shift, "Shift"),
        (m.platform, "Win"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| name.to_owned())
    .collect();
    parts.push(match keystroke.key.as_str() {
        "pagedown" => "PageDown".to_owned(),
        "pageup" => "PageUp".to_owned(),
        key => {
            let mut chars = key.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    });
    parts.join("+")
}

/// A shortcut must use Ctrl or Alt (or be a function key), so it can't take over typing.
pub fn validate(keys: &str) -> Result<String, String> {
    let keys = keys.trim().to_ascii_lowercase();
    let keystroke =
        gpui_kit::Keystroke::parse(&keys).map_err(|_| "That isn't a key combination.")?;
    let function_key = keystroke.key.len() > 1
        && keystroke.key.starts_with('f')
        && keystroke.key[1..].parse::<u8>().is_ok();
    let m = &keystroke.modifiers;
    if !(m.control || m.alt || m.platform || function_key) {
        return Err("Use Ctrl or Alt with a key (or an F-key), so typing isn't affected.".into());
    }
    if let Some(other) = HOTKEYS.iter().find(|h| keys == self::keys(h)) {
        return Err(format!("{} already uses {}.", other.label, label(&keys)));
    }
    Ok(keystroke.unparse())
}

/// Use `keys` for a hotkey (None = back to its default).
pub fn set(id: &str, keys: Option<String>, cx: &mut App) {
    let Some(hotkey) = HOTKEYS.iter().find(|h| h.id == id) else {
        return;
    };
    let before = self::keys(hotkey);
    if let Ok(mut overrides) = OVERRIDES.lock() {
        match keys.filter(|k| k != hotkey.default) {
            Some(keys) => overrides.insert(id.to_owned(), keys),
            None => overrides.remove(id),
        };
    }
    let after = self::keys(hotkey);
    if before != after {
        // An empty key means "not set": nothing to disable or bind.
        let mut bindings = Vec::new();
        if !before.is_empty() {
            bindings.push(KeyBinding::new(&before, NoAction, Some(hotkey.context)));
        }
        if !after.is_empty() {
            bindings.push((hotkey.bind)(&after, Some(hotkey.context)));
        }
        cx.bind_keys(bindings);
    }
}

/// What "Run command N" sends in the channel you're in (a /command or a message).
pub static COMMANDS: std::sync::RwLock<[String; 4]> =
    std::sync::RwLock::new([String::new(), String::new(), String::new(), String::new()]);
pub fn command(index: usize) -> String {
    COMMANDS
        .read()
        .ok()
        .and_then(|c| c.get(index).cloned())
        .unwrap_or_default()
}
pub fn set_command(index: usize, text: &str) {
    let text: String = text.chars().filter(|c| !c.is_control()).take(500).collect();
    if let Ok(mut commands) = COMMANDS.write()
        && let Some(slot) = commands.get_mut(index)
    {
        *slot = text.trim().to_owned();
    }
}
pub fn commands_json() -> serde_json::Value {
    serde_json::json!((0..4).map(command).collect::<Vec<_>>())
}
pub fn load_commands(value: &serde_json::Value) {
    for index in 0..4 {
        set_command(index, value[index].as_str().unwrap_or(""));
    }
}
pub fn load(value: &serde_json::Value, cx: &mut App) {
    for hotkey in HOTKEYS {
        if let Some(keys) = value[hotkey.id].as_str()
            && let Ok(keys) = gpui_kit::Keystroke::parse(keys).map(|k| k.unparse())
        {
            set(hotkey.id, Some(keys), cx);
        }
    }
}
pub fn json() -> serde_json::Value {
    serde_json::json!(overrides())
}

#[cfg(test)]
mod tests {
    /// Every default shortcut must also be bound at startup (bind_keys).
    #[test]
    fn defaults_are_bound() {
        let source = [include_str!("workspace.rs"), include_str!("main.rs")].concat();
        for hotkey in super::HOTKEYS.iter().filter(|h| !h.default.is_empty()) {
            assert!(
                source.contains(&format!("KeyBinding::new(\"{}\"", hotkey.default)),
                "{} ({}) isn't bound at startup",
                hotkey.id,
                hotkey.default
            );
        }
    }
    #[test]
    fn labels_and_validation() {
        assert_eq!(super::label("ctrl-shift-k"), "Ctrl+Shift+K");
        assert_eq!(super::label("ctrl--"), "Ctrl+-");
        assert!(super::validate("k").is_err());
        assert!(super::validate("ctrl-p").is_err()); // taken by Switch channel
        assert_eq!(super::validate("alt-j").as_deref(), Ok("alt-j"));
        assert_eq!(super::validate("f9").as_deref(), Ok("f9"));
    }
}
