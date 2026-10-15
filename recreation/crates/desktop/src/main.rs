// A windowed app: no console window behind Jawjack in release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use gpui_kit::prelude::FluentBuilder;
mod activity_feed;
mod ads;
mod anon_chat;
mod assets;
mod attention;
mod attention_navigation;
mod auth;
mod automod;
mod automod_ui;
mod autorun;
mod blocks;
mod bttv_events;
mod capabilities;
mod catalog;
mod chat_logs;
mod chat_pause;
mod chat_search;
mod chat_text;
mod chatter_appearance;
mod chatter_notes;
mod chatterino_badges;
/// Settings → Appearance → Collapse long messages: lines shown (0 = never).
pub static COLLAPSE_LINES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
mod chatterino_import;
mod chatters;
mod chatters_ui;
mod combined;
mod commands;
mod community;
mod control;
mod custom_commands;
mod dock;
mod duplicates;
mod emoji;
mod emote_picker;
mod engagement;
mod event_highlights;
mod ffz_badges;
mod ffz_role_badges;
mod follows_live;
mod frame_sampler;
mod google_auth;
mod highlight_rules;
mod hotkeys;
mod hype;
mod ignores;
mod inline_chat;
mod input_history;
mod link_preview;
mod live;
mod media;
mod media_hover;
mod mention_names;
mod message_actions;
mod message_scroller;
mod mod_buttons;
mod name_paint;
mod nickname_rules;
mod obs;
mod owned_emotes;
mod private_link;
mod profile;
mod profiles;
mod pronouns;
mod recent_messages;
mod replies;
mod rewards;
mod rich_messages;
mod room_settings;
mod seven_badges;
mod seven_entitlements;
mod seven_events;
mod seven_lookup;
mod seven_paints;
mod seven_presence;
mod shared_chat;
mod similarity;
mod sound;
mod spell;
mod storage;
mod stream_preview;
mod stream_status;
mod streamer_bot;
mod streamer_mode;
mod streamlink;
mod studio;
mod suspicious;
mod theme;
mod twitch_actions;
mod twitch_assets;
mod updates;
mod whispers;
mod workspace;
mod youtube;
use workspace::Workbench;

use chat_core::{Timeline, selection::Selection};
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{
    Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    input::{InputEvent, Textarea, TextareaState},
    notification::{Notification, NotificationDelivery},
};
use gpui_kit::*;
use message_scroller::{MessageScroller, MessageScrollerState};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

gpui_kit::actions!(
    chat_workbench,
    [
        CopyChatSelection,
        ClearChatSelection,
        SelectAllChat,
        CompleteEmote,
        FindChat,
        NextChatMatch,
        PreviousChatMatch,
        PreviousInput,
        NextInput,
        ToggleChatPause,
        ResumeChat,
        SentHistory,
        CatchUpChat,
        PreviousHighlight,
        NextHighlight,
        OpenEmotePicker,
        OpenChatterList,
        WatchInStreamlink,
        OpenStreamInBrowser,
        OpenPlayerPopout
    ]
);

#[derive(Clone)]
struct DraggedChannel {
    pane: Entity<ChannelPane>,
    name: String,
}
struct DragPreview(String);
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(rgb(theme::panel()))
            .text_color(rgb(theme::text()))
            .border_1()
            .border_color(rgb(crate::theme::tone(0xA99CF4)))
            .rounded(px(5.))
            .child(self.0.clone())
    }
}
struct CopyFeedback;
struct ComposerFeedback;
#[derive(Clone)]
struct MessageEntrance {
    row: u64,
    received: Instant,
    started: Rc<Cell<Option<Instant>>>,
}
impl MessageEntrance {
    fn active(&self, now: Instant) -> bool {
        self.started.get().map_or_else(
            || now.duration_since(self.received) < Duration::from_secs(1),
            |start| now.duration_since(start) < Duration::from_millis(260),
        )
    }
}
fn twitch_message_text(text: &str) -> String {
    text.replace("\r\n", " ")
        .replace(['\r', '\n', '\u{2028}', '\u{2029}'], " ")
}
#[derive(Clone)]
enum PaneEvent {
    DragStarted,
    Send {
        request: u64,
        text: String,
        reply_parent: Option<String>,
    },
    Close,
    RequestClose,
    DraftChanged,
    EmotePreferencesChanged,
    EmoteRecentsChanged,
    OpenAccount,
    OpenStreamInfo,
    OpenModTools,
    /// Open the YouTube setup guide (a YouTube tab without an API key).
    SetUpYouTube,
    /// Open this channel in its own window (/popout).
    PopOut,
    /// The view filter changed (saved with the workspace).
    ViewFilterChanged,
    /// Show or hide the inline moderator buttons (all channels).
    ToggleModButtons,
    /// Choose which signed-in account sends from this pane (vault entry).
    SendAs(SharedString),
    /// A new mention or highlight arrived (message id, for de-duplication across panes).
    Ping(attention::Ping, String),
    /// You tagged yourself: no ping, but it belongs in Mentions (message id).
    SelfMention(String),
    /// Allow (true) or deny a held AutoMod message.
    AutoMod(String, bool),
    /// Remove a held message from your screen without acting on it.
    DismissAutoMod(String),
    /// Stop showing (and subscribing to) AutoMod in this channel.
    HideAutoMod,
    RunBotAction {
        action: streamer_bot::Action,
        args: serde_json::Value,
    },
    /// A parsed Twitch command; `draft` is cleared only once Twitch confirms.
    TwitchCommand {
        request: twitch_actions::Request,
        draft: String,
    },
    /// /ignore or /unignore changed the local ignore list (save it).
    IgnoresChanged,
    /// A word was added to the personal spellcheck dictionary (save it).
    SpellingWordAdded,
    /// User card → Open channel: jump to it, or add it as a tab here.
    OpenChannel(String),
    /// Mentions tab: show this message in the channel it was said in
    /// (and start a reply there).
    GoToMessage {
        channel: String,
        id: String,
        reply: bool,
    },
    /// Whispers tab: open the whisper conversation with this login.
    OpenWhisper(String),
    /// Keep a chatter's card open in its own window.
    PinCard {
        user_id: String,
        name: String,
        login: Option<String>,
    },
    /// Delete / time out / ban on YouTube: (YouTube source name, action).
    YouTubeModerate(String, google_auth::Moderation),
    OpenCommands(String),
    EditChatter(chatter_appearance::Target),
    EditChatterNote(chatter_appearance::Target),
    /// The user card wants this chatter's follow age (looked up with your login).
    WantFollowAge(String),
    SearchAll(String),
}
impl EventEmitter<PaneEvent> for ChannelPane {}

