//! Recent chat history when a channel opens, from the recent-messages service
//! Chatterino uses (recent-messages.robotty.de). Its IRC lines are converted into
//! the EventSub event shape so they go through the same message builder as live
//! chat. History rows are marked `historical` and never count as unread.
use chat_core::Message;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::mpsc};

pub const SERVICE: &str = "recent-messages.robotty.de";
/// Your own recent-messages server (Settings): an https URL with
/// `{channel}` where the channel login goes. Empty uses SERVICE.
pub static CUSTOM_URL: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());
/// Check a custom URL template; returns it trimmed.
pub fn validate_url(template: &str) -> Result<String, String> {
    let template = template.trim();
    if template.is_empty() {
        return Ok(String::new());
    }
    let probe = template.replace("{channel}", "channel");
    let url = reqwest::Url::parse(&probe).map_err(|_| "That isn't a web address.".to_owned())?;
    if url.scheme() != "https" || !template.contains("{channel}") || template.len() > 300 {
        return Err("Use an https:// address with {channel} where the channel name goes.".into());
    }
    Ok(template.to_owned())
}
fn endpoint(login: &str) -> String {
    let custom = CUSTOM_URL.read().map(|u| u.clone()).unwrap_or_default();
    if custom.is_empty() {
        format!("https://{SERVICE}/api/v2/recent-messages/{login}")
    } else {
        custom.replace("{channel}", login)
    }
}
/// Where history comes from, for Settings.
pub fn source() -> String {
    let custom = CUSTOM_URL.read().map(|u| u.clone()).unwrap_or_default();
    reqwest::Url::parse(&custom.replace("{channel}", "c"))
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| SERVICE.to_owned())
}
/// Settings → Chat → Load recent messages: how many (an index into `LIMITS`).
pub const LIMITS: [usize; 4] = [100, 250, 500, 800];
pub static LIMIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(2);
fn limit() -> usize {
    LIMITS[LIMIT
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(LIMITS.len() - 1)]
}
const MAX_BODY: u64 = 4 * 1024 * 1024;

pub struct RecentMessages {
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<(String, Result<Vec<Message>, String>)>,
}
impl RecentMessages {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::channel::<String>();
        let (results, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(client) = crate::live::client() else {
                return;
            };
            while let Ok(channel) = jobs.recv() {
                let result = fetch(&client, &channel);
                if results.send((channel, result)).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx }
    }
    pub fn request(&self, channel: &str) {
        let _ = self.tx.send(channel.to_owned());
    }
    pub fn poll(&self) -> Option<(String, Result<Vec<Message>, String>)> {
        self.rx.try_recv().ok()
    }
}

