//! Chatterino's Twitch slash commands, executed through Helix.
//!
//! Commands are parsed in the composer, checked against the channel role and
//! granted scopes (capabilities.rs), then run off the UI thread. Every result is
//! Twitch's own acknowledgement; nothing is assumed to have worked.
use crate::{auth::CLIENT_ID, capabilities::Action, live::Identity};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Ban {
        login: String,
        reason: String,
    },
    Timeout {
        login: String,
        seconds: u32,
        reason: String,
    },
    Unban {
        login: String,
    },
    Warn {
        login: String,
        reason: String,
    },
    Delete {
        message_id: String,
    },
    Clear,
    /// A partial Helix chat-settings patch.
    Settings {
        patch: Value,
        label: String,
    },
    Announce {
        message: String,
        color: &'static str,
    },
    Shoutout {
        login: String,
    },
    Moderator {
        login: String,
        add: bool,
    },
    Vip {
        login: String,
        add: bool,
    },
    Color {
        color: String,
    },
    Whisper {
        login: String,
        message: String,
    },
    Block {
        login: String,
        add: bool,
    },
    Title {
        title: String,
    },
    Game {
        query: String,
    },
    Marker {
        description: String,
    },
    Commercial {
        seconds: u32,
    },
    Raid {
        login: String,
    },
    Unraid,
    /// Clip the last ~30 seconds of this channel's live stream.
    Clip,
    Shield {
        on: bool,
    },
    Chatters,
    Moderators,
    Vips,
}

pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
}
pub const COMMANDS: &[Command] = &[
    Command {
        name: "/clip",
        usage: "/clip",
        description: "Clip the last ~30 seconds of this live stream",
    },
    Command {
        name: "/ban",
        usage: "/ban login [reason]",
        description: "Ban a chatter (moderator)",
    },
    Command {
        name: "/timeout",
        usage: "/timeout login [10m] [reason]",
        description: "Time out a chatter; default 10 minutes, up to 2 weeks",
    },
    Command {
        name: "/unban",
        usage: "/unban login",
        description: "Lift a ban or timeout (also /untimeout)",
    },
    Command {
        name: "/warn",
        usage: "/warn login reason",
        description: "Warn a chatter; they must acknowledge it",
    },
    Command {
        name: "/delete",
        usage: "/delete message-id",
        description: "Delete one message",
    },
    Command {
        name: "/clear",
        usage: "/clear",
        description: "Clear chat for everyone",
    },
    Command {
        name: "/slow",
        usage: "/slow [seconds]",
        description: "Slow mode (default 30 s); /slowoff ends it",
    },
    Command {
        name: "/followers",
        usage: "/followers [30m]",
        description: "Followers-only mode; /followersoff ends it",
    },
    Command {
        name: "/subscribers",
        usage: "/subscribers",
        description: "Subscriber-only mode; /subscribersoff ends it",
    },
    Command {
        name: "/emoteonly",
        usage: "/emoteonly",
        description: "Emote-only mode; /emoteonlyoff ends it",
    },
    Command {
        name: "/uniquechat",
        usage: "/uniquechat",
        description: "Unique-chat mode; /uniquechatoff ends it",
    },
    Command {
        name: "/announce",
        usage: "/announce message",
        description: "Announcement (also /announceblue, green, orange, purple)",
    },
    Command {
        name: "/shoutout",
        usage: "/shoutout login",
        description: "Shout out another channel",
    },
    Command {
        name: "/mod",
        usage: "/mod login",
        description: "Make a moderator (broadcaster); /unmod removes",
    },
    Command {
        name: "/vip",
        usage: "/vip login",
        description: "Make a VIP (broadcaster); /unvip removes",
    },
    Command {
        name: "/color",
        usage: "/color name-or-#hex",
        description: "Change your chat name colour",
    },
    Command {
        name: "/w",
        usage: "/w login message",
        description: "Send a whisper",
    },
    Command {
        name: "/block",
        usage: "/block login",
        description: "Block a user on Twitch; /unblock reverses",
    },
    Command {
        name: "/settitle",
        usage: "/settitle title",
        description: "Change your stream title",
    },
    Command {
        name: "/setgame",
        usage: "/setgame category",
        description: "Change your stream category",
    },
    Command {
        name: "/marker",
        usage: "/marker [description]",
        description: "Add a stream marker while live",
    },
    Command {
        name: "/commercial",
        usage: "/commercial [30]",
        description: "Run an ad (30–180 s) while live",
    },
    Command {
        name: "/raid",
        usage: "/raid login",
        description: "Start a raid; /unraid cancels",
    },
    Command {
        name: "/shield",
        usage: "/shield",
        description: "Turn on Shield Mode; /shieldoff turns it off",
    },
    Command {
        name: "/mods",
        usage: "/mods",
        description: "List this channel's moderators (broadcaster)",
    },
    Command {
        name: "/vips",
        usage: "/vips",
        description: "List this channel's VIPs (broadcaster)",
    },
    Command {
        name: "/chatters",
        usage: "/chatters",
        description: "How many chatters are connected (moderator)",
    },
];

