//! Streamer activity feed: follows, subs, gifts, Bits, raids, redemptions and
//! donations for the account's own channel, from Twitch and Streamer.bot.
//! Bounded and session-local; nothing here is sent anywhere.
use chat_core::Message;
use serde_json::Value;
use std::collections::VecDeque;

const LIMIT: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Follow,
    Subscription,
    Bits,
    Raid,
    Redemption,
    Donation,
    /// A moderator action in your channel (channel.moderate).
    Moderation,
    Other,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Follow => "Follow",
            Kind::Subscription => "Sub",
            Kind::Bits => "Bits",
            Kind::Raid => "Raid",
            Kind::Redemption => "Reward",
            Kind::Donation => "Donation",
            Kind::Moderation => "Mod",
            Kind::Other => "Event",
        }
    }
    /// Index into the highlight palette, matching event highlights.
    pub fn color(self) -> usize {
        match self {
            Kind::Follow => 2,
            Kind::Subscription => 0,
            Kind::Bits => 4,
            Kind::Raid => 1,
            Kind::Redemption => 2,
            Kind::Donation => 3,
            Kind::Moderation => 5,
            Kind::Other => 2,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Item {
    pub at: chrono::DateTime<chrono::Local>,
    pub kind: Kind,
    /// "Twitch", "YouTube", "Kick", "Ko-fi", …
    pub source: String,
    pub who: String,
    pub detail: String,
}
/// "timed out bob for 10m: spam", "turned on slow mode (30s)", … for a
/// `channel.moderate` v2 event.
pub fn moderation(e: &Value) -> Option<String> {
    let action = e["action"].as_str()?;
    // Shared-chat variants carry the same object under a prefixed key.
    let key = action.strip_prefix("shared_chat_").unwrap_or(action);
    let object = if e[action].is_object() {
        &e[action]
    } else {
        &e[key]
    };
    let user = text(&object["user_name"], 60);
    let reason = text(&object["reason"], 120);
    let with_reason = |s: String| {
        if reason.is_empty() {
            s
        } else {
            format!("{s}: {reason}")
        }
    };
    let detail = match key {
        "ban" => with_reason(format!("banned {user}")),
        "unban" => format!("unbanned {user}"),
        "timeout" => {
            let length = object["expires_at"]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| {
                    (t.with_timezone(&chrono::Utc) - chrono::Utc::now())
                        .num_seconds()
                        .max(0)
                })
                .map(|s| match s {
                    0..=89 => format!("{s}s"),
                    90..=5399 => format!("{}m", (s + 30) / 60),
                    5400..=129_599 => format!("{}h", (s + 1800) / 3600),
                    _ => format!("{}d", (s + 43_200) / 86_400),
                });
            with_reason(match length {
                Some(l) => format!("timed out {user} for {l}"),
                None => format!("timed out {user}"),
            })
        }
        "untimeout" => format!("removed {user}'s timeout"),
        "delete" => {
            let body = text(&object["message_body"], 80);
            if body.is_empty() {
                format!("deleted a message from {user}")
            } else {
                format!("deleted {user}'s message: {body}")
            }
        }
        "warn" => with_reason(format!("warned {user}")),
        "clear" => "cleared chat".into(),
        "emoteonly" => "turned on emote-only".into(),
        "emoteonlyoff" => "turned off emote-only".into(),
        "followers" => match object["follow_duration_minutes"].as_u64() {
            Some(0) | None => "turned on followers-only".into(),
            Some(m) => format!("turned on followers-only ({m}m)"),
        },
        "followersoff" => "turned off followers-only".into(),
        "slow" => match object["wait_time_seconds"].as_u64() {
            Some(s) => format!("turned on slow mode ({s}s)"),
            None => "turned on slow mode".into(),
        },
        "slowoff" => "turned off slow mode".into(),
        "subscribers" => "turned on subscribers-only".into(),
        "subscribersoff" => "turned off subscribers-only".into(),
        "uniquechat" => "turned on unique chat".into(),
        "uniquechatoff" => "turned off unique chat".into(),
        "vip" => format!("made {user} a VIP"),
        "unvip" => format!("removed {user} as VIP"),
        "mod" => format!("made {user} a moderator"),
        "unmod" => format!("removed {user} as moderator"),
        "raid" => format!("started a raid to {user}"),
        "unraid" => "cancelled the raid".into(),
        "add_blocked_term"
        | "add_permitted_term"
        | "remove_blocked_term"
        | "remove_permitted_term" => {
            let terms = e["automod_terms"]["terms"]
                .as_array()
                .map(|t| {
                    t.iter()
                        .filter_map(|t| t.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!("{} {}", key.replace('_', " "), terms)
                .trim()
                .to_owned()
        }
        "approve_unban_request" | "deny_unban_request" => {
            let verb = if key.starts_with("approve") {
                "approved"
            } else {
                "denied"
            };
            format!(
                "{verb} {}'s unban request",
                text(&e["unban_request"]["user_name"], 60)
            )
        }
        other => other.replace('_', " "),
    };
    Some(if action.starts_with("shared_chat_") {
        format!("{detail} (shared chat)")
    } else {
        detail
    })
}
fn text(value: &Value, limit: usize) -> String {
    value
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

#[derive(Default)]
pub struct Feed {
    pub items: VecDeque<Item>,
    pub unseen: usize,
}
impl Feed {
    pub fn push(&mut self, item: Item) {
        if self.items.len() >= LIMIT {
            self.items.pop_back();
        }
        self.items.push_front(item);
        self.unseen += 1;
    }
    /// Own-channel chat events that belong in the feed.
    pub fn from_message(message: &Message) -> Option<Item> {
        if message.presentation.historical || message.presentation.source.is_some() {
            return None;
        }
        let p = &message.presentation;
        let kind = match p.notice_type.as_deref() {
            Some(
                "sub" | "resub" | "sub_gift" | "community_sub_gift" | "gift_paid_upgrade"
                | "prime_paid_upgrade" | "pay_it_forward",
            ) => Kind::Subscription,
            Some("raid") => Kind::Raid,
            Some("bits_badge_tier") => Kind::Bits,
            Some("charity_donation") => Kind::Donation,
            _ if p.bits > 0 => Kind::Bits,
            _ => return None,
        };
        let detail = match kind {
            Kind::Bits if p.bits > 0 => format!("{} Bits · {}", p.bits, message.body()),
            _ => p.notice.clone().unwrap_or_else(|| message.body()),
        };
        Some(Item {
            at: chrono::Local::now(),
            kind,
            source: "Twitch".into(),
            who: message.author_label().to_owned(),
            detail: detail.chars().take(300).collect(),
        })
    }
    /// `channel.follow` and reward redemptions from EventSub.
    pub fn from_eventsub(kind: &str, event: &Value) -> Option<Item> {
        let who = text(&event["user_name"], 60);
        let (kind, detail) = match kind {
            "channel.follow" => (Kind::Follow, "followed".to_owned()),
            "channel.channel_points_custom_reward_redemption.add" => {
                let reward = text(&event["reward"]["title"], 80);
                let cost = event["reward"]["cost"].as_u64().unwrap_or(0);
                let input = text(&event["user_input"], 200);
                (
                    Kind::Redemption,
                    if input.is_empty() {
                        format!("{reward} ({cost})")
                    } else {
                        format!("{reward} ({cost}) · {input}")
                    },
                )
            }
            "channel.moderate" => {
                return moderation(event).map(|detail| Item {
                    at: chrono::Local::now(),
                    kind: Kind::Moderation,
                    source: "Twitch".into(),
                    who: text(&event["moderator_user_name"], 60),
                    detail,
                });
            }
            _ => return None,
        };
        Some(Item {
            at: chrono::Local::now(),
            kind,
            source: "Twitch".into(),
            who,
            detail,
        })
    }
    /// A Streamer.bot WebSocket event (`{"event":{"source","type"},"data":{…}}`).
    pub fn from_streamer_bot(value: &Value) -> Option<Item> {
        let source = text(&value["event"]["source"], 40);
        let event = text(&value["event"]["type"], 60);
        if source.is_empty() || event.is_empty() {
            return None;
        }
        let data = &value["data"];
        let who = [
            &data["user"]["name"],
            &data["user"]["display_name"],
            &data["displayName"],
            &data["userName"],
            &data["from"],
            &data["name"],
            &data["user"]["login"],
        ]
        .into_iter()
        .map(|v| text(v, 60))
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| "Someone".into());
        let amount = [
            &data["amount"],
            &data["formattedAmount"],
            &data["amountString"],
        ]
        .into_iter()
        .find_map(|v| {
            v.as_str()
                .map(|s| s.chars().take(30).collect::<String>())
                .or_else(|| v.as_f64().map(|n| format!("{n:.2}")))
        });
        let currency = text(&data["currency"], 8);
        let message = text(&data["message"], 200);
        let lower = event.to_ascii_lowercase();
        let kind = if lower.contains("donation")
            || lower.contains("superchat")
            || lower.contains("tip")
            || lower.contains("order")
            || lower.contains("purchase")
            || lower.contains("kicksgifted")
        {
            Kind::Donation
        } else if lower.contains("follow") {
            Kind::Follow
        } else if lower.contains("sub") || lower.contains("sponsor") || lower.contains("member") {
            Kind::Subscription
        } else if lower.contains("raid") {
            Kind::Raid
        } else {
            Kind::Other
        };
        let mut detail = event.clone();
        if let Some(amount) = amount {
            detail = format!("{detail} · {amount} {currency}")
                .trim_end()
                .to_owned();
        }
        if !message.is_empty() {
            detail = format!("{detail} · {message}");
        }
        Some(Item {
            at: chrono::Local::now(),
            kind,
            source: match source.as_str() {
                "Kofi" => "Ko-fi".into(),
                other => other.to_owned(),
            },
            who,
            detail,
        })
    }
}

/// Streamer.bot events requested when available: platforms Jawjack doesn't read
/// natively yet, plus donation services. Twitch events come from EventSub instead.
pub const STREAMER_BOT_EVENTS: &[(&str, &[&str])] = &[
    (
        "YouTube",
        &[
            "SuperChat",
            "SuperSticker",
            "NewSponsor",
            "MembershipGift",
            "GiftMembershipReceived",
            "NewSubscriber",
        ],
    ),
    (
        "Kick",
        &["Follow", "Subscription", "GiftSubscription", "KicksGifted"],
    ),
    (
        "Kofi",
        &[
            "Donation",
            "Subscription",
            "Resubscription",
            "ShopOrder",
            "Commission",
        ],
    ),
    (
        "Streamlabs",
        &["Donation", "CharityDonation", "Merchandise"],
    ),
    ("StreamElements", &["Tip", "Merch"]),
    (
        "Fourthwall",
        &[
            "Donation",
            "OrderPlaced",
            "GiftPurchase",
            "SubscriptionPurchased",
        ],
    ),
    ("TipeeeStream", &["Donation"]),
    ("Patreon", &["PledgeCreated"]),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kofi_donation_summary() {
        let value: Value = serde_json::from_str(r#"{"event":{"source":"Kofi","type":"Donation"},"data":{"from":"Alice","amount":"5.00","currency":"USD","message":"gl"}}"#).unwrap();
        let item = Feed::from_streamer_bot(&value).unwrap();
        assert_eq!(item.kind, Kind::Donation);
        assert_eq!(item.source, "Ko-fi");
        assert_eq!(item.who, "Alice");
        assert_eq!(item.detail, "Donation · 5.00 USD · gl");
    }
    #[test]
    fn follow_from_eventsub() {
        let value: Value = serde_json::from_str(r#"{"user_name":"Bob"}"#).unwrap();
        let item = Feed::from_eventsub("channel.follow", &value).unwrap();
        assert_eq!((item.kind, item.who.as_str()), (Kind::Follow, "Bob"));
    }
}

#[cfg(test)]
mod moderation_tests {
    use serde_json::json;
    #[test]
    fn describes_actions() {
        let ban = json!({"action":"ban","ban":{"user_name":"Bob","reason":"spam"}});
        assert_eq!(super::moderation(&ban).as_deref(), Some("banned Bob: spam"));
        let slow = json!({"action":"slow","slow":{"wait_time_seconds":30}});
        assert_eq!(
            super::moderation(&slow).as_deref(),
            Some("turned on slow mode (30s)")
        );
        let shared = json!({"action":"shared_chat_delete","shared_chat_delete":{"user_name":"Ann","message_body":"hi"}});
        assert_eq!(
            super::moderation(&shared).as_deref(),
            Some("deleted Ann's message: hi (shared chat)")
        );
    }
}