fn fetch(client: &reqwest::blocking::Client, channel: &str) -> Result<Vec<Message>, String> {
    use std::io::Read;
    let login = chat_core::twitch_login(channel).ok_or("Invalid channel")?;
    let response = client
        .get(endpoint(&login))
        .query(&[("limit", limit().to_string())])
        .header("User-Agent", concat!("Jawjack/", env!("CARGO_PKG_VERSION")))
        .send()
        .map_err(|_| "Recent messages unavailable")?;
    if !response.status().is_success() {
        return Err(format!(
            "Recent messages unavailable (HTTP {})",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Recent messages interrupted")?;
    if bytes.len() as u64 > MAX_BODY {
        return Err("Recent messages response too large".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Unexpected recent-messages reply")?;
    let lines: Vec<&str> = value["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(limit())
        .collect();
    Ok(convert(&lines, &login))
}

/// Convert raw IRC lines into messages, applying CLEARCHAT/CLEARMSG redactions.
pub fn convert(lines: &[&str], channel: &str) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    for line in lines {
        let Some(irc) = Irc::parse(line) else {
            continue;
        };
        match irc.command {
            "PRIVMSG" | "USERNOTICE" => {
                if let Some(message) = message(&irc, channel) {
                    messages.push(message);
                }
            }
            "CLEARMSG" => {
                if let Some(target) = irc.tags.get("target-msg-id") {
                    for m in messages.iter_mut().filter(|m| &m.id == target) {
                        redact(m);
                    }
                }
            }
            "CLEARCHAT" => {
                let user = irc.tags.get("target-user-id");
                for m in messages
                    .iter_mut()
                    .filter(|m| user.is_none_or(|u| &m.user_id == u))
                {
                    redact(m);
                }
                // "bob was timed out for 10m.", named as they last chatted.
                if let Some((user_id, mut text, sent_at)) = timeout(&irc) {
                    if let Some(m) = messages.iter().rev().find(|m| m.user_id == user_id) {
                        text = text.replacen(irc.text, &m.display_name, 1);
                    }
                    let id = format!("mod:{user_id}:{}", sent_at.unwrap_or(0));
                    let mut line = chat_core::moderation_notice(
                        channel,
                        &user_id,
                        id,
                        sent_at,
                        text,
                        chat_core::Detail::Duration,
                    );
                    line.presentation.historical = true;
                    messages.push(line);
                }
            }
            _ => {}
        }
    }
    messages
}
fn redact(message: &mut Message) {
    // Kept readable and shown greyed out, like a live deletion.
    message.deleted = true;
}

/// A CLEARCHAT for one chatter: (user ID, "login was timed out for 10m." or
/// "login was banned.", sent at). IRC gives every viewer the length.
fn timeout(irc: &Irc) -> Option<(String, String, Option<i64>)> {
    let user_id = irc.tags.get("target-user-id")?.clone();
    let login = irc.text.trim();
    if login.is_empty() {
        return None;
    }
    let text = match irc
        .tags
        .get("ban-duration")
        .and_then(|s| s.parse::<u64>().ok())
    {
        Some(seconds) => format!("{login} was timed out for {}.", duration(seconds)),
        None => format!("{login} was banned."),
    };
    let sent_at = irc
        .tags
        .get("tmi-sent-ts")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|ms| ms / 1000);
    Some((user_id, text, sent_at))
}
/// "45s", "10m", "1h 30m", "7d", like Chatterino.
pub fn duration(seconds: u64) -> String {
    let parts = [
        (seconds / 86_400, "d"),
        (seconds % 86_400 / 3600, "h"),
        (seconds % 3600 / 60, "m"),
        (seconds % 60, "s"),
    ];
    let text: Vec<String> = parts
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, unit)| format!("{n}{unit}"))
        .collect();
    if text.is_empty() {
        "0s".into()
    } else {
        text.join(" ")
    }
}

/// What a live IRC line means (guest reading, and timeout lengths when signed in).
pub enum Live {
    Message(Message),
    Delete(String),
    /// A chatter timed out or banned: (user ID, the line to show, sent at).
    ClearUser(String, String, Option<i64>),
    Clear,
    /// The channel's Twitch ID (ROOMSTATE), for its emotes.
    Room(String),
    /// A chatter joined (true) or left (false); Twitch only sends these in
    /// channels with fewer than 1,000 chatters.
    Membership(String, bool),
}
/// Parse one live IRC line into (channel login, event).
pub fn live_line(line: &str) -> Option<(String, Live)> {
    let irc = Irc::parse(line)?;
    let channel = irc.target.strip_prefix('#')?.to_ascii_lowercase();
    let live = match irc.command {
        "PRIVMSG" | "USERNOTICE" => {
            let mut message = message(&irc, &channel)?;
            message.presentation.historical = false;
            Live::Message(message)
        }
        "CLEARMSG" => Live::Delete(irc.tags.get("target-msg-id")?.clone()),
        "CLEARCHAT" => match timeout(&irc) {
            Some((user_id, text, sent_at)) => Live::ClearUser(user_id, text, sent_at),
            None if irc.tags.contains_key("target-user-id") => return None,
            None => Live::Clear,
        },
        "ROOMSTATE" => Live::Room(irc.tags.get("room-id").filter(|id| !id.is_empty())?.clone()),
        // Guests (justinfan…) are anonymous readers like us; never shown.
        "JOIN" | "PART" if !irc.nick.is_empty() && !irc.nick.starts_with("justinfan") => {
            Live::Membership(irc.nick.to_ascii_lowercase(), irc.command == "JOIN")
        }
        _ => return None,
    };
    Some((channel, live))
}

struct Irc<'a> {
    tags: HashMap<&'a str, String>,
    nick: &'a str,
    command: &'a str,
    /// The first parameter: "#channel" for chat lines.
    target: &'a str,
    text: &'a str,
}
impl<'a> Irc<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let mut rest = line.trim_end_matches(['\r', '\n']);
        let mut tags = HashMap::new();
        if let Some(stripped) = rest.strip_prefix('@') {
            let (raw, after) = stripped.split_once(' ')?;
            for tag in raw.split(';') {
                let (key, value) = tag.split_once('=').unwrap_or((tag, ""));
                tags.insert(key, unescape(value));
            }
            rest = after;
        }
        let mut nick = "";
        if let Some(stripped) = rest.strip_prefix(':') {
            let (prefix, after) = stripped.split_once(' ')?;
            nick = prefix.split('!').next().unwrap_or("");
            rest = after;
        }
        let (command, after) = rest.split_once(' ').unwrap_or((rest, ""));
        let target = after.split(' ').next().unwrap_or("");
        // The trailing parameter only needs a colon when it contains spaces,
        // so a one-word message arrives as `#channel word`.
        let text = match after.split_once(" :") {
            Some((_, t)) => t,
            None => after.split_once(' ').map(|(_, t)| t).unwrap_or(""),
        };
        Some(Self {
            tags,
            nick,
            command,
            target,
            text,
        })
    }
}
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some(':') => out.push(';'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// `25:0-4,12-16/1902:6-10` → text/emote fragments in EventSub's shape.
fn fragments(text: &str, emotes: Option<&String>) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    let mut ranges: Vec<(usize, usize, &str)> = Vec::new();
    for entry in emotes.map(String::as_str).unwrap_or("").split('/') {
        let Some((id, positions)) = entry.split_once(':') else {
            continue;
        };
        for range in positions.split(',') {
            if let Some((a, b)) = range.split_once('-')
                && let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>())
                && a <= b
                && b < chars.len()
            {
                ranges.push((a, b, id));
            }
        }
    }
    ranges.sort_by_key(|r| r.0);
    let mut out = Vec::new();
    let mut at = 0;
    for (a, b, id) in ranges {
        if a < at {
            continue;
        }
        if a > at {
            out.push(json!({"type":"text","text":chars[at..a].iter().collect::<String>()}));
        }
        out.push(json!({"type":"emote","text":chars[a..=b].iter().collect::<String>(),"emote":{"id":id}}));
        at = b + 1;
    }
    if at < chars.len() {
        out.push(json!({"type":"text","text":chars[at..].iter().collect::<String>()}));
    }
    out
}