fn login(raw: &str) -> Result<String, String> {
    chat_core::twitch_login(raw.trim().trim_start_matches('@'))
        .ok_or_else(|| "Enter a Twitch login.".to_owned())
}
fn split(value: &str) -> (&str, &str) {
    let value = value.trim();
    value
        .find(char::is_whitespace)
        .map_or((value, ""), |at| (&value[..at], value[at..].trim()))
}
/// `90`, `90s`, `10m`, `2h`, `1d`, `1w` → seconds.
pub fn duration(value: &str) -> Option<u32> {
    let value = value.trim().to_ascii_lowercase();
    let (number, unit) = match value.find(|c: char| !c.is_ascii_digit()) {
        Some(at) => value.split_at(at),
        None => (value.as_str(), "s"),
    };
    let number: u64 = number.parse().ok()?;
    let scale = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => 1,
        "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hour" | "hours" => 3600,
        "d" | "day" | "days" => 86_400,
        "w" | "week" | "weeks" => 604_800,
        _ => return None,
    };
    u32::try_from(number.checked_mul(scale)?).ok()
}
fn text(value: &str, limit: usize, what: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("Enter {what}."));
    }
    if value.chars().count() > limit {
        return Err(format!("{what} can be up to {limit} characters."));
    }
    Ok(value.to_owned())
}
fn mode(field: &str, on: bool, label: &str) -> Request {
    Request::Settings {
        patch: json!({ field: on }),
        label: format!("{label} {}", if on { "on" } else { "off" }),
    }
}

