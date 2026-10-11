//! AutoMod queue: messages Twitch held for review in a channel you moderate,
//! from EventSub `automod.message.hold`/`update` (v2), resolved through Helix.
use serde_json::{Value, json};

/// How many held messages a channel keeps; older ones are dropped (Twitch
/// expires them after a few minutes anyway).
const LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub id: String,
    pub user_id: String,
    pub login: String,
    pub name: String,
    pub text: String,
    /// "AutoMod: sexual (level 3)" or "Blocked term".
    pub reason: String,
    pub at: chrono::DateTime<chrono::Local>,
    /// Set while Allow/Deny is in flight, so the buttons can't be pressed twice.
    pub pending: bool,
}

/// A held message, or the outcome of one (approved, denied, expired) by anyone.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Held(Held),
    Resolved {
        id: String,
        status: String,
        by: String,
    },
}

pub fn parse(kind: &str, e: &Value) -> Option<Update> {
    let text = |v: &Value| {
        v.as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(500)
            .collect::<String>()
    };
    let id = text(&e["message_id"]);
    if id.is_empty() {
        return None;
    }
    match kind {
        "automod.message.hold" => {
            let reason = if e["reason"] == "blocked_term" {
                "Blocked term".to_owned()
            } else {
                let category = text(&e["automod"]["category"]);
                match e["automod"]["level"].as_u64() {
                    Some(level) if !category.is_empty() => {
                        format!("AutoMod: {category} (level {level})")
                    }
                    _ => "AutoMod".to_owned(),
                }
            };
            Some(Update::Held(Held {
                id,
                user_id: text(&e["user_id"]),
                login: text(&e["user_login"]),
                name: text(&e["user_name"]),
                text: text(&e["message"]["text"]),
                reason,
                at: chrono::Local::now(),
                pending: false,
            }))
        }
        "automod.message.update" => Some(Update::Resolved {
            id,
            status: text(&e["status"]),
            by: text(&e["moderator_user_name"]),
        }),
        _ => None,
    }
}

/// Apply an update to a channel's queue. Returns a short notice for a
/// resolution, e.g. "Allowed by Mod", or None.
pub fn apply(queue: &mut Vec<Held>, update: Update) -> Option<String> {
    match update {
        Update::Held(held) => {
            if !queue.iter().any(|h| h.id == held.id) {
                if queue.len() >= LIMIT {
                    queue.remove(0);
                }
                queue.push(held);
            }
            None
        }
        Update::Resolved { id, status, by } => {
            let held = queue
                .iter()
                .position(|h| h.id == id)
                .map(|i| queue.remove(i))?;
            let verb = match status.to_ascii_lowercase().as_str() {
                "approved" | "allowed" => "allowed",
                "denied" => "denied",
                _ => "expired",
            };
            Some(if by.is_empty() || verb == "expired" {
                format!("{}'s held message {verb}", held.name)
            } else {
                format!("{}'s held message {verb} by {by}", held.name)
            })
        }
    }
}

/// Allow or deny a held message (Helix, `moderator:manage:automod`).
pub fn resolve(
    identity: &crate::live::Identity,
    message_id: &str,
    allow: bool,
) -> Result<(), String> {
    let client = crate::live::client()?;
    let body = json!({"user_id": identity.user_id, "msg_id": message_id, "action": if allow { "ALLOW" } else { "DENY" }});
    let response = client
        .post("https://api.twitch.tv/helix/moderation/automod/message")
        .header("Client-Id", crate::auth::CLIENT_ID)
        .bearer_auth(&identity.access)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .map_err(|_| "Twitch didn't answer. Try again.".to_owned())?;
    match response.status().as_u16() {
        200 | 204 => Ok(()),
        400 => Err("This message was already handled.".into()),
        403 => Err("You can't moderate AutoMod here.".into()),
        404 => Err("The held message expired.".into()),
        429 => Err("Rate limited. Try again in a moment.".into()),
        code => Err(format!("Twitch refused ({code}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hold_then_resolve() {
        let held = json!({"message_id":"m1","user_id":"9","user_login":"bob","user_name":"Bob","message":{"text":"bad words"},"reason":"automod","automod":{"category":"swearing","level":2}});
        let Some(Update::Held(h)) = parse("automod.message.hold", &held) else {
            panic!()
        };
        assert_eq!(h.reason, "AutoMod: swearing (level 2)");
        let mut queue = vec![];
        assert_eq!(apply(&mut queue, Update::Held(h.clone())), None);
        apply(&mut queue, Update::Held(h));
        assert_eq!(queue.len(), 1, "duplicate deliveries are ignored");
        let update = parse(
            "automod.message.update",
            &json!({"message_id":"m1","status":"Approved","moderator_user_name":"Mod"}),
        )
        .unwrap();
        assert_eq!(
            apply(&mut queue, update).as_deref(),
            Some("Bob's held message allowed by Mod")
        );
        assert!(queue.is_empty());
        let blocked = json!({"message_id":"m2","user_name":"Ann","message":{"text":"x"},"reason":"blocked_term"});
        let Some(Update::Held(h)) = parse("automod.message.hold", &blocked) else {
            panic!()
        };
        assert_eq!(h.reason, "Blocked term");
    }
}
