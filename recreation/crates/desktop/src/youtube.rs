//! YouTube live chat through the official YouTube Data API v3, signed in with
//! Google (or a user-supplied API key): videos.list resolves a live video's
//! chat, then liveChatMessages.streamList holds one connection open and YouTube
//! pushes messages down it. That costs 5 quota units per connection instead of
//! 5 per poll, which is what keeps one shared daily quota (10,000 units,
//! reset at midnight Pacific) usable by everyone. If streaming is unavailable
//! it falls back to polling liveChatMessages.list, slowing down while chat is
//! quiet. Pane names are "yt:<videoId>"; they never reach Twitch code paths.
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

const API: &str = "https://www.googleapis.com/youtube/v3";
const MIN_POLL: Duration = Duration::from_secs(3);
/// Polling slows to this while chat is quiet.
const IDLE_POLL: Duration = Duration::from_secs(12);
const QUOTA: &str = "YouTube's daily API limit is used up";
const ENDED: &str = "Live chat not found (ended or private)";

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
    /// A deletion or ban made by the channel's moderators.
    Moderation(chat_core::Event),
}
/// messageDeletedEvent / userBannedEvent as chat events (the row is greyed, as on Twitch).
pub fn moderation(name: &str, item: &Value) -> Option<chat_core::Event> {
    let snippet = &item["snippet"];
    match snippet["type"].as_str()? {
        "messageDeletedEvent" => Some(chat_core::Event::DeleteMessage {
            channel_id: name.to_owned(),
            message_id: text(&snippet["messageDeletedDetails"]["deletedMessageId"], 128),
        }),
        "userBannedEvent" => Some(chat_core::Event::ClearUser {
            channel_id: name.to_owned(),
            user_id: format!(
                "yt:{}",
                text(
                    &snippet["userBannedDetails"]["bannedUserDetails"]["channelId"],
                    64
                )
            ),
        }),
        _ => None,
    }
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

/// How to read: a Google sign-in (preferred; it can also send) or an API key.
#[derive(Default)]
pub struct Auth {
    pub key: Option<String>,
    pub google: Option<(crate::google_auth::OAuthClient, crate::google_auth::Tokens)>,
}
impl Auth {
    fn ready(&self) -> bool {
        self.key.is_some() || self.google.is_some()
    }
}

/// Turns an error response into a short, user-facing reason.
fn refusal(status: u16, value: &Value) -> String {
    let text = value.to_string();
    match status {
        403 if text.contains("quotaExceeded")
            || text.contains("dailyLimitExceeded")
            || text.contains("quota") =>
        {
            QUOTA.into()
        }
        404 => ENDED.into(),
        403 if text.contains("liveChatEnded") || text.contains("liveChatDisabled") => ENDED.into(),
        400 | 401 | 403 => format!(
            "YouTube refused: {}",
            value["error"]["message"]
                .as_str()
                .unwrap_or("check the YouTube connection")
                .chars()
                .take(120)
                .collect::<String>()
        ),
        code => format!("YouTube error {code}"),
    }
}

/// The next daily quota reset: midnight in America/Los_Angeles.
pub fn next_quota_reset(now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    use chrono::{Datelike, Duration as Span, NaiveDate, Weekday};
    // US daylight time: from the second Sunday in March to the first Sunday in
    // November; at local midnight the switch (2 AM) hasn't happened yet.
    let nth_sunday =
        |year, month, n: u8| NaiveDate::from_weekday_of_month_opt(year, month, Weekday::Sun, n);
    let daylight_at_midnight = |date: NaiveDate| match (
        nth_sunday(date.year(), 3, 2),
        nth_sunday(date.year(), 11, 1),
    ) {
        (Some(start), Some(end)) => date > start && date <= end,
        _ => false,
    };
    [-7i64, -8]
        .into_iter()
        .filter_map(|offset| {
            let local = (now + Span::hours(offset)).date_naive().succ_opt()?;
            (daylight_at_midnight(local) == (offset == -7))
                .then(|| local.and_hms_opt(0, 0, 0))
                .flatten()
                .map(|m| m.and_utc() - Span::hours(offset))
        })
        .filter(|t| *t > now)
        .min()
        .unwrap_or(now + Span::hours(24))
}

/// Pulls complete JSON objects out of a growing JSON array, the shape Google's
/// REST server-streaming responses arrive in ("[{...},\r\n{...}" and so on).
#[derive(Default)]
struct JsonObjects {
    buf: Vec<u8>,
    scanned: usize,
    depth: usize,
    start: Option<(usize, usize)>,
    in_string: bool,
    escaped: bool,
}
impl JsonObjects {
    fn push(&mut self, bytes: &[u8]) -> Vec<Value> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        let mut i = self.scanned;
        while i < self.buf.len() {
            let b = self.buf[i];
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if b == b'\\' {
                    self.escaped = true;
                } else if b == b'"' {
                    self.in_string = false;
                }
            } else {
                match b {
                    b'"' => self.in_string = true,
                    b'{' => {
                        if self.start.is_none() {
                            self.start = Some((i, self.depth));
                        }
                        self.depth += 1;
                    }
                    b'[' => self.depth += 1,
                    b'}' | b']' => {
                        self.depth = self.depth.saturating_sub(1);
                        if b == b'}'
                            && let Some((from, depth)) = self.start
                            && depth == self.depth
                        {
                            if let Ok(v) = serde_json::from_slice(&self.buf[from..=i]) {
                                out.push(v);
                            }
                            self.start = None;
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        // Keep only the unfinished object; drop everything already parsed.
        let keep = self.start.map_or(self.buf.len(), |(from, _)| from);
        self.buf.drain(..keep);
        if let Some((from, _)) = self.start.as_mut() {
            *from -= keep;
        }
        self.scanned = self.buf.len();
        out
    }
}

fn watch(
    name: String,
    auth: Arc<Mutex<Auth>>,
    stop: Arc<AtomicBool>,
    tx: mpsc::Sender<(String, Update)>,
) {
    // A stopped watcher (pane closed, or restarted after a sign-in change) can
    // still be blocked on a stream read; it must never post into the new one.
    let send = |u: Update| !stop.load(Ordering::Relaxed) && tx.send((name.clone(), u)).is_ok();
    let status = |connected: bool, text: String| send(Update::Status { connected, text });
    let Ok(client) = crate::live::client() else {
        return;
    };
    let Ok(streamer) = reqwest::blocking::Client::builder()
        .timeout(None)
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return;
    };
    let video = name.trim_start_matches("yt:").to_owned();
    let authorize = |request: reqwest::blocking::RequestBuilder| -> Result<reqwest::blocking::RequestBuilder, String> {
        let mut auth = auth
            .lock()
            .map_err(|_| "YouTube sign-in unavailable".to_owned())?;
        if let Some((oauth, tokens)) = auth.google.as_mut() {
            Ok(request.bearer_auth(crate::google_auth::access(oauth, tokens)?))
        } else if let Some(key) = &auth.key {
            Ok(request.query(&[("key", key.as_str())]))
        } else {
            Err("Connect YouTube to read chat".into())
        }
    };
    let get = |path: &str, query: &[(&str, &str)]| -> Result<Value, String> {
        let response = authorize(client.get(format!("{API}/{path}")).query(query))?
            .send()
            .map_err(|_| "YouTube didn't answer".to_owned())?;
        let code = response.status().as_u16();
        let value: Value = response
            .text()
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if code == 200 {
            Ok(value)
        } else {
            Err(refusal(code, &value))
        }
    };
    // Sleeps in short steps so a closed pane stops promptly; false once stopped.
    let pause = |total: Duration| {
        let until = std::time::Instant::now() + total;
        while std::time::Instant::now() < until {
            if stop.load(Ordering::Relaxed) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        !stop.load(Ordering::Relaxed)
    };
    // Out of quota: say when it comes back, then wait for the reset instead of
    // spending more requests on refusals.
    let wait_for_quota = || {
        let reset = next_quota_reset(chrono::Utc::now());
        let at = reset.with_timezone(&chrono::Local).format("%-I:%M %p");
        status(false, format!("{QUOTA} · back at {at}"));
        let wait = (reset - chrono::Utc::now()).to_std().unwrap_or_default();
        pause(wait + Duration::from_secs(30))
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
                        status(
                            false,
                            "Not live, or live chat is off · checking again in a minute".into(),
                        );
                        if !pause(Duration::from_secs(60)) {
                            return;
                        }
                    }
                }
            }
            Err(e) if e == QUOTA => {
                if !wait_for_quota() {
                    return;
                }
            }
            Err(e) => {
                status(false, format!("{e} · retrying in a minute"));
                if !pause(Duration::from_secs(60)) {
                    return;
                }
            }
        }
    };
    send(Update::ChatId(chat.clone()));
    status(true, "Connected (YouTube)".into());
    let deliver = |v: &Value| {
        for item in v["items"].as_array().into_iter().flatten() {
            if let Some(message) = message(&name, item)
                && !send(Update::Message(message))
            {
                return false;
            }
            if let Some(event) = moderation(&name, item)
                && !send(Update::Moderation(event))
            {
                return false;
            }
        }
        true
    };
    // One streaming connection; Ok when YouTube closes it normally.
    let stream = |page: &mut Option<String>| -> Result<(), String> {
        let mut query = vec![
            ("liveChatId", chat.as_str()),
            ("part", "snippet,authorDetails"),
        ];
        if let Some(token) = page.as_deref() {
            query.push(("pageToken", token));
        }
        let mut response = authorize(
            streamer
                .get(format!("{API}/liveChat/messages/stream"))
                .query(&query),
        )?
        .send()
        .map_err(|_| "YouTube didn't answer".to_owned())?;
        let code = response.status().as_u16();
        if code != 200 {
            let value = response
                .text()
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
            return Err(refusal(code, &value));
        }
        let mut objects = JsonObjects::default();
        let mut chunk = [0u8; 16384];
        loop {
            let n = response
                .read(&mut chunk)
                .map_err(|_| "YouTube chat connection dropped".to_owned())?;
            if n == 0 || stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            for v in objects.push(&chunk[..n]) {
                if v["error"].is_object() {
                    let code = v["error"]["code"].as_u64().unwrap_or(500) as u16;
                    return Err(refusal(code, &v));
                }
                if let Some(token) = v["nextPageToken"].as_str() {
                    *page = Some(token.to_owned());
                }
                if !deliver(&v) {
                    return Ok(());
                }
                if v["offlineAt"].is_string() {
                    return Err(ENDED.into());
                }
            }
        }
    };
    let mut page: Option<String> = None;
    let mut streamed = false;
    let mut failures = 0;
    while !stop.load(Ordering::Relaxed) && failures < 3 {
        let opened = std::time::Instant::now();
        match stream(&mut page) {
            Ok(()) => {
                streamed = true;
                failures = 0;
                // Each reconnect costs 5 units; never reconnect in a tight loop.
                let lived = opened.elapsed();
                if !pause(Duration::from_secs(2).max(Duration::from_secs(30).saturating_sub(lived)))
                {
                    return;
                }
            }
            Err(e) if e == QUOTA => {
                if !wait_for_quota() {
                    return;
                }
            }
            Err(e) if e == ENDED => {
                status(false, "Live chat ended".into());
                return;
            }
            Err(e) => {
                // A refusal before any stream worked means streaming isn't
                // available here: fall back to polling.
                failures += if streamed { 1 } else { 3 };
                status(false, format!("{e} · reconnecting"));
                if !pause(Duration::from_secs(15)) {
                    return;
                }
                status(true, "Connected (YouTube)".into());
            }
        }
    }
    // Polling fallback, slowing down while chat is quiet.
    let mut quiet = MIN_POLL;
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
                if !deliver(&v) {
                    return;
                }
                page = v["nextPageToken"].as_str().map(str::to_owned);
                let asked = v["pollingIntervalMillis"]
                    .as_u64()
                    .map(Duration::from_millis)
                    .unwrap_or(MIN_POLL)
                    .max(MIN_POLL);
                quiet = if v["items"].as_array().is_some_and(|i| !i.is_empty()) {
                    asked
                } else {
                    (quiet + quiet / 2).clamp(asked, IDLE_POLL.max(asked))
                };
                if !pause(quiet) {
                    return;
                }
            }
            Err(e) if e == QUOTA => {
                if !wait_for_quota() {
                    return;
                }
            }
            Err(e) if e == ENDED => {
                status(false, "Live chat ended".into());
                return;
            }
            Err(e) => {
                status(false, format!("{e} · retrying"));
                if !pause(Duration::from_secs(30)) {
                    return;
                }
            }
        }
    }
}

