//! Highlight colours and sounds for kinds of chat events: mentions of you,
//! raids, subs, Bits, rewards, announcements, first-time chatters and whispers.
//! Each kind is off or one of the highlight colours, and can play a sound.
//! Custom highlight rules still take priority.
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
    Whisper,
    AutoMod,
}
pub const KINDS: [(Kind, &str, &str, &str); 10] = [
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
    (
        Kind::Whisper,
        "whisper",
        "Whispers",
        "The colour of whisper lines in chat (Settings › Chat › show whispers in chat); the sound plays for every whisper",
    ),
    (
        Kind::AutoMod,
        "automod",
        "AutoMod",
        "Messages AutoMod held in channels you moderate, shown in chat with Allow and Deny",
    ),
];
/// Per-kind sound: the Highlights sound setting, nothing, or a fixed tone
/// (the first four of highlight_rules::SOUNDS).
pub const SOUNDS: [&str; 4] = ["Default sound", "No sound", "Chime", "Soft"];
const SILENT: u8 = 1;
const RAID_WINDOW: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Colour index into `COLORS` per kind (`None` turns the colour off), and a
/// SOUNDS index per kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EventHighlights {
    colors: [Option<usize>; 10],
    sounds: [u8; 10],
    /// Flash the taskbar (when that's on in Notifications) for this kind.
    flashes: [bool; 10],
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
                Some(0),
                Some(4),
            ],
            sounds: [SILENT; 10],
            flashes: [false; 10],
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
    /// The SOUNDS index for a kind.
    pub fn sound(&self, kind: Kind) -> u8 {
        self.sounds[kind as usize]
    }
    pub fn set_sound(&mut self, kind: Kind, sound: u8) {
        self.sounds[kind as usize] = sound.min(SOUNDS.len() as u8 - 1);
    }
    pub fn flash(&self, kind: Kind) -> bool {
        self.flashes[kind as usize]
    }
    pub fn set_flash(&mut self, kind: Kind, on: bool) {
        self.flashes[kind as usize] = on;
    }
    /// Whether a new message of this kind pings (a sound or a flash).
    pub fn pings(&self, kind: Kind) -> bool {
        self.sound(kind) != SILENT || self.flash(kind)
    }
    /// The tone a kind plays, if any (`default` is the Highlights sound).
    pub fn tone(&self, kind: Kind, default: &crate::sound::Tone) -> Option<crate::sound::Tone> {
        match self.sound(kind) {
            0 => Some(default.clone()),
            2 => Some(crate::sound::Tone::Chime),
            3 => Some(crate::sound::Tone::Soft),
            _ => None,
        }
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
            Some("whisper") => return Some(Kind::Whisper),
            _ => {}
        }
        match p.kind.as_str() {
            "user_intro" => Some(Kind::FirstChatter),
            crate::automod::KIND => Some(Kind::AutoMod),
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
            if let Some(sound) = value[format!("{key}_sound")].as_u64() {
                events.set_sound(kind, sound as u8);
            }
            if let Some(on) = value[format!("{key}_flash")].as_bool() {
                events.set_flash(kind, on);
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
    for (kind, key, ..) in KINDS {
        map.insert(
            format!("{key}_sound"),
            serde_json::json!(events.sound(kind)),
        );
        map.insert(
            format!("{key}_flash"),
            serde_json::json!(events.flash(kind)),
        );
    }
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
                    .child("Colours and sounds for kinds of chat events. Your highlight rules win when both match. Applies to new messages."),
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
                                    .label("Off")
                                    .when(current.is_none(), |b| b.icon(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Check)))
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
                                    .label(*name)
                                    .when(selected, |b| b.icon(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Check)))
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
                    // Your own messages never ping; mention sounds are in
                    // Notifications › Sounds.
                    .when(kind != Kind::OwnMessage && kind != Kind::Mention, |el| {
                        let sound = events.sound(kind);
                        let flash = events.flash(kind);
                        el.child(
                            div()
                                .h_flex()
                                .flex_wrap()
                                .items_center()
                                .gap_1()
                                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).pr_1().child("Sound"))
                                .children(SOUNDS.iter().enumerate().map(|(index, label)| {
                                    let index = index as u8;
                                    Button::new(SharedString::from(format!("event-{key}-sound-{index}")))
                                        .xsmall()
                                        .label(*label)
                                        .selected(sound == index)
                                        .when(sound == index, |b| b.primary())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.events.borrow_mut().set_sound(kind, index);
                                            match index {
                                                2 => crate::sound::play(&crate::sound::Tone::Chime, 0.6),
                                                3 => crate::sound::play(&crate::sound::Tone::Soft, 0.6),
                                                _ => {}
                                            }
                                            cx.emit(Changed);
                                            cx.notify();
                                        }))
                                }))
                                .child(
                                    gpui_kit::component::checkbox::Checkbox::new(SharedString::from(format!("event-{key}-flash")))
                                        .label("Flash taskbar")
                                        .checked(flash)
                                        .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                            this.events.borrow_mut().set_flash(kind, *on);
                                            cx.emit(Changed);
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
            }))
    }
}
