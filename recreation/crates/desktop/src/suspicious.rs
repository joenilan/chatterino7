//! Suspicious-user flags on messages (EventSub `channel.suspicious_user.message`,
//! `moderator:read:suspicious_users`): monitored/restricted chatters and
//! likely ban evaders, as moderators see them on twitch.tv.
use serde_json::Value;

/// The message ID and a short label, e.g. "Suspicious · Monitored · likely ban evader".
pub fn parse(e: &Value) -> Option<(String, String)> {
    let id = e["message"]["message_id"]
        .as_str()
        .filter(|s| !s.is_empty())?
        .to_owned();
    let mut parts = vec!["Suspicious".to_owned()];
    match e["low_trust_status"].as_str() {
        Some("active_monitoring") => parts.push("Monitored".into()),
        Some("restricted") => parts.push("Restricted".into()),
        _ => {}
    }
    match e["ban_evasion_evaluation"].as_str() {
        Some("likely") => parts.push("likely ban evader".into()),
        Some("possible") => parts.push("possible ban evader".into()),
        _ => {}
    }
    let shared = e["shared_ban_channel_ids"].as_array().map_or(0, Vec::len);
    if shared > 0 {
        parts.push(format!(
            "banned in {shared} other channel{}",
            if shared == 1 { "" } else { "s" }
        ));
    }
    // Nothing beyond the bare flag: Twitch sent an event without a reason.
    Some((id, parts.join(" · ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn labels() {
        let e = json!({"low_trust_status":"active_monitoring","ban_evasion_evaluation":"likely","shared_ban_channel_ids":["1","2"],"message":{"message_id":"m1","text":"hi"}});
        assert_eq!(
            parse(&e),
            Some((
                "m1".into(),
                "Suspicious · Monitored · likely ban evader · banned in 2 other channels".into()
            ))
        );
        assert_eq!(parse(&json!({"message":{}})), None);
    }
}
