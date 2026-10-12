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
    BanId {
        user_id: String,
        reason: String,
    },
    /// Monitor or restrict (`restrict`), or clear (`remove`) suspicious treatment.
    Suspicious {
        login: String,
        restrict: bool,
        remove: bool,
    },
    Poll {
        title: String,
        choices: Vec<String>,
        seconds: u32,
        points: Option<u32>,
    },
    /// End the running poll: show results (TERMINATED) or hide it (ARCHIVED).
    EndPoll {
        archive: bool,
    },
    Prediction {
        title: String,
        choices: Vec<String>,
        seconds: u32,
    },
    /// LOCKED, CANCELED, or RESOLVED with the winning choice (title or 1-based index).
    UpdatePrediction {
        status: &'static str,
        winner: Option<String>,
    },
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
    Command {
        name: "/banid",
        usage: "/banid user-id [reason]",
        description: "Ban by Twitch user ID",
    },
    Command {
        name: "/monitor",
        usage: "/monitor login",
        description: "Monitor a suspicious chatter; /unmonitor ends it",
    },
    Command {
        name: "/restrict",
        usage: "/restrict login",
        description: "Restrict a suspicious chatter; /unrestrict ends it",
    },
    Command {
        name: "/poll",
        usage: "/poll --title \"…\" --choice \"…\" --choice \"…\" --duration 2m [--points 100]",
        description: "Start a poll; /endpoll shows results, /cancelpoll hides it",
    },
    Command {
        name: "/prediction",
        usage: "/prediction --title \"…\" --choice \"…\" --choice \"…\" --duration 2m",
        description: "Start a prediction; /lockprediction, /cancelprediction, /completeprediction --index 1",
    },
];

