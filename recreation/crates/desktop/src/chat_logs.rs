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

/// Settings → Chat → Save chat logs: the line timestamp (an index into
/// `TIME_FORMATS`), and whether only chosen channels are logged.
pub const TIME_FORMATS: [(&str, &str); 4] = [
    ("13:34:56", "%H:%M:%S"),
    ("13:34", "%H:%M"),
    ("1:34:56 PM", "%-I:%M:%S %p"),
    ("Date + time", "%Y-%m-%d %H:%M:%S"),
];
pub static TIME_FORMAT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static ONLY_CHOSEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A separate file per stream while the channel is live (Chatterino's option);
/// offline chat still goes to the day's file.
pub static PER_STREAM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A stream's start time (Twitch RFC 3339) as a file-name stamp, local time.
pub fn stream_stamp(started_at: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(started_at)
        .ok()
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d-%H%M")
                .to_string()
        })
}
/// Channels logged when only chosen ones are (tab menu → Log this channel).
pub static CHOSEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
pub fn chosen(channel: &str) -> bool {
    CHOSEN.lock().is_ok_and(|c| c.iter().any(|x| x == channel))
}
pub fn toggle_chosen(channel: &str) {
    if let Ok(mut chosen) = CHOSEN.lock() {
        match chosen.iter().position(|x| x == channel) {
            Some(at) => {
                chosen.remove(at);
            }
            None => chosen.push(channel.to_owned()),
        }
    }
}
/// Whether lines from `channel` go to the logs (logging itself must be on).
pub fn wanted(channel: &str) -> bool {
    !ONLY_CHOSEN.load(std::sync::atomic::Ordering::Relaxed) || chosen(channel)
}

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
    /// The stream's start stamp, when logging per stream.
    stream: Option<String>,
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

/// `<folder>/<channel>/<channel>-stream-YYYY-MM-DD-HHMM.log` for one stream.
pub fn stream_file_for(folder: &Path, channel: &str, stamp: &str) -> Option<PathBuf> {
    chat_core::twitch_login(channel).filter(|c| c == channel)?;
    if stamp.len() != 15 || !stamp.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return None;
    }
    Some(
        folder
            .join(channel)
            .join(format!("{channel}-stream-{stamp}.log")),
    )
}
pub fn format(at: chrono::DateTime<chrono::Local>, text: &str) -> String {
    let time = TIME_FORMATS[TIME_FORMAT
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(TIME_FORMATS.len() - 1)]
    .1;
    format_with(at, text, time)
}
fn format_with(at: chrono::DateTime<chrono::Local>, text: &str, time: &str) -> String {
    format!(
        "[{}] {}\n",
        at.format(time),
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
                    let path = match &line.stream {
                        Some(stamp) => stream_file_for(&line.folder, &line.channel, stamp),
                        None => file_for(&line.folder, &line.channel, line.at.date_naive()),
                    };
                    let Some(path) = path else {
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
    pub fn write(&self, folder: &Path, channel: &str, stream: Option<String>, text: String) {
        let _ = self.tx.send(Line {
            folder: folder.to_owned(),
            channel: channel.to_owned(),
            stream,
            at: chrono::Local::now(),
            text,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn per_stream_files() {
        let path = stream_file_for(Path::new("L"), "bessvibes", "2026-10-12-1930").unwrap();
        assert_eq!(
            path,
            Path::new("L")
                .join("bessvibes")
                .join("bessvibes-stream-2026-10-12-1930.log")
        );
        assert!(stream_file_for(Path::new("L"), "bessvibes", "../x").is_none());
        assert_eq!(
            stream_stamp("2026-10-12T19:30:00Z").map(|s| s.len()),
            Some(15)
        );
    }
    #[test]
    fn chosen_channels_and_time_formats() {
        use std::sync::atomic::Ordering::Relaxed;
        assert!(wanted("a"));
        ONLY_CHOSEN.store(true, Relaxed);
        assert!(!wanted("a"));
        toggle_chosen("a");
        assert!(wanted("a"));
        toggle_chosen("a");
        assert!(!wanted("a"));
        ONLY_CHOSEN.store(false, Relaxed);
        let at = chrono::Local::now()
            .with_time(chrono::NaiveTime::from_hms_opt(15, 4, 5).unwrap())
            .unwrap();
        assert_eq!(
            format_with(at, "hi", TIME_FORMATS[2].1),
            "[3:04:05 PM] hi
"
        );
    }
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
