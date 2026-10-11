//! YouTube live chat, read-only, through the official YouTube Data API v3 with
//! a user-supplied API key: videos.list resolves a live video's chat, then
//! liveChatMessages.list is polled at the interval YouTube asks for (never
//! faster than every 3 s, to spare the key's daily quota). Pane names are
//! "yt:<videoId>"; they never reach Twitch code paths.
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

const API: &str = "https://www.googleapis.com/youtube/v3";
const MIN_POLL: Duration = Duration::from_secs(3);

pub fn is_youtube(name: &str) -> bool {
    name.starts_with("yt:")
}
/// "yt:<id>" from a YouTube live URL or a bare "yt:<id>".
pub fn parse(input: &str) -> Option<String> {
    let input = input.trim();
    let id = if let Some(id) = input.strip_prefix("yt:") {
        id.to_owned()
    } else {
        let rest = input
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .trim_start_matches("m.");
        if let Some(r) = rest.strip_prefix("youtu.be/") {
            r.split(['?', '&', '/']).next()?.to_owned()
        } else if let Some(r) = rest.strip_prefix("youtube.com/live/") {
            r.split(['?', '&', '/']).next()?.to_owned()
        } else if let Some(r) = rest.strip_prefix("youtube.com/watch?") {
            r.split('&')
                .find_map(|kv| kv.strip_prefix("v="))?
                .to_owned()
        } else {
            return None;
        }
    };
    (id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then(|| format!("yt:{id}"))
}

pub enum Update {
    /// The live chat's ID, needed to send.
    ChatId(String),
    Status {
        connected: bool,
        text: String,
    },
    Title(String),
    Message(chat_core::Message),
}

fn text(v: &Value, max: usize) -> String {
    v.as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}
/// A chat_core message from a liveChatMessage resource (text messages and Super Chats).
pub fn message(name: &str, item: &Value) -> Option<chat_core::Message> {
    let snippet = &item["snippet"];
    let author = &item["authorDetails"];
    let kind = snippet["type"].as_str().unwrap_or("");
    let body = match kind {
        "textMessageEvent" => text(&snippet["displayMessage"], 500),
        "superChatEvent" | "superStickerEvent" => {
            let amount = text(&snippet["superChatDetails"]["amountDisplayString"], 30)
                .if_empty(|| text(&snippet["superStickerDetails"]["amountDisplayString"], 30));
            let comment = text(&snippet["superChatDetails"]["userComment"], 500);
            format!("[Super Chat {amount}] {comment}").trim().to_owned()
        }
        "newSponsorEvent" | "memberMilestoneChatEvent" => text(&snippet["displayMessage"], 500),
        _ => return None,
    };
    let mut badges = Vec::new();
    let badge = |set: &str| chat_core::Badge {
        set_id: set.into(),
        id: "1".into(),
    };
    if author["isChatOwner"] == true {
        badges.push(badge("broadcaster"));
    }
    if author["isChatModerator"] == true {
        badges.push(badge("moderator"));
    }
    if author["isChatSponsor"] == true {
        badges.push(badge("subscriber"));
    }
    let display = text(&author["displayName"], 60);
    // YouTube names have no colour; a stable one per author keeps chat readable.
    const COLORS: [u32; 8] = [
        0xF07A7A, 0x91D7BA, 0x91C7E8, 0xE69DCD, 0xE8C68A, 0xA99CF4, 0x7FD6E0, 0xC5E07F,
    ];
    let channel_id = text(&author["channelId"], 64);
    let hue = channel_id
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as usize
        % COLORS.len();
    Some(chat_core::Message {
        sent_at: snippet["publishedAt"]
            .as_str()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.timestamp()),
        id: text(&item["id"], 128),
        channel_id: name.to_owned(),
        user_id: format!("yt:{channel_id}"),
        display_name: display,
        login: None,
        reply: None,
        mentions: vec![],
        replyable: false,
        name_color: Some(COLORS[hue]),
        badges,
        fragments: vec![chat_core::Fragment::Text(body)],
        presentation: chat_core::MessagePresentation {
            kind: if kind == "textMessageEvent" {
                "text".into()
            } else {
                "youtube_paid".into()
            },
            notice_type: (kind == "newSponsorEvent" || kind == "memberMilestoneChatEvent")
                .then(|| "membership".into()),
            ..Default::default()
        },
        deleted: false,
    })
}
trait IfEmpty {
    fn if_empty(self, other: impl FnOnce() -> String) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, other: impl FnOnce() -> String) -> String {
        if self.is_empty() { other() } else { self }
    }
}

