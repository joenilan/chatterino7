//! Streamer mode: while you're broadcasting, keep private things off screen
//! (notes, whisper text, stream thumbnails) and optionally mute pings.
//! Automatic mode watches for broadcasting software, as Chatterino does.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// Broadcasting software that turns automatic streamer mode on (lowercase
/// executable names).
const BROADCASTERS: [&str; 12] = [
    "obs64.exe",
    "obs32.exe",
    "obs.exe",
    "streamlabs obs.exe",
    "streamlabs desktop.exe",
    "xsplit.core.exe",
    "vmix64.exe",
    "vmix.exe",
    "twitch studio.exe",
    "twitchstudio.exe",
    "prismlivestudio.exe",
    "meld studio.exe",
];
const POLL: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Off,
    On,
    #[default]
    Automatic,
}
impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Off, Mode::On, Mode::Automatic];
    pub fn key(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::On => "on",
            Mode::Automatic => "automatic",
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
            Mode::Off => "Off",
            Mode::On => "On",
            Mode::Automatic => "Automatic",
        }
    }
}

/// Whether any broadcasting software appears in `tasklist /FO CSV /NH` output.
pub fn broadcasting(tasklist: &str) -> bool {
    tasklist.lines().any(|line| {
        let name = line
            .trim_start_matches('"')
            .split('"')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        BROADCASTERS.contains(&name.as_str())
    })
}

/// Background check for broadcasting software; started once, on first need.
pub struct Detector {
    detected: Arc<AtomicBool>,
    started: bool,
}
impl Detector {
    pub fn new() -> Self {
        Self {
            detected: Arc::new(AtomicBool::new(false)),
            started: false,
        }
    }
    pub fn detected(&mut self) -> bool {
        if !self.started {
            self.started = true;
            let detected = self.detected.clone();
            let _ = std::thread::Builder::new()
                .name("jawjack-streamer-mode".into())
                .spawn(move || {
                    loop {
                        detected.store(scan(), Ordering::Relaxed);
                        std::thread::sleep(POLL);
                    }
                });
        }
        self.detected.load(Ordering::Relaxed)
    }
}
#[cfg(windows)]
fn scan() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .is_some_and(|o| broadcasting(&String::from_utf8_lossy(&o.stdout)))
}
#[cfg(not(windows))]
fn scan() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_card_avatars_are_not_a_streamer_privacy_option() {
        assert!(!OPTIONS.iter().any(|(key, _, _, _)| *key == "card_avatars"));
        assert_eq!(OPTIONS[MOD_ACTIONS].0, "mod_actions");
        assert_eq!(OPTIONS[VIEWER_COUNTS].0, "viewer_counts");
        assert_eq!(OPTIONS[LIVE_ALERTS].0, "live_alerts");
    }

    #[test]
    fn finds_broadcasting_software() {
        let list = "\"explorer.exe\",\"4\",\"Console\",\"1\",\"9 K\"\r\n\"obs64.exe\",\"88\",\"Console\",\"1\",\"300 K\"";
        assert!(broadcasting(list));
        assert!(!broadcasting("\"explorer.exe\",\"4\""));
        assert!(broadcasting("\"Streamlabs OBS.exe\",\"5\""));
        assert_eq!(Mode::from_key("on"), Mode::On);
        assert_eq!(Mode::from_key("?"), Mode::Automatic);
    }
}

/// Whether streamer mode is on right now (set by the workspace).
pub static LIVE: AtomicBool = AtomicBool::new(false);
/// What streamer mode hides, as in Chatterino (key, label, detail, default).
pub const OPTIONS: [(&str, &str, &str, bool); 4] = [
    (
        "link_previews",
        "Link previews",
        "Hovering a link shows just the address; nothing is loaded.",
        true,
    ),
    (
        "mod_actions",
        "Moderation lines",
        "Timeouts, bans and deletions don't show as lines in chat.",
        true,
    ),
    (
        "viewer_counts",
        "Viewer counts and uptime",
        "Hidden in the live sidebar and channel hover cards.",
        false,
    ),
    (
        "live_alerts",
        "Live notifications",
        "\"Channel is live\" pop-ups don't show.",
        false,
    ),
];
pub const LINK_PREVIEWS: usize = 0;
pub const MOD_ACTIONS: usize = 1;
pub const VIEWER_COUNTS: usize = 2;
pub const LIVE_ALERTS: usize = 3;
pub static HIDE: [AtomicBool; 4] = [
    AtomicBool::new(true),
    AtomicBool::new(true),
    AtomicBool::new(false),
    AtomicBool::new(false),
];
/// True when streamer mode is on and set to hide `option` (an index into OPTIONS).
pub fn hides(option: usize) -> bool {
    LIVE.load(Ordering::Relaxed) && HIDE[option].load(Ordering::Relaxed)
}
