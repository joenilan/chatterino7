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
