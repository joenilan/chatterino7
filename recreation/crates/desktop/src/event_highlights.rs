//! Highlight colours for kinds of chat events: mentions of you, raids, subs,
//! Bits, rewards, announcements and first-time chatters. Each kind is off or one
//! of the highlight colours. Custom highlight rules still take priority.
use crate::highlight_rules::COLORS;
use crate::theme;
use chat_core::Message;
use gpui_kit::component::{
    Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Mention,
    Raid,
    Subscription,
    Bits,
    Reward,
    Announcement,
    FirstChatter,
    OwnMessage,
}
pub const KINDS: [(Kind, &str, &str, &str); 8] = [
    (
        Kind::Mention,
        "mention",
        "Mentions and replies to you",
        "@you, your login, replies to your messages",
    ),
    (
        Kind::Raid,
        "raid",
        "Raids",
        "The raid notice, and the raiding streamer's messages for 30 minutes. Twitch doesn't say which viewers came with a raid.",
    ),
    (
        Kind::Subscription,
        "subscription",
        "Subs and gifts",
        "Subs, resubs, gifted subs, upgrades and pay-it-forward",
    ),
    (Kind::Bits, "bits", "Bits", "Cheers and Bits power-ups"),
    (
        Kind::Reward,
        "reward",
        "Channel point rewards",
        "Highlighted messages and rewards with a message",
    ),
    (
        Kind::Announcement,
        "announcement",
        "Announcements",
        "/announce messages",
    ),
    (
        Kind::FirstChatter,
        "first_chatter",
        "First-time chatters",
        "Twitch's first-chat introduction messages",
    ),
    (
        Kind::OwnMessage,
        "own_message",
        "Your own messages",
        "What you send, so it's easy to find (never pings)",
    ),
];
const RAID_WINDOW: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Colour index into `COLORS` per kind; `None` turns that kind off.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EventHighlights {
    colors: [Option<usize>; 8],
}
impl Default for EventHighlights {
    fn default() -> Self {
        // As on Twitch: mentions red, raids green.
        Self {
            colors: [
                Some(5),
                Some(1),
                Some(0),
                Some(4),
                Some(2),
                Some(2),
                Some(3),
                None,
            ],
        }
    }
}
pub type Shared = Rc<RefCell<EventHighlights>>;
impl EventHighlights {
    pub fn get(&self, kind: Kind) -> Option<usize> {
        self.colors[kind as usize]
    }
    pub fn set(&mut self, kind: Kind, color: Option<usize>) {
        self.colors[kind as usize] = color.filter(|c| *c < COLORS.len());
    }
    /// The event kind a message represents, ignoring mentions (handled by attention).
    pub fn kind(message: &Message) -> Option<Kind> {
        let p = &message.presentation;
        match p.notice_type.as_deref() {
            Some("raid") => return Some(Kind::Raid),
            Some(
                "sub"
                | "resub"
                | "sub_gift"
                | "community_sub_gift"
                | "gift_paid_upgrade"
                | "prime_paid_upgrade"
                | "pay_it_forward"
                | "shared_chat_sub"
                | "shared_chat_resub"
                | "shared_chat_sub_gift"
                | "shared_chat_community_sub_gift"
                | "shared_chat_gift_paid_upgrade"
                | "shared_chat_prime_paid_upgrade"
                | "shared_chat_pay_it_forward",
            ) => return Some(Kind::Subscription),
            Some("announcement" | "shared_chat_announcement") => return Some(Kind::Announcement),
            Some("bits_badge_tier") => return Some(Kind::Bits),
            _ => {}
        }
        match p.kind.as_str() {
            "user_intro" => Some(Kind::FirstChatter),
            "power_ups_message_effect" | "power_ups_gigantified_emote" => Some(Kind::Bits),
            "channel_points_highlighted" | "channel_points_sub_only" => Some(Kind::Reward),
            _ if p.bits > 0 => Some(Kind::Bits),
            _ if p.reward_id.is_some() => Some(Kind::Reward),
            _ => None,
        }
    }
}

/// Raiding streamers seen per channel, so their follow-up messages stand out.
#[derive(Default)]
pub struct Raiders(Vec<(String, std::time::Instant)>);
impl Raiders {
    pub fn note(&mut self, message: &Message) {
        if message.presentation.notice_type.as_deref() == Some("raid")
            && let Some(login) = &message.login
        {
            self.0.retain(|(l, _)| l != login);
            self.0.push((login.clone(), std::time::Instant::now()));
            if self.0.len() > 16 {
                self.0.remove(0);
            }
        }
    }
    pub fn recent(&self, message: &Message) -> bool {
        message.login.as_ref().is_some_and(|login| {
            self.0
                .iter()
                .any(|(l, at)| l == login && at.elapsed() < RAID_WINDOW)
        })
    }
}

pub fn load(value: &serde_json::Value) -> Shared {
    let mut events = EventHighlights::default();
    if value.is_object() {
        for (kind, key, ..) in KINDS {
            match &value[key] {
                serde_json::Value::Null if value.get(key).is_some() => events.set(kind, None),
                v => {
                    if let Some(color) = v.as_u64() {
                        events.set(kind, Some(color as usize));
                    }
                }
            }
        }
    }
    // Version 1 saved the old green mention default; follow Twitch's red instead.
    if value.is_object()
        && value["version"].as_u64().is_none()
        && events.get(Kind::Mention) == Some(1)
    {
        events.set(Kind::Mention, Some(5));
    }
    Rc::new(RefCell::new(events))
}
pub fn json(events: &Shared) -> serde_json::Value {
    let events = events.borrow();
    let mut map: serde_json::Map<String, serde_json::Value> = KINDS
        .iter()
        .map(|(kind, key, ..)| ((*key).to_owned(), serde_json::json!(events.get(*kind))))
        .collect();
    map.insert("version".into(), serde_json::json!(2));
    serde_json::Value::Object(map)
}

pub struct Changed;
impl EventEmitter<Changed> for Editor {}
pub struct Editor {
    events: Shared,
}
impl Editor {
    pub fn new(events: Shared) -> Self {
        Self { events }
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (f32::from(window.viewport_size().height) - 150.).clamp(100., 560.);
        let events = self.events.borrow().clone();
        div()
            .id("event-highlights")
            .v_flex()
            .gap_3()
            .max_h(px(height))
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme::MUTED))
                    .child("Colours for kinds of chat events. Your highlight rules win when both match. Applies to new messages."),
            )
            .children(KINDS.iter().map(|(kind, key, label, detail)| {
                let current = events.get(*kind);
                let kind = *kind;
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(13.)).child(*label))
                    .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(*detail))
                    .child(
                        div()
                            .h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                Button::new(SharedString::from(format!("event-{key}-off")))
                                    .xsmall()
                                    .label(if current.is_none() { "✓ Off" } else { "Off" })
                                    .selected(current.is_none())
                                    .when(current.is_none(), |b| b.primary())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.events.borrow_mut().set(kind, None);
                                        cx.emit(Changed);
                                        cx.notify();
                                    })),
                            )
                            .children(COLORS.iter().enumerate().map(|(index, (name, accent, background))| {
                                let selected = current == Some(index);
                                Button::new(SharedString::from(format!("event-{key}-{index}")))
                                    .xsmall()
                                    .label(if selected { format!("✓ {name}") } else { (*name).to_owned() })
                                    .selected(selected)
                                    .bg(rgb(*background))
                                    .border_1()
                                    .border_color(rgb(*accent))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.events.borrow_mut().set(kind, Some(index));
                                        cx.emit(Changed);
                                        cx.notify();
                                    }))
                            })),
                    )
            }))
    }
}