/// Parse a Twitch command. `None` means the name isn't a Twitch command.
pub fn parse(name: &str, args: &str) -> Option<Result<Request, String>> {
    let (first, rest) = split(args);
    let no_args = |request: Request| {
        if args.is_empty() {
            Ok(request)
        } else {
            Err(format!("{name} takes no arguments."))
        }
    };
    Some(match name {
        "/ban" => login(first).map(|login| Request::Ban {
            login,
            reason: rest.chars().take(500).collect(),
        }),
        "/timeout" => login(first).and_then(|login| {
            let (length, reason) = split(rest);
            let (seconds, reason) = match duration(length) {
                Some(seconds) => (seconds, reason),
                None => (600, rest),
            };
            if !(1..=1_209_600).contains(&seconds) {
                return Err("Timeouts last 1 second to 2 weeks.".into());
            }
            Ok(Request::Timeout {
                login,
                seconds,
                reason: reason.chars().take(500).collect(),
            })
        }),
        "/unban" | "/untimeout" => login(first).map(|login| Request::Unban { login }),
        "/warn" => login(first).and_then(|login| {
            Ok(Request::Warn {
                login,
                reason: text(rest, 500, "a reason")?,
            })
        }),
        "/delete" => {
            if first.is_empty() || !rest.is_empty() {
                Err("Use /delete message-id, or Delete on the message's card.".into())
            } else {
                Ok(Request::Delete {
                    message_id: first.chars().take(64).collect(),
                })
            }
        }
        "/clear" => no_args(Request::Clear),
        "/slow" => {
            let seconds = if args.is_empty() {
                Some(30)
            } else {
                duration(args)
            };
            match seconds {
                Some(s) if (3..=120).contains(&s) => Ok(Request::Settings {
                    patch: json!({"slow_mode": true, "slow_mode_wait_time": s}),
                    label: format!("Slow mode {s} s"),
                }),
                _ => Err("Slow mode waits 3–120 seconds.".into()),
            }
        }
        "/slowoff" => no_args(mode("slow_mode", false, "Slow mode")),
        "/followers" => {
            let minutes = if args.is_empty() {
                Some(0)
            } else {
                duration(args).map(|s| s / 60)
            };
            match minutes {
                Some(m) if m <= 129_600 => Ok(Request::Settings {
                    patch: json!({"follower_mode": true, "follower_mode_duration": m}),
                    label: if m == 0 {
                        "Followers-only mode on".into()
                    } else {
                        format!("Followers-only mode ({m} min)")
                    },
                }),
                _ => Err("Use /followers or /followers 30m (up to 3 months).".into()),
            }
        }
        "/followersoff" => no_args(mode("follower_mode", false, "Followers-only mode")),
        "/subscribers" => no_args(mode("subscriber_mode", true, "Subscriber-only mode")),
        "/subscribersoff" => no_args(mode("subscriber_mode", false, "Subscriber-only mode")),
        "/emoteonly" => no_args(mode("emote_mode", true, "Emote-only mode")),
        "/emoteonlyoff" => no_args(mode("emote_mode", false, "Emote-only mode")),
        "/uniquechat" | "/r9kbeta" => no_args(mode("unique_chat_mode", true, "Unique-chat mode")),
        "/uniquechatoff" | "/r9kbetaoff" => {
            no_args(mode("unique_chat_mode", false, "Unique-chat mode"))
        }
        "/announce" | "/announceblue" | "/announcegreen" | "/announceorange"
        | "/announcepurple" => {
            text(args, 500, "an announcement").map(|message| Request::Announce {
                message,
                color: match name {
                    "/announceblue" => "blue",
                    "/announcegreen" => "green",
                    "/announceorange" => "orange",
                    "/announcepurple" => "purple",
                    _ => "primary",
                },
            })
        }
        "/shoutout" | "/so" => login(first).map(|login| Request::Shoutout { login }),
        "/mod" | "/unmod" => login(first).map(|login| Request::Moderator {
            login,
            add: name == "/mod",
        }),
        "/vip" | "/unvip" => login(first).map(|login| Request::Vip {
            login,
            add: name == "/vip",
        }),
        "/color" => {
            if first.is_empty() || !rest.is_empty() {
                Err("Use /color name or /color #hex (hex needs Turbo or Prime).".into())
            } else {
                Ok(Request::Color {
                    color: first.to_ascii_lowercase(),
                })
            }
        }
        "/w" | "/whisper" => login(first).and_then(|login| {
            Ok(Request::Whisper {
                login,
                message: text(rest, 500, "a message")?,
            })
        }),
        "/block" | "/unblock" => login(first).map(|login| Request::Block {
            login,
            add: name == "/block",
        }),
        "/settitle" => text(args, 140, "a title").map(|title| Request::Title { title }),
        "/setgame" => text(args, 100, "a category").map(|query| Request::Game { query }),
        "/marker" => Ok(Request::Marker {
            description: args.chars().take(140).collect(),
        }),
        "/commercial" => {
            let seconds = if args.is_empty() {
                Some(30)
            } else {
                duration(args)
            };
            match seconds {
                Some(s) if (30..=180).contains(&s) => Ok(Request::Commercial { seconds: s }),
                _ => Err("Ads run 30–180 seconds.".into()),
            }
        }
        "/raid" => login(first).map(|login| Request::Raid { login }),
        "/unraid" => no_args(Request::Unraid),
        "/clip" => no_args(Request::Clip),
        "/shield" | "/shieldoff" => no_args(Request::Shield {
            on: name == "/shield",
        }),
        "/chatters" => no_args(Request::Chatters),
        "/mods" => no_args(Request::Moderators),
        "/vips" => no_args(Request::Vips),
        _ => return None,
    })
}

