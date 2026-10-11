//! Optional chat logs (off by default): one text file per channel per day,
//! `<folder>/<channel>/<channel>-YYYY-MM-DD.log`, lines like Chatterino's
//! `[12:34:56] name: message`. Written on a background thread.
use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
};

pub fn default_folder() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Documents").join("Jawjack Logs")
}

struct Line {
    folder: PathBuf,
    channel: String,
    at: chrono::DateTime<chrono::Local>,
    text: String,
}

/// The file for a channel on a day. Channel names are Twitch logins, so they are
/// already safe as path parts; anything else is refused.
pub fn file_for(folder: &Path, channel: &str, day: chrono::NaiveDate) -> Option<PathBuf> {
    chat_core::twitch_login(channel).filter(|c| c == channel)?;
    Some(
        folder
            .join(channel)
            .join(format!("{channel}-{}.log", day.format("%Y-%m-%d"))),
    )
}

pub fn format(at: chrono::DateTime<chrono::Local>, text: &str) -> String {
    format!(
        "[{}] {}\n",
        at.format("%H:%M:%S"),
        text.replace(['\r', '\n'], " ")
    )
}

/// A chat message as one log line: `name: text`, `name (login): text` for
/// localized names, or the notice text for system messages.
pub fn message_text(message: &chat_core::Message) -> String {
    let body = message.body();
    let name = &message.display_name;
    match message.login.as_deref() {
        _ if name.is_empty() => body,
        Some(login) if !login.eq_ignore_ascii_case(name) => format!("{name} ({login}): {body}"),
        _ => format!("{name}: {body}"),
    }
}

pub struct Logs {
    tx: mpsc::Sender<Line>,
}
impl Logs {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Line>();
        let _ = std::thread::Builder::new()
            .name("jawjack-logs".into())
            .spawn(move || {
                // Open files stay open until the day or folder changes.
                let mut open: HashMap<String, (PathBuf, File)> = HashMap::new();
                while let Ok(line) = rx.recv() {
                    let Some(path) = file_for(&line.folder, &line.channel, line.at.date_naive())
                    else {
                        continue;
                    };
                    // Reopen on a new day or folder, or if the file was deleted meanwhile.
                    if open.get(&line.channel).is_none_or(|(p, _)| *p != path) || !path.exists() {
                        open.remove(&line.channel);
                        let file = path
                            .parent()
                            .map(std::fs::create_dir_all)
                            .and_then(Result::ok)
                            .and_then(|_| {
                                std::fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(&path)
                                    .ok()
                            });
                        let Some(file) = file else { continue };
                        open.insert(line.channel.clone(), (path, file));
                    }
                    if let Some((_, file)) = open.get_mut(&line.channel) {
                        let _ = file.write_all(format(line.at, &line.text).as_bytes());
                    }
                }
            });
        Self { tx }
    }
    pub fn write(&self, folder: &Path, channel: &str, text: String) {
        let _ = self.tx.send(Line {
            folder: folder.to_owned(),
            channel: channel.to_owned(),
            at: chrono::Local::now(),
            text,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_lines() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        let path = file_for(Path::new("L"), "bessvibes", day).unwrap();
        assert_eq!(
            path,
            Path::new("L")
                .join("bessvibes")
                .join("bessvibes-2026-10-11.log")
        );
        assert!(file_for(Path::new("L"), "../evil", day).is_none());
        let at = chrono::Local::now();
        assert!(format(at, "a\nb").ends_with("a b\n"));
    }
}