/// The signed-in channel's live broadcast right now, as "yt:<videoId>"
/// (liveBroadcasts.list with broadcastStatus=active; 1 quota unit).
pub fn my_live(access: &str) -> Result<Option<String>, String> {
    let client = crate::live::client()?;
    let response = client
        .get(format!("{API}/liveBroadcasts"))
        .query(&[
            ("part", "id"),
            ("broadcastStatus", "active"),
            ("broadcastType", "all"),
            ("maxResults", "5"),
        ])
        .bearer_auth(access)
        .send()
        .map_err(|_| "YouTube didn't answer".to_owned())?;
    let code = response.status().as_u16();
    let value: Value = response
        .text()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if code != 200 {
        return Err(refusal(code, &value));
    }
    Ok(value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["id"].as_str())
        .find_map(|id| parse(&format!("yt:{id}"))))
}

/// One watcher thread per open YouTube pane.
pub struct YouTube {
    watching: HashMap<String, Arc<AtomicBool>>,
    auth: Arc<Mutex<Auth>>,
    tx: mpsc::Sender<(String, Update)>,
    rx: mpsc::Receiver<(String, Update)>,
}
impl YouTube {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            watching: HashMap::new(),
            auth: Default::default(),
            tx,
            rx,
        }
    }
    fn restart(&mut self) {
        for (_, stop) in self.watching.drain() {
            stop.store(true, Ordering::Relaxed);
        }
    }
    pub fn set_key(&mut self, key: Option<String>) {
        let changed = self.auth.lock().map(|mut a| {
            let changed = a.key != key;
            a.key = key;
            changed
        });
        if changed.unwrap_or(false) {
            self.restart();
        }
    }
    /// Read with this Google sign-in (None: signed out).
    pub fn set_google(
        &mut self,
        google: Option<(crate::google_auth::OAuthClient, crate::google_auth::Tokens)>,
    ) {
        crate::google_auth::SIGNED_IN.store(google.is_some(), std::sync::atomic::Ordering::Relaxed);
        let changed = self.auth.lock().map(|mut a| {
            let changed = a.google.is_some() != google.is_some();
            a.google = google;
            changed
        });
        if changed.unwrap_or(false) {
            self.restart();
        }
    }
    pub fn has_key(&self) -> bool {
        self.auth.lock().map(|a| a.key.is_some()).unwrap_or(false)
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
        let ready = self.auth.lock().map(|a| a.ready()).unwrap_or(false);
        let mut updates = Vec::new();
        for name in names {
            if self.watching.contains_key(name) {
                continue;
            }
            if !ready {
                updates.push((
                    name.clone(),
                    Update::Status {
                        connected: false,
                        text: "Connect YouTube to read and chat here".into(),
                    },
                ));
                continue;
            }
            let auth = self.auth.clone();
            let stop = Arc::new(AtomicBool::new(false));
            self.watching.insert(name.clone(), stop.clone());
            let (name, tx) = (name.clone(), self.tx.clone());
            let _ = std::thread::Builder::new()
                .name("jawjack-youtube".into())
                .spawn(move || watch(name, auth, stop, tx));
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
    fn moderation_events() {
        let deleted = json!({"snippet": {"type": "messageDeletedEvent", "messageDeletedDetails": {"deletedMessageId": "m1"}}});
        assert!(
            matches!(moderation("yt", &deleted), Some(chat_core::Event::DeleteMessage { message_id, .. }) if message_id == "m1")
        );
        let banned = json!({"snippet": {"type": "userBannedEvent", "userBannedDetails": {"bannedUserDetails": {"channelId": "UC1"}}}});
        assert!(
            matches!(moderation("yt", &banned), Some(chat_core::Event::ClearUser { user_id, .. }) if user_id == "yt:UC1")
        );
        assert!(moderation("yt", &json!({"snippet": {"type": "textMessageEvent"}})).is_none());
    }
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
    #[test]
    fn stream_objects_split_across_reads() {
        let body = br#"[{"nextPageToken":"a","items":[{"id":"1","s":"br}ace\"q"}]},
{"nextPageToken":"b","items":[]}
,{"error":{"code":403}}]"#;
        let mut parser = JsonObjects::default();
        let mut got = Vec::new();
        for piece in body.chunks(7) {
            got.extend(parser.push(piece));
        }
        assert_eq!(got.len(), 3);
        assert_eq!(got[0]["items"][0]["s"], "br}ace\"q");
        assert_eq!(got[1]["nextPageToken"], "b");
        assert!(got[2]["error"].is_object());
        // A plain (non-streamed) object works too.
        assert_eq!(JsonObjects::default().push(br#"{"items":[]}"#).len(), 1);
    }
    #[test]
    fn quota_resets_at_pacific_midnight() {
        use chrono::TimeZone;
        // Summer (PDT, UTC-7): 20:00 UTC on 11 Oct -> 07:00 UTC on 12 Oct.
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 10, 11, 20, 0, 0)
            .unwrap();
        assert_eq!(
            next_quota_reset(now),
            chrono::Utc.with_ymd_and_hms(2026, 10, 12, 7, 0, 0).unwrap()
        );
        // Winter (PST, UTC-8).
        let now = chrono::Utc.with_ymd_and_hms(2026, 12, 1, 12, 0, 0).unwrap();
        assert_eq!(
            next_quota_reset(now),
            chrono::Utc.with_ymd_and_hms(2026, 12, 2, 8, 0, 0).unwrap()
        );
        // Just after midnight PT: the next reset is a day later, not now.
        let now = chrono::Utc.with_ymd_and_hms(2026, 10, 12, 7, 0, 1).unwrap();
        assert_eq!(
            next_quota_reset(now),
            chrono::Utc.with_ymd_and_hms(2026, 10, 13, 7, 0, 0).unwrap()
        );
        // DST ends 1 Nov 2026: midnight 1 Nov is still PDT; midnight 2 Nov is PST.
        let now = chrono::Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
        assert_eq!(
            next_quota_reset(now),
            chrono::Utc.with_ymd_and_hms(2026, 11, 2, 8, 0, 0).unwrap()
        );
    }
}
