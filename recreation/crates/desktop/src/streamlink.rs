//! "Open in Streamlink", as in Chatterino: runs the Streamlink app, which plays
//! the stream in your own video player. Jawjack only starts it; nothing is
//! downloaded or installed.
use std::path::PathBuf;

pub const SITE: &str = "https://streamlink.github.io";

/// Streamlink on PATH or in its usual install folders.
pub fn find() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "streamlink.exe"
    } else {
        "streamlink"
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if cfg!(windows) {
        for base in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = std::env::var_os(base) {
                dirs.push(PathBuf::from(base).join("Streamlink").join("bin"));
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(
                PathBuf::from(local)
                    .join("Programs")
                    .join("Streamlink")
                    .join("bin"),
            );
        }
    }
    dirs.into_iter().map(|d| d.join(exe)).find(|p| p.is_file())
}

/// Start Streamlink for a Twitch channel at its best quality.
pub fn open(channel: &str) -> Result<(), String> {
    let login = chat_core::twitch_login(channel).ok_or("Streamlink works with Twitch channels.")?;
    let path = find().ok_or(format!(
        "Streamlink isn't installed. Get it from {SITE}, then try again."
    ))?;
    let mut command = std::process::Command::new(path);
    command
        .arg(format!("https://www.twitch.tv/{login}"))
        .arg("best");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window; Streamlink opens the video player itself.
        command.creation_flags(0x0800_0000);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|_| "Streamlink didn't start.".into())
}

/// Twitch's own pop-out player for a channel.
pub fn player_url(login: &str) -> String {
    format!("https://player.twitch.tv/?channel={login}&parent=twitch.tv&enableExtensions=true")
}

/// Your own stream player (Chatterino's custom player URI): a link with
/// {channel} where the channel login goes, e.g. mpv://twitch.tv/{channel}.
/// Empty: no "In your player" item.
pub static CUSTOM_PLAYER: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());
pub fn validate_player(template: &str) -> Result<String, String> {
    let template = template.trim();
    if template.is_empty() {
        return Ok(String::new());
    }
    let scheme = template
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if template.len() > 300
        || !template.contains("{channel}")
        || template.chars().any(char::is_whitespace)
        || scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        || matches!(scheme.as_str(), "file" | "javascript" | "data" | "vbscript")
        || !template.contains("://")
    {
        return Err("Use a player link with {channel} where the channel name goes, e.g. mpv://twitch.tv/{channel}.".into());
    }
    Ok(template.to_owned())
}
/// The player link for a channel, if one is set.
pub fn custom_player(channel: &str) -> Option<String> {
    let template = CUSTOM_PLAYER.read().ok()?.clone();
    (!template.is_empty()).then(|| template.replace("{channel}", channel))
}

#[cfg(test)]
mod player_tests {
    use super::validate_player;
    #[test]
    fn player_links_are_checked() {
        assert_eq!(validate_player(" ").unwrap(), "");
        assert!(validate_player("mpv://twitch.tv/{channel}").is_ok());
        assert!(validate_player("potplayer://https://twitch.tv/{channel}").is_ok());
        assert!(validate_player("file://C:/x/{channel}").is_err());
        assert!(validate_player("mpv://twitch.tv/").is_err());
        assert!(validate_player("javascript://{channel}").is_err());
    }
}