impl Request {
    pub fn action(&self) -> Action {
        use Request::*;
        match self {
            Ban { .. } | Timeout { .. } | Unban { .. } => Action::Ban,
            Warn { .. } => Action::Warn,
            Delete { .. } | Clear => Action::DeleteMessages,
            Settings { .. } => Action::ManageRoom,
            Announce { .. } => Action::Announce,
            Shoutout { .. } => Action::Shoutout,
            Moderator { .. } => Action::ManageModerators,
            Vip { .. } => Action::ManageVips,
            Color { .. } => Action::ChatColor,
            Whisper { .. } => Action::Whisper,
            Block { .. } => Action::Block,
            Title { .. } | Game { .. } => Action::EditStreamInfo,
            Marker { .. } => Action::Marker,
            Commercial { .. } => Action::Commercial,
            Raid { .. } | Unraid => Action::Raid,
            Clip => Action::Clip,
            Shield { .. } => Action::Shield,
            Chatters => Action::ReadChatters,
            Moderators => Action::ReadModerators,
            Vips => Action::ReadVips,
        }
    }
}

struct Helix<'a> {
    client: reqwest::blocking::Client,
    identity: &'a Identity,
}
impl Helix<'_> {
    fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<(u16, Value), String> {
        let mut request = self
            .client
            .request(method, format!("https://api.twitch.tv/helix/{path}"))
            .query(query)
            .header("Client-Id", CLIENT_ID)
            .bearer_auth(&self.identity.access);
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body.to_string());
        }
        let response = request
            .send()
            .map_err(|_| "Could not reach Twitch; nothing was changed.".to_owned())?;
        let status = response.status().as_u16();
        if status == 204 {
            return Ok((status, Value::Null));
        }
        crate::live::response_json(response)
    }
    /// Run and require a 2xx answer, explaining Twitch's refusal otherwise.
    fn ok(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let (status, value) = self.call(method, path, query, body)?;
        if (200..300).contains(&status) {
            return Ok(value);
        }
        let message: String = value["message"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(240)
            .collect();
        Err(match status {
            401 => "Twitch rejected the session. Update Twitch permissions from the account menu."
                .into(),
            429 => "Twitch is rate limiting; try again in a moment.".into(),
            _ if !message.is_empty() => format!("Twitch: {message}"),
            _ => format!("Twitch refused (HTTP {status})."),
        })
    }
    fn user_id(&self, login: &str) -> Result<String, String> {
        let value = self.ok(reqwest::Method::GET, "users", &[("login", login)], None)?;
        value["data"][0]["id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("No Twitch account named {login}."))
    }
}

/// Execute against `channel` as the signed-in user; returns Twitch-confirmed text.
pub fn run(identity: &Identity, channel: &str, request: &Request) -> Result<String, String> {
    use reqwest::Method as M;
    let helix = Helix {
        client: crate::live::client()?,
        identity,
    };
    let me = identity.user_id.as_str();
    let room = helix.user_id(channel)?;
    let room = room.as_str();
    let moderation = [("broadcaster_id", room), ("moderator_id", me)];
    match request {
        Request::Ban { login, reason } | Request::Timeout { login, reason, .. } => {
            let target = helix.user_id(login)?;
            let mut data = json!({"user_id": target});
            if !reason.is_empty() {
                data["reason"] = json!(reason);
            }
            if let Request::Timeout { seconds, .. } = request {
                data["duration"] = json!(seconds);
            }
            helix.ok(
                M::POST,
                "moderation/bans",
                &moderation,
                Some(json!({"data": data})),
            )?;
            Ok(match request {
                Request::Timeout { seconds, .. } => {
                    format!("@{login} timed out for {}", human(*seconds))
                }
                _ => format!("@{login} banned"),
            })
        }
        Request::Unban { login } => {
            let target = helix.user_id(login)?;
            helix.ok(
                M::DELETE,
                "moderation/bans",
                &[
                    ("broadcaster_id", room),
                    ("moderator_id", me),
                    ("user_id", &target),
                ],
                None,
            )?;
            Ok(format!("@{login} unbanned"))
        }
        Request::Warn { login, reason } => {
            let target = helix.user_id(login)?;
            helix.ok(
                M::POST,
                "moderation/warnings",
                &moderation,
                Some(json!({"data": {"user_id": target, "reason": reason}})),
            )?;
            Ok(format!("@{login} warned"))
        }
        Request::Delete { message_id } => {
            helix.ok(
                M::DELETE,
                "moderation/chat",
                &[
                    ("broadcaster_id", room),
                    ("moderator_id", me),
                    ("message_id", message_id),
                ],
                None,
            )?;
            Ok("Message deleted".into())
        }
        Request::Clear => {
            helix.ok(M::DELETE, "moderation/chat", &moderation, None)?;
            Ok("Chat cleared".into())
        }
        Request::Settings { patch, label } => {
            helix.ok(M::PATCH, "chat/settings", &moderation, Some(patch.clone()))?;
            Ok(label.clone())
        }
        Request::Announce { message, color } => {
            helix.ok(
                M::POST,
                "chat/announcements",
                &moderation,
                Some(json!({"message": message, "color": color})),
            )?;
            Ok("Announcement sent".into())
        }
        Request::Shoutout { login } => {
            let target = helix.user_id(login)?;
            helix.ok(
                M::POST,
                "chat/shoutouts",
                &[
                    ("from_broadcaster_id", room),
                    ("to_broadcaster_id", &target),
                    ("moderator_id", me),
                ],
                None,
            )?;
            Ok(format!("Shoutout to @{login} sent"))
        }
        Request::Moderator { login, add } | Request::Vip { login, add } => {
            let target = helix.user_id(login)?;
            let path = if matches!(request, Request::Moderator { .. }) {
                "moderation/moderators"
            } else {
                "channels/vips"
            };
            helix.ok(
                if *add { M::POST } else { M::DELETE },
                path,
                &[("broadcaster_id", room), ("user_id", &target)],
                None,
            )?;
            let role = if matches!(request, Request::Moderator { .. }) {
                "a moderator"
            } else {
                "a VIP"
            };
            Ok(if *add {
                format!("@{login} is now {role}")
            } else {
                format!("@{login} is no longer {role}")
            })
        }
        Request::Color { color } => {
            helix.ok(
                M::PUT,
                "chat/color",
                &[("user_id", me), ("color", color)],
                None,
            )?;
            Ok(format!("Name colour changed to {color}"))
        }
        Request::Whisper { login, message } => {
            let target = helix.user_id(login)?;
            helix.ok(
                M::POST,
                "whispers",
                &[("from_user_id", me), ("to_user_id", &target)],
                Some(json!({"message": message})),
            )?;
            Ok(format!("Whisper sent to @{login}"))
        }
        Request::Block { login, add } => {
            let target = helix.user_id(login)?;
            helix.ok(
                if *add { M::PUT } else { M::DELETE },
                "users/blocks",
                &[("target_user_id", &target)],
                None,
            )?;
            Ok(if *add {
                format!("@{login} blocked")
            } else {
                format!("@{login} unblocked")
            })
        }
        Request::Title { title } => {
            helix.ok(
                M::PATCH,
                "channels",
                &[("broadcaster_id", me)],
                Some(json!({"title": title})),
            )?;
            Ok(format!("Title set: {title}"))
        }
        Request::Game { query } => {
            let found = helix.ok(
                M::GET,
                "search/categories",
                &[("query", query), ("first", "10")],
                None,
            )?;
            let list = found["data"].as_array().cloned().unwrap_or_default();
            // Prefer an exact name match, as Chatterino does, else Twitch's best result.
            let pick = list
                .iter()
                .find(|c| {
                    c["name"]
                        .as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case(query))
                })
                .or_else(|| list.first())
                .ok_or_else(|| format!("No Twitch category matches “{query}”."))?;
            let (id, name) = (
                pick["id"].as_str().unwrap_or(""),
                pick["name"].as_str().unwrap_or(""),
            );
            helix.ok(
                M::PATCH,
                "channels",
                &[("broadcaster_id", me)],
                Some(json!({"game_id": id})),
            )?;
            Ok(format!("Category set: {name}"))
        }
        Request::Marker { description } => {
            let mut body = json!({"user_id": me});
            if !description.is_empty() {
                body["description"] = json!(description);
            }
            helix.ok(M::POST, "streams/markers", &[], Some(body))?;
            Ok("Stream marker added".into())
        }
        Request::Commercial { seconds } => {
            let value = helix.ok(
                M::POST,
                "channels/commercial",
                &[],
                Some(json!({"broadcaster_id": me, "length": seconds})),
            )?;
            let length = value["data"][0]["length"]
                .as_u64()
                .unwrap_or(u64::from(*seconds));
            Ok(format!("Ad running for {length} s"))
        }
        Request::Raid { login } => {
            let target = helix.user_id(login)?;
            helix.ok(
                M::POST,
                "raids",
                &[("from_broadcaster_id", me), ("to_broadcaster_id", &target)],
                None,
            )?;
            Ok(format!(
                "Raid to @{login} is pending; it starts after Twitch's countdown (/unraid cancels)"
            ))
        }
        Request::Unraid => {
            helix.ok(M::DELETE, "raids", &[("broadcaster_id", me)], None)?;
            Ok("Raid cancelled".into())
        }
        Request::Clip => {
            let value = helix.ok(M::POST, "clips", &[("broadcaster_id", room)], None)?;
            let id = value["data"][0]["id"].as_str().unwrap_or("");
            if id.is_empty() {
                return Err("Twitch didn't create a clip (is the channel live?)".into());
            }
            Ok(format!(
                "Clip created: https://clips.twitch.tv/{id} (it can take a few seconds to process)"
            ))
        }
        Request::Shield { on } => {
            helix.ok(
                M::PUT,
                "moderation/shield_mode",
                &moderation,
                Some(json!({"is_active": on})),
            )?;
            Ok(if *on {
                "Shield Mode on".into()
            } else {
                "Shield Mode off".into()
            })
        }
        Request::Chatters => {
            let value = helix.ok(
                M::GET,
                "chat/chatters",
                &[
                    ("broadcaster_id", room),
                    ("moderator_id", me),
                    ("first", "1"),
                ],
                None,
            )?;
            let total = value["total"].as_u64().unwrap_or(0);
            Ok(format!("{total} chatters connected to #{channel}"))
        }
        Request::Moderators | Request::Vips => {
            let (path, what) = if matches!(request, Request::Moderators) {
                ("moderation/moderators", "moderators")
            } else {
                ("channels/vips", "VIPs")
            };
            let mut names = Vec::new();
            let mut cursor = String::new();
            // Up to 1,000 names, 100 per page.
            for _ in 0..10 {
                let mut query = vec![("broadcaster_id", room), ("first", "100")];
                if !cursor.is_empty() {
                    query.push(("after", cursor.as_str()));
                }
                let value = helix.ok(M::GET, path, &query, None)?;
                names.extend(
                    value["data"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|u| u["user_name"].as_str().map(str::to_owned)),
                );
                cursor = value["pagination"]["cursor"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                if cursor.is_empty() {
                    break;
                }
            }
            if names.is_empty() {
                return Ok(format!("#{channel} has no {what}"));
            }
            names.sort_by_key(|n| n.to_lowercase());
            Ok(format!(
                "#{channel} {what} ({}): {}",
                names.len(),
                names.join(", ")
            ))
        }
    }
}
fn human(seconds: u32) -> String {
    match seconds {
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3600 == 0 => format!("{}h", s / 3600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durations() {
        assert_eq!(duration("90"), Some(90));
        assert_eq!(duration("10m"), Some(600));
        assert_eq!(duration("2h"), Some(7200));
        assert_eq!(duration("1w"), Some(604_800));
        assert_eq!(duration("abc"), None);
    }
    #[test]
    fn timeout_defaults_and_reason() {
        assert_eq!(
            parse("/timeout", "@Someone spam").unwrap().unwrap(),
            Request::Timeout {
                login: "someone".into(),
                seconds: 600,
                reason: "spam".into()
            }
        );
        assert_eq!(
            parse("/timeout", "someone 1h rude").unwrap().unwrap(),
            Request::Timeout {
                login: "someone".into(),
                seconds: 3600,
                reason: "rude".into()
            }
        );
        assert!(parse("/timeout", "someone 3w").unwrap().is_err());
        assert!(parse("/notacommand", "").is_none());
    }
}

/// When `user_id` followed the channel (RFC 3339), for the user card. Needs the
/// broadcaster or a moderator (moderator:read:followers); None when not following.
pub fn followed_at(
    identity: &Identity,
    channel: &str,
    user_id: &str,
) -> Result<Option<String>, String> {
    let helix = Helix {
        client: crate::live::client()?,
        identity,
    };
    let room = helix.user_id(channel)?;
    let value = helix.ok(
        reqwest::Method::GET,
        "channels/followers",
        &[("broadcaster_id", room.as_str()), ("user_id", user_id)],
        None,
    )?;
    Ok(value["data"][0]["followed_at"].as_str().map(str::to_owned))
}