struct ChannelPane {
    name: SharedString,
    /// The signed-in account's role here, when known.
    role: Option<capabilities::Role>,
    /// Twitch's chatter list for @-completion (moderated or own channels only).
    chatter_list: std::rc::Rc<Vec<String>>,
    /// Latest stream snapshot, for the header hover card.
    stream: Option<stream_status::StreamInfo>,
    /// Moderator/Streamer setup shows the 🛡 button where the role allows.
    mod_tools: bool,
    /// Pinned Streamer.bot actions offered in the message menu while connected.
    bot_actions: std::rc::Rc<Vec<streamer_bot::Action>>,
    /// Signed-in accounts (vault, login), active first; the picker shows with 2+.
    send_as_options: std::rc::Rc<Vec<(SharedString, String)>>,
    /// Messages AutoMod is holding in this channel for you to review.
    automod: std::rc::Rc<Vec<automod::Held>>,
    /// What happened to held messages shown inline: id -> "allowed by Mod".
    automod_outcomes: std::rc::Rc<std::collections::HashMap<String, String>>,
    /// While copying existing chat into a new popout: no pings for old messages.
    quiet_pings: bool,
    /// Streamer mode is active: no stream thumbnails in the header card.
    streamer_mode: bool,
    /// What to do with a chatter's near-repeats of their recent messages.
    similar_mode: similarity::Mode,
    /// Inline delete/timeout/ban buttons (moderator mode), where you moderate.
    mod_buttons: bool,
    /// Your own moderator buttons, shown after the built-in ones.
    custom_mod_buttons: std::rc::Rc<Vec<mod_buttons::ModButton>>,
    /// Combined feed: where messages typed here are sent (one of the sources).
    send_to: Option<String>,
    /// Spellcheck the message box, and what it found in the current draft.
    spellcheck: bool,
    spelling: Vec<(String, Vec<String>)>,
    /// View filter (search syntax): only matching messages are shown; nothing is dropped.
    view_filter: Option<(String, std::rc::Rc<chat_search::Query>)>,
    /// Combined feeds: platforms whose messages are hidden here.
    pub(crate) hidden_platforms: Vec<combined::Platform>,
    /// Shared Chat: broadcaster IDs in this channel's session (host first), this
    /// channel's own ID, and the channels whose messages are hidden here.
    pub(crate) shared_session: Option<Vec<String>>,
    pub(crate) shared_own: Option<String>,
    pub(crate) hidden_sources: Vec<String>,
    /// When each session channel last spoke (its bubble pulses): broadcaster ID → time.
    pub(crate) shared_pulse: std::collections::HashMap<String, Instant>,
    /// User card: follow dates by user ID (Ok(None) = not following, Err = unavailable).
    pub(crate) follow_ages: std::collections::HashMap<String, Result<Option<String>, String>>,
    /// User card in your own channel: subscription by user ID (None = not subscribed).
    pub(crate) sub_status: std::collections::HashMap<String, Option<String>>,
    /// The last message sent here and when, so an identical one can still go through.
    last_sent: Option<(String, Instant)>,
    /// A once-a-second repaint is queued for the slow-mode countdown.
    slow_tick: bool,
    /// Extra sources shown in this pane: your own YouTube live chat in your own channel.
    pub(crate) linked: Vec<String>,
    /// A friendlier title than the pane name (YouTube: "Channel · Stream title").
    display_title: Option<String>,
    /// Suspicious labels for messages that haven't arrived yet (by message ID).
    suspicious_pending: std::collections::HashMap<String, String>,
    send_as: Option<SharedString>,
    connection: String,
    connected: bool,
    room_settings: Option<room_settings::RoomSettings>,
    pending: Option<(u64, String, Option<String>, u64)>,
    /// A slash command is waiting for a Twitch response; repeated Enter must
    /// not queue the same consequential API request (including /raid).
    pub(crate) twitch_command_in_flight: bool,
    compose_revision: u64,
    input_history: input_history::History,
    reply_target: Option<replies::Target>,
    attention: attention::Attention,
    attention_cursor: Option<u64>,
    jump_pulse: Option<(u64, Instant)>,
    read_eligible: Rc<Cell<bool>>,
    presented_row: Rc<Cell<Option<u64>>>,
    send_status: String,
    /// Locally estimated countdown after a successful POST /helix/raids:
    /// (target login, when Twitch accepted the request).
    pub(crate) raid_pending: Option<(String, Instant)>,
    raid_tick: bool,
    timeline: Rc<RefCell<Timeline>>,
    media: Rc<RefCell<media::MediaCache>>,
    catalog: Rc<RefCell<catalog::Catalog>>,
    custom_commands: custom_commands::Definitions,
    /// While a custom command runs from a message's menu: (login, user ID,
    /// message ID, text) for {user.name} {user.id} {msg.id} {msg.text}.
    command_message: Option<(String, String, String, String)>,
    chatter_appearance: chatter_appearance::Preferences,
    ignores: ignores::Shared,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    draft: Entity<TextareaState>,
    font_size: f32,
    timestamps: u8,
    pause_mode: u8,
    transcript_hover: bool,
    transcript_available: bool,
    picker: emote_picker::Picker,
    search: chat_search::Search,
    link_press: message_actions::PressState,
    emote_search: Entity<TextareaState>,
    last_copy_result: Option<&'static str>,
    scroller: Entity<MessageScrollerState>,
    next_id: usize,
    entrances: Vec<MessageEntrance>,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    /// Long messages the reader expanded (Settings → Appearance → Collapse).
    expanded: Rc<RefCell<std::collections::HashSet<String>>>,
    /// Overlay mode (Chatterino's overlay window): only the messages, over a
    /// background this opaque (0 = fully see-through).
    pub overlay: Option<f32>,
    /// The open right-click menu on a message. Held here, not on the row, so new
    /// chat re-rendering the rows doesn't close it.
    row_menu: Option<(
        Entity<gpui_kit::component::menu::PopupMenu>,
        Point<Pixels>,
        Subscription,
    )>,
}
impl ChannelPane {
    /// Shows a message's right-click menu at `at` until it's dismissed.
    fn open_row_menu(
        &mut self,
        menu: Entity<gpui_kit::component::menu::PopupMenu>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dismissed = cx.subscribe_in(&menu, window, |this, menu, _: &DismissEvent, _, cx| {
            if this
                .row_menu
                .as_ref()
                .is_some_and(|(open, _, _)| open == menu)
            {
                this.row_menu = None;
                cx.notify();
            }
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.row_menu = Some((menu, at, dismissed));
        cx.notify();
    }

    fn new(
        name: &str,
        media: Rc<RefCell<media::MediaCache>>,
        catalog: Rc<RefCell<catalog::Catalog>>,
        saved_draft: &str,
        font_size: f32,
        history_limit: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut timeline = Timeline::new(name, history_limit);
        if combined::is_combined(name) {
            timeline.accept_channels(combined::sources(name));
        }
        let draft = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .submit_on_enter(true)
                .placeholder("Write a message…");
            input.set_value(twitch_message_text(saved_draft), window, cx);
            input
        });
        cx.subscribe_in(&draft, window, |this: &mut Self, _, event, _window, cx| {
            if let InputEvent::Change = event {
                this.compose_revision = this.compose_revision.wrapping_add(1);
                this.complete_query(false, cx);
                this.check_spelling(cx);
                cx.emit(PaneEvent::DraftChanged);
            }
            cx.notify();
        })
        .detach();
        let emote_search = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("Search emotes…")
        });
        cx.subscribe_in(&emote_search, window, |this: &mut Self, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.reset_browser_search(cx);
                cx.notify();
            }
        })
        .detach();
        let search = chat_search::create(window, cx);
        let scroller = cx.new(|cx| MessageScrollerState::new(timeline.messages().len(), cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();
        Self {
            name: name.to_owned().into(),
            role: None,
            chatter_list: Default::default(),
            stream: None,
            mod_tools: false,
            bot_actions: Default::default(),
            send_as_options: Default::default(),
            automod: Default::default(),
            automod_outcomes: Default::default(),
            quiet_pings: false,
            streamer_mode: false,
            similar_mode: Default::default(),
            mod_buttons: false,
            custom_mod_buttons: Default::default(),
            send_to: combined::is_combined(name).then(|| combined::sources(name)[0].clone()),
            spellcheck: false,
            spelling: Vec::new(),
            view_filter: None,
            hidden_platforms: Vec::new(),
            shared_session: None,
            shared_own: None,
            hidden_sources: Vec::new(),
            shared_pulse: Default::default(),
            follow_ages: Default::default(),
            sub_status: Default::default(),
            last_sent: None,
            slow_tick: false,
            linked: Vec::new(),
            display_title: None,
            suspicious_pending: Default::default(),
            send_as: None,
            media,
            catalog,
            custom_commands: Default::default(),
            command_message: None,
            chatter_appearance: Default::default(),
            ignores: Default::default(),
            connection: "Sign in to connect".into(),
            connected: false,
            room_settings: None,
            pending: None,
            twitch_command_in_flight: false,
            reply_target: None,
            attention: Default::default(),
            attention_cursor: None,
            jump_pulse: None,
            read_eligible: Rc::new(Cell::new(false)),
            presented_row: Rc::new(Cell::new(None)),
            compose_revision: 0,
            input_history: Default::default(),
            send_status: String::new(),
            raid_pending: None,
            raid_tick: false,
            timeline: Rc::new(RefCell::new(timeline)),
            selection: Rc::new(RefCell::new(Selection::default())),
            focus: cx.focus_handle(),
            draft,
            font_size,
            timestamps: 0,
            pause_mode: 0,
            transcript_hover: false,
            transcript_available: false,
            picker: emote_picker::Picker::default(),
            emote_search,
            search,
            link_press: Default::default(),
            last_copy_result: None,
            scroller,
            next_id: 0,
            entrances: Vec::new(),
            viewport: Rc::new(RefCell::new(None)),
            expanded: Default::default(),
            row_menu: None,
            overlay: None,
        }
    }
    /// Tab menu → "Clear messages locally": empties this view; nobody else is affected.
    pub(crate) fn clear_locally(&mut self, cx: &mut Context<Self>) {
        self.search.dirty = true;
        let removed = self.timeline.borrow_mut().clear();
        self.selection.borrow_mut().clear();
        self.entrances.clear();
        if removed > 0 {
            self.scroller
                .update(cx, |s, cx| s.splice(0..removed, 0, cx));
        }
        self.attention
            .reconcile(&self.timeline.borrow(), self.next_id as u64);
        cx.notify();
    }
    /// Seconds until slow mode lets you send again (Chatterino's send wait timer).
    /// Moderators and the broadcaster are exempt. Queues a repaint each second.
    fn slow_mode_wait(&mut self, cx: &mut Context<Self>) -> Option<u64> {
        if matches!(
            self.role,
            Some(capabilities::Role::Moderator | capabilities::Role::Broadcaster)
        ) {
            return None;
        }
        let slow = self.room_settings.as_ref()?.slow?;
        let (_, at) = self.last_sent.as_ref()?;
        let left = slow.saturating_sub(at.elapsed().as_secs());
        if left == 0 {
            return None;
        }
        if !self.slow_tick {
            self.slow_tick = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = this.update(cx, |p, cx| {
                    p.slow_tick = false;
                    cx.notify();
                });
            })
            .detach();
        }
        Some(left)
    }
    /// Twitch's public API can queue or cancel a raid, not complete its
    /// countdown early. Show status and a real Cancel action only.
    fn render_raid_status(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some((target, queued_at)) = self.raid_pending.clone() else {
            return Vec::new();
        };
        let remaining = 90_u64.saturating_sub(queued_at.elapsed().as_secs());
        if remaining == 0 {
            self.raid_pending = None;
            return Vec::new();
        }
        if !self.raid_tick {
            self.raid_tick = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = this.update(cx, |pane, cx| {
                    pane.raid_tick = false;
                    cx.notify();
                });
            })
            .detach();
        }
        vec![
            div()
                .h_flex()
                .items_center()
                .flex_wrap()
                .gap_2()
                .py_1()
                .px_2()
                .rounded(px(5.))
                .bg(rgb(theme::control()))
                .child(div().text_size(px(11.)).child(format!(
                    "Raid queued → @{target} · ~{remaining}s (Twitch countdown)"
                )))
                .child(
                    Button::new("raid-cancel")
                        .xsmall()
                        .ghost()
                        .label("Cancel")
                        .tooltip("Cancel the queued Twitch raid; does not affect your stream")
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(PaneEvent::TwitchCommand {
                                request: crate::twitch_actions::Request::Unraid,
                                draft: String::new(),
                            });
                        })),
                )
                .into_any_element(),
        ]
    }
    fn change_history_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        self.search.dirty = true;
        let removed = self.timeline.borrow_mut().set_capacity(limit);
        if removed > 0 {
            let first = (self.next_id - self.timeline.borrow().messages().len()) as u64;
            let mut selection = self.selection.borrow_mut();
            if selection.anchor.is_some_and(|p| p.row < first)
                || selection.head.is_some_and(|p| p.row < first)
            {
                selection.clear();
            }
            drop(selection);
            self.entrances.retain(|e| e.row >= first);
            self.scroller
                .update(cx, |s, cx| s.splice(0..removed, 0, cx));
        }
        self.attention.reconcile(
            &self.timeline.borrow(),
            (self.next_id - self.timeline.borrow().messages().len()) as u64,
        );
        cx.notify();
    }
    /// Run `text` here as if typed: a /command runs; anything else is sent,
    /// unless you have a draft (it's never overwritten).
    pub fn run_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.route_command(text, window, cx) {
            return;
        }
        if !self.draft.read(cx).value().is_empty() {
            self.command_feedback(
                "Clear your message box first: the shortcut won't replace your draft.",
                window,
                cx,
            );
            return;
        }
        let text = text.to_owned();
        self.draft.update(cx, |d, cx| d.set_value(text, window, cx));
        self.submit(window, cx);
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() || self.twitch_command_in_flight {
            return;
        }
        let text = twitch_message_text(&self.draft.read(cx).value());
        if self.route_command(&text, window, cx) {
            return;
        }
        // Mentions/Whispers tabs only collect; chat goes to a real channel.
        if combined::is_special(&self.name) {
            if !text.trim().is_empty() {
                self.command_feedback(
                    "This tab collects messages. Reply from the message's channel (right-click → Reply), or type /w name message to whisper. Your draft is kept.",
                    window,
                    cx,
                );
            }
            return;
        }
        if !self.connected || text.trim().is_empty() || text.chars().count() > 500 {
            window.push_notification(
                Notification::info(if !self.connected {
                    "Connect this channel before sending. Your draft is saved."
                } else {
                    "Write a message of 1–500 characters."
                })
                .id::<ComposerFeedback>()
                .delivery(NotificationDelivery::InApp),
                cx,
            );
            return;
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let request = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self
            .reply_target
            .as_ref()
            .is_some_and(|target| target.deleted)
        {
            window.push_notification(Notification::info("The reply target was deleted. Cancel the reply or choose another message; your draft is kept."),cx);
            return;
        }
        let reply_parent = self.reply_target.as_ref().map(|target| target.id.clone());
        // Twitch repeats may need a wire-only invisible suffix. Retain the
        // *original draft* for the success acknowledgement so a repeated
        // message clears normally instead of lingering and being sent again.
        let (pending_text, outgoing_text) = duplicates::pending_and_wire(
            &mut self.last_sent,
            text,
            youtube::is_youtube(&self.name),
        );
        self.pending = Some((
            request,
            pending_text,
            reply_parent.clone(),
            self.compose_revision,
        ));
        self.send_status = "Sending…".into();
        cx.emit(PaneEvent::Send {
            request,
            text: outgoing_text,
            reply_parent,
        });
        cx.notify();
    }
    fn received(&mut self, mut event: chat_core::Event, cx: &mut Context<Self>) {
        let historical =
            matches!(&event, chat_core::Event::Message(m) if m.presentation.historical);
        if let chat_core::Event::Message(message) = &mut event {
            // Shared Chat: pulse the bubble of the channel this was said in.
            if !historical
                && self.shared_session.is_some()
                && let Some(id) = message
                    .presentation
                    .source_id
                    .clone()
                    .or_else(|| self.shared_own.clone())
            {
                self.shared_pulse.insert(id, Instant::now());
            }
            self.catalog.borrow_mut().want_cosmetics(&message.user_id);
            // In a combined feed, each message uses its own channel's emotes.
            let emote_channel = message
                .presentation
                .origin
                .clone()
                .unwrap_or_else(|| self.name.to_string());
            message.fragments =
                self.catalog
                    .borrow()
                    .expand(&emote_channel, &message.user_id, &message.fragments);
            if let ignores::Outcome::Hide = self.ignores.borrow().apply(message) {
                return;
            }
            if let Some(label) = self.suspicious_pending.remove(&message.id) {
                message.presentation.suspicious = Some(label);
            }
            if self.similar_mode != similarity::Mode::Show
                && !historical
                && similarity::is_repeat(
                    message,
                    self.timeline.borrow().messages().iter(),
                    self.attention.identity().map(|(id, _)| id.as_str()),
                )
            {
                if self.similar_mode == similarity::Mode::Hide {
                    return;
                }
                message.presentation.similar = true;
            }
            chatter_appearance::apply(&self.chatter_appearance, message);
            message_actions::lowercase_link_hosts(message);
            emoji::to_images(message);
            mention_names::note(message);
            mention_names::link_names(message);
            if replies::option(3) {
                replies::strip_leading_mention(message);
            }
        }
        self.observe_reply_redaction(&event, cx);
        let change = self.timeline.borrow_mut().apply(event);
        match change {
            chat_core::Change::Appended { evicted } => {
                let now = Instant::now();
                self.entrances.retain(|entry| entry.active(now));
                if !historical && self.scroller.read(cx).is_following_tail() && !cx.reduce_motion()
                {
                    // Keep animating the newest visible arrivals even in busy chat.
                    // Retire the oldest effect rather than disabling all motion.
                    if self.entrances.len() >= 12 {
                        self.entrances.remove(0);
                    }
                    self.entrances.push(MessageEntrance {
                        row: self.next_id as u64,
                        received: now,
                        started: Rc::new(Cell::new(None)),
                    });
                }
                self.next_id += 1;
                let ping = {
                    let timeline = self.timeline.borrow();
                    timeline.messages().back().and_then(|message| {
                        let first = (self.next_id - timeline.messages().len()) as u64;
                        self.search
                            .appended(message, (self.next_id - 1) as u64, first);
                        self.attention
                            .appended(message, (self.next_id - 1) as u64, first)
                            .map(|ping| (ping, message.id.clone()))
                    })
                };
                let self_mention = ping
                    .is_none()
                    .then(|| {
                        let timeline = self.timeline.borrow();
                        timeline
                            .messages()
                            .back()
                            .filter(|m| self.attention.mentions_self(m))
                            .map(|m| m.id.clone())
                    })
                    .flatten();
                if let Some((ping, id)) = ping.filter(|_| !self.quiet_pings) {
                    cx.emit(PaneEvent::Ping(ping, id));
                } else if let Some(id) = self_mention.filter(|_| !self.quiet_pings) {
                    cx.emit(PaneEvent::SelfMention(id));
                }

                self.scroller.update(cx, |scroller, cx| {
                    if evicted {
                        scroller.splice(0..1, 0, cx);
                    }
                    scroller.append(1, cx);
                });
            }
            chat_core::Change::Updated => {
                self.attention.reconcile(
                    &self.timeline.borrow(),
                    (self.next_id - self.timeline.borrow().messages().len()) as u64,
                );
                self.search.dirty = true;
                self.selection.borrow_mut().clear();
                self.scroller.update(cx, |s, cx| s.remeasure(cx));
            }
            chat_core::Change::Ignored => return,
        }
        cx.notify();
    }
    fn sent(
        &mut self,
        request: u64,
        result: &Result<(), String>,
        current_session: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((pending, text, parent, revision)) = &self.pending else {
            return;
        };
        if *pending != request {
            return;
        }
        if result.is_ok() && current_session {
            self.input_history.record(text);
            self.catalog
                .borrow_mut()
                .record_sent_emotes(&self.name, text);
        }
        let clear_reply = result.is_ok()
            && current_session
            && self.compose_revision == *revision
            && twitch_message_text(&self.draft.read(cx).value()) == *text
            && self.reply_target.as_ref().map(|r| &r.id) == parent.as_ref();
        if clear_reply {
            self.reply_target = None;
        }
        if clear_reply {
            self.draft
                .update(cx, |draft, cx| draft.set_value("", window, cx));
            cx.emit(PaneEvent::DraftChanged);
        }
        self.send_status = match result {
            Ok(()) => "Sent".into(),
            Err(e) => e.clone(),
        };
        self.pending = None;
        cx.notify();
    }
    fn selected_text(&self) -> String {
        let timeline = self.timeline.borrow();
        let first = self.next_id - timeline.messages().len();
        let lines: Vec<_> = timeline
            .messages()
            .iter()
            .map(|message| message.copy_line())
            .collect();

        self.selection.borrow().copy(
            lines
                .iter()
                .enumerate()
                .map(|(index, text)| ((first + index) as u64, text.as_str())),
        )
    }
    fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.selected_text();
        if text.is_empty() {
            self.last_copy_result = Some("empty_selection");
            window.push_notification(
                Notification::info("Select some chat text first")
                    .id::<CopyFeedback>()
                    .delivery(NotificationDelivery::InApp),
                cx,
            );
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        // A write has no Result. Confirm read-back before claiming success or
        // discarding the user's selection; a busy clipboard can be retried.
        if cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref()
            == Some(text.as_str())
        {
            self.last_copy_result = Some("clipboard_verified");
            self.selection.borrow_mut().clear();
            window.push_notification(
                Notification::success(format!("Copied · {} characters", text.chars().count()))
                    .id::<CopyFeedback>()
                    .delivery(NotificationDelivery::InApp),
                cx,
            );
        } else {
            self.last_copy_result = Some("clipboard_unverified");
            window.push_notification(
                Notification::warning(
                    "Clipboard copy could not be verified. Selection kept; try again.",
                )
                .id::<CopyFeedback>()
                .delivery(NotificationDelivery::InApp),
                cx,
            );
        }
        cx.notify();
        window.refresh();
    }
}
impl Render for ChannelPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let channel_name = self.name.clone();
        let picker = (self.picker.open && !window.has_active_dialog(cx)).then(|| {
            let position = self
                .picker
                .button_bounds
                .get()
                .map(|b| point(b.right(), b.origin.y - px(6.)))
                .or_else(|| self.viewport.borrow().map(|b| point(b.right(), b.bottom())))
                .unwrap_or(point(
                    window.viewport_size().width - px(8.),
                    window.viewport_size().height - px(60.),
                ));
            deferred(
                anchored()
                    .anchor(Anchor::BottomRight)
                    .position(position)
                    .snap_to_window_with_margin(px(8.))
                    .child(self.render_picker(window, cx)),
            )
            .with_priority(1)
            .into_any_element()
        });
        let suggestions = (!self.picker.suggestions.is_empty() && !self.picker.open)
            .then(|| self.render_suggestions(cx));
        let emote_button_bounds = self.picker.button_bounds.clone();
        let emote_button_owner = cx.entity().downgrade();
        let emote_picker_open = self.picker.open;
        let catalog = self.catalog.clone();
        let appearance = self.chatter_appearance.clone();
        let timestamps = self.timestamps;
        let timeline = self.timeline.clone();
        let media = self.media.clone();
        let retained = timeline.borrow().messages().len();
        let first_order = (self.next_id - retained) as u64;
        // Only unread rows actually below the view are "new below"; rows that
        // arrived while the window was inactive may already be on screen or above.
        let routine_status = matches!(self.send_status.as_str(), "Sent" | "Sending…");
        let unread_below = {
            let scroller = self.scroller.read(cx);
            self.attention
                .unread
                .iter()
                .rev()
                .take_while(|row| {
                    row.checked_sub(first_order)
                        .is_some_and(|ix| scroller.item_is_below_viewport(ix as usize))
                })
                .take(1000)
                .count()
        };
        self.search.refresh(&timeline.borrow(), first_order);
        let search_hits = if self.search.open {
            self.search.hits.clone()
        } else {
            Rc::default()
        };
        let read_boundary = self.attention.read_boundary;
        let jump_pulse = self.jump_pulse;
        let highlight_rows = self.attention.highlights.clone();
        let highlight_colors = self.attention.colors.clone();
        let expanded = self.expanded.clone();
        let collapse_lines = COLLAPSE_LINES.load(std::sync::atomic::Ordering::Relaxed);
        let view_filter = self.view_filter.as_ref().map(|(_, q)| q.clone());
        let ignores = self.ignores.clone();
        let hidden_platforms = self.hidden_platforms.clone();
        // Shared Chat: colours per guest channel, hidden channels, and the look.
        let shared_style = shared_chat::style();
        let shared_colors = self
            .shared_session
            .as_ref()
            .map(|s| shared_chat::colors(s, self.shared_own.as_deref()))
            .unwrap_or_default();
        let hidden_sources = self.hidden_sources.clone();
        let own_hidden = self
            .shared_own
            .as_ref()
            .is_some_and(|own| self.hidden_sources.contains(own));
        // Name the Twitch channel on each message only when several are mixed.
        let special_tab = combined::is_special(&self.name);
        let live_tab = self.name.as_ref() == combined::LIVE;
        let origin_names = self
            .all_sources()
            .iter()
            .filter(|s| !youtube::is_youtube(s))
            .count()
            > 1
            || combined::is_special(&self.name);
        let moderating = matches!(
            self.role,
            Some(capabilities::Role::Moderator | capabilities::Role::Broadcaster)
        );
        let mod_buttons = self.mod_buttons && moderating;
        let own_user = self.attention.own_user().map(str::to_owned);
        let custom_buttons = self.custom_mod_buttons.clone();
        // Scrollbar ticks: highlights in their colour, search hits in lilac.
        let markers = {
            let count = self.timeline.borrow().messages().len().max(1);
            let first = (self.next_id - count.min(self.next_id)) as u64;
            let at = |row: u64| (row.saturating_sub(first) as f32 + 0.5) / count as f32;
            let mut markers: Vec<(f32, u32)> = highlight_rows
                .iter()
                .filter(|row| **row >= first)
                .map(|row| {
                    let color = highlight_colors.get(row).copied().unwrap_or(0);
                    (at(*row), crate::highlight_rules::colors(color).0)
                })
                .collect();
            if self.search.open {
                markers.extend(
                    self.search
                        .hits
                        .keys()
                        .filter(|row| **row >= first)
                        .map(|row| (at(*row), theme::accent())),
                );
            }
            // A tall list can have thousands; keep the newest few hundred.
            let excess = markers.len().saturating_sub(400);
            markers.drain(..excess);
            markers
        };
        let presented_row = self.presented_row.clone();
        let read_eligible = self.read_eligible.clone();
        let search_current = self.search.open.then_some(self.search.current).flatten();
        let search_bar = self.search.open.then(|| self.render_search(cx));
        let selection = self.selection.clone();
        let focus = self.focus.clone();
        let viewport = self.viewport.clone();
        let viewport_layout = self.viewport.clone();
        let picker_layout_owner = cx.entity().downgrade();
        let picker_is_open = self.picker.open;
        let finish = self.selection.clone();
        let copy_owner = cx.weak_entity();
        let menu_owner = cx.weak_entity();
        // Chatterino's "hide input when empty": back as soon as this chat is
        // focused, you start typing, reply, or something is sending.
        let hide_input = self.overlay.is_some()
            || crate::replies::option(crate::replies::HIDE_EMPTY_INPUT)
                && self.draft.read(cx).value().is_empty()
                && self.reply_target.is_none()
                && self.pending.is_none()
                && !self.picker.open
                && !self.focus.is_focused(window)
                && !self.draft.read(cx).focus_handle(cx).is_focused(window);
        let automod_held = self.automod.clone();
        let automod_outcomes = self.automod_outcomes.clone();
        let link_press = self.link_press.clone();
        let finish_link = self.link_press.clone();
        let copy_viewport = self.viewport.clone();
        let following = self.scroller.read(cx).is_following_tail();
        let paused = self.scroller.read(cx).paused();
        let now = Instant::now();
        self.entrances.retain(|entry| entry.active(now));
        let entrances = if following && !cx.reduce_motion() && {
            let selection = self.selection.borrow();
            !selection.dragging && selection.anchor == selection.head
        } {
            self.entrances.clone()
        } else {
            Vec::new()
        };
        div()
            .id("channel-pane")
            .key_context("JawjackChannel")
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Search,w,cx|{this.open_search(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&CatchUpChat,w,cx|{this.navigate_attention(0,w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&PreviousHighlight,w,cx|{this.navigate_attention(-1,w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&OpenEmotePicker,w,cx|{this.toggle_picker(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&NextHighlight,w,cx|{this.navigate_attention(1,w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&SentHistory,w,cx|{this.open_input_history(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&OpenChatterList,w,cx|{this.open_chatter_list(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&WatchInStreamlink,w,cx|{
                if !combined::is_special(&this.name) {
                    let note=match streamlink::open(&this.name){Ok(())=>format!("Starting #{} in Streamlink…",this.name),Err(problem)=>problem};
                    w.push_notification(Notification::info(note),cx);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this,_:&OpenStreamInBrowser,_,cx|{if !combined::is_special(&this.name){cx.open_url(&format!("https://www.twitch.tv/{}",this.name));}cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&OpenPlayerPopout,_,cx|{if !combined::is_special(&this.name){cx.open_url(&streamlink::player_url(&this.name));}cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&ToggleChatPause,_,cx|{this.scroller.update(cx,|s,cx|s.toggle_pause(cx));cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&ResumeChat,_,cx|{this.scroller.update(cx,|s,cx|s.scroll_to_end(cx));cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&FindChat,w,cx|{this.open_search(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&NextChatMatch,_,cx|{this.next_search(true,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&PreviousChatMatch,_,cx|{this.next_search(false,cx);cx.stop_propagation();}))
            .v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .map(|el| match self.overlay {
                Some(opacity) => el.bg(rgba((theme::canvas() << 8) | (opacity.clamp(0., 1.) * 255.) as u32)),
                None => el.bg(rgb(theme::canvas())).border_1().border_color(rgb(theme::border())),
            })
            .rounded(px(10.))
            .overflow_hidden()
            .child(
                div()
                    .h_flex()
                    .when(self.overlay.is_some(), |el| el.hidden())
                    .h(px(26.))
                    .px_2()
                    .gap_2()
                    .bg(rgb(theme::panel()))
                    .border_b_1()
                    .border_color(rgb(theme::border()))
                    .child(
                        // Name and status truncate first so the pane controls stay reachable.
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(13.))
                            .id("channel-drag-title")
                            .cursor(CursorStyle::OpenHand)
                            .on_drag(DraggedChannel { pane: cx.entity(), name: self.name.to_string() }, |drag, _, _, cx| {
                                drag.pane.update(cx, |_,cx|cx.emit(PaneEvent::DragStarted));
                                cx.stop_propagation();
                                cx.new(|_| DragPreview(format!("# {}", drag.name)))
                            })
                            .on_mouse_up(MouseButton::Middle, cx.listener(|this, _, _, cx| {
                                cx.open_url(&format!("https://www.twitch.tv/{}", this.name));
                                cx.stop_propagation();
                            }))
                            .tooltip({
                                let (channel, info, media, hide) = (self.name.to_string(), self.stream.clone(), self.media.clone(), self.streamer_mode);
                                move |_, cx| {
                                    let (channel, info, media) = (channel.clone(), info.clone(), media.clone());
                                    cx.new(|cx| stream_preview::StreamPreview::new(channel, info, media, hide, cx)).into()
                                }
                            })
                            .child(div().h_flex().gap_1().items_center()
                                .when(youtube::is_youtube(&self.name), |el| el.child(crate::assets::platform_icon(combined::Platform::YouTube).small()))
                                .when(self.all_sources().iter().any(|s| youtube::is_youtube(s)), |el| el.child(crate::assets::beta_badge("youtube-beta-pane", crate::assets::YOUTUBE_BETA)))
                                .child(match &self.display_title {
                                    Some(title) => title.clone(),
                                    None if youtube::is_youtube(&self.name) => "YouTube stream".into(),
                                    None => format!("# {}", combined::title(&self.name)),
                                })
                                // Live details beside the name (Settings → Appearance → Channel header).
                                .children(self.stream.as_ref().filter(|s| s.live).map(|info| {
                                    let counts = !streamer_mode::hides(streamer_mode::VIEWER_COUNTS);
                                    let parts = [
                                        (theme::header_shows(0) && counts).then(|| format!("{} viewers", follows_live::viewers(info.viewers))),
                                        (theme::header_shows(1) && counts).then(|| info.uptime()).flatten(),
                                        (theme::header_shows(2) && !info.category.is_empty()).then(|| info.category.clone()),
                                        (theme::header_shows(3) && !info.title.is_empty()).then(|| info.title.clone()),
                                    ];
                                    let text = parts.into_iter().flatten().collect::<Vec<_>>().join(" · ");
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(rgb(theme::muted()))
                                        .child(text)
                                }))),
                    )
                    .children(self.render_filter_chip(cx))
                    .children(self.render_platform_toggles(cx))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(12.))
                            .text_color(rgb(theme::muted()))
                            .child(if self.connected { if paused { "Paused".to_owned() } else if following { "Latest".to_owned() } else { format!("{retained}/{} · History", self.timeline.borrow().capacity()) } } else { self.connection.clone() }),
                    )
                    .child(Button::new("chatter-list").ghost().xsmall().icon(Icon::new(gpui_kit::assets::IconName::Users)).tooltip("Chatters").on_click(cx.listener(|this,_,window,cx|this.open_chatter_list(window,cx))))
                    .when(moderating, |el| el.child(Button::new("mod-buttons").ghost().xsmall().icon(Icon::new(gpui_kit::assets::IconName::Swords)).when(self.mod_buttons, |b| gpui_kit::component::Selectable::selected(b, true)).tooltip(if self.mod_buttons { "Hide moderator buttons on messages" } else { "Show moderator buttons on messages: delete, 10-minute timeout, ban" }).on_click(cx.listener(|_,_,_,cx|cx.emit(PaneEvent::ToggleModButtons)))))
                    .when(self.mod_tools && matches!(self.role, Some(capabilities::Role::Moderator | capabilities::Role::Broadcaster)), |el| el.child(Button::new("mod-tools").ghost().xsmall().icon(Icon::new(gpui_kit::assets::IconName::Shield)).tooltip("Moderation: chat modes, clear chat").on_click(cx.listener(|_,_,_,cx|cx.emit(PaneEvent::OpenModTools)))))
                    .when(self.role == Some(capabilities::Role::Broadcaster), |el| el.child(Button::new("stream-info").ghost().xsmall().icon(Icon::new(gpui_kit::assets::IconName::Pencil)).tooltip("Edit stream title and category").on_click(cx.listener(|_,_,_,cx|cx.emit(PaneEvent::OpenStreamInfo)))))
                    .child(Button::new("pause-chat").ghost().xsmall().icon(if paused{IconName::Play}else{IconName::Pause}).tooltip(if paused{"Resume live chat · Ctrl+End"}else{"Pause scrolling · Ctrl+Space"}).on_click(cx.listener(|this,_,_,cx|{this.scroller.update(cx,|s,cx|{if s.paused(){s.scroll_to_end(cx);}else{s.toggle_pause(cx);}});})))
                    .child(Button::new("find-chat").ghost().xsmall().icon(IconName::Search).tooltip("Find in chat · Ctrl+F").on_click(cx.listener(|this,_,w,cx|this.open_search(w,cx))))
                    .child(Button::new("close-pane").ghost().xsmall().icon(IconName::Close).tooltip("Close this split…; keep draft")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PaneEvent::RequestClose)))),
            )
            .children(search_bar)
            .child(
                div()
                    .id("transcript")
                    .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
                    .capture_any_mouse_down(cx.listener(|this,event:&MouseDownEvent,_,cx|{if event.button==MouseButton::Left{this.scroller.update(cx,|s,_|s.cancel_auto_resume());}}))
                    .on_hover(cx.listener(|this,over,w,cx|{this.transcript_hover = *over;this.update_hover_pause(w,cx);}))
                    .on_modifiers_changed(cx.listener(|this,_,w,cx|this.update_hover_pause(w,cx)))
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
            .track_focus(&self.focus)
            .key_context("ChatTranscript")
                    // With the message box hidden while empty, typing in the chat opens it.
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let keys = &event.keystroke;
                        if !crate::replies::option(crate::replies::HIDE_EMPTY_INPUT)
                            || keys.modifiers.control || keys.modifiers.alt || keys.modifiers.platform
                            || !this.draft.read(cx).value().is_empty()
                        {
                            return;
                        }
                        let Some(text) = keys.key_char.clone().filter(|t| !t.is_empty() && !t.chars().any(char::is_control)) else {
                            return;
                        };
                        let handle = this.draft.read(cx).focus_handle(cx);
                        handle.focus(window, cx);
                        this.draft.update(cx, |input, cx| input.replace(text, window, cx));
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.focus.focus(window, cx);
                        this.selection.borrow_mut().clear();
                        cx.notify();
                    }))
                    .text_size(px(self.font_size))
                    .line_height(px(self.font_size + 8.))
            .on_action(cx.listener(|this, _: &CopyChatSelection, window, cx| {
                this.copy(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &ClearChatSelection, window, cx| {
                if this.search.open {this.close_search(window,cx);}else{this.selection.borrow_mut().clear();}
                cx.notify();
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &SelectAllChat, _, cx| {
                let timeline = this.timeline.borrow();
                let first = this.next_id - timeline.messages().len();
                if let Some(last) = timeline.messages().back() {
                    this.selection.borrow_mut().select_span(
                        chat_core::selection::Point { row: first as u64, byte: 0 },
                        chat_core::selection::Point { row: (this.next_id - 1) as u64, byte: last.copy_line().len() },
                    );
                }
                cx.notify();
                cx.stop_propagation();
            }))
                    .on_prepaint(move |bounds, _, cx| {
                        let changed=*viewport_layout.borrow()!=Some(bounds);
                        *viewport_layout.borrow_mut() = Some(bounds);
                        if changed&&picker_is_open{let owner=picker_layout_owner.clone();cx.defer(move|cx|{let _=owner.update(cx,|_,cx|cx.notify());});}
                    })
                    .relative()
                    .when_some(theme::chat_font(), |el, font| el.font_family(font))
                    .when_some(self.row_menu.as_ref().map(|(menu, at, _)| (menu.clone(), *at)), |el, (menu, at)| {
                        el.child(deferred(anchored().position(at).snap_to_window_with_margin(px(8.)).child(menu)).with_priority(1))
                    })
                    .when(retained == 0, |el| el.child(div().v_flex().p_6().gap_2().text_color(rgb(theme::muted()))
                        .child(div().text_color(rgb(theme::text())).text_size(px(16.)).child(if let Some(title) = combined::special_title(&self.name) { title.to_owned() } else if youtube::is_youtube(&self.name) { format!("{} is ready", combined::title(&self.name)) } else { format!("#{} is ready", combined::title(&self.name)) }))
                        .child(if self.connected && !combined::is_special(&self.name) { "Connected. Waiting for messages…".to_string() } else { self.connection.clone() })
                        .when(youtube::is_youtube(&self.name) && self.connection.contains("Connect YouTube"), |el| el.child(div().pt_2().h_flex().child(
                            Button::new("youtube-setup").small().primary().label("Connect YouTube").on_click(cx.listener(|_, _, _, cx| cx.emit(PaneEvent::SetUpYouTube))))))))
                    .child(
                        MessageScroller::new("chat", self.scroller.clone(), move |index, window, cx| {
                            let messages = timeline.borrow();
                            let Some(message) = messages.messages().get(index) else {
                                return div().into_any_element();
                            };
                            // A filtered-out row draws nothing (rows have no padding of their own).
                            // Someone blocked after their messages arrived disappears too.
                            let blocked = ignores.borrow().blocked.contains(&message.user_id);
                            if blocked && ignores.borrow().blocked_placeholder {
                                return div().text_size(px(11.)).text_color(rgb(theme::muted())).italic().py(px(2.))
                                    .child("Message from someone you blocked")
                                    .into_any_element();
                            }
                            if blocked
                                || (message.deleted && theme::flag(&theme::HIDE_DELETED))
                                || (message.presentation.kind == chat_core::MODERATION && streamer_mode::hides(streamer_mode::MOD_ACTIONS))
                                || match &message.presentation.source_id {
                                    Some(source) => hidden_sources.contains(source),
                                    None => own_hidden,
                                }
                                || (view_filter.as_ref().is_some_and(|q| q.matches(message).is_none())
                                    && !(chat_search::FILTER_KEEPS_OWN.load(std::sync::atomic::Ordering::Relaxed)
                                        && own_user.as_deref() == Some(message.user_id.as_str())))
                                || (!hidden_platforms.is_empty() && hidden_platforms.contains(&message.presentation.origin.as_deref().map(combined::Platform::of).unwrap_or(combined::Platform::Twitch))) {
                                return div().into_any_element();
                            }
                            let row = first_order + index as u64;
                            // Rows on screen keep their chatter's 7TV paint and badge current
                            // (looked up at most every 30 minutes per chatter).
                            catalog.borrow_mut().want_cosmetics(&message.user_id);
                            let paint=if appearance.borrow().get(&message.user_id).is_some_and(|a|a.color.is_some()) || !name_paint::paint_option(0){None}else{
                                let catalog=catalog.borrow();catalog.paints.for_user(&message.user_id).filter(|p|p.limitation.is_none()).cloned().map(|mut paint|{
                                    if !name_paint::paint_option(1){paint.shadows.clear();}
                                    // Image paints load (and animate) through the media cache.
                                    let image=paint.image().and_then(media::EmoteKey::paint_image).and_then(|key|media.borrow_mut().get(&key,cx));
                                    crate::name_paint::Request{paint,cache:catalog.paint_cache.clone(),image}
                                })
                            };
                            let inline = inline_chat::InlineChat::new(message, paint.clone(), row, &mut media.borrow_mut(), selection.clone(), focus.clone(), viewport.clone(), cx)
                                .unwrap_or_else(|| inline_chat::InlineChat::plain(message, paint.clone(), row, selection.clone(), focus.clone(), viewport.clone()));
                            let first_line_height = inline.visual_line_height(window);
                            // Roughly how many lines this message wraps to at the pane's width.
                            let collapsed = collapse_lines > 0
                                && message.presentation.kind != chat_core::MODERATION
                                && message.presentation.kind != chat_core::SYSTEM
                                && !expanded.borrow().contains(&message.id)
                                && viewport.borrow().is_some_and(|bounds| {
                                    let width = (f32::from(bounds.size.width) - 120.).max(80.);
                                    let per_line = width / (f32::from(window.rem_size()) * 0.5);
                                    message.copy_line().chars().count() as f32 / per_line > collapse_lines as f32 + 0.5
                                });
                            let show_mod_buttons = mod_buttons && !message.deleted && !message.id.starts_with(automod::LINE_PREFIX) && message.login.is_some() && own_user.as_deref() != Some(message.user_id.as_str());
                            let mut badge_images=message.badges.iter().filter(|badge|theme::badge_shown(theme::badge_kind(&badge.set_id))).filter_map(|badge|{
                                let custom=if message.presentation.source.is_none(){catalog.borrow().community.channels.get(channel_name.as_ref()).filter(|set|set.ffz_room_id==message.channel_id).and_then(|set|set.roles.get(&badge.set_id)).cloned()}else{None};
                                if let Some(custom)=custom{
                                    let image=media.borrow_mut().get(&custom.key,cx);
                                    if image.is_some(){return Some((custom,image,(badge.set_id=="moderator").then_some(0x34AE0A)));}
                                }
                                catalog.borrow().twitch.badge(&message.channel_id,&badge.set_id,&badge.id).cloned().map(|badge|{let image=media.borrow_mut().get(&badge.key,cx);(badge,image,None)})
                            }).collect::<Vec<_>>();
                            {
                                let catalog=catalog.borrow();
                                let room=if message.presentation.source.is_none(){catalog.community.channels.get(channel_name.as_ref()).filter(|set|set.ffz_room_id==message.channel_id).map(|set|&set.roles.grants)}else{None};
                                for definition in catalog.ffz_badges.for_user(&message.user_id,room).into_iter().filter(|_|theme::badge_shown(theme::FFZ_BADGES)){let image=media.borrow_mut().get(&definition.badge.key,cx);badge_images.push((definition.badge,image,definition.background));}
                            }
                            if let Some(badge)=catalog.borrow().badges.for_user(&message.user_id).cloned().filter(|_|theme::badge_shown(theme::SEVEN_TV_BADGES)){let image=media.borrow_mut().get(&badge.key,cx);badge_images.push((badge,image,None));}
                            if theme::badge_shown(theme::CHATTERINO_BADGES){for badge in catalog.borrow().chatterino_badges.for_user(&message.user_id).iter().cloned(){let image=media.borrow_mut().get(&badge.key,cx);badge_images.push((badge,image,None));}}
                            // A definitively failed badge drops its slot instead of leaving a blank gap;
                            // pending art keeps its slot so loading does not reflow the row.
                            badge_images.retain(|(badge,image,_)|image.is_some()||!media.borrow().failed(&badge.key));
                            let row_owner=menu_owner.clone();let row_id=message.id.clone();
                            let timestamp=if timestamps>0{message.sent_at.and_then(|time|chrono::DateTime::from_timestamp(time,0)).map(|time|{
                                let local=time.with_timezone(&chrono::Local);(local.format(match timestamps{2=>"%H:%M:%S",3=>"%-I:%M %p",4=>"%-I:%M:%S %p",_=>"%H:%M"}).to_string(),local.format("%Y-%m-%d %H:%M:%S %:z").to_string())
                            })}else{None};
                            // The metadata is overlaid on line 1, not kept as a permanent
                            // flex column. Measure the same fixed-width slots and variable
                            // labels to reserve its width only for the first text line.
                            let small_label_width = |label: &str| -> f32 {
                                if label.is_empty() { return 0.; }
                                let style = window.text_style();
                                let shaped = window.text_system().shape_line(label.to_owned().into(), px(10.), &[style.to_run(label.len())], None);
                                f32::from(shaped.width())
                            };
                            let mut prefix_parts = Vec::<f32>::new();
                            if timestamp.is_some() {
                                prefix_parts.push(match timestamps { 2 => 55., 3 => 58., 4 => 76., _ => 37. });
                            }
                            if show_mod_buttons {
                                // Three 16px built-in actions, 1px inner gaps, and
                                // optional icon/text custom moderation buttons.
                                let mut buttons_width = 3. * 16. + 2.;
                                for button in custom_buttons.iter() {
                                    buttons_width += 1. + 6. + if mod_buttons::icon(&button.label).is_some() {
                                        12.
                                    } else {
                                        small_label_width(&button.label)
                                    };
                                }
                                prefix_parts.push(buttons_width);
                            }
                            if let Some(origin) = message.presentation.origin.as_deref() {
                                let origin_label = origin_names && combined::Platform::of(origin) == combined::Platform::Twitch;
                                prefix_parts.push(12. + if origin_label { 4. + 8. + small_label_width(origin) } else { 0. });
                            }
                            if message.presentation.source_id.is_some() {
                                prefix_parts.push(20.);
                            }
                            prefix_parts.extend(std::iter::repeat_n(18., badge_images.len()));
                            if message.deleted {
                                prefix_parts.push(8. + small_label_width("deleted"));
                            }
                            // Outer .gap(px(4.)) also separates the final metadata
                            // item from the first text glyph.
                            let first_line_inset = if prefix_parts.is_empty() { px(0.) } else {
                                px(prefix_parts.iter().sum::<f32>() + 4. * prefix_parts.len() as f32)
                            };
                            let pulse=jump_pulse.filter(|(target,at)|*target==row&&at.elapsed()<Duration::from_millis(1200));
                            if pulse.is_some()&&!cx.reduce_motion(){window.request_animation_frame();}
                            let rich_heading=rich_messages::heading(message);
                            let automod_controls=message.id.strip_prefix(automod::LINE_PREFIX).map(|held_id|automod_row(held_id,&automod_held,&automod_outcomes,menu_owner.clone()));
                            let rich_attachments=rich_messages::attachments(message,&mut media.borrow_mut(),cx);
                            let reply_line=if replies::option(1){replies::row_context(message,&messages,menu_owner.clone())}else{None};
                            let interaction=message_actions::Interaction{owner:menu_owner.clone(),message:message.id.clone(),author_len:message.author_label().len(),links:message_actions::message_links(message),mentions:message_actions::message_mentions(message),pressed:link_press.clone()};
                            let presented=presented_row.clone();let eligible=read_eligible.clone();
                            let matches=search_hits.get(&row).cloned().unwrap_or_default();
                            let progress = entrances.iter().find(|entry| entry.row == row)
                                .map(|entry| {
                                    let now = Instant::now();
                                    let start = entry.started.get().unwrap_or_else(|| {
                                        entry.started.set(Some(now));
                                        now
                                    });
                                    (now.duration_since(start).as_secs_f32() / 0.26).clamp(0.0, 1.0)
                                }).unwrap_or(1.0);
                            if progress < 1.0 { window.request_animation_frame(); }
                            let eased = 1.0 - (1.0 - progress).powi(3);
                            // Reply button on hover (Settings → Chat → Input & replies).
                            let hover_reply = (replies::option(2) && !special_tab && message.replyable && !message.deleted).then(|| (menu_owner.clone(), message.id.clone()));
                            let row_group = SharedString::from(format!("row-{}", message.id));
                            div()
                                .relative()
                                .group(row_group.clone())
                                .when_some(hover_reply, |el, (owner, id)| {
                                    el.child(div().id(SharedString::from(format!("hover-reply-{id}"))).absolute().top(px(2.)).right(px(6.)).size(px(20.)).rounded(px(4.)).flex().items_center().justify_center()
                                        .bg(rgb(theme::elevated())).border_1().border_color(rgb(theme::border())).text_color(rgb(theme::muted())).cursor_pointer()
                                        .opacity(0.).group_hover(row_group.clone(), |s| s.opacity(1.)).hover(|s| s.text_color(rgb(theme::text())))
                                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Reply").build(w, cx))
                                        .capture_any_mouse_down(move |event: &MouseDownEvent, window, cx| {
                                            if event.button != MouseButton::Left { return; }
                                            cx.stop_propagation();
                                            let _ = owner.update(cx, |p, cx| p.message_action(&id, message_actions::Action::Reply, window, cx));
                                        })
                                        .child(Icon::new(gpui_kit::assets::IconName::Reply).xsmall()))
                                })
                                // Every row keeps the same inset so text never touches a highlight edge
                                // and highlighting a row doesn't shift its text sideways.
                                .border_l_2().border_color(rgba(0x00000000)).pl(px(6.)).pr(px(4.))
                                .when(message.deleted, |el| el.opacity(0.45))
                                // Settings → Appearance → Alternate row backgrounds (Chatterino's option).
                                .when(theme::alternate_rows() && row % 2 == 1, |el| el.bg(rgba(crate::theme::tone_a(0xFFFFFF07))))
                                // Shared Chat: a short rounded bar in the guest channel's colour, in the row's left gutter.
                                .when_some(message.presentation.source_id.as_ref().filter(|_| shared_style == 0 || shared_style == 2).map(|id| shared_colors.get(id).copied().unwrap_or_else(|| shared_chat::color_of(id))), |el, color| el.child(div().absolute().left(px(1.)).top(px(4.)).bottom(px(4.)).w(px(3.)).rounded_full().bg(rgb(color))))
                                .when(theme::flag(&theme::SEPARATORS), |el| el.child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(rgb(theme::border()))))
                                .when_some(rich_messages::accent(message),|el,color|el.border_color(rgb(color)).bg(rgba(crate::theme::tone_a(0xA99CF40D))).py(px(2.)).rounded_r(px(4.)))
                                .when(highlight_rows.contains(&row),|el|{let (accent,background)=highlight_rules::colors(highlight_colors.get(&row).copied().unwrap_or(0));el.bg(rgb(background)).border_color(rgb(accent)).py(px(2.)).rounded_r(px(4.))})
                                .on_prepaint(move|bounds,window,cx|{
                                    let clip=window.content_mask().bounds;
                                    if following&&eligible.get()&&window.is_window_active()&&!window.has_active_dialog(cx)
                                        && bounds.origin.x>=clip.origin.x&&bounds.bottom()>clip.origin.y
                                        && bounds.right()<=clip.right()&&bounds.bottom()<=clip.bottom(){
                                        presented.set(Some(presented.get().map_or(row,|old|old.max(row))));
                                    }
                                })
                                .when(search_current==Some(row),|el|el.bg(rgb(crate::theme::tone(0x272331))))
                                .when_some(pulse,|el,(_,at)|{let alpha=if cx.reduce_motion(){255}else{(255.*(1.-at.elapsed().as_secs_f32()/1.2).clamp(0.,1.))as u32};el.border_color(rgba((0x9DD8CB<<8)|alpha))})
                                .left(px(12.0 * (1.0 - eased)))
                                .top(px(6.0 * (1.0 - eased)))
                                // Recent history is dimmed, as in Chatterino.
                                .opacity((if message.presentation.similar { 0.4 } else if message.presentation.historical { if theme::is_light() { 0.7 } else { 0.6 } } else { 0.25 + 0.75 * eased }) * if shared_style == 2 && message.presentation.source_id.is_some() { 0.55 } else { 1.0 })
                                .id(SharedString::from(message.id.clone()))
                                .min_w_0()
                                .cursor_text()
                                .on_mouse_down(MouseButton::Right,move|event,window,cx|{let at=event.position;let owner=row_owner.clone();let id=row_id.clone();window.defer(cx,move|window,cx|{let build=owner.clone();let menu=gpui_kit::component::menu::PopupMenu::build(window,cx,move|menu,window,cx|message_actions::menu(build,id,menu,window,cx));let _=owner.update(cx,|pane,cx|pane.open_row_menu(menu,at,window,cx));});})
                                .when(read_boundary==Some(row)&&index>0,|el|el.child(div().h_flex().items_center().gap_2().h(px(18.)).text_size(px(10.)).text_color(rgb(crate::theme::tone(0x9DBFCE))).child(div().h(px(1.)).flex_1().bg(rgb(crate::theme::tone(0x40535E)))).child("Last read position").child(div().h(px(1.)).flex_1().bg(rgb(crate::theme::tone(0x40535E))))))
                                .children(rich_heading)
                                .children(automod_controls)
                                .children(reply_line)
                                 .child(div().relative().v_flex().min_w_0().w_full()
                                .child(div().absolute().left_0().top_0().h_flex().items_start().gap(px(4.))
                                .when_some(timestamp,|el,(label,full)|el.child(div().id(SharedString::from(format!("timestamp-{}",message.id))).w(px(match timestamps{2=>55.,3=>58.,4=>76.,_=>37.})).h(first_line_height).flex_shrink_0().flex().items_center().text_size(px(10.)).text_color(rgb(theme::muted())).tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w,cx)).child(label)))
                                .when(show_mod_buttons, |el| {
                                    let tool = |id: &'static str, icon: gpui_kit::assets::IconName, tip: &'static str, hover: u32, action: message_actions::Action| {
                                        let (owner, message_id) = (menu_owner.clone(), message.id.clone());
                                        div().id(SharedString::from(format!("{id}-{}", message.id))).size(px(16.)).h(first_line_height).flex_shrink_0().flex().items_center().justify_center()
                                            .text_color(rgb(theme::muted())).cursor_pointer().hover(move |s| s.text_color(rgb(hover)))
                                            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
                                            // Act on press, in capture: the row's own handlers would swallow a click.
                                            .capture_any_mouse_down(move |event: &MouseDownEvent, window, cx| {
                                                if event.button != MouseButton::Left { return; }
                                                cx.stop_propagation();
                                                let action = action.clone();
                                                let _ = owner.update(cx, |p, cx| p.message_action(&message_id, action, window, cx));
                                            })
                                            .child(Icon::new(icon).xsmall())
                                    };
                                    el.child(div().h_flex().flex_shrink_0().gap(px(1.))
                                        .child(tool("mod-delete", gpui_kit::assets::IconName::Trash, "Delete message", 0xF29D9D, message_actions::Action::ModDelete))
                                        .child(tool("mod-timeout", gpui_kit::assets::IconName::Clock, "Timeout 10 minutes", 0xE8C468, message_actions::Action::ModTimeout(600)))
                                        .child(tool("mod-ban", gpui_kit::assets::IconName::Ban, "Ban…", 0xF07A7A, message_actions::Action::ModBan))
                                        .children(custom_buttons.iter().enumerate().map(|(index, button)| {
                                            let (owner, message_id, command, tip) = (menu_owner.clone(), message.id.clone(), button.command.clone(), button.command.clone());
                                            div().id(SharedString::from(format!("mod-custom-{index}-{}", message.id))).h(first_line_height).px(px(3.)).flex_shrink_0().flex().items_center()
                                                .rounded(px(3.)).text_size(px(10.)).text_color(rgb(theme::muted())).cursor_pointer()
                                                .hover(|s| s.text_color(rgb(theme::text())).bg(rgb(theme::hover())))
                                                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                                                .capture_any_mouse_down(move |event: &MouseDownEvent, window, cx| {
                                                    if event.button != MouseButton::Left { return; }
                                                    cx.stop_propagation();
                                                    let action = message_actions::Action::ModCustom(command.clone());
                                                    let _ = owner.update(cx, |p, cx| p.message_action(&message_id, action, window, cx));
                                                })
                                                .child(match mod_buttons::icon(&button.label) {
                                                    Some(icon) => Icon::new(icon).xsmall().into_any_element(),
                                                    None => button.label.clone().into_any_element(),
                                                })
                                        })))
                                })
                                // Combined feed: which channel this came from, colour-keyed per channel.
                                .when_some(message.presentation.origin.clone(), |el, origin| {
                                    let platform = combined::Platform::of(&origin);
                                    let icon = div().id(SharedString::from(format!("src-{origin}"))).flex().items_center()
                                        .tooltip({ let label = if platform == combined::Platform::YouTube { "From YouTube".to_owned() } else { format!("From Twitch · #{origin}") }; move |w, cx| gpui_kit::component::tooltip::Tooltip::new(label.clone()).build(w, cx) })
                                        .child(crate::assets::platform_icon(platform).xsmall());
                                    // In a Mentions tab, the tag takes you to the message in its channel.
                                    let go = special_tab.then(|| (menu_owner.clone(), origin.clone(), message.id.clone()));
                                    el.child(div().id(SharedString::from(format!("origin-{}", message.id))).h(first_line_height).flex().items_center().gap_1().flex_shrink_0().child(icon)
                                        .when_some(go, |el, (owner, channel, id)| {
                                            let tip = if live_tab { format!("Open #{channel}") } else { format!("Go to message in #{channel}") };
                                            el.cursor_pointer().rounded(px(3.)).hover(|s| s.bg(rgb(theme::hover())))
                                                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                                                .capture_any_mouse_down(move |event: &MouseDownEvent, _, cx| {
                                                    // Capture: the message text would otherwise take the press for selecting.
                                                    if event.button != MouseButton::Left { return; }
                                                    cx.stop_propagation();
                                                    let _ = owner.update(cx, |_, cx| cx.emit(PaneEvent::GoToMessage { channel: channel.clone(), id: id.clone(), reply: false }));
                                                })
                                        })
                                        .when(platform == combined::Platform::Twitch && origin_names, |el| {
                                            let hue = origin.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as usize % highlight_rules::COLORS.len();
                                            let (_, accent, background) = highlight_rules::COLORS[hue];
                                            el.child(div().px(px(4.)).rounded(px(3.)).bg(rgb(background)).text_color(rgb(accent)).text_size(px(10.)).child(origin.clone()))
                                        }))
                                })
                                .when_some(message.presentation.source_id.clone(), |el, source_id| {
                                    // Shared Chat: the channel it was sent in, as that streamer's avatar (like Twitch).
                                    let (avatar, profile_name) = { let mut c = catalog.borrow_mut(); c.profiles.request(&source_id); let p = c.profiles.get(&source_id); (p.and_then(|p| p.avatar.clone()), p.map(|p| p.display_name.clone()).filter(|n| !n.is_empty())) };
                                    let name = profile_name.or_else(|| message.presentation.source.clone().filter(|s| s != "Shared Chat")).unwrap_or_else(|| "another channel".into());
                                    let image = avatar.and_then(|key| media.borrow_mut().get(&key, cx));
                                    // Ringed in the channel's colour, matching its bubble at the top of the chat.
                                    let ring = (shared_style < 3).then(|| shared_colors.get(&source_id).copied().unwrap_or_else(|| shared_chat::color_of(&source_id)));
                                    el.child(div().id(SharedString::from(format!("shared-{source_id}"))).w(px(20.)).h(first_line_height).flex().items_center().flex_shrink_0()
                                        .tooltip(media_hover::image_card(name.clone(), "Shared Chat · sent in their channel".into(), image.clone()))
                                        .child(div().size(px(20.)).rounded_full().flex().items_center().justify_center().when_some(ring, |el, ring| el.border_2().border_color(rgb(ring))).child(match image {
                                            Some(image) => emote_picker::avatar(image, if ring.is_some() { 14. } else { 18. }),
                                            None => div().size(px(14.)).rounded_full().bg(rgb(crate::theme::tone(0x3A3350))).flex().items_center().justify_center().text_size(px(9.)).text_color(rgb(theme::text()))
                                                .child(message.presentation.source.as_deref().and_then(|n| n.chars().next()).unwrap_or('?').to_uppercase().to_string()).into_any_element(),
                                        })))
                                })
                                .children(badge_images.into_iter().map(|(badge,image,background)|{
                                    div().id(SharedString::from(badge.key.id.clone())).w(px(18.)).h(first_line_height).flex().items_center().flex_shrink_0().overflow_hidden().tooltip(media_hover::image_card(badge.title.clone(),media_hover::source(badge.key.provider()).to_owned(),image.clone())).child(div().size(px(18.)).when_some(background,|el,color|el.bg(rgb(color))).child(emote_picker::icon(image,String::new(),18.,18.)))
                                }).collect::<Vec<_>>())
                                .when(message.deleted, |el| el.child(div().h(first_line_height).flex().items_center().flex_shrink_0()
                                    .child(div().px(px(4.)).rounded(px(3.)).bg(rgb(crate::theme::tone(0x3A2328))).text_color(rgb(crate::theme::tone(0xF29D9D))).text_size(px(10.)).child("deleted"))))
                                )
                                .child(div().w_full().min_w_0().v_flex().child(div().min_w_0()
                                    // Collapsed: only the first lines, with "Show more" under them.
                                    .when(collapsed, |el| el.max_h(first_line_height * collapse_lines as f32).overflow_hidden())
                                    // `/me`: the whole message in the chatter's colour, like Chatterino.
                                    .when(message.presentation.kind == "action", |el| el.text_color(rgb(message.author_color().unwrap_or(theme::text()))).italic())
                                    .child(if message.presentation.kind == chat_core::MODERATION || message.presentation.kind == chat_core::SYSTEM {
                                    let repeats = message.presentation.repeats;
                                    let text = message.presentation.notice.clone().unwrap_or_default();
                                    let label = if repeats > 0 { format!("{text} ({} times)", repeats + 1) } else { text };
                                    div().text_color(rgb(theme::muted())).italic()
                                        .child(inline_chat::InlineChat::notice(&message.id, label, row, selection.clone(), focus.clone(), viewport.clone())
                                            .with_first_line_inset(first_line_inset))
                                        .into_any_element()
                                } else {
                                    inline.with_first_line_inset(first_line_inset).with_search(matches).with_interaction(interaction).into_any_element()
                                }))
                                .when(collapsed, |el| {
                                    let (owner, id) = (menu_owner.clone(), message.id.clone());
                                    el.child(div().id(SharedString::from(format!("expand-{id}"))).text_size(px(11.)).text_color(rgb(theme::accent())).cursor_pointer().hover(|s| s.underline())
                                        .capture_any_mouse_down(move |event: &MouseDownEvent, _, cx| {
                                            if event.button != MouseButton::Left { return; }
                                            cx.stop_propagation();
                                            let id = id.clone();
                                            let _ = owner.update(cx, |p, cx| {
                                                p.expanded.borrow_mut().insert(id);
                                                p.scroller.update(cx, |s, cx| s.remeasure(cx));
                                                cx.notify();
                                            });
                                        })
                                        .child("Show more"))
                                })))
                                .children(rich_attachments)
                                .into_any_element()
                        })
                        .flex_1()
                        .min_h_0()
                        .markers(markers)
                        .with_jump_button_renderer(move |button| {
                            // Show how much arrived below, like Chatterino's "more messages below".
                            if unread_below == 0 { button } else { button.label(format!("{} new", if unread_below > 999 { "999+".to_string() } else { unread_below.to_string() })) }
                        })
                        .with_row_style(StyleRefinement::default().px_3().pb_0())
                        .with_list_style(StyleRefinement::default().py_2())
                        .h_full(),
                    )
                    .children(self.render_shared_avatars(window, cx)),
            )
            .child(div().key_context("JawjackComposer").relative().v_flex().when(hide_input,|el|el.hidden()).p_1().gap_1().bg(rgb(theme::panel())).border_t_1().border_color(rgb(theme::border()))
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    // Handle the *physical* Enter before the multiline Textarea can
                    // change the draft. An inserted newline changes compose_revision,
                    // which otherwise leaves an already-sent message in the composer.
                    // One handler owns Enter, regardless of completion/picker state.
                    if event.keystroke.key == "enter" {
                        if event.keystroke.modifiers.shift {
                            window.prevent_default();
                            cx.stop_propagation();
                        } else if !event.keystroke.modifiers.control
                            && !event.keystroke.modifiers.platform
                            && !event.keystroke.modifiers.alt
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                            if this.picker.open {
                                this.picker_action("enter", window, cx);
                            } else if !this.completion_action("enter", window, cx) {
                                this.submit(window, cx);
                            }
                        }
                        return;
                    }
                    if this.picker.open {
                        if event.keystroke.modifiers.alt && matches!(event.keystroke.key.as_str(),"left"|"right"){this.picker_action(&event.keystroke.key,window,cx);}
                        return;
                    }
                    if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform
                        && let Some(text) = &event.keystroke.key_char {
                            let input = this.draft.read(cx);
                            let current = input.value().to_string();
                            let selected = input.selected_range();
                            let count = current[..selected.start].chars().count() + text.chars().count() + current[selected.end..].chars().count();
                            if count > 500 { window.prevent_default(); cx.stop_propagation(); }
                        }
                }))
                .on_action(cx.listener(|this,_: &PreviousInput,window,cx|this.recall_input(true,window,cx)))
                .on_action(cx.listener(|this,_: &NextInput,window,cx|this.recall_input(false,window,cx)))
                // Up/Down: the completion list when it's open, else sent-message history
                // (like Chatterino) while the draft is a single line.
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::MoveDown,window,cx|{if !this.completion_action("down",window,cx)&&!this.draft.read(cx).value().contains('\n'){this.recall_input(false,window,cx);}}))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::MoveUp,window,cx|{if !this.completion_action("up",window,cx)&&!this.draft.read(cx).value().contains('\n'){this.recall_input(true,window,cx);}}))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::IndentInline,window,cx|{this.completion_action("tab",window,cx);}))
                .capture_action(cx.listener(|this,_: &CompleteEmote,window,cx|{
                    if !this.draft.read(cx).focus_handle(cx).is_focused(window)||!this.completion_action("tab",window,cx){window.focus_next(cx);}
                    cx.stop_propagation();
                }))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::Escape,window,cx|{
                    if this.picker.open {this.toggle_picker(window,cx);cx.stop_propagation();}else if !this.completion_action("escape",window,cx) && this.reply_target.is_some(){this.cancel_reply(window,cx);cx.stop_propagation();}
                }))
                .capture_action(cx.listener(|this, _: &gpui_kit::base::input::Paste, window, cx| {
                    if this.picker.open {return;}
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = twitch_message_text(&text);
                        let input = this.draft.read(cx);
                        let current = input.value().to_string();
                        let selected = input.selected_range();
                        let count = current[..selected.start].chars().count() + text.chars().count() + current[selected.end..].chars().count();
                        if count <= 500 {
                            this.draft.update(cx, |input, cx| input.replace(text, window, cx));
                            cx.emit(PaneEvent::DraftChanged);
                        } else {
                            window.push_notification(Notification::info("That paste would exceed Twitch’s 500-character limit. Your draft is unchanged.").id::<ComposerFeedback>().delivery(NotificationDelivery::InApp), cx);
                        }
                        cx.stop_propagation();
                    }
                }))
                .children(self.render_raid_status(cx))
                .children(self.render_automod(cx))
                .children(self.render_spelling(cx))
                .children(self.render_reply_target(cx))
                .children(picker)
                .children(suggestions)
                .child(Textarea::new(&self.draft))
                // Routine progress sits inline so side-by-side composers keep equal height;
                // failures keep a full-width row so their explanation is never truncated.
                .when(!routine_status && !self.send_status.is_empty(), |el|el.child(div().text_size(px(11.)).text_color(rgb(theme::muted())).child(self.send_status.clone())))
                .child(div().h_flex().justify_between().gap_2().text_size(px(11.)).text_color(rgb(theme::muted()))
                    .child(div().h_flex().gap_2().flex_shrink_0()
                        .child(div().when(self.draft.read(cx).value().chars().count() > 500, |el|el.text_color(rgb(crate::theme::tone(0xF29D9D)))).child(format!("{} / 500", self.draft.read(cx).value().chars().count())))
                        .children(self.slow_mode_wait(cx).map(|left| div().text_color(rgb(crate::theme::tone(0xE8C468))).child(format!("Slow mode · {left}s"))))
                        .when(routine_status && !self.send_status.is_empty(), |el|el.child(self.send_status.clone())))
                    .when_some(self.room_settings.as_ref().filter(|r|!r.labels().is_empty()).map(|r|r.summary()),|el,summary|el.child(div().id("room-modes-hint").max_w(px(120.)).min_w_0().flex_shrink(1.).overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(rgb(crate::theme::tone(0xC5B8E6))).tooltip({let summary=summary.clone();move|w,cx|gpui_kit::component::tooltip::Tooltip::new(format!("{summary}. Your role may grant exemptions; Twitch validates sending. Ctrl+I for channel details.")).build(w,cx)}).child(summary)))
                    .child(div().h_flex().gap_2().when_some(self.send_to.clone(), |el, current| {
                        // Combined feed: one explicit destination, never all of them.
                        let sources = self.all_sources();
                        let owner = cx.entity().downgrade();
                        el.child(Button::new("send-to").ghost().xsmall().icon(crate::assets::platform_icon(combined::Platform::of(&current))).label(format!("to {}", combined::label(&current))).tooltip("Where your messages go: one channel or stream, never all of them")
                            .dropdown_menu(move |mut menu, _, _| {
                                for source in &sources {
                                    let (owner, source) = (owner.clone(), source.clone());
                                    menu = menu.item(gpui_kit::component::menu::PopupMenuItem::new(combined::label(&source))
                                        .checked(source == current)
                                        .on_click(move |_, _, cx| { let _ = owner.update(cx, |p, cx| { p.send_to = Some(source.clone()); cx.notify(); }); }));
                                }
                                menu
                            }))
                    }).when(self.send_as_options.len() > 1, |el| {
                        let options = self.send_as_options.clone();
                        let current = self.send_as.clone().filter(|v| options.iter().any(|(o, _)| o == v)).unwrap_or_else(|| options[0].0.clone());
                        let login = options.iter().find(|(v, _)| *v == current).map(|(_, l)| l.clone()).unwrap_or_default();
                        let owner = cx.entity().downgrade();
                        el.child(Button::new("send-as").ghost().xsmall().label(format!("as {login}")).tooltip("Send as another signed-in account")
                            .dropdown_menu(move |mut menu, _, _| {
                                for (index, (vault, login)) in options.iter().enumerate() {
                                    let (owner, vault) = (owner.clone(), vault.clone());
                                    menu = menu.item(gpui_kit::component::menu::PopupMenuItem::new(if index == 0 { format!("{login} (active)") } else { login.clone() })
                                        .checked(vault == current)
                                        .on_click(move |_, _, cx| { let _ = owner.update(cx, |_, cx| cx.emit(PaneEvent::SendAs(vault.clone()))); }));
                                }
                                menu
                            }))
                    }).child(Button::new("sent-history").ghost().small().icon(Icon::new(gpui_kit::assets::IconName::RotateCcwClock)).tooltip("Sent message history · Ctrl+Shift+H").on_click(cx.listener(|this,_,w,cx|this.open_input_history(w,cx)))).child(div().id("emote-button-area").on_prepaint(move|bounds,_,cx|{if emote_button_bounds.replace(Some(bounds))!=Some(bounds)&&emote_picker_open{let owner=emote_button_owner.clone();cx.defer(move|cx|{let _=owner.update(cx,|_,cx|cx.notify());});}}).on_hover(cx.listener(|this,over,_,cx|{if *over{this.prepare_picker_media(cx);}})).child(Button::new("emotes-toggle").ghost().small().icon(Icon::new(gpui_kit::assets::IconName::Sticker)).tooltip("Emotes · type :name or press Tab to complete").on_click(cx.listener(|this,_,w,cx|this.toggle_picker(w,cx)))))
                    .when(crate::replies::option(0), |el| el.child(Button::new("send").small().label(if self.pending.is_some() { "Sending…" } else if self.twitch_command_in_flight { "Working…" } else if commands::is_command(&self.draft.read(cx).value()) { "Run" } else { "Send" }).disabled(self.pending.is_some() || self.twitch_command_in_flight || (!self.connected && !commands::is_command(&self.draft.read(cx).value()))).tooltip("Send a message or run a local /command").on_click(cx.listener(|this,_,window,cx|this.submit(window,cx))))))))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let copy_owner = copy_owner.clone();
                        let copy_viewport = copy_viewport.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            // Right-click on a selection copies it; Shift+right-click opens the menu (Search the web).
                            if phase.capture() && event.button == MouseButton::Right && !event.modifiers.shift && !window.has_active_dialog(cx)
                                && !emote_picker::covers(event.position,window,cx)
                                && copy_viewport.borrow().is_some_and(|bounds| bounds.contains(&event.position))
                            {
                                let copied=copy_owner.update(cx, |pane, cx| {
                                    if pane.selected_text().is_empty(){return false;}
                                    pane.focus.focus(window,cx);pane.copy(window,cx);true
                                }).unwrap_or(false);
                                if copied{window.prevent_default();cx.stop_propagation();}
                            }
                        });
                        let finish = finish.clone();
                        let finish_link=finish_link.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase.capture() && event.button == MouseButton::Left {
                                finish.borrow_mut().finish();
                                let link=finish_link.clone();window.defer(cx,move|_,_|{link.borrow_mut().take();});
                                window.refresh();
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

fn caption_control(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    close: bool,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(32.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(theme::muted()))
        .hover(move |style| {
            style
                .bg(rgb(if close { 0xC42B1C } else { theme::hover() }))
                .text_color(rgb(theme::text()))
        })
        .window_control_area(area)
        .child(Icon::new(icon).small())
}
const MIN_WINDOW_WIDTH: f32 = 360.;
const MIN_WINDOW_HEIGHT: f32 = 280.;
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = profile::init(&mut args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    if args.first().is_some_and(|arg| arg == "--control-call") {
        let result = if args.len() == 3 {
            control::client(&args[1], &args[2])
        } else {
            Err("Usage: --control-call SESSION JSON".into())
        };
        match result {
            // A windowed (GUI-subsystem) build has no console: callers must
            // redirect output to read it, and a closed pipe must not panic.
            Ok(value) => {
                use std::io::Write;
                let _ = writeln!(std::io::stdout(), "{value}");
            }
            Err(error) => {
                use std::io::Write;
                let _ = writeln!(std::io::stderr(), "{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    // One Jawjack window per profile: a second launch brings the first forward.
    // (Control sessions are test-owned and may run beside it.)
    let title = match profile::label() {
        Some(label) => format!("Jawjack {label}"),
        None => "Jawjack".to_owned(),
    };
    if args.is_empty()
        && !jawjack_taskbar::claim_single_window(profile::name().unwrap_or("default"), &title)
    {
        return;
    }
    let server = if args.is_empty() {
        None
    } else if args.len() == 2 && args[0] == "--control-session" {
        match control::start(&args[1]) {
            Ok(server) => Some(server),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Usage: chat-workbench [--control-session NEW-NAME]");
        std::process::exit(1);
    };

    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_app_identity("digital.zombie.jawjack", "Jawjack");
            workspace::bind_keys(cx);
            theme::install(cx);
            cx.bind_keys([
                KeyBinding::new("alt-pageup", PreviousHighlight, Some("JawjackChannel")),
                KeyBinding::new("alt-pagedown", NextHighlight, Some("JawjackChannel")),
                KeyBinding::new("ctrl-home", CatchUpChat, Some("ChatTranscript")),
                KeyBinding::new("ctrl-shift-h", SentHistory, Some("JawjackChannel")),
                KeyBinding::new("ctrl-space", ToggleChatPause, Some("JawjackChannel")),
                KeyBinding::new("ctrl-end", ResumeChat, Some("ChatTranscript")),
                KeyBinding::new("ctrl-f", FindChat, Some("JawjackChannel")),
                KeyBinding::new("cmd-f", FindChat, Some("JawjackChannel")),
                KeyBinding::new("f3", NextChatMatch, Some("JawjackChannel")),
                KeyBinding::new("shift-f3", PreviousChatMatch, Some("JawjackChannel")),
                KeyBinding::new("shift-enter", PreviousChatMatch, Some("JawjackSearch")),
                KeyBinding::new("alt-up", PreviousInput, Some("JawjackComposer")),
                KeyBinding::new("alt-down", NextInput, Some("JawjackComposer")),
                KeyBinding::new("tab", CompleteEmote, Some("JawjackComposer")),
                KeyBinding::new("ctrl-c", CopyChatSelection, Some("ChatTranscript")),
                KeyBinding::new("cmd-c", CopyChatSelection, Some("ChatTranscript")),
                KeyBinding::new("ctrl-insert", CopyChatSelection, Some("ChatTranscript")),
                KeyBinding::new("escape", ClearChatSelection, Some("ChatTranscript")),
                KeyBinding::new("ctrl-a", SelectAllChat, Some("ChatTranscript")),
                KeyBinding::new("cmd-a", SelectAllChat, Some("ChatTranscript")),
            ]);
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.clone().into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(820.)),
                    cx,
                ))),
                window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT))),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let entity = cx.new(|cx| Workbench::new(server.is_some(), window, cx));
                entity.update(cx, |view, cx| view.focus_workspace(window, cx));
                let closing = entity.downgrade();
                window.on_window_should_close(cx, move |window, app| {
                    closing
                        .update(app, |view, cx| view.request_close(window, cx))
                        .unwrap_or(true)
                });
                if let Some(server) = server {
                    control::attach(server, &entity, window, cx);
                }
                entity
            })
            .expect("Could not open the Jawjack window");
        });
}