fn message(irc: &Irc, channel: &str) -> Option<Message> {
    let tag = |key: &str| irc.tags.get(key).cloned().unwrap_or_default();
    let id = tag("id");
    if id.is_empty() {
        return None;
    }
    let login = irc
        .tags
        .get("login")
        .map(String::as_str)
        .unwrap_or(irc.nick);
    let notice = irc.command == "USERNOTICE";
    let notice_type = notice.then(|| match tag("msg-id").as_str() {
        "subgift" => "sub_gift".to_owned(),
        "submysterygift" => "community_sub_gift".into(),
        "giftpaidupgrade" => "gift_paid_upgrade".into(),
        "primepaidupgrade" => "prime_paid_upgrade".into(),
        "paybackgift" => "pay_it_forward".into(),
        "bitsbadgetier" => "bits_badge_tier".into(),
        // Watch streaks (as EventSub names them).
        "viewermilestone" => "watch_streak".into(),
        // Shared Chat: the original notice type is in source-msg-id.
        "sharedchatnotice" => format!("shared_chat_{}", tag("source-msg-id")),
        other => other.to_owned(),
    });
    let action = irc.text.starts_with("\u{1}ACTION ");
    let message_type = match tag("msg-id").as_str() {
        _ if action => "action",
        "highlighted-message" => "channel_points_highlighted",
        "user-intro" => "user_intro",
        _ => "text",
    };
    // `/me` arrives wrapped in CTCP ACTION markers.
    let body = irc
        .text
        .strip_prefix("\u{1}ACTION ")
        .map(|t| t.trim_end_matches('\u{1}'))
        .unwrap_or(irc.text);
    let badges: Vec<Value> = tag("badges")
        .split(',')
        .filter_map(|b| b.split_once('/'))
        .map(|(set, version)| json!({"set_id": set, "id": version}))
        .collect();
    let mut event = json!({
        "message_id": id,
        "chatter_user_id": tag("user-id"),
        "chatter_user_login": login,
        "chatter_user_name": irc.tags.get("display-name").filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| login.to_owned()),
        "color": tag("color"),
        "badges": badges,
        "message_type": message_type,
        "message": {"text": body, "fragments": fragments(body, irc.tags.get("emotes"))},
    });
    // Shared Chat: sent in another channel of the session.
    if let Some(source) = irc
        .tags
        .get("source-room-id")
        .filter(|source| !source.is_empty() && **source != tag("room-id"))
    {
        event["source_broadcaster_user_id"] = json!(source);
    }
    if let Some(kind) = &notice_type {
        event["notice_type"] = json!(kind);
        event["system_message"] = json!(tag("system-msg"));
    }
    if let Ok(bits) = tag("bits").parse::<u64>() {
        event["cheer"] = json!({"bits": bits});
    }
    if let Some(reward) = irc.tags.get("custom-reward-id") {
        event["channel_points_custom_reward_id"] = json!(reward);
    }
    if let Some(parent) = irc.tags.get("reply-parent-msg-id") {
        event["reply"] = json!({
            "parent_message_id": parent,
            "parent_user_id": tag("reply-parent-user-id"),
            "parent_user_name": tag("reply-parent-display-name"),
            "thread_message_id": irc.tags.get("reply-thread-parent-msg-id").cloned().unwrap_or_else(|| parent.clone()),
        });
    }
    let sent_at = tag("tmi-sent-ts").parse::<i64>().ok().map(|ms| ms / 1000);
    let room = tag("room-id");
    let mut message = crate::live::chat_message(&event, &id, channel, &room, sent_at, !notice);
    message.presentation.historical = true;
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privmsg_with_emotes_and_redaction() {
        let lines = [
            "@badges=subscriber/12;color=#FF0000;display-name=Alice;emotes=25:6-10;id=a1;room-id=1;tmi-sent-ts=1700000000000;user-id=10 :alice!alice@alice.tmi.twitch.tv PRIVMSG #chan :hello Kappa there",
            "@display-name=Bob;id=b1;room-id=1;tmi-sent-ts=1700000001000;user-id=11 :bob!bob@bob.tmi.twitch.tv PRIVMSG #chan :spam",
            "@room-id=1;target-user-id=11;tmi-sent-ts=1700000002000 :tmi.twitch.tv CLEARCHAT #chan :bob",
        ];
        let messages = convert(&lines, "chan");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2].body(), "Bob was banned.");
        assert_eq!(messages[0].display_name, "Alice");
        assert_eq!(messages[0].name_color, Some(0xFF0000));
        assert!(messages[0].presentation.historical);
        assert!(
            matches!(&messages[0].fragments[1], chat_core::Fragment::Emote { label, .. } if label == "Kappa")
        );
        assert!(messages[1].deleted);
        assert_eq!(messages[0].sent_at, Some(1_700_000_000));
    }
    #[test]
    fn single_word_without_colon() {
        let lines =
            ["@display-name=K;id=k1;room-id=1;user-id=12 :k!k@k.tmi.twitch.tv PRIVMSG #chan HOT*"];
        assert_eq!(convert(&lines, "chan")[0].body(), "HOT*");
    }
    #[test]
    fn live_lines() {
        let (channel, live) = live_line("@badges=;display-name=Al;id=x1;room-id=5;user-id=9 :al!al@al.tmi.twitch.tv PRIVMSG #Chan :hi there").unwrap();
        assert_eq!(channel, "chan");
        assert!(
            matches!(live, Live::Message(m) if m.body() == "hi there" && !m.presentation.historical)
        );
        assert!(
            matches!(live_line("@room-id=5;slow=0 :tmi.twitch.tv ROOMSTATE #chan").unwrap().1, Live::Room(id) if id == "5")
        );
        assert!(matches!(
            live_line("@room-id=5 :tmi.twitch.tv CLEARCHAT #chan")
                .unwrap()
                .1,
            Live::Clear
        ));
        assert!(matches!(
            live_line("@ban-duration=5400;room-id=5;target-user-id=7;tmi-sent-ts=1700000002000 :tmi.twitch.tv CLEARCHAT #chan :bob")
                .unwrap()
                .1,
            Live::ClearUser(id, text, Some(1_700_000_002)) if id == "7" && text == "bob was timed out for 1h 30m."
        ));
        assert_eq!(duration(600), "10m");
        assert_eq!(duration(1_209_600), "14d");
        assert!(live_line(":tmi.twitch.tv 001 justinfan1 :Welcome").is_none());
        assert!(matches!(
            live_line(":bob!bob@bob.tmi.twitch.tv JOIN #chan").unwrap().1,
            Live::Membership(login, true) if login == "bob"
        ));
        assert!(live_line(":justinfan5!justinfan5@justinfan5.tmi.twitch.tv PART #chan").is_none());
    }
    #[test]
    fn escaped_tags() {
        assert_eq!(unescape(r"a\sb\:c\\d"), r"a b;c\d");
    }
}

#[cfg(test)]
mod url_tests {
    use super::{CUSTOM_URL, endpoint, validate_url};
    #[test]
    fn custom_server_urls_are_checked_and_used() {
        assert_eq!(validate_url("  ").unwrap(), "");
        assert!(validate_url("http://x.example/{channel}").is_err());
        assert!(validate_url("https://x.example/recent").is_err());
        let url = validate_url("https://x.example/api/v2/recent-messages/{channel}").unwrap();
        *CUSTOM_URL.write().unwrap() = url;
        assert_eq!(
            endpoint("chan"),
            "https://x.example/api/v2/recent-messages/chan"
        );
        CUSTOM_URL.write().unwrap().clear();
        assert!(endpoint("chan").contains("robotty"));
    }
}
