//! Recent chat history when a channel opens, from the recent-messages service
//! Chatterino uses (recent-messages.robotty.de). Its IRC lines are converted into
//! the EventSub event shape so they go through the same message builder as live
//! chat. History rows are marked `historical` and never count as unread.
use chat_core::Message;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::mpsc};

pub const SERVICE: &str = "recent-messages.robotty.de";
const LIMIT: usize = 500;
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
        .get(format!("https://{SERVICE}/api/v2/recent-messages/{login}"))
        .query(&[("limit", LIMIT.to_string())])
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
        .take(LIMIT)
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

struct Irc<'a> {
    tags: HashMap<&'a str, String>,
    nick: &'a str,
    command: &'a str,
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
        assert_eq!(messages.len(), 2);
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
    fn escaped_tags() {
        assert_eq!(unescape(r"a\sb\:c\\d"), r"a b;c\d");
    }
}