/// Allow / Deny under an inline AutoMod line while it's held; afterwards, what
/// happened to it.
fn automod_row(
    held_id: &str,
    held: &[automod::Held],
    outcomes: &std::collections::HashMap<String, String>,
    owner: WeakEntity<ChannelPane>,
) -> AnyElement {
    let row = div().h_flex().items_center().gap_1().pb_1();
    if let Some(entry) = held.iter().find(|h| h.id == held_id) {
        let (allow, deny) = (held_id.to_owned(), held_id.to_owned());
        let (allow_owner, deny_owner) = (owner.clone(), owner);
        return row
            .child(
                Button::new(SharedString::from(format!("inline-allow-{held_id}")))
                    .xsmall()
                    .primary()
                    .label("Allow")
                    .disabled(entry.pending)
                    .on_click(move |_, _, cx| {
                        let _ = allow_owner
                            .update(cx, |_, cx| cx.emit(PaneEvent::AutoMod(allow.clone(), true)));
                    }),
            )
            .child(
                Button::new(SharedString::from(format!("inline-deny-{held_id}")))
                    .xsmall()
                    .ghost()
                    .label("Deny")
                    .disabled(entry.pending)
                    .on_click(move |_, _, cx| {
                        let _ = deny_owner
                            .update(cx, |_, cx| cx.emit(PaneEvent::AutoMod(deny.clone(), false)));
                    }),
            )
            .into_any_element();
    }
    let outcome = outcomes
        .get(held_id)
        .cloned()
        .unwrap_or_else(|| "No longer held".into());
    row.child(
        div()
            .text_size(px(10.))
            .text_color(rgb(theme::muted()))
            .child(outcome),
    )
    .into_any_element()
}
