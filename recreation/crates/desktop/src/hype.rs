//! Live channel moments for the Studio hype strip: hype trains, creator goals,
//! charity campaigns and Shield Mode, from own-channel EventSub events.
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Train {
    pub level: u64,
    pub progress: u64,
    pub goal: u64,
    pub expires_at: Option<DateTime<Utc>>,
    pub golden: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Goal {
    pub label: String,
    pub current: u64,
    pub target: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Charity {
    pub name: String,
    pub raised: String,
    pub target: String,
    pub ratio: f32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hype {
    pub train: Option<Train>,
    pub goals: BTreeMap<String, Goal>,
    pub charity: Option<Charity>,
    /// Shield Mode is on; who turned it on.
    pub shield: Option<String>,
}
impl Hype {
    pub fn is_empty(&self) -> bool {
        self.train.is_none()
            && self.goals.is_empty()
            && self.charity.is_none()
            && self.shield.is_none()
    }
}

pub const KINDS: [&str; 11] = [
    "channel.hype_train.begin",
    "channel.hype_train.progress",
    "channel.hype_train.end",
    "channel.goal.begin",
    "channel.goal.progress",
    "channel.goal.end",
    "channel.charity_campaign.start",
    "channel.charity_campaign.progress",
    "channel.charity_campaign.stop",
    "channel.shield_mode.begin",
    "channel.shield_mode.end",
];

fn text(v: &Value, max: usize) -> String {
    v.as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}
fn time(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc))
}
/// "$1,234.50" from Twitch's {value, decimal_places, currency}.
fn money(v: &Value) -> (String, f64) {
    let value = v["value"].as_i64().unwrap_or(0) as f64;
    let places = v["decimal_places"].as_u64().unwrap_or(2).min(4) as i32;
    let amount = value / 10f64.powi(places);
    let currency = text(&v["currency"], 3);
    let symbol = match currency.as_str() {
        "USD" | "CAD" | "AUD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        _ => "",
    };
    let shown = if symbol.is_empty() {
        format!("{amount:.2} {currency}")
    } else {
        format!("{symbol}{amount:.2}")
    };
    (shown, amount)
}
fn goal_label(kind: &str, description: &str) -> String {
    let kind = match kind {
        "follow" | "follower" => "Follower goal",
        "subscription" | "subscription_count" | "new_subscription" | "new_subscription_count" => {
            "Sub goal"
        }
        "new_bit" => "Bits goal",
        "new_cheerer" => "Cheerer goal",
        _ => "Goal",
    };
    if description.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind} · {description}")
    }
}

/// Apply one event; returns true if it was a hype-strip event.
pub fn apply(hype: &mut Hype, kind: &str, e: &Value) -> bool {
    match kind {
        "channel.hype_train.begin" | "channel.hype_train.progress" => {
            hype.train = Some(Train {
                level: e["level"].as_u64().unwrap_or(1),
                progress: e["progress"].as_u64().unwrap_or(0),
                goal: e["goal"].as_u64().unwrap_or(0),
                expires_at: time(&e["expires_at"]),
                golden: e["type"] == "golden_kappa" || e["is_golden_kappa_train"] == true,
            });
        }
        "channel.hype_train.end" => hype.train = None,
        "channel.goal.begin" | "channel.goal.progress" => {
            let id = text(&e["id"], 64);
            hype.goals.insert(
                id,
                Goal {
                    label: goal_label(
                        e["type"].as_str().unwrap_or(""),
                        &text(&e["description"], 60),
                    ),
                    current: e["current_amount"].as_u64().unwrap_or(0),
                    target: e["target_amount"].as_u64().unwrap_or(0),
                },
            );
            while hype.goals.len() > 4 {
                let first = hype.goals.keys().next().cloned().unwrap_or_default();
                hype.goals.remove(&first);
            }
        }
        "channel.goal.end" => {
            hype.goals.remove(&text(&e["id"], 64));
        }
        "channel.charity_campaign.start" | "channel.charity_campaign.progress" => {
            let (raised, now) = money(&e["current_amount"]);
            let (target, goal) = money(&e["target_amount"]);
            hype.charity = Some(Charity {
                name: text(&e["charity_name"], 60),
                raised,
                target,
                ratio: if goal > 0. {
                    (now / goal).clamp(0., 1.) as f32
                } else {
                    0.
                },
            });
        }
        "channel.charity_campaign.stop" => hype.charity = None,
        "channel.shield_mode.begin" => hype.shield = Some(text(&e["moderator_user_name"], 60)),
        "channel.shield_mode.end" => hype.shield = None,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn strip_state() {
        let mut h = Hype::default();
        assert!(apply(
            &mut h,
            "channel.hype_train.progress",
            &json!({"level":3,"progress":400,"goal":1000})
        ));
        assert_eq!(h.train.as_ref().unwrap().level, 3);
        apply(
            &mut h,
            "channel.goal.begin",
            &json!({"id":"g","type":"subscription","description":"New cam","current_amount":5,"target_amount":10}),
        );
        assert_eq!(h.goals["g"].label, "Sub goal · New cam");
        apply(
            &mut h,
            "channel.charity_campaign.progress",
            &json!({"charity_name":"Kids","current_amount":{"value":12050,"decimal_places":2,"currency":"USD"},"target_amount":{"value":50000,"decimal_places":2,"currency":"USD"}}),
        );
        let c = h.charity.as_ref().unwrap();
        assert_eq!(
            (c.raised.as_str(), c.target.as_str()),
            ("$120.50", "$500.00")
        );
        apply(&mut h, "channel.hype_train.end", &json!({}));
        apply(&mut h, "channel.goal.end", &json!({"id":"g"}));
        apply(&mut h, "channel.charity_campaign.stop", &json!({}));
        assert!(h.is_empty());
        assert!(!apply(&mut h, "channel.follow", &json!({})));
    }
}
