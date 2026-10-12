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
mod message_actions;
mod message_scroller;
mod mod_buttons;
mod name_paint;
mod obs;
mod owned_emotes;
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
use chat_text::ChatText;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{
    Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    input::{InputEvent, Textarea, TextareaState},
    menu::ContextMenuExt,
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
        NextHighlight
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
            .bg(rgb(theme::PANEL))
            .text_color(rgb(theme::TEXT))
            .border_1()
            .border_color(rgb(0xA99CF4))
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
    /// User card: follow dates by user ID (Ok(None) = not following, Err = unavailable).
    pub(crate) follow_ages: std::collections::HashMap<String, Result<Option<String>, String>>,
    /// User card in your own channel: subscription by user ID (None = not subscribed).
    pub(crate) sub_status: std::collections::HashMap<String, Option<String>>,
    /// The last message sent here and when, so an identical one can still go through.
    last_sent: Option<(String, Instant)>,
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
    compose_revision: u64,
    input_history: input_history::History,
    reply_target: Option<replies::Target>,
    attention: attention::Attention,
    attention_cursor: Option<u64>,
    jump_pulse: Option<(u64, Instant)>,
    read_eligible: Rc<Cell<bool>>,
    presented_row: Rc<Cell<Option<u64>>>,
    send_status: String,
    timeline: Rc<RefCell<Timeline>>,
    media: Rc<RefCell<media::MediaCache>>,
    catalog: Rc<RefCell<catalog::Catalog>>,
    custom_commands: custom_commands::Definitions,
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
}
impl ChannelPane {
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
            follow_ages: Default::default(),
            sub_status: Default::default(),
            last_sent: None,
            linked: Vec::new(),
            display_title: None,
            suspicious_pending: Default::default(),
            send_as: None,
            media,
            catalog,
            custom_commands: Default::default(),
            chatter_appearance: Default::default(),
            ignores: Default::default(),
            connection: "Sign in to connect".into(),
            connected: false,
            room_settings: None,
            pending: None,
            reply_target: None,
            attention: Default::default(),
            attention_cursor: None,
            jump_pulse: None,
            read_eligible: Rc::new(Cell::new(false)),
            presented_row: Rc::new(Cell::new(None)),
            compose_revision: 0,
            input_history: Default::default(),
            send_status: String::new(),
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
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        let text = twitch_message_text(&self.draft.read(cx).value());
        if self.route_command(&text, window, cx) {
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
        let text =
            duplicates::duplicate_safe(&mut self.last_sent, text, youtube::is_youtube(&self.name));
        self.pending = Some((
            request,
            text.clone(),
            reply_parent.clone(),
            self.compose_revision,
        ));
        self.send_status = "Sending…".into();
        cx.emit(PaneEvent::Send {
            request,
            text,
            reply_parent,
        });
        cx.notify();
    }
    fn received(&mut self, mut event: chat_core::Event, cx: &mut Context<Self>) {
        let historical =
            matches!(&event, chat_core::Event::Message(m) if m.presentation.historical);
        if let chat_core::Event::Message(message) = &mut event {
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
                && similarity::is_repeat(message, self.timeline.borrow().messages().iter())
            {
                if self.similar_mode == similarity::Mode::Hide {
                    return;
                }
                message.presentation.similar = true;
            }
            chatter_appearance::apply(&self.chatter_appearance, message);
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
                if let Some((ping, id)) = ping.filter(|_| !self.quiet_pings) {
                    cx.emit(PaneEvent::Ping(ping, id));
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
        if clear_reply {
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
        let view_filter = self.view_filter.as_ref().map(|(_, q)| q.clone());
        let hidden_platforms = self.hidden_platforms.clone();
        // Name the Twitch channel on each message only when several are mixed.
        let origin_names = self
            .all_sources()
            .iter()
            .filter(|s| !youtube::is_youtube(s))
            .count()
            > 1;
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
                    (at(*row), crate::highlight_rules::COLORS[color.min(5)].1)
                })
                .collect();
            if self.search.open {
                markers.extend(
                    self.search
                        .hits
                        .keys()
                        .filter(|row| **row >= first)
                        .map(|row| (at(*row), theme::ACCENT)),
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
            .on_action(cx.listener(|this,_:&NextHighlight,w,cx|{this.navigate_attention(1,w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&SentHistory,w,cx|{this.open_input_history(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&ToggleChatPause,_,cx|{this.scroller.update(cx,|s,cx|s.toggle_pause(cx));cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&ResumeChat,_,cx|{this.scroller.update(cx,|s,cx|s.scroll_to_end(cx));cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&FindChat,w,cx|{this.open_search(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&NextChatMatch,_,cx|{this.next_search(true,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&PreviousChatMatch,_,cx|{this.next_search(false,cx);cx.stop_propagation();}))
            .v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(rgb(theme::CANVAS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .rounded(px(10.))
            .overflow_hidden()
            .child(
                div()
                    .h_flex()
                    .h(px(26.))
                    .px_2()
                    .gap_2()
                    .bg(rgb(theme::PANEL))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
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
                                })),
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
                            .text_color(rgb(theme::MUTED))
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
                    .when(retained == 0, |el| el.child(div().v_flex().p_6().gap_2().text_color(rgb(theme::MUTED))
                        .child(div().text_color(rgb(theme::TEXT)).text_size(px(16.)).child(if youtube::is_youtube(&self.name) { format!("{} is ready", combined::title(&self.name)) } else { format!("#{} is ready", combined::title(&self.name)) }))
                        .child(if self.connected { "Connected. Waiting for messages…".to_string() } else { self.connection.clone() })
                        .when(youtube::is_youtube(&self.name) && self.connection.contains("Connect YouTube"), |el| el.child(div().pt_2().h_flex().child(
                            Button::new("youtube-setup").small().primary().label("Connect YouTube").on_click(cx.listener(|_, _, _, cx| cx.emit(PaneEvent::SetUpYouTube))))))))
                    .child(
                        MessageScroller::new("chat", self.scroller.clone(), move |index, window, cx| {
                            let messages = timeline.borrow();
                            let Some(message) = messages.messages().get(index) else {
                                return div().into_any_element();
                            };
                            // A filtered-out row draws nothing (rows have no padding of their own).
                            if view_filter.as_ref().is_some_and(|q| q.matches(message).is_none())
                                || (!hidden_platforms.is_empty() && hidden_platforms.contains(&message.presentation.origin.as_deref().map(combined::Platform::of).unwrap_or(combined::Platform::Twitch))) {
                                return div().into_any_element();
                            }
                            let row = first_order + index as u64;
                            let paint=if appearance.borrow().get(&message.user_id).is_some_and(|a|a.color.is_some()){None}else{
                                let catalog=catalog.borrow();catalog.paints.for_user(&message.user_id).filter(|p|p.limitation.is_none()).cloned().map(|paint|{
                                    // Image paints load (and animate) through the media cache.
                                    let image=paint.image().and_then(media::EmoteKey::paint_image).and_then(|key|media.borrow_mut().get(&key,cx));
                                    crate::name_paint::Request{paint,cache:catalog.paint_cache.clone(),image}
                                })
                            };
                            let inline = inline_chat::InlineChat::new(message, paint.clone(), row, &mut media.borrow_mut(), selection.clone(), focus.clone(), viewport.clone(), cx);
                            let first_line_height = if inline.is_some() { inline_chat::InlineChat::line_height(window,message) } else { window.line_height() };
                            let mut badge_images=message.badges.iter().filter_map(|badge|{
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
                                for definition in catalog.ffz_badges.for_user(&message.user_id,room){let image=media.borrow_mut().get(&definition.badge.key,cx);badge_images.push((definition.badge,image,definition.background));}
                            }
                            if let Some(badge)=catalog.borrow().badges.for_user(&message.user_id).cloned(){let image=media.borrow_mut().get(&badge.key,cx);badge_images.push((badge,image,None));}
                            // A definitively failed badge drops its slot instead of leaving a blank gap;
                            // pending art keeps its slot so loading does not reflow the row.
                            badge_images.retain(|(badge,image,_)|image.is_some()||!media.borrow().failed(&badge.key));
                            let row_owner=menu_owner.clone();let row_id=message.id.clone();
                            let timestamp=if timestamps>0{message.sent_at.and_then(|time|chrono::DateTime::from_timestamp(time,0)).map(|time|{
                                let local=time.with_timezone(&chrono::Local);(local.format(if timestamps==2{"%H:%M:%S"}else{"%H:%M"}).to_string(),local.format("%Y-%m-%d %H:%M:%S %:z").to_string())
                            })}else{None};
                            let pulse=jump_pulse.filter(|(target,at)|*target==row&&at.elapsed()<Duration::from_millis(1200));
                            if pulse.is_some()&&!cx.reduce_motion(){window.request_animation_frame();}
                            let rich_heading=rich_messages::heading(message);
                            let rich_attachments=rich_messages::attachments(message,&mut media.borrow_mut(),cx);
                            let reply_line=replies::row_context(message,&messages,menu_owner.clone());
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
                            div()
                                .relative()
                                // Every row keeps the same inset so text never touches a highlight edge
                                // and highlighting a row doesn't shift its text sideways.
                                .border_l_2().border_color(rgba(0x00000000)).pl(px(6.)).pr(px(4.))
                                .when(message.deleted, |el| el.opacity(0.45))
                                // Settings → Appearance → Alternate row backgrounds (Chatterino's option).
                                .when(theme::alternate_rows() && row % 2 == 1, |el| el.bg(rgba(0xFFFFFF07)))
                                .when_some(rich_messages::accent(message),|el,color|el.border_color(rgb(color)).bg(rgba(0xA99CF40D)).py(px(2.)).rounded_r(px(4.)))
                                .when(highlight_rows.contains(&row),|el|{let (_,accent,background)=highlight_rules::COLORS[highlight_colors.get(&row).copied().unwrap_or(0)];el.bg(rgb(background)).border_color(rgb(accent)).py(px(2.)).rounded_r(px(4.))})
                                .on_prepaint(move|bounds,window,cx|{
                                    let clip=window.content_mask().bounds;
                                    if following&&eligible.get()&&window.is_window_active()&&!window.has_active_dialog(cx)
                                        && bounds.origin.x>=clip.origin.x&&bounds.bottom()>clip.origin.y
                                        && bounds.right()<=clip.right()&&bounds.bottom()<=clip.bottom(){
                                        presented.set(Some(presented.get().map_or(row,|old|old.max(row))));
                                    }
                                })
                                .when(search_current==Some(row),|el|el.bg(rgb(0x272331)))
                                .when_some(pulse,|el,(_,at)|{let alpha=if cx.reduce_motion(){255}else{(255.*(1.-at.elapsed().as_secs_f32()/1.2).clamp(0.,1.))as u32};el.border_color(rgba((0x9DD8CB<<8)|alpha))})
                                .left(px(12.0 * (1.0 - eased)))
                                .top(px(6.0 * (1.0 - eased)))
                                // Recent history is dimmed, as in Chatterino.
                                .opacity(if message.presentation.similar { 0.4 } else if message.presentation.historical { 0.6 } else { 0.25 + 0.75 * eased })
                                .id(SharedString::from(message.id.clone()))
                                .min_w_0()
                                .cursor_text()
                                .context_menu(move|menu,_,cx|message_actions::menu(row_owner.clone(),row_id.clone(),menu,cx))
                                .when(read_boundary==Some(row)&&index>0,|el|el.child(div().h_flex().items_center().gap_2().h(px(18.)).text_size(px(10.)).text_color(rgb(0x9DBFCE)).child(div().h(px(1.)).flex_1().bg(rgb(0x40535E))).child("Last read position").child(div().h(px(1.)).flex_1().bg(rgb(0x40535E)))))
                                .children(rich_heading)
                                .children(reply_line)
                                .child(div().h_flex().items_start().min_w_0().gap_1()
                                .when_some(timestamp,|el,(label,full)|el.child(div().id(SharedString::from(format!("timestamp-{}",message.id))).w(px(if timestamps==2{55.}else{37.})).h(first_line_height).flex_shrink_0().flex().items_center().text_size(px(10.)).text_color(rgb(theme::MUTED)).tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w,cx)).child(label)))
                                .when(mod_buttons && !message.deleted && message.login.is_some() && own_user.as_deref() != Some(message.user_id.as_str()), |el| {
                                    let tool = |id: &'static str, icon: gpui_kit::assets::IconName, tip: &'static str, hover: u32, action: message_actions::Action| {
                                        let (owner, message_id) = (menu_owner.clone(), message.id.clone());
                                        div().id(SharedString::from(format!("{id}-{}", message.id))).size(px(16.)).h(first_line_height).flex_shrink_0().flex().items_center().justify_center()
                                            .text_color(rgb(theme::MUTED)).cursor_pointer().hover(move |s| s.text_color(rgb(hover)))
                                            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                            .on_click(move |_, window, cx| {
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
                                                .rounded(px(3.)).text_size(px(10.)).text_color(rgb(theme::MUTED)).cursor_pointer()
                                                .hover(|s| s.text_color(rgb(theme::TEXT)).bg(rgb(theme::HOVER)))
                                                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                                .on_click(move |_, window, cx| {
                                                    cx.stop_propagation();
                                                    let action = message_actions::Action::ModCustom(command.clone());
                                                    let _ = owner.update(cx, |p, cx| p.message_action(&message_id, action, window, cx));
                                                })
                                                .child(button.label.clone())
                                        })))
                                })
                                // Combined feed: which channel this came from, colour-keyed per channel.
                                .when_some(message.presentation.origin.clone(), |el, origin| {
                                    let platform = combined::Platform::of(&origin);
                                    let icon = div().id(SharedString::from(format!("src-{origin}"))).flex().items_center()
                                        .tooltip({ let label = if platform == combined::Platform::YouTube { "From YouTube".to_owned() } else { format!("From Twitch · #{origin}") }; move |w, cx| gpui_kit::component::tooltip::Tooltip::new(label.clone()).build(w, cx) })
                                        .child(crate::assets::platform_icon(platform).xsmall());
                                    el.child(div().h(first_line_height).flex().items_center().gap_1().flex_shrink_0().child(icon)
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
                                    el.child(div().id(SharedString::from(format!("shared-{source_id}"))).w(px(18.)).h(first_line_height).flex().items_center().flex_shrink_0()
                                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(format!("Shared Chat · sent in {name}'s channel")).build(w, cx))
                                        .child(match image {
                                            Some(image) => div().size(px(18.)).rounded_full().overflow_hidden().child(emote_picker::icon(Some(image), String::new(), 18., 18.)).into_any_element(),
                                            None => div().size(px(18.)).rounded_full().bg(rgb(0x3A3350)).flex().items_center().justify_center().text_size(px(10.)).text_color(rgb(theme::TEXT))
                                                .child(message.presentation.source.as_deref().and_then(|n| n.chars().next()).unwrap_or('?').to_uppercase().to_string()).into_any_element(),
                                        }))
                                })
                                .children(badge_images.into_iter().map(|(badge,image,background)|{
                                    div().id(SharedString::from(badge.key.id.clone())).w(px(18.)).h(first_line_height).flex().items_center().flex_shrink_0().overflow_hidden().tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(badge.title.clone()).build(w,cx)).child(div().size(px(18.)).when_some(background,|el,color|el.bg(rgb(color))).child(emote_picker::icon(image,String::new(),18.,18.)))
                                }).collect::<Vec<_>>())
                                .when(message.deleted, |el| el.child(div().h(first_line_height).flex().items_center().flex_shrink_0()
                                    .child(div().px(px(4.)).rounded(px(3.)).bg(rgb(0x3A2328)).text_color(rgb(0xF29D9D)).text_size(px(10.)).child("deleted"))))
                                .child(div().flex_1().min_w_0()
                                    // `/me`: the whole message in the chatter's colour, like Chatterino.
                                    .when(message.presentation.kind == "action", |el| el.text_color(rgb(message.author_color().unwrap_or(theme::TEXT))).italic())
                                    .child(if message.presentation.kind == chat_core::MODERATION {
                                    let repeats = message.presentation.repeats;
                                    let text = message.presentation.notice.clone().unwrap_or_default();
                                    div().text_color(rgb(theme::MUTED)).italic()
                                        .child(if repeats > 0 { format!("{text} ({} times)", repeats + 1) } else { text })
                                        .into_any_element()
                                } else if let Some(inline) = inline {
                                    inline.with_search(matches).with_interaction(interaction).into_any_element()
                                } else {
                                    ChatText::new(SharedString::from(format!("text-{}", message.id)), row, message.copy_line(), selection.clone(), focus.clone(), viewport.clone())
                                        .with_author(message.author_label(), message.author_color()).with_paint(paint).with_search(matches).with_interaction(interaction).into_any_element()
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
                    ),
            )
            .child(div().key_context("JawjackComposer").relative().v_flex().p_1().gap_1().bg(rgb(theme::PANEL)).border_t_1().border_color(rgb(theme::BORDER))
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if this.picker.open {
                        if event.keystroke.modifiers.alt && matches!(event.keystroke.key.as_str(),"left"|"right"){this.picker_action(&event.keystroke.key,window,cx);}
                        return;
                    }
                    if event.keystroke.key == "enter" && event.keystroke.modifiers.shift {
                        window.prevent_default();
                        cx.stop_propagation();
                    } else if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform
                        && let Some(text) = &event.keystroke.key_char {
                            let input = this.draft.read(cx);
                            let current = input.value().to_string();
                            let selected = input.selected_range();
                            let count = current[..selected.start].chars().count() + text.chars().count() + current[selected.end..].chars().count();
                            if count > 500 { window.prevent_default(); cx.stop_propagation(); }
                        }
                }))
                .capture_action(cx.listener(|this, action: &gpui_kit::base::input::Enter, window, cx| {
                    // Enter has exactly one semantic owner. Never also submit from
                    // InputEvent::PressEnter after inserting a completion.
                    if !action.shift {
                        if this.picker.open {
                            this.picker_action("enter",window,cx);
                        } else if !this.completion_action("enter",window,cx) {this.submit(window,cx);}
                    }
                    cx.stop_propagation();
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
                .children(self.render_automod(cx))
                .children(self.render_spelling(cx))
                .children(self.render_reply_target(cx))
                .children(picker)
                .children(suggestions)
                .child(Textarea::new(&self.draft))
                // Routine progress sits inline so side-by-side composers keep equal height;
                // failures keep a full-width row so their explanation is never truncated.
                .when(!routine_status && !self.send_status.is_empty(), |el|el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(self.send_status.clone())))
                .child(div().h_flex().justify_between().gap_2().text_size(px(11.)).text_color(rgb(theme::MUTED))
                    .child(div().h_flex().gap_2().flex_shrink_0()
                        .child(div().when(self.draft.read(cx).value().chars().count() > 500, |el|el.text_color(rgb(0xF29D9D))).child(format!("{} / 500", self.draft.read(cx).value().chars().count())))
                        .when(routine_status && !self.send_status.is_empty(), |el|el.child(self.send_status.clone())))
                    .when_some(self.room_settings.as_ref().filter(|r|!r.labels().is_empty()).map(|r|r.summary()),|el,summary|el.child(div().id("room-modes-hint").max_w(px(120.)).min_w_0().flex_shrink(1.).overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(rgb(0xC5B8E6)).tooltip({let summary=summary.clone();move|w,cx|gpui_kit::component::tooltip::Tooltip::new(format!("{summary}. Your role may grant exemptions; Twitch validates sending. Ctrl+I for channel details.")).build(w,cx)}).child(summary)))
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
                    .child(Button::new("send").small().label(if self.pending.is_some() { "Sending…" } else if commands::is_command(&self.draft.read(cx).value()) { "Run" } else { "Send" }).disabled(self.pending.is_some() || (!self.connected && !commands::is_command(&self.draft.read(cx).value()))).tooltip("Send a message or run a local /command").on_click(cx.listener(|this,_,window,cx|this.submit(window,cx)))))))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let copy_owner = copy_owner.clone();
                        let copy_viewport = copy_viewport.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase.capture() && event.button == MouseButton::Right && !window.has_active_dialog(cx)
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
        .text_color(rgb(theme::MUTED))
        .hover(move |style| {
            style
                .bg(rgb(if close { 0xC42B1C } else { theme::HOVER }))
                .text_color(rgb(theme::TEXT))
        })
        .window_control_area(area)
        .child(Icon::new(icon).small())
}
const MIN_WINDOW_WIDTH: f32 = 360.;
const MIN_WINDOW_HEIGHT: f32 = 280.;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--control-call") {
        let result = if args.len() == 3 {
            control::client(&args[1], &args[2])
        } else {
            Err("Usage: --control-call SESSION JSON".into())
        };
        match result {
            Ok(value) => println!("{value}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
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
                    title: Some("Jawjack".into()),
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