/// `--title "a b" --choice x` → ("title", "a b"), ("choice", "x"). Quotes group words.
fn flags(args: &str) -> Result<Vec<(String, String)>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in args.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                any = false;
            }
            c => current.push(c),
        }
    }
    if quoted {
        return Err("A quote isn't closed.".into());
    }
    if any || !current.is_empty() {
        words.push(current);
    }
    let mut out = Vec::new();
    let mut words = words.into_iter();
    while let Some(word) = words.next() {
        let Some(flag) = word.strip_prefix("--") else {
            return Err(format!("Expected an option like --title, found “{word}”."));
        };
        let value = words
            .next()
            .ok_or_else(|| format!("--{flag} needs a value."))?;
        out.push((flag.to_ascii_lowercase(), value));
    }
    Ok(out)
}
/// Shared parsing for /poll and /prediction.
fn engagement(
    args: &str,
    title_limit: usize,
    choice_range: (usize, usize),
    choice_limit: usize,
) -> Result<(String, Vec<String>, u32, Option<u32>), String> {
    let flags = flags(args)?;
    let get = |key: &'static str| {
        flags
            .iter()
            .filter(move |(k, _)| k == key)
            .map(|(_, v)| v.trim())
    };
    let title = get("title").next().unwrap_or("").to_owned();
    if title.is_empty() || title.chars().count() > title_limit {
        return Err(format!("Give a --title of 1–{title_limit} characters."));
    }
    let choices: Vec<String> = get("choice").map(str::to_owned).collect();
    if choices.len() < choice_range.0
        || choices.len() > choice_range.1
        || choices
            .iter()
            .any(|c| c.is_empty() || c.chars().count() > choice_limit)
    {
        return Err(format!(
            "Give {}–{} --choice options of up to {choice_limit} characters.",
            choice_range.0, choice_range.1
        ));
    }
    let seconds = get("duration")
        .next()
        .and_then(duration)
        .filter(|s| (15..=1800).contains(s))
        .ok_or("Give a --duration from 15s to 30m.")?;
    let points = match get("points").next() {
        Some(p) => Some(
            p.parse::<u32>()
                .ok()
                .filter(|p| (1..=1_000_000).contains(p))
                .ok_or("--points must be 1–1000000.")?,
        ),
        None => None,
    };
    Ok((title, choices, seconds, points))
}

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
        "/banid" => {
            if first.is_empty() || !first.bytes().all(|b| b.is_ascii_digit()) || first.len() > 20 {
                Err("Use /banid user-id [reason].".into())
            } else {
                Ok(Request::BanId {
                    user_id: first.to_owned(),
                    reason: rest.chars().take(500).collect(),
                })
            }
        }
        "/monitor" | "/restrict" | "/unmonitor" | "/unrestrict" => {
            login(first).map(|login| Request::Suspicious {
                login,
                restrict: name.ends_with("restrict"),
                remove: name.starts_with("/un"),
            })
        }
        "/poll" => engagement(args, 60, (2, 5), 25).map(|(title, choices, seconds, points)| {
            Request::Poll {
                title,
                choices,
                seconds,
                points,
            }
        }),
        "/endpoll" => no_args(Request::EndPoll { archive: false }),
        "/cancelpoll" => no_args(Request::EndPoll { archive: true }),
        "/prediction" => {
            engagement(args, 45, (2, 10), 25).and_then(|(title, choices, seconds, points)| {
                if points.is_some() {
                    return Err("Predictions don't take --points.".into());
                }
                Ok(Request::Prediction {
                    title,
                    choices,
                    seconds,
                })
            })
        }
        "/lockprediction" => no_args(Request::UpdatePrediction {
            status: "LOCKED",
            winner: None,
        }),
        "/cancelprediction" => no_args(Request::UpdatePrediction {
            status: "CANCELED",
            winner: None,
        }),
        "/completeprediction" => flags(args).and_then(|flags| {
            let pick = |key: &str| flags.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
            match (pick("choice"), pick("index")) {
                (Some(_), Some(_)) | (None, None) => {
                    Err("Use /completeprediction --choice \"…\" or --index N.".into())
                }
                (Some(choice), None) => Ok(Request::UpdatePrediction {
                    status: "RESOLVED",
                    winner: Some(choice),
                }),
                (None, Some(index)) => match index.parse::<usize>() {
                    Ok(i) if i >= 1 => Ok(Request::UpdatePrediction {
                        status: "RESOLVED",
                        winner: Some(format!("#{i}")),
                    }),
                    _ => Err("--index is the winning choice's number, from 1.".into()),
                },
            }
        }),
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
            BanId { .. } => Action::Ban,
            Suspicious { .. } => Action::Suspicious,
            Poll { .. } | EndPoll { .. } => Action::Polls,
            Prediction { .. } | UpdatePrediction { .. } => Action::Predictions,
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
        Request::BanId { user_id, reason } => {
            let mut data = json!({"user_id": user_id});
            if !reason.is_empty() {
                data["reason"] = json!(reason);
            }
            helix.ok(
                M::POST,
                "moderation/bans",
                &moderation,
                Some(json!({"data": data})),
            )?;
            Ok(format!("User {user_id} banned"))
        }
        Request::Suspicious {
            login,
            restrict,
            remove,
        } => {
            let target = helix.user_id(login)?;
            if *remove {
                helix.ok(
                    M::DELETE,
                    "moderation/suspicious_users",
                    &[
                        ("broadcaster_id", room),
                        ("moderator_id", me),
                        ("user_id", &target),
                    ],
                    None,
                )?;
                Ok(format!("@{login} is no longer monitored or restricted"))
            } else {
                helix.ok(
                    M::POST,
                    "moderation/suspicious_users",
                    &moderation,
                    Some(json!({
                        "user_id": target,
                        "status": if *restrict { "RESTRICTED" } else { "ACTIVE_MONITORING" },
                    })),
                )?;
                Ok(if *restrict {
                    format!("@{login} is restricted")
                } else {
                    format!("@{login} is monitored")
                })
            }
        }
        Request::Poll {
            title,
            choices,
            seconds,
            points,
        } => {
            let mut body = json!({
                "broadcaster_id": me,
                "title": title,
                "choices": choices.iter().map(|c| json!({"title": c})).collect::<Vec<_>>(),
                "duration": seconds,
            });
            if let Some(points) = points {
                body["channel_points_voting_enabled"] = json!(true);
                body["channel_points_per_vote"] = json!(points);
            }
            helix.ok(M::POST, "polls", &[], Some(body))?;
            Ok(format!("Poll started: {title}"))
        }
        Request::EndPoll { archive } => {
            let found = helix.ok(
                M::GET,
                "polls",
                &[("broadcaster_id", me), ("first", "1")],
                None,
            )?;
            let poll = &found["data"][0];
            if poll["status"].as_str() != Some("ACTIVE") {
                return Err("There's no poll running.".into());
            }
            helix.ok(
                M::PATCH,
                "polls",
                &[],
                Some(json!({
                    "broadcaster_id": me,
                    "id": poll["id"],
                    "status": if *archive { "ARCHIVED" } else { "TERMINATED" },
                })),
            )?;
            Ok(if *archive {
                "Poll cancelled".into()
            } else {
                "Poll ended; results are showing".into()
            })
        }
        Request::Prediction {
            title,
            choices,
            seconds,
        } => {
            helix.ok(
                M::POST,
                "predictions",
                &[],
                Some(json!({
                    "broadcaster_id": me,
                    "title": title,
                    "outcomes": choices.iter().map(|c| json!({"title": c})).collect::<Vec<_>>(),
                    "prediction_window": seconds,
                })),
            )?;
            Ok(format!("Prediction started: {title}"))
        }
        Request::UpdatePrediction { status, winner } => {
            let found = helix.ok(
                M::GET,
                "predictions",
                &[("broadcaster_id", me), ("first", "1")],
                None,
            )?;
            let prediction = &found["data"][0];
            let current = prediction["status"].as_str().unwrap_or("");
            if !(current == "ACTIVE" || (current == "LOCKED" && *status != "LOCKED")) {
                return Err("There's no open prediction.".into());
            }
            let mut body = json!({
                "broadcaster_id": me,
                "id": prediction["id"],
                "status": status,
            });
            if let Some(winner) = winner {
                let outcomes = prediction["outcomes"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let picked = match winner
                    .strip_prefix('#')
                    .and_then(|i| i.parse::<usize>().ok())
                {
                    Some(i) => outcomes.get(i - 1),
                    None => outcomes
                        .iter()
                        .find(|o| o["title"].as_str() == Some(winner.as_str())),
                }
                .ok_or("No choice matches; use the exact wording or --index N.")?;
                body["winning_outcome_id"] = picked["id"].clone();
            }
            helix.ok(M::PATCH, "predictions", &[], Some(body))?;
            Ok(match *status {
                "LOCKED" => "Prediction locked".into(),
                "CANCELED" => "Prediction cancelled; points refunded".into(),
                _ => "Prediction resolved".into(),
            })
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
    fn poll_and_prediction_syntax() {
        match parse(
            "/poll",
            r#"--title "Best map?" --choice "Dust 2" --choice Mirage --duration 2m --points 50"#,
        ) {
            Some(Ok(Request::Poll {
                title,
                choices,
                seconds,
                points,
            })) => {
                assert_eq!(title, "Best map?");
                assert_eq!(choices, ["Dust 2", "Mirage"]);
                assert_eq!((seconds, points), (120, Some(50)));
            }
            _ => panic!("poll didn't parse"),
        }
        assert!(matches!(
            parse("/poll", r#"--title x --choice only --duration 60"#),
            Some(Err(_))
        ));
        assert!(matches!(
            parse("/poll", r#"--title "open --choice a"#),
            Some(Err(_))
        ));
        assert!(matches!(
            parse("/completeprediction", "--index 2"),
            Some(Ok(Request::UpdatePrediction { status: "RESOLVED", winner: Some(w) })) if w == "#2"
        ));
        assert!(matches!(
            parse("/restrict", "@Bob"),
            Some(Ok(Request::Suspicious {
                restrict: true,
                remove: false,
                ..
            }))
        ));
        assert!(matches!(
            parse("/unmonitor", "bob"),
            Some(Ok(Request::Suspicious {
                restrict: false,
                remove: true,
                ..
            }))
        ));
        assert!(matches!(parse("/banid", "abc"), Some(Err(_))));
    }
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
/// In your own channel: a chatter's subscription ("Tier 1 · gifted by X"), or None.
pub fn subscription(identity: &Identity, user_id: &str) -> Result<Option<String>, String> {
    let helix = Helix {
        client: crate::live::client()?,
        identity,
    };
    let value = helix.ok(
        reqwest::Method::GET,
        "subscriptions",
        &[
            ("broadcaster_id", identity.user_id.as_str()),
            ("user_id", user_id),
        ],
        None,
    )?;
    let sub = &value["data"][0];
    let Some(tier) = sub["tier"].as_str() else {
        return Ok(None);
    };
    let tier = match tier {
        "2000" => "Tier 2",
        "3000" => "Tier 3",
        _ => "Tier 1",
    };
    Ok(Some(
        match sub["gifter_name"].as_str().filter(|g| !g.is_empty()) {
            Some(gifter) if sub["is_gift"].as_bool() == Some(true) => {
                format!("{tier} subscriber · gifted by {gifter}")
            }
            _ => format!("{tier} subscriber"),
        },
    ))
}
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