fn watch(name: String, key: String, stop: Arc<AtomicBool>, tx: mpsc::Sender<(String, Update)>) {
    let send = |u: Update| tx.send((name.clone(), u)).is_ok();
    let Ok(client) = crate::live::client() else {
        return;
    };
    let video = name.trim_start_matches("yt:").to_owned();
    let get = |path: &str, query: &[(&str, &str)]| -> Result<Value, String> {
        let response = client
            .get(format!("{API}/{path}"))
            .query(query)
            .query(&[("key", key.as_str())])
            .send()
            .map_err(|_| "YouTube didn't answer".to_owned())?;
        let status = response.status().as_u16();
        let value: Value = response
            .text()
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        match status {
            200 => Ok(value),
            403 if value.to_string().contains("quota") => {
                Err("YouTube API quota used up for today".into())
            }
            400 | 403 => Err(format!(
                "YouTube refused: {}",
                value["error"]["message"]
                    .as_str()
                    .unwrap_or("check the API key")
                    .chars()
                    .take(120)
                    .collect::<String>()
            )),
            404 => Err("Live chat not found (ended or private)".into()),
            code => Err(format!("YouTube error {code}")),
        }
    };
    // Resolve the video's live chat.
    let chat = loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match get(
            "videos",
            &[
                ("part", "snippet,liveStreamingDetails"),
                ("id", video.as_str()),
            ],
        ) {
            Ok(v) => {
                let item = &v["items"][0];
                let title = text(&item["snippet"]["title"], 120);
                let channel = text(&item["snippet"]["channelTitle"], 60);
                if !title.is_empty() && !send(Update::Title(format!("{channel} · {title}"))) {
                    return;
                }
                match item["liveStreamingDetails"]["activeLiveChatId"].as_str() {
                    Some(chat) => break chat.to_owned(),
                    None => {
                        send(Update::Status {
                            connected: false,
                            text: "Not live, or live chat is off · checking again in a minute"
                                .into(),
                        });
                        std::thread::sleep(Duration::from_secs(60));
                    }
                }
            }
            Err(e) => {
                send(Update::Status {
                    connected: false,
                    text: format!("{e} · retrying in a minute"),
                });
                std::thread::sleep(Duration::from_secs(60));
            }
        }
    };
    send(Update::ChatId(chat.clone()));
    send(Update::Status {
        connected: true,
        text: "Connected (YouTube)".into(),
    });
    let mut page: Option<String> = None;
    while !stop.load(Ordering::Relaxed) {
        let mut query = vec![
            ("liveChatId", chat.as_str()),
            ("part", "snippet,authorDetails"),
            ("maxResults", "200"),
        ];
        if let Some(token) = &page {
            query.push(("pageToken", token.as_str()));
        }
        match get("liveChat/messages", &query) {
            Ok(v) => {
                for item in v["items"].as_array().into_iter().flatten() {
                    if let Some(message) = message(&name, item)
                        && !send(Update::Message(message))
                    {
                        return;
                    }
                }
                page = v["nextPageToken"].as_str().map(str::to_owned);
                let wait = v["pollingIntervalMillis"]
                    .as_u64()
                    .map(Duration::from_millis)
                    .unwrap_or(MIN_POLL)
                    .max(MIN_POLL);
                std::thread::sleep(wait);
            }
            Err(e) => {
                let ended = e.contains("not found");
                send(Update::Status {
                    connected: false,
                    text: if ended {
                        "Live chat ended".into()
                    } else {
                        format!("{e} · retrying")
                    },
                });
                if ended {
                    return;
                }
                std::thread::sleep(Duration::from_secs(30));
            }
        }
    }
}

/// One watcher thread per open YouTube pane.
pub struct YouTube {
    watching: HashMap<String, Arc<AtomicBool>>,
    key: Arc<Mutex<Option<String>>>,
    tx: mpsc::Sender<(String, Update)>,
    rx: mpsc::Receiver<(String, Update)>,
}
impl YouTube {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            watching: HashMap::new(),
            key: Default::default(),
            tx,
            rx,
        }
    }
    pub fn set_key(&mut self, key: Option<String>) {
        if let Ok(mut k) = self.key.lock() {
            if *k != key {
                *k = key;
                // Restart watchers with the new key.
                for (_, stop) in self.watching.drain() {
                    stop.store(true, Ordering::Relaxed);
                }
            }
        }
    }
    pub fn has_key(&self) -> bool {
        self.key.lock().map(|k| k.is_some()).unwrap_or(false)
    }
    /// Watch exactly `names` (open YouTube panes); returns new updates.
    pub fn pump(&mut self, names: &[String]) -> Vec<(String, Update)> {
        self.watching.retain(|name, stop| {
            let keep = names.contains(name);
            if !keep {
                stop.store(true, Ordering::Relaxed);
            }
            keep
        });
        let key = self.key.lock().ok().and_then(|k| k.clone());
        let mut updates = Vec::new();
        for name in names {
            if self.watching.contains_key(name) {
                continue;
            }
            let Some(key) = key.clone() else {
                updates.push((
                    name.clone(),
                    Update::Status {
                        connected: false,
                        text: "Add a YouTube API key in Settings > Connections".into(),
                    },
                ));
                continue;
            };
            let stop = Arc::new(AtomicBool::new(false));
            self.watching.insert(name.clone(), stop.clone());
            let (name, tx) = (name.clone(), self.tx.clone());
            let _ = std::thread::Builder::new()
                .name("jawjack-youtube".into())
                .spawn(move || watch(name, key, stop, tx));
        }
        while let Ok(update) = self.rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn urls_and_messages() {
        assert_eq!(
            parse("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1").as_deref(),
            Some("yt:dQw4w9WgXcQ")
        );
        assert_eq!(
            parse("youtu.be/dQw4w9WgXcQ").as_deref(),
            Some("yt:dQw4w9WgXcQ")
        );
        assert_eq!(
            parse("https://youtube.com/live/dQw4w9WgXcQ?si=x").as_deref(),
            Some("yt:dQw4w9WgXcQ")
        );
        assert_eq!(parse("bessvibes"), None);
        let item = json!({"id":"m1","snippet":{"type":"textMessageEvent","displayMessage":"hello","publishedAt":"2026-10-11T20:00:00Z"},
            "authorDetails":{"displayName":"Ann","channelId":"UC1","isChatModerator":true}});
        let m = message("yt:dQw4w9WgXcQ", &item).unwrap();
        assert_eq!(m.body(), "hello");
        assert!(m.badges.iter().any(|b| b.set_id == "moderator"));
        let paid = json!({"id":"m2","snippet":{"type":"superChatEvent","superChatDetails":{"amountDisplayString":"$5.00","userComment":"gg"}},"authorDetails":{"displayName":"B","channelId":"UC2"}});
        assert_eq!(
            message("yt:x", &paid).unwrap().body(),
            "[Super Chat $5.00] gg"
        );
    }
}
