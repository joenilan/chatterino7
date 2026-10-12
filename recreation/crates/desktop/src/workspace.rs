#[path = "accounts_ui.rs"]
mod accounts_ui;
#[path = "activity.rs"]
mod activity;
#[path = "automod_actions.rs"]
mod automod_actions;
#[path = "channel_details.rs"]
mod channel_details;
#[path = "channel_switcher.rs"]
mod channel_switcher;
#[path = "engagement_ui.rs"]
mod engagement_ui;
#[path = "guides.rs"]
mod guides;
#[path = "live_stats.rs"]
mod live_stats;
#[path = "mod_onboarding.rs"]
mod mod_onboarding;
#[path = "obs_ui.rs"]
mod obs_ui;
#[path = "pane_navigation.rs"]
mod pane_navigation;
#[path = "popout.rs"]
mod popout;
#[path = "recap.rs"]
mod recap;
#[path = "rewards_ui.rs"]
mod rewards_ui;
#[path = "roles.rs"]
mod roles;
#[path = "settings_ui.rs"]
mod settings_ui;
#[path = "sound_settings.rs"]
mod sound_settings;
#[path = "streamer_bot_ui.rs"]
mod streamer_bot_ui;
#[path = "whispers_ui.rs"]
mod whispers_ui;
#[path = "workspace_search.rs"]
mod workspace_search;
use crate::dock::Dock;
use crate::{ChannelPane, DragPreview, DraggedChannel, PaneEvent, caption_control, storage, theme};
use gpui_kit::base::ElementExt;
use gpui_kit::component::Selectable;
use gpui_kit::component::{
    Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
    notification::Notification,
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};

gpui_kit::actions!(
    workspace,
    [
        FocusNextPane,
        FocusPreviousPane,
        FocusPaneLeft,
        FocusPaneRight,
        FocusPaneUp,
        FocusPaneDown,
        EqualizeSplit,
        EqualizeAllSplits,
        NewTab,
        CloseTab,
        ReopenTab,
        NextTab,
        PreviousTab,
        NextChannel,
        PreviousChannel,
        AddChannel,
        RenameWorkspace,
        MoveTabLeft,
        MoveTabRight,
        ToggleSidebar,
        ToggleAppearance,
        ToggleOrientation,
        ToggleLiveWorkspaces,
        ToggleLiveChannels,
        QuitWithoutSaving,
        OpenActivity,
        MarkAllRead,
        FontLarger,
        FontSmaller,
        ResetFont,
        ChannelDetails,
        SwitchChannel,
        SearchOpenChannels,
        EditCustomCommands,
        EditIgnores,
        EditEventHighlights,
        OpenWhispers,
        ChooseSetup,
        ToggleStudio,
        ImportChatterino,
        EditChatterAppearance,
        ToggleStreamerMode
    ]
);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f6", FocusNextPane, Some("ChatWorkspace")),
        KeyBinding::new("shift-f6", FocusPreviousPane, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-alt-left", FocusPaneLeft, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-alt-right", FocusPaneRight, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-alt-up", FocusPaneUp, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-alt-down", FocusPaneDown, Some("ChatWorkspace")),
        // Input owns vertical multicursor shortcuts; pane movement takes precedence here.
        KeyBinding::new("ctrl-alt-up", FocusPaneUp, Some("ChatWorkspace > Input")),
        KeyBinding::new(
            "ctrl-alt-down",
            FocusPaneDown,
            Some("ChatWorkspace > Input"),
        ),
        KeyBinding::new("ctrl-shift-f", SearchOpenChannels, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-p", SwitchChannel, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-s", ToggleStudio, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-i", ChannelDetails, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-m", OpenActivity, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-r", MarkAllRead, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-=", FontLarger, Some("ChatWorkspace")),
        KeyBinding::new("ctrl--", FontSmaller, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-0", ResetFont, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-t", NewTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-w", CloseTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-t", ReopenTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-tab", NextTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-pagedown", NextChannel, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-pageup", PreviousChannel, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-f", crate::FindChat, Some("ChatWorkspace")),
        KeyBinding::new("cmd-f", crate::FindChat, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-k", AddChannel, Some("ChatWorkspace")),
    ]);
}
#[derive(Clone)]
struct DraggedTab {
    id: u64,
    name: String,
}
#[derive(Clone, Copy, PartialEq)]
enum DropEdge {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}
impl DropEdge {
    fn vertical(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
    fn first(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
    fn label(self) -> &'static str {
        match self {
            Self::Center => "Add as channel tab",
            Self::Left => "Place left",
            Self::Right => "Place right",
            Self::Top => "Place above",
            Self::Bottom => "Place below",
        }
    }
}
struct WorkspaceTab {
    id: u64,
    name: String,
    panes: Vec<Entity<ChannelPane>>,
    vertical: bool,
    sizes: Vec<f32>,
    split: Entity<ResizableState>,
    dock: Option<Dock>,
    dock_states: BTreeMap<String, Entity<ResizableState>>,
    /// Channels pinned to the front of their group; protected from closing.
    pinned: Vec<String>,
    /// Lock layout: tabs can't be dragged or closed until unlocked.
    locked: bool,
}
pub struct Workbench {
    account: Entity<crate::auth::TwitchAccount>,
    /// Every signed-in or signing-in account; `account` is the active one.
    accounts: Vec<Entity<crate::auth::TwitchAccount>>,
    account_focus: Option<usize>,
    /// "Send as" choice per channel: the chosen account's vault entry.
    send_as: std::collections::HashMap<String, SharedString>,
    /// Held AutoMod messages per channel (session only).
    automod: std::collections::HashMap<String, std::rc::Rc<Vec<crate::automod::Held>>>,
    /// Own-channel extra subscriptions Twitch refused (None until the own channel connects).
    own_extras_refused: Option<Vec<&'static str>>,
    /// Channels where you chose not to see AutoMod (not subscribed either).
    automod_hidden: std::collections::BTreeSet<String>,
    /// Moderator onboarding: channels ticked, and which are live (None while checking).
    mod_pick: std::collections::BTreeSet<String>,
    mod_live: Option<std::collections::BTreeSet<String>>,
    /// Moderated channels with a live AutoMod queue subscription.
    mod_automod_channels: Vec<String>,
    /// Ping sounds (Chatterino defaults: mentions on, other highlights off,
    /// silent while Jawjack is the active window).
    sound_mentions: bool,
    sound_highlights: bool,
    sound_focused: bool,
    /// Flash the taskbar button on mentions/highlights while Jawjack is in the background.
    flash_taskbar: bool,
    sound_tone: crate::sound::Tone,
    sound_volume: f32,
    /// Message ids already pinged (the same channel can be open in several panes).
    pinged: std::collections::VecDeque<String>,
    /// Channels in their own windows; reopened on the next launch.
    popouts: Vec<popout::Popout>,
    /// Twitch block list; blocked users' messages are hidden unless turned off.
    blocks: crate::blocks::Blocks,
    hide_blocked: bool,
    similar_mode: crate::similarity::Mode,
    spellcheck: bool,
    /// Saved view filters by "{tab id}:{channel}".
    view_filters: std::collections::HashMap<String, String>,
    /// Combined feeds: hidden platforms per "tab:pane".
    platform_filters: std::collections::HashMap<String, Vec<String>>,
    /// Twitch Shared Chat sessions of open channels ("+N" on tabs).
    shared_chat: crate::shared_chat::SharedChat,
    /// Your own channel's live YouTube broadcast ("yt:<id>"), linked into your own tab.
    own_youtube: Option<String>,
    own_youtube_checked: Option<std::time::Instant>,
    own_youtube_busy: bool,
    mod_buttons: bool,
    custom_mod_buttons: std::rc::Rc<Vec<crate::mod_buttons::ModButton>>,
    mod_button_inputs: Option<(Entity<InputState>, Entity<InputState>)>,
    follows_live: crate::follows_live::FollowsLive,
    follows_open: bool,
    /// LIVE NOW: sort order, and the filter box (shown while `live_filter_open`).
    live_sort: crate::follows_live::Sort,
    live_filter: Entity<InputState>,
    live_filter_open: bool,
    /// Studio raid finder sort.
    pub(crate) raid_sort: crate::follows_live::Sort,
    stream_presets: crate::studio::Presets,
    obs: crate::obs::Obs,
    obs_settings: crate::streamer_bot::Settings,
    obs_started: bool,
    obs_tick: u64,
    ads: crate::ads::Ads,
    hype: crate::hype::Hype,
    /// The account whose running goals/charity/Shield Mode were loaded.
    hype_loaded_for: Option<String>,
    /// Go-live checklist: your own items, which are ticked this stream, and whether
    /// it was dismissed for this stream.
    checklist_items: Vec<String>,
    checklist_done: std::collections::BTreeSet<String>,
    checklist_dismissed: bool,
    checklist_input: Option<Entity<InputState>>,
    /// When the own channel was last seen going live (for the 10-minute window).
    went_live_at: Option<std::time::Instant>,
    was_live: bool,
    peak_viewers: u64,
    stats_tick: u64,
    /// Latest connection state per channel (for combined feeds).
    channel_ready: std::collections::HashMap<String, bool>,
    youtube: crate::youtube::YouTube,
    youtube_key_loaded: bool,
    /// The open setup guide's result line: (ok, text).
    guide_status: Option<(bool, String)>,
    /// Google sign-in for sending on YouTube.
    google_client: Option<crate::google_auth::OAuthClient>,
    google_tokens: Option<crate::google_auth::Tokens>,
    /// Live chat IDs of open YouTube tabs.
    youtube_chats: std::collections::HashMap<String, String>,
    rewards: crate::rewards::Rewards,
    last_stream_info: Option<(String, String)>,
    /// Latest incoming raid on your channel: (login, display name, viewers, when).
    last_raid: Option<(String, String, u64, std::time::Instant)>,
    shouted: std::collections::BTreeSet<String>,
    /// "Find OBS": None = not searched, Some(None) = searching, Some(Some(ips)) = done.
    obs_found: Option<Option<Vec<crate::obs::Found>>>,
    /// Optional chat logs (off by default).
    logs: crate::chat_logs::Logs,
    logs_enabled: bool,
    logs_folder: std::path::PathBuf,
    streamer_mode: crate::streamer_mode::Mode,
    streamer_detector: crate::streamer_mode::Detector,
    /// Mute ping sounds while streamer mode is active.
    streamer_mute: bool,
    /// Streamer mode is active right now (On, or Automatic with broadcasting software).
    streamer_live: bool,
    restore_popouts: Vec<String>,
    live: crate::live::LiveChat,
    /// Guest (read-only) chat while no account is signed in.
    anon: crate::anon_chat::AnonChat,
    /// Signed out and chose "Read chat as guest" on the sign-in screen (this run only).
    guest_reading: bool,
    /// When the startup splash finished (it fades out from here).
    splash_done: Option<std::time::Instant>,
    /// When this window opened (caps how long the splash waits).
    opened: std::time::Instant,
    /// The splash's give-up wake-up is scheduled (once).
    splash_wake: bool,
    /// Live tabs have been given the "who's live now" list.
    live_seeded: bool,
    /// Settings → Shortcuts: listening for a new shortcut's keys.
    pub(crate) hotkey_capture: Option<Subscription>,
    /// Settings → General: keep the main window above other windows.
    pub(crate) on_top: bool,
    /// Settings → General: Start with Windows (read from the registry when Settings opens).
    pub(crate) autorun: Option<bool>,
    streams: crate::stream_status::Streams,
    capabilities: crate::capabilities::Capabilities,
    experience: roles::Experience,
    streamer_bot: crate::streamer_bot::StreamerBot,
    feed: crate::activity_feed::Feed,
    whispers: crate::whispers::Whispers,
    whisper_input: Option<Entity<InputState>>,
    whispers_drawn: std::time::Instant,
    engagement: crate::engagement::Engagement,
    feed_filter: Option<crate::activity_feed::Kind>,
    /// Studio tab: 0 Activity, 1 Stream, 2 Tools.
    studio_tab: usize,
    studio_tab_switched: std::time::Instant,
    /// A divider double-click easing a split to equal sizes: tab, path, start weights, start.
    equalizing: Option<(u64, Vec<usize>, Vec<f32>, std::time::Instant)>,
    bot_settings: crate::streamer_bot::Settings,
    bot_pinned: Vec<crate::streamer_bot::Action>,
    bot_runs: std::collections::HashMap<u64, String>,
    bot_started: bool,
    studio_open: bool,
    studio_editor: Option<Entity<crate::studio::StreamEditor>>,
    raid_input: Option<Entity<InputState>>,
    setup_offered: bool,
    chatters: crate::chatters::Chatters,
    recent: crate::recent_messages::RecentMessages,
    recent_enabled: bool,
    /// Go-live alerts: channels opted in from the tab menu, or every open channel.
    live_notify: Vec<String>,
    /// Channels whose mentions and highlights don't play a sound.
    muted_pings: Vec<String>,
    live_notify_all: bool,
    /// Alerts for every followed channel going live, open as a tab or not.
    live_notify_follows: bool,
    /// Counter for local system notice IDs.
    next_notice: u64,
    presence: crate::seven_presence::Presence,
    recent_requested: std::collections::HashSet<String>,
    updates: crate::updates::Updates,
    announced_update: Option<String>,
    live_workspaces: bool,
    live_channels: bool,
    catalog: std::rc::Rc<std::cell::RefCell<crate::catalog::Catalog>>,
    media: std::rc::Rc<std::cell::RefCell<crate::media::MediaCache>>,
    drop_edge: Option<DropEdge>,
    drop_target: Option<String>,
    dock_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    strip_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    tab_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    last_drop: Option<Value>,
    release_position: Option<Point<Pixels>>,
    control_pointer_active: bool,
    release_from_control: bool,
    pointer_owner: Option<bool>,
    control_enabled: bool,
    tabs: Vec<WorkspaceTab>,
    closed_tabs: Vec<WorkspaceTab>,
    active: usize,
    next_id: u64,
    focus: FocusHandle,
    channel_input: Entity<InputState>,
    adding: bool,
    renaming: bool,
    /// Tab menu → Change channel: the channel the Add box replaces.
    replacing: Option<String>,
    add_target: Option<String>,
    selected_channel: Option<String>,
    /// Workspace IDs, most recently activated first; orders live connections.
    recent_workspaces: Vec<u64>,
    add_error: Option<String>,
    sidebar: bool,
    settings: bool,
    settings_section: usize,
    settings_opened: std::time::Instant,
    font_size: f32,
    timestamps: u8,
    pause_mode: u8,
    history_limit: usize,
    highlight_input: Entity<InputState>,
    highlight_terms: Vec<String>,
    highlight_rules: crate::highlight_rules::Rules,
    highlight_editor: Option<Entity<crate::highlight_rules::Editor>>,
    chatter_appearance: crate::chatter_appearance::Preferences,
    ignores: crate::ignores::Shared,
    ignores_editor: Option<Entity<crate::ignores::Editor>>,
    event_highlights: crate::event_highlights::Shared,
    /// Nicknames from a Chatterino import awaiting login-to-ID lookup.
    /// A short-lived status message that takes precedence over save status.
    notice: Option<(String, std::time::Instant)>,
    /// Missing-permission count already announced this run.
    announced_missing_scopes: usize,
    pending_nicknames: Option<std::sync::mpsc::Receiver<Vec<crate::chatterino_import::Nickname>>>,
    chatter_editor: Option<Entity<crate::chatter_appearance::Editor>>,
    chatter_notes: crate::chatter_notes::Notes,
    notes_editor: Option<Entity<crate::chatter_notes::Editor>>,
    custom_commands: crate::custom_commands::Definitions,
    activity_all: bool,
    zoom_accumulator: f32,
    drafts: BTreeMap<String, String>,
    reply_drafts: BTreeMap<String, Value>,
    save_revision: u64,
    save_enabled: bool,
    save_status: String,
    close_failed: bool,
    close_tab_pending: Option<u64>,
    close_channel_pending: bool,
}
impl Workbench {
    pub fn control_pointer(&mut self, active: bool) {
        self.control_pointer_active = active;
    }
    pub fn focus_workspace(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }
    pub fn visible_panes(&self, cx: &App) -> Vec<Entity<ChannelPane>> {
        let tab = &self.tabs[self.active];
        let mut names = Vec::new();
        if let Some(dock) = &tab.dock {
            dock.active_names(&mut names);
        }
        names
            .iter()
            .filter_map(|n| {
                tab.panes
                    .iter()
                    .find(|p| p.read(cx).name.as_ref() == n)
                    .cloned()
            })
            .collect()
    }
    pub fn focus_target(
        &self,
        target: &str,
        pane: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match target {
            "channel_input" if self.adding => self
                .channel_input
                .update(cx, |input, cx| input.focus(window, cx)),
            "composer" => {
                let panes = self.visible_panes(cx);
                let pane = panes.get(pane).ok_or("No such visible pane")?;
                let draft = pane.read(cx).draft.clone();
                draft.update(cx, |input, cx| input.focus(window, cx));
            }
            "search" => {
                let panes = self.visible_panes(cx);
                let pane = panes.get(pane).ok_or("No such visible pane")?;
                pane.update(cx, |p, cx| p.open_search(window, cx));
            }
            "transcript" => {
                let panes = self.visible_panes(cx);
                let pane = panes.get(pane).ok_or("No such visible pane")?;
                let focus = pane.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            "workspace" => self.focus.focus(window, cx),
            _ => return Err("Target is unavailable".into()),
        }
        Ok(())
    }
    pub fn media_inspection(&self) -> Value {
        json!({"cache":self.media.borrow().inspection(),"7tv":self.catalog.borrow().inspection()})
    }
    pub fn inspection(&self, cx: &App) -> Value {
        json!({"own_extras_refused":self.own_extras_refused,"automod_channels":self.mod_automod_channels,"last_drop":self.last_drop,"pointer_owner":self.pointer_owner.map(|control|if control{"local-control"}else{"native-window"}),"control_enabled":self.control_enabled,"active":self.active,"tabs":self.tabs.iter().map(|t|json!({"id":t.id,"name":t.name,"vertical":t.vertical,"dock":t.dock.as_ref().map(Dock::json),"channels":t.panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>(),"adding_channel":self.adding,"appearance_open":self.settings,"font_size":self.font_size,"timestamps":self.timestamps,"pause_mode":self.pause_mode,"history_limit":self.history_limit,"save_status":self.save_status,"close_tab_pending":self.close_tab_pending,"live_workspace_filter":self.live_workspaces,"live_channel_filter":self.live_channels,"streams":self.panes().iter().map(|p|{let n=p.read(cx).name.to_string();(n.clone(),self.streams.get(&n))}).collect::<BTreeMap<_,_>>(),"capabilities":self.capabilities.inspection(),"seventv_presence":self.presence.last.lock().map(|s|s.clone()).unwrap_or_default(),"roles":self.panes().iter().map(|p|{let p=p.read(cx);(p.name.to_string(),p.role.map(|r|r.label()))}).collect::<BTreeMap<_,_>>(),"shared_sessions":self.panes().iter().filter_map(|p|{let p=p.read(cx);p.shared_session.clone().map(|s|(p.name.to_string(),json!({"participants":s,"hidden":p.hidden_sources})))}).collect::<BTreeMap<_,_>>(),"disconnected":self.panes().iter().filter(|p|!p.read(cx).connected).map(|p|{let p=p.read(cx);(p.name.to_string(),p.connection.clone())}).collect::<BTreeMap<_,_>>()})
    }
    /// Channels to connect, highest priority first: visible panes, the rest of the
    /// active workspace, then other workspaces by most recent use. Beyond the
    /// EventSub cap, the lowest-priority channels wait until room frees up.
    fn connection_priority(&self, cx: &App) -> Vec<String> {
        let name = |p: &Entity<ChannelPane>| p.read(cx).name.to_string();
        // Popouts are on screen too.
        let mut order: Vec<String> = self.popouts.iter().map(|p| name(&p.pane)).collect();
        order.extend(self.visible_panes(cx).iter().map(name));
        let active = self.tabs.get(self.active);
        order.extend(active.into_iter().flat_map(|t| t.panes.iter().map(name)));
        let rank = |id: u64| {
            self.recent_workspaces
                .iter()
                .position(|r| *r == id)
                .unwrap_or(usize::MAX)
        };
        let mut others: Vec<&WorkspaceTab> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.active)
            .map(|(_, t)| t)
            .collect();
        others.sort_by_key(|t| rank(t.id));
        order.extend(others.iter().flat_map(|t| t.panes.iter().map(name)));
        // Combined feeds stand for their source channels; repeats are dropped downstream.
        order
            .iter()
            .filter(|n| !crate::combined::is_special(n))
            .flat_map(|n| crate::combined::sources(n))
            // YouTube tabs never reach Twitch.
            .filter(|n| !crate::youtube::is_youtube(n))
            .collect()
    }
    /// Every pane fed by the live connection: workspace tabs and popouts.
    pub fn panes(&self) -> Vec<Entity<ChannelPane>> {
        self.tab_panes()
            .into_iter()
            .chain(self.popouts.iter().map(|p| p.pane.clone()))
            .collect()
    }
    pub fn tab_panes(&self) -> Vec<Entity<ChannelPane>> {
        self.tabs
            .iter()
            .flat_map(|t| t.panes.iter().cloned())
            .collect()
    }
    pub fn new(control_enabled: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loaded = storage::load();
        let save_enabled = loaded.is_ok();
        let save_status = loaded
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| "Workspace saved locally".into());
        let state = loaded.unwrap_or(json!({"version":1}));
        let (accounts, active_account) = Self::load_accounts(&state, cx);
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                "Twitch channel or link, a YouTube live link, /mentions, /whispers or /live",
            )
        });
        cx.subscribe_in(&input, window, |this: &mut Self, _, event, window, cx| {
            if this.adding && matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                this.accept_input(window, cx);
            }
        })
        .detach();
        let drafts = state["drafts"]
            .as_object()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(k, v)| {
                        v.as_str()
                            .filter(|s| s.len() <= 1024 * 1024)
                            .map(|s| (k.clone(), s.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let highlight_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder("Words or phrases, comma separated")
                .validate(|text, _| text.chars().count() <= 400);
            input.set_value(state["highlight_words"].as_str().unwrap_or(""), window, cx);
            input
        });
        cx.subscribe_in(
            &highlight_input,
            window,
            |this: &mut Self, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.highlight_terms =
                        crate::attention::terms(&this.highlight_input.read(cx).value());
                    this.schedule_save(cx);
                    cx.notify();
                }
            },
        )
        .detach();
        let mut this = Self {
            custom_commands: crate::custom_commands::load(&state["custom_commands"]),
            highlight_rules: crate::highlight_rules::load(&state["highlight_rules"]),
            highlight_editor: None,
            chatter_appearance: crate::chatter_appearance::load(&state["chatter_appearance"]),
            ignores: crate::ignores::load(&state["ignores"]),
            ignores_editor: None,
            event_highlights: crate::event_highlights::load(&state["event_highlights"]),
            pending_nicknames: None,
            notice: None,
            announced_missing_scopes: 0,
            chatter_editor: None,
            chatter_notes: crate::chatter_notes::load(&state["chatter_notes"]),
            notes_editor: None,
            highlight_terms: crate::attention::terms(
                state["highlight_words"].as_str().unwrap_or(""),
            ),
            highlight_input,
            activity_all: false,
            zoom_accumulator: 0.,
            live: crate::live::LiveChat::new(),
            anon: crate::anon_chat::AnonChat::new(),
            guest_reading: false,
            splash_done: None,
            opened: std::time::Instant::now(),
            splash_wake: false,
            live_seeded: false,
            hotkey_capture: None,
            on_top: state["on_top"].as_bool().unwrap_or(false),
            autorun: None,
            streams: crate::stream_status::Streams::new(),
            capabilities: crate::capabilities::Capabilities::new(),
            experience: roles::Experience::from_key(state["experience"].as_str().unwrap_or("")),
            streamer_bot: crate::streamer_bot::StreamerBot::new(),
            feed: Default::default(),
            whispers: Default::default(),
            whisper_input: None,
            whispers_drawn: std::time::Instant::now() - Duration::from_secs(60),
            engagement: crate::engagement::Engagement::new(),
            feed_filter: None,
            // Tabs: Activity, Stream, OBS, Tools ("studio_tab" predates the OBS tab).
            studio_tab: state["studio_page"]
                .as_u64()
                .map(|t| t.min(3) as usize)
                .or_else(|| {
                    state["studio_tab"]
                        .as_u64()
                        .map(|t| if t >= 2 { 3 } else { t as usize })
                })
                .unwrap_or(0),
            studio_tab_switched: std::time::Instant::now(),
            equalizing: None,
            bot_settings: streamer_bot_ui::load_settings(&state["streamer_bot"]).0,
            bot_pinned: streamer_bot_ui::load_settings(&state["streamer_bot"]).1,
            bot_runs: Default::default(),
            bot_started: false,
            // Streamer starts with Studio open unless the owner closed it.
            studio_open: state["studio_open"]
                .as_bool()
                .unwrap_or(state["experience"].as_str() == Some("streamer")),
            studio_editor: None,
            raid_input: None,
            setup_offered: state["setup_offered"].as_bool().unwrap_or(false),
            chatters: crate::chatters::Chatters::new(),
            recent: crate::recent_messages::RecentMessages::new(),
            recent_enabled: state["recent_messages"].as_bool().unwrap_or(true),
            live_notify: state["live_notify"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().and_then(chat_core::twitch_login))
                .take(500)
                .collect(),
            live_notify_all: state["live_notify_all"].as_bool().unwrap_or(false),
            live_notify_follows: state["live_notify_follows"].as_bool().unwrap_or(false),
            next_notice: 0,
            muted_pings: state["muted_pings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().map(str::to_owned))
                .take(500)
                .collect(),
            presence: crate::seven_presence::Presence::new(
                state["seventv_activity"].as_bool().unwrap_or(true),
            ),
            recent_requested: Default::default(),
            updates: {
                let mut updates = crate::updates::Updates::new();
                updates.enabled = state["update_checks"].as_bool().unwrap_or(true);
                updates
            },
            announced_update: None,
            live_workspaces: state["live_workspaces"].as_bool().unwrap_or(false),
            live_channels: state["live_channels"].as_bool().unwrap_or(false),
            catalog: std::rc::Rc::new(std::cell::RefCell::new(crate::catalog::Catalog::new())),
            media: std::rc::Rc::new(std::cell::RefCell::new(crate::media::MediaCache::new())),
            account: accounts[active_account].clone(),
            accounts,
            account_focus: None,
            automod: Default::default(),
            own_extras_refused: None,
            mod_pick: Default::default(),
            mod_live: None,
            automod_hidden: state["automod_hidden"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .filter_map(chat_core::twitch_login)
                .collect(),
            mod_automod_channels: Vec::new(),
            sound_mentions: state["sound"]["mentions"].as_bool().unwrap_or(true),
            sound_highlights: state["sound"]["highlights"].as_bool().unwrap_or(false),
            sound_focused: state["sound"]["focused"].as_bool().unwrap_or(false),
            flash_taskbar: state["sound"]["flash"].as_bool().unwrap_or(true),
            sound_tone: crate::sound::Tone::from_key(state["sound"]["tone"].as_str().unwrap_or("")),
            sound_volume: state["sound"]["volume"]
                .as_f64()
                .map(|v| v as f32)
                .filter(|v| (0.05..=1.).contains(v))
                .unwrap_or(0.6),
            pinged: Default::default(),
            popouts: Vec::new(),
            blocks: crate::blocks::Blocks::new(),
            hide_blocked: state["hide_blocked"].as_bool().unwrap_or(true),
            mod_buttons: state["mod_buttons"].as_bool().unwrap_or(false),
            custom_mod_buttons: std::rc::Rc::new(crate::mod_buttons::load(
                &state["custom_mod_buttons"],
            )),
            mod_button_inputs: None,
            follows_live: crate::follows_live::FollowsLive::new(),
            follows_open: state["follows_open"].as_bool().unwrap_or(true),
            live_sort: crate::follows_live::Sort::from_key(
                state["live_sort"].as_str().unwrap_or(""),
                crate::follows_live::Sort::SIDEBAR,
            ),
            live_filter: {
                let filter = cx.new(|cx| {
                    InputState::new(window, cx).placeholder("Filter by name, game or title")
                });
                cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
                .detach();
                filter
            },
            live_filter_open: false,
            raid_sort: crate::follows_live::Sort::from_key(
                state["raid_sort"].as_str().unwrap_or(""),
                crate::follows_live::Sort::RAID,
            ),
            stream_presets: crate::studio::load_presets(&state["stream_presets"]),
            obs: crate::obs::Obs::new(),
            obs_settings: obs_ui::load_obs_settings(&state["obs"]),
            obs_started: false,
            obs_tick: 0,
            ads: crate::ads::Ads::new(),
            hype: Default::default(),
            hype_loaded_for: None,
            checklist_items: state["checklist"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .map(|v| v.chars().filter(|c| !c.is_control()).take(60).collect())
                .take(12)
                .collect(),
            checklist_done: Default::default(),
            checklist_dismissed: false,
            checklist_input: None,
            went_live_at: None,
            was_live: false,
            peak_viewers: 0,
            stats_tick: 0,
            channel_ready: Default::default(),
            youtube: crate::youtube::YouTube::new(),
            youtube_key_loaded: false,
            guide_status: None,
            google_client: None,
            google_tokens: None,
            youtube_chats: Default::default(),
            rewards: crate::rewards::Rewards::new(),
            last_stream_info: None,
            last_raid: None,
            shouted: Default::default(),
            obs_found: None,
            spellcheck: {
                crate::link_preview::ENABLED.store(
                    state["link_previews"].as_bool().unwrap_or(true),
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::chat_search::load_saved(&state["saved_filters"]);
                for (index, flag) in crate::theme::HEADER.iter().enumerate() {
                    if let Some(on) = state["header"][index].as_bool() {
                        flag.store(on, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                crate::media::ANIMATE.store(
                    state["animate"].as_u64().unwrap_or(0).min(2) as u8,
                    std::sync::atomic::Ordering::Relaxed,
                );
                for (index, flag) in crate::theme::BADGES.iter().enumerate() {
                    flag.store(
                        state["badges"][index].as_bool().unwrap_or(true),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                crate::shared_chat::STYLE.store(
                    state["shared_chat_look"].as_u64().unwrap_or(0).min(3) as u8,
                    std::sync::atomic::Ordering::Relaxed,
                );
                chat_core::NAME_MODE.store(
                    state["name_mode"].as_u64().unwrap_or(2).min(2) as u8,
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::theme::SEPARATORS.store(
                    state["separators"].as_bool().unwrap_or(false),
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::theme::HIDE_DELETED.store(
                    state["hide_deleted"].as_bool().unwrap_or(false),
                    std::sync::atomic::Ordering::Relaxed,
                );
                if let Ok(mut font) = crate::theme::CHAT_FONT.lock() {
                    *font = state["chat_font"]
                        .as_str()
                        .filter(|f| !f.is_empty() && f.len() <= 80)
                        .map(str::to_owned);
                }
                crate::pronouns::ENABLED.store(
                    state["pronouns"].as_bool().unwrap_or(false),
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::hotkeys::load(&state["hotkeys"], cx);
                crate::inline_chat::EMOTE_SCALE.store(
                    state["emote_scale"].as_u64().unwrap_or(100).clamp(50, 200) as u32,
                    std::sync::atomic::Ordering::Relaxed,
                );
                for (index, name) in crate::inline_chat::EMOTE_OPTION_NAMES.iter().enumerate() {
                    if let Some(on) = state["emote_options"][name].as_bool() {
                        crate::inline_chat::EMOTE_OPTIONS[index]
                            .store(on, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                for (index, name) in crate::replies::OPTION_NAMES.iter().enumerate() {
                    if let Some(on) = state["input_options"][name].as_bool() {
                        crate::replies::OPTIONS[index]
                            .store(on, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                crate::mention_names::COLOR_MENTIONS.store(
                    state["mentions"]["color"].as_bool().unwrap_or(true),
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::mention_names::FIND_NAMES.store(
                    state["mentions"]["find_names"].as_bool().unwrap_or(false),
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::catalog::SHOW_UNLISTED.store(
                    state["emote_options"]["unlisted_7tv"]
                        .as_bool()
                        .unwrap_or(false),
                    std::sync::atomic::Ordering::Relaxed,
                );
                crate::theme::ALTERNATE_ROWS.store(
                    state["alternate_rows"].as_bool().unwrap_or(false),
                    std::sync::atomic::Ordering::Relaxed,
                );
                for (key, flag) in [
                    ("7tv", &crate::catalog::SEVEN_TV),
                    ("bttv", &crate::catalog::BTTV),
                    ("ffz", &crate::catalog::FFZ),
                ] {
                    flag.store(
                        state["emote_providers"][key].as_bool().unwrap_or(true),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                state["spellcheck"].as_bool().unwrap_or(false)
            },
            own_youtube: None,
            own_youtube_checked: None,
            own_youtube_busy: false,
            shared_chat: crate::shared_chat::SharedChat::new(),
            platform_filters: state["platform_filters"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| {
                            let keys = v
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|k| k.as_str().map(str::to_owned))
                                .collect();
                            (k.clone(), keys)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            view_filters: state["view_filters"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| {
                            v.as_str()
                                .map(|v| (k.clone(), v.chars().take(256).collect()))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            similar_mode: crate::similarity::Mode::from_key(
                state["similar_messages"].as_str().unwrap_or(""),
            ),
            logs: crate::chat_logs::Logs::new(),
            logs_enabled: state["logs"]["enabled"].as_bool().unwrap_or(false),
            logs_folder: state["logs"]["folder"]
                .as_str()
                .filter(|f| !f.is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(crate::chat_logs::default_folder),
            streamer_mode: crate::streamer_mode::Mode::from_key(
                state["streamer_mode"]["mode"].as_str().unwrap_or(""),
            ),
            streamer_detector: crate::streamer_mode::Detector::new(),
            streamer_mute: {
                for (index, (key, _, _, default)) in
                    crate::streamer_mode::OPTIONS.iter().enumerate()
                {
                    crate::streamer_mode::HIDE[index].store(
                        state["streamer_mode"]["hide"][key]
                            .as_bool()
                            .unwrap_or(*default),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                state["streamer_mode"]["mute"].as_bool().unwrap_or(false)
            },
            streamer_live: false,
            restore_popouts: state["popouts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .filter_map(chat_core::twitch_login)
                .take(8)
                .collect(),
            send_as: state["send_as"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(c, v)| {
                    v.as_str()
                        .map(|v| (c.clone(), SharedString::from(v.to_owned())))
                })
                .collect(),
            drop_edge: None,
            drop_target: None,
            dock_bounds: Default::default(),
            strip_bounds: Default::default(),
            tab_bounds: Default::default(),
            last_drop: None,
            release_position: None,
            control_pointer_active: false,
            release_from_control: false,
            pointer_owner: None,
            control_enabled,
            tabs: vec![],
            closed_tabs: vec![],
            active: 0,
            next_id: state["next_id"]
                .as_u64()
                .filter(|id| *id > 0 && *id < 1_000_000)
                .unwrap_or(1),
            focus: cx.focus_handle(),
            channel_input: input,
            adding: false,
            renaming: false,
            replacing: None,
            add_target: None,
            selected_channel: None,
            recent_workspaces: Vec::new(),
            add_error: None,
            sidebar: state["sidebar"].as_bool().unwrap_or(false),
            settings: false,
            settings_section: 0,
            settings_opened: std::time::Instant::now(),
            font_size: state["font_size"]
                .as_f64()
                .filter(|n| n.is_finite())
                .unwrap_or(14.)
                .clamp(12., 24.) as f32,
            pause_mode: state["pause_mode"].as_u64().unwrap_or(0).min(2) as u8,
            timestamps: state["timestamps"].as_u64().unwrap_or(0).min(4) as u8,
            history_limit: state["history_limit"]
                .as_u64()
                .unwrap_or(10_000)
                .clamp(500, 10_000) as usize,
            drafts,
            reply_drafts: state["reply_drafts"]
                .as_object()
                .map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
            save_revision: 0,
            save_enabled,
            save_status,
            close_failed: false,
            close_tab_pending: None,
            close_channel_pending: false,
        };
        this.catalog
            .borrow_mut()
            .restore_favorites(&state["emote_favorites"]);
        if let Some(tabs) = state["tabs"].as_array() {
            for item in tabs.iter() {
                let id = item["id"]
                    .as_u64()
                    .filter(|id| {
                        *id > 0 && *id < 1_000_000 && !this.tabs.iter().any(|t| t.id == *id)
                    })
                    .unwrap_or(this.next_id);
                this.next_id = this.next_id.max(id + 1);
                let name = item["name"]
                    .as_str()
                    .unwrap_or("Workspace")
                    .chars()
                    .take(40)
                    .collect();
                let mut panes = Vec::new();
                if let Some(channels) = item["channels"].as_array() {
                    for channel in channels.iter() {
                        if let Some(name) = channel.as_str().and_then(|n| {
                            // Special tabs first: channel cleanup would turn "/mentions" into "mentions".
                            crate::combined::special_title(n)
                                .map(|_| n.to_owned())
                                .or_else(|| valid_channel(n))
                                .or_else(|| crate::combined::from_saved(n))
                                .or_else(|| crate::youtube::parse(n))
                        }) && !panes.iter().any(|p: &Entity<ChannelPane>| {
                            p.read(cx).name.as_ref() == name.as_str()
                        }) {
                            panes.push(this.make_pane(id, &name, window, cx));
                        }
                    }
                }
                let sizes: Vec<f32> = item["sizes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .take(4)
                            .map(|v| {
                                v.as_f64()
                                    .filter(|n| n.is_finite())
                                    .unwrap_or(480.)
                                    .clamp(180., 3000.) as f32
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let names = panes
                    .iter()
                    .map(|p| p.read(cx).name.to_string())
                    .collect::<Vec<_>>();
                // A saved layout that doesn't match the channels that loaded is
                // repaired (missing names dropped, unplaced channels added as tabs),
                // never thrown away: the fallback would put every channel in its own split.
                let dock = Dock::from_json(&item["dock"])
                    .and_then(|d| crate::dock::reconcile(d, &names))
                    .or_else(|| {
                        Dock::legacy(&names, item["vertical"].as_bool().unwrap_or(false), &sizes)
                    });
                this.tabs.push(WorkspaceTab {
                    id,
                    name,
                    panes,
                    vertical: item["vertical"].as_bool().unwrap_or(false),
                    sizes,
                    split: cx.new(|_| ResizableState::default()),
                    dock,
                    dock_states: BTreeMap::new(),
                    locked: item["locked"].as_bool().unwrap_or(false),
                    pinned: item["pinned"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|c| c.as_str())
                        .filter(|c| names.iter().any(|n| n == c))
                        .map(str::to_owned)
                        .collect(),
                });
            }
        }
        if this.tabs.is_empty() {
            this.tabs.push(WorkspaceTab {
                id: this.next_id,
                name: "Workspace 1".into(),
                panes: vec![],
                vertical: false,
                sizes: vec![],
                split: cx.new(|_| ResizableState::default()),
                dock: None,
                dock_states: BTreeMap::new(),
                locked: false,
                pinned: Vec::new(),
            });
            this.next_id += 1;
        }
        this.active = (state["active"].as_u64().unwrap_or(0) as usize).min(this.tabs.len() - 1);
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if view
                    .update_in(cx, |this, window, cx| this.pump_chat(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        if this.on_top {
            sound_settings::set_on_top(window, true);
        }
        crate::media::set_shared(this.media.clone());
        // "Animate only while focused": track whether a Jawjack window is in front.
        cx.observe_window_activation(window, |_, window, cx| {
            crate::media::FOCUSED.store(
                window.is_window_active() || cx.active_window().is_some(),
                std::sync::atomic::Ordering::Relaxed,
            );
            cx.notify();
        })
        .detach();
        this
    }
    fn pump_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let identity = self.account.read(cx).identity();
        let panes = self.panes();
        for pane in panes
            .iter()
            .chain(self.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
        {
            pane.update(cx, |p, _| {
                p.input_history
                    .account(identity.as_ref().map(|i| i.user_id.as_str()))
            });
        }
        self.sync_attention(window, cx);
        if self.streams.pump(
            identity.clone(),
            panes
                .iter()
                .flat_map(|p| crate::combined::sources(&p.read(cx).name))
                .filter(|n| !crate::youtube::is_youtube(n))
                .collect(),
        ) {
            let visible = self.visible_panes(cx);
            for pane in &panes {
                let info = self
                    .streams
                    .detail(&pane.read(cx).name)
                    .map(|(i, _)| i.clone());
                if pane.read(cx).stream != info {
                    // Warm the hover preview for visible live channels.
                    if visible.contains(pane)
                        && info.as_ref().is_some_and(|i| i.live)
                        && let Some(key) =
                            crate::media::EmoteKey::stream_preview(&pane.read(cx).name)
                    {
                        self.media.borrow_mut().get(&key, cx);
                    }
                    pane.update(cx, |p, _| p.stream = info);
                }
            }
            cx.notify();
        }
        self.pump_live_alerts(window, cx);
        self.pump_imported_nicknames(cx);
        self.pump_recent_messages(cx);
        self.ensure_anchor(window, cx);
        if !self.bot_started {
            self.bot_started = true;
            self.start_streamer_bot(cx);
        }
        self.pump_streamer_bot(window, cx);
        if !self.obs_started {
            self.obs_started = true;
            self.start_obs(cx);
        }
        self.pump_obs(cx);
        self.pump_ads(identity.as_ref(), window, cx);
        self.pump_rewards(identity.as_ref(), window, cx);
        self.pump_youtube(cx);
        self.pump_shared_chat(identity.as_ref(), cx);
        self.sync_link_preview_identity(cx);
        // Goals, charity and Shield Mode already running at sign-in.
        if let Some(identity) = identity.clone()
            && self.hype_loaded_for.as_deref() != Some(identity.user_id.as_str())
        {
            self.hype_loaded_for = Some(identity.user_id.clone());
            self.hype = Default::default();
            let task = cx.background_spawn(async move { crate::hype::initial(&identity) });
            cx.spawn(async move |view, cx| {
                let events = task.await;
                let _ = view.update(cx, |this, cx| {
                    for (kind, event) in events {
                        crate::hype::apply(&mut this.hype, &kind, &event);
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        // A new stream starts a fresh checklist.
        let live = self.own_live(cx);
        if live && !self.was_live {
            self.went_live_at = Some(std::time::Instant::now());
        }
        if live {
            self.track_stream(cx);
            // Live stats refresh every few seconds while Studio is open.
            let tick = self.went_live_at.map_or(0, |t| t.elapsed().as_secs() / 5);
            if self.studio_open && tick != self.stats_tick {
                self.stats_tick = tick;
                cx.notify();
            }
        }
        if !live && self.was_live {
            self.show_recap(window, cx);
            self.checklist_done.clear();
            self.checklist_dismissed = false;
        }
        self.was_live = live;
        self.pump_engagement(window, cx);
        if !self.setup_offered && identity.is_some() {
            self.setup_offered = true;
            self.notice = Some((
                "New: choose a Viewer, Moderator or Streamer setup · gear menu → Choose your setup"
                    .into(),
                std::time::Instant::now(),
            ));
            self.schedule_save(cx);
        }
        // Keep each composer's "send as" picker in step with the signed-in accounts.
        let options = std::rc::Rc::new(self.send_as_options(cx));
        for pane in &panes {
            if !std::rc::Rc::ptr_eq(&pane.read(cx).custom_mod_buttons, &self.custom_mod_buttons) {
                let buttons = self.custom_mod_buttons.clone();
                pane.update(cx, |p, cx| {
                    p.custom_mod_buttons = buttons;
                    cx.notify();
                });
            }
            if pane.read(cx).mod_buttons != self.mod_buttons {
                let on = self.mod_buttons;
                pane.update(cx, |p, cx| {
                    p.mod_buttons = on;
                    cx.notify();
                });
            }
            if pane.read(cx).spellcheck != self.spellcheck {
                let on = self.spellcheck;
                pane.update(cx, |p, cx| {
                    p.spellcheck = on;
                    p.check_spelling(cx);
                    cx.notify();
                });
            }
            if pane.read(cx).similar_mode != self.similar_mode {
                let mode = self.similar_mode;
                pane.update(cx, |p, _| p.similar_mode = mode);
            }
            crate::streamer_mode::LIVE
                .store(self.streamer_live, std::sync::atomic::Ordering::Relaxed);
            if pane.read(cx).streamer_mode != self.streamer_live {
                let live = self.streamer_live;
                pane.update(cx, |p, _| p.streamer_mode = live);
            }
            let held = self
                .automod
                .get(pane.read(cx).name.as_ref())
                .cloned()
                .unwrap_or_default();
            if !std::rc::Rc::ptr_eq(&pane.read(cx).automod, &held)
                && !(held.is_empty() && pane.read(cx).automod.is_empty())
            {
                pane.update(cx, |p, cx| {
                    p.automod = held;
                    cx.notify();
                });
            }
            let send_as = self.send_as.get(pane.read(cx).name.as_ref()).cloned();
            if *pane.read(cx).send_as_options != *options || pane.read(cx).send_as != send_as {
                let options = options.clone();
                pane.update(cx, |p, cx| {
                    p.send_as_options = options;
                    p.send_as = send_as;
                    cx.notify();
                });
            }
        }
        let mod_tools = self.experience != roles::Experience::Viewer;
        for pane in &panes {
            if pane.read(cx).mod_tools != mod_tools {
                pane.update(cx, |p, cx| {
                    p.mod_tools = mod_tools;
                    cx.notify();
                });
            }
        }
        if self.updates.auto_check_due() {
            self.updates.check();
        }
        if self.updates.poll() {
            if let Some(version) = self.updates.available()
                && self.announced_update.as_ref() != Some(&version)
            {
                self.notice = Some((
                    format!("Jawjack {version} is available. Workspaces sidebar → update."),
                    std::time::Instant::now(),
                ));
                self.announced_update = Some(version);
            }
            cx.notify();
        }
        // A newer Jawjack may need permissions an older saved grant lacks.
        let missing = self.account.read(cx).missing_scopes().len();
        if identity.is_some() && missing > self.announced_missing_scopes {
            self.announced_missing_scopes = missing;
            self.notice = Some((
                format!(
                    "This version can use {missing} Twitch permissions your login hasn't granted yet. Account menu → Update Twitch permissions."
                ),
                std::time::Instant::now(),
            ));
            cx.notify();
        }
        let login = self.account.read(cx).login();
        if self.capabilities.pump(identity.clone(), login) {
            cx.notify();
        }
        // Chatter lists for visible channels the account moderates or owns.
        let listed: Vec<String> = self
            .visible_panes(cx)
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .filter(|c| {
                self.capabilities
                    .role(c)
                    .is_some_and(|r| r != crate::capabilities::Role::Viewer)
                    && identity
                        .as_ref()
                        .is_some_and(|i| i.has("moderator:read:chatters"))
            })
            .collect();
        for channel in self.chatters.pump(identity.as_ref(), &listed) {
            if let Some(list) = self.chatters.get(&channel) {
                for pane in &panes {
                    if pane.read(cx).name.as_ref() == channel {
                        pane.update(cx, |p, _| p.chatter_list = list.clone());
                    }
                }
            }
        }
        // A combined feed is connected while any of its source channels is.
        for pane in &panes {
            let name = pane.read(cx).name.to_string();
            if !crate::combined::is_combined(&name) {
                continue;
            }
            let sources = crate::combined::sources(&name);
            let any = sources
                .iter()
                .any(|s| self.channel_ready.get(s).copied().unwrap_or(false));
            if pane.read(cx).connected != any {
                pane.update(cx, |p, cx| {
                    p.connected = any;
                    p.connection = if any {
                        "Connected".into()
                    } else {
                        "Connecting…".into()
                    };
                    cx.notify();
                });
            }
        }
        for pane in &panes {
            let role = self.capabilities.role(&pane.read(cx).name);
            if pane.read(cx).role != role {
                pane.update(cx, |p, cx| {
                    p.role = role;
                    cx.notify();
                });
            }
        }
        let profiles_changed = self.catalog.borrow_mut().profiles.pump(identity.clone());
        if profiles_changed {
            for pane in &panes {
                pane.update(cx, |p, cx| {
                    if p.picker.open {
                        p.refresh_picker(cx);
                    }
                    cx.notify();
                });
            }
            cx.notify();
        }
        if self.follows_live.pump(identity.as_ref()) {
            cx.notify();
        }
        if self.blocks.pump(identity.as_ref()) {
            self.apply_blocks();
        }
        let priority = self.connection_priority(cx);
        // Ask 7TV for the cosmetics of people in open channels (visible first).
        if let (Some(identity), Some(session)) =
            (&identity, self.catalog.borrow().seventv_session())
        {
            for channel in priority.iter().take(40) {
                if let Some(id) = self.catalog.borrow().channel_id(channel) {
                    self.presence.join(&identity.user_id, &id, &session);
                }
            }
        }
        let notices = {
            let changed = self
                .catalog
                .borrow_mut()
                .pump(priority.clone(), identity.clone());
            (changed, self.catalog.borrow_mut().take_notices())
        };
        for (channel, text) in notices.1 {
            for pane in &panes {
                if pane.read(cx).all_sources().contains(&channel) {
                    let id = format!("system:{}:{}", channel, self.next_notice);
                    self.next_notice += 1;
                    let notice = chat_core::system_notice(
                        &channel,
                        id,
                        Some(chrono::Utc::now().timestamp()),
                        text.clone(),
                    );
                    pane.update(cx, |p, cx| {
                        p.received(chat_core::Event::Message(notice), cx)
                    });
                }
            }
        }
        if notices.0 {
            for pane in &panes {
                pane.update(cx, |p, cx| {
                    p.search.dirty = true;
                    p.timeline.borrow_mut().enrich(|message| {
                        message.fragments = self.catalog.borrow().expand(
                            &p.name,
                            &message.user_id,
                            &message.fragments,
                        )
                    });
                    p.attention.reconcile(
                        &p.timeline.borrow(),
                        (p.next_id - p.timeline.borrow().messages().len()) as u64,
                    );
                    p.scroller.update(cx, |s, cx| s.remeasure(cx));
                    if p.picker.open {
                        p.refresh_picker(cx);
                    } else {
                        p.complete_query(false, cx);
                    }
                    cx.notify();
                });
            }
            cx.notify();
        }
        let visible_picker_keys = if window.has_active_dialog(cx) {
            Vec::new()
        } else {
            self.visible_panes(cx)
                .iter()
                .filter(|pane| pane.read(cx).picker.open)
                .flat_map(|pane| pane.read(cx).picker.visible_media().to_vec())
                .collect::<Vec<_>>()
        };
        let (media_changed, layout_changed) = {
            let mut media = self.media.borrow_mut();
            media.prioritize_picker(&visible_picker_keys, cx);
            (media.pump(cx), media.take_layout_dirty())
        };
        if media_changed || layout_changed {
            for pane in &panes {
                pane.update(cx, |p, cx| {
                    if layout_changed {
                        p.scroller.update(cx, |s, cx| s.remeasure(cx));
                    }
                    cx.notify();
                });
            }
        }
        let priority = self.connection_priority(cx);
        // Moderated channels (not your own) get AutoMod queues, on-screen ones first.
        let moderated: Vec<String> = priority
            .iter()
            .filter(|channel| {
                self.panes().iter().any(|p| {
                    let p = p.read(cx);
                    p.name.as_ref() == channel.as_str()
                        && p.role == Some(crate::capabilities::Role::Moderator)
                        && !self.automod_hidden.contains(channel.as_str())
                })
            })
            .cloned()
            .collect();
        let own = self.account.read(cx).login();
        // Signed out: read chat as a guest only once that's chosen on the sign-in screen.
        self.anon
            .configure(if identity.is_none() && self.guest_reading {
                priority.clone()
            } else {
                Vec::new()
            });
        if self
            .live
            .configure(identity.clone(), priority, moderated, own)
        {
            // Special tabs (Mentions, Whispers) never connect, so they never reset.
            let connecting: Vec<_> = panes
                .iter()
                .filter(|p| !crate::combined::is_special(&p.read(cx).name))
                .cloned()
                .collect();
            for pane in &connecting {
                pane.update(cx, |p, cx| {
                    p.connected = false;
                    p.room_settings = None;
                    p.connection = if identity.is_some() {
                        "Connecting…"
                    } else {
                        "Sign in to connect"
                    }
                    .into();
                    cx.notify();
                });
            }
        }
        let guest = identity.is_none();
        for _ in 0..500 {
            // Guest events count as current only while still signed out.
            let Ok((version, event)) = self.live.events.try_recv().or_else(|_| {
                self.anon
                    .events
                    .try_recv()
                    .map(|event| (if guest { self.live.version() } else { u64::MAX }, event))
            }) else {
                break;
            };
            if version != self.live.version() && !matches!(event, crate::live::Event::Sent(..)) {
                continue;
            }
            match event {
                crate::live::Event::RoomSettings(channel, settings, initial) => {
                    for pane in &panes {
                        if pane.read(cx).name.as_ref() == channel {
                            pane.update(cx, |p, cx| {
                                if !initial || p.room_settings.is_none() {
                                    p.room_settings = Some(settings.clone());
                                    cx.notify();
                                }
                            });
                        }
                    }
                    cx.notify();
                }
                crate::live::Event::Whisper {
                    id,
                    from_login,
                    from_name,
                    text,
                } => {
                    self.whisper_received(id, from_login, from_name, text, window, cx);
                }
                crate::live::Event::Hype(kind, event) => {
                    if crate::hype::apply(&mut self.hype, &kind, &event) {
                        self.animate_for(Duration::from_millis(240), cx);
                        cx.notify();
                    }
                }
                crate::live::Event::Suspicious(channel, message, label) => {
                    for pane in &panes {
                        if pane.read(cx).name.as_ref() == channel {
                            let (message, label) = (message.clone(), label.clone());
                            pane.update(cx, |p, cx| p.mark_suspicious(message, label, cx));
                        }
                    }
                }
                crate::live::Event::ModAutomod(channels) => {
                    self.mod_automod_channels = channels;
                }
                crate::live::Event::OwnExtras(refused) => {
                    self.own_extras_refused = Some(refused);
                    cx.notify();
                }
                crate::live::Event::AutoMod(channel, _)
                    if self.automod_hidden.contains(channel.as_str()) => {}
                crate::live::Event::AutoMod(channel, update) => {
                    let queue = self.automod.entry(channel.clone()).or_default();
                    let notice = crate::automod::apply(std::rc::Rc::make_mut(queue), update);
                    let queue = queue.clone();
                    self.share_automod(&channel, queue, cx);
                    if let Some(notice) = notice {
                        self.notice = Some((notice, std::time::Instant::now()));
                    }
                    cx.notify();
                }
                crate::live::Event::Activity(item) => {
                    // A redemption may need handling in the Studio queue.
                    if item.kind == crate::activity_feed::Kind::Redemption {
                        self.rewards.refresh();
                    }
                    self.feed.push(item);
                    self.animate_for(Duration::from_millis(320), cx);
                    cx.notify();
                }
                crate::live::Event::Resolved(channel, id) => {
                    self.catalog.borrow_mut().channel(&channel, &id)
                }
                crate::live::Event::State(channel, status, ready) => {
                    self.channel_ready.insert(channel.clone(), ready);
                    for pane in &panes {
                        let name = pane.read(cx).name.to_string();
                        if crate::combined::is_combined(&name)
                            && crate::combined::sources(&name).contains(&channel)
                        {
                            if ready && !pane.read(cx).connected {
                                pane.update(cx, |p, cx| {
                                    p.connected = true;
                                    p.connection = "Connected".into();
                                    cx.notify();
                                });
                            }
                            continue;
                        }
                        if pane.read(cx).name.as_ref() == channel {
                            pane.update(cx, |p, cx| {
                                p.connection = status.clone();
                                p.connected = ready;
                                if !ready {
                                    p.room_settings = None;
                                }
                                cx.notify();
                            });
                        }
                    }
                    cx.notify();
                }
                crate::live::Event::Chat(channel, event) => {
                    cx.notify();
                    // An incoming raid on your channel offers a shoutout card in Studio.
                    if let chat_core::Event::Message(message) = &event
                        && self.account.read(cx).login().as_deref() == Some(channel.as_str())
                        && message.presentation.notice_type.as_deref() == Some("raid")
                        && !message.presentation.historical
                        && let Some(login) = message.login.clone()
                    {
                        let viewers = message
                            .presentation
                            .notice
                            .as_deref()
                            .and_then(|n| {
                                n.split_whitespace()
                                    .find_map(|w| w.replace(',', "").parse::<u64>().ok())
                            })
                            .unwrap_or(0);
                        self.last_raid = Some((
                            login,
                            message.display_name.clone(),
                            viewers,
                            std::time::Instant::now(),
                        ));
                    }
                    // Subs, gifts, raids and Bits in the own channel also go to the feed.
                    if let chat_core::Event::Message(message) = &event
                        && self.account.read(cx).login().as_deref() == Some(channel.as_str())
                        && let Some(item) = crate::activity_feed::Feed::from_message(message)
                    {
                        self.feed.push(item);
                        self.animate_for(Duration::from_millis(320), cx);
                    }
                    if self.logs_enabled {
                        self.log_event(&channel, &event, cx);
                    }
                    let moderation = !matches!(&event, chat_core::Event::Message(_));
                    for pane in panes.iter().chain(
                        self.closed_tabs
                            .iter()
                            .flat_map(|tab| tab.panes.iter())
                            .filter(|_| moderation),
                    ) {
                        let name = pane.read(cx).name.to_string();
                        if crate::combined::is_combined(&name) {
                            // Clearing one channel must not wipe the whole combined feed.
                            if crate::combined::sources(&name).contains(&channel)
                                && !matches!(event, chat_core::Event::ClearChannel { .. })
                            {
                                let mut tagged = event.clone();
                                if let chat_core::Event::Message(m) = &mut tagged {
                                    m.presentation.origin = Some(channel.clone());
                                }
                                pane.update(cx, |p, cx| p.received(tagged, cx));
                            }
                        } else if name == channel {
                            pane.update(cx, |p, cx| p.received(event.clone(), cx));
                        }
                    }
                }
                crate::live::Event::Sent(request, result) => {
                    // Share 7TV activity for the channel this send came from.
                    if result.is_ok()
                        && let Some(identity) = &identity
                        && let Some(channel) = panes
                            .iter()
                            .find(|p| {
                                p.read(cx)
                                    .pending
                                    .as_ref()
                                    .is_some_and(|(r, ..)| *r == request)
                            })
                            .map(|p| p.read(cx).name.to_string())
                        && let Some(channel_id) = self.catalog.borrow().channel_id(&channel)
                    {
                        self.presence.activity(&identity.user_id, &channel_id);
                    }
                    for pane in panes
                        .iter()
                        .chain(self.closed_tabs.iter().flat_map(|t| t.panes.iter()))
                    {
                        pane.update(cx, |p, cx| {
                            p.sent(request, &result, version == self.live.version(), window, cx)
                        });
                    }
                    if result.is_ok() && version == self.live.version() {
                        for pane in &panes {
                            pane.update(cx, |p, cx| {
                                if p.picker.open {
                                    p.refresh_picker(cx);
                                    cx.notify();
                                }
                            });
                        }
                    }
                }
            }
        }
    }
    fn open_chatter_note(
        &mut self,
        target: crate::chatter_appearance::Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings = false;
        if self.streamer_live {
            window.push_notification(
                Notification::info("Private notes stay hidden while streamer mode is on (Settings → Streamer mode)."),
                cx,
            );
            return;
        }
        let editor = if let Some(editor) = &self.notes_editor {
            editor.clone()
        } else {
            let editor = cx.new(|cx| {
                crate::chatter_notes::Editor::new(self.chatter_notes.clone(), window, cx)
            });
            cx.subscribe(&editor, |this, _, _: &crate::chatter_notes::Changed, cx| {
                this.schedule_save(cx);
                cx.notify();
            })
            .detach();
            self.notes_editor = Some(editor.clone());
            editor
        };
        editor.update(cx, |editor, cx| editor.open(target, window, cx));
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 500.);
        let focus_editor = editor.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Private chatter notes")
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(editor.clone())
        });
        focus_editor.update(cx, |editor, cx| editor.focus(window, cx));
    }
    fn open_chatter_appearance(
        &mut self,
        target: Option<crate::chatter_appearance::Target>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings = false;
        let editor = if let Some(editor) = &self.chatter_editor {
            editor.clone()
        } else {
            let editor = cx.new(|cx| {
                crate::chatter_appearance::Editor::new(self.chatter_appearance.clone(), window, cx)
            });
            cx.subscribe(
                &editor,
                |this, _, _: &crate::chatter_appearance::Changed, cx| {
                    for pane in this
                        .panes()
                        .iter()
                        .chain(this.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
                    {
                        pane.update(cx, |p, cx| {
                            crate::chatter_appearance::restyle(
                                &this.chatter_appearance,
                                &mut p.timeline.borrow_mut(),
                            );
                            p.selection.borrow_mut().clear();
                            p.link_press.borrow_mut().take();
                            p.search.dirty = true;
                            p.scroller.update(cx, |s, cx| s.remeasure(cx));
                            cx.notify();
                        });
                    }
                    this.schedule_save(cx);
                    cx.notify();
                },
            )
            .detach();
            self.chatter_editor = Some(editor.clone());
            editor
        };
        editor.update(cx, |editor, cx| editor.open(target, window, cx));
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 520.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Chatter nicknames & colors")
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(editor.clone())
        });
    }
    fn open_highlight_rules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let channel = self
            .selected_channel
            .clone()
            .or_else(|| {
                self.visible_panes(cx)
                    .first()
                    .map(|p| p.read(cx).name.to_string())
            })
            .unwrap_or_else(|| "channel".into());
        self.settings = false;
        let editor = if let Some(editor) = &self.highlight_editor {
            editor.update(cx, |editor, cx| editor.set_channel(channel, cx));
            editor.clone()
        } else {
            let editor = cx.new(|cx| {
                crate::highlight_rules::Editor::new(
                    self.highlight_rules.clone(),
                    channel,
                    window,
                    cx,
                )
            });
            cx.subscribe(
                &editor,
                |this, _, _: &crate::highlight_rules::Changed, cx| {
                    this.schedule_save(cx);
                    cx.notify();
                },
            )
            .detach();
            self.highlight_editor = Some(editor.clone());
            editor
        };
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 540.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Highlight rules")
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(editor.clone())
        });
    }
    fn open_chatterino_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = false;
        let preview = cx.new(|_| crate::chatterino_import::Preview::load());
        cx.subscribe_in(
            &preview,
            window,
            |this, preview, _: &crate::chatterino_import::Apply, window, cx| {
                let Some(plan) = preview.update(cx, |p, _| p.plan.take()) else {
                    return;
                };
                window.close_dialog(cx);
                this.apply_chatterino_import(plan, window, cx);
            },
        )
        .detach();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 520.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Import from Chatterino")
                .child(preview.clone())
        });
    }
    /// Add an imported Chatterino setup alongside existing workspaces and settings.
    fn apply_chatterino_import(
        &mut self,
        plan: crate::chatterino_import::Plan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut notes = Vec::new();
        let first_new = self.tabs.len();
        for (name, dock) in plan.workspaces {
            let id = self.next_id;
            self.next_id += 1;
            let mut unique = name.clone();
            let mut n = 2;
            while self.tabs.iter().any(|t| t.name == unique) {
                unique = format!("{name} {n}");
                n += 1;
            }
            let mut names = Vec::new();
            dock.names(&mut names);
            let panes = names
                .iter()
                .map(|channel| self.make_pane(id, channel, window, cx))
                .collect();
            self.tabs.push(WorkspaceTab {
                id,
                name: unique,
                panes,
                vertical: false,
                sizes: vec![],
                split: cx.new(|_| ResizableState::default()),
                dock: Some(dock),
                dock_states: BTreeMap::new(),
                locked: false,
                pinned: Vec::new(),
            });
        }
        {
            let mut rules = self.highlight_rules.borrow_mut();
            let before = rules.len();
            for rule in plan.highlights {
                if rules
                    .iter()
                    .any(|r| r.pattern == rule.pattern && r.author == rule.author)
                {
                    continue;
                }
                if rules.len() >= 24 {
                    notes.push("Highlight rules are full (24); some were not added".to_string());
                    break;
                }
                rules.push(rule);
            }
            if rules.len() != before {
                notes.push(format!("{} highlight rules", rules.len() - before));
            }
        }
        {
            let mut commands = self.custom_commands.borrow_mut();
            for (name, template) in plan.commands {
                if commands.len() >= 32 && !commands.contains_key(&name) {
                    notes.push("Custom commands are full (32); some were not added".into());
                    break;
                }
                commands.entry(name).or_insert(template);
            }
        }
        {
            let mut ignores = self.ignores.borrow_mut();
            for phrase in plan.ignored_phrases {
                if let Err(e) = ignores.add_phrase(phrase) {
                    notes.push(e);
                    break;
                }
            }
        }
        if let Some(mode) = plan.timestamps {
            self.change_timestamps(mode, cx);
        }
        if let Some(size) = plan.font_size {
            self.change_font(size - self.font_size, cx);
        }
        if let Some(limit) = plan.history_limit {
            self.change_history_limit(limit, cx);
        }
        if !plan.nicknames.is_empty() {
            match self.account.read(cx).identity() {
                Some(identity) => {
                    self.pending_nicknames = Some(crate::chatterino_import::resolve_nicknames(
                        identity,
                        plan.nicknames,
                    ));
                }
                None => notes.push("Sign in to import nicknames".into()),
            }
        }
        if first_new < self.tabs.len() {
            self.select_tab(first_new, cx);
        }
        let summary = if notes.is_empty() {
            "Imported from Chatterino".into()
        } else {
            format!("Imported from Chatterino · {}", notes.join(" · "))
        };
        self.notice = Some((summary, std::time::Instant::now()));
        self.schedule_save(cx);
        cx.notify();
    }
    /// Apply nicknames once their logins resolve to stable Twitch user IDs.
    fn pump_imported_nicknames(&mut self, cx: &mut Context<Self>) {
        let Some(rx) = &self.pending_nicknames else {
            return;
        };
        let Ok(resolved) = rx.try_recv() else {
            return;
        };
        self.pending_nicknames = None;
        {
            let mut preferences = self.chatter_appearance.borrow_mut();
            for (id, login, name, alias) in resolved {
                if preferences.len() >= 256 && !preferences.contains_key(&id) {
                    break;
                }
                let entry = preferences.entry(id).or_default();
                if entry.alias.is_empty() {
                    entry.alias = alias;
                }
                entry.login = login;
                entry.name = name;
            }
        }
        for pane in self.panes() {
            pane.update(cx, |p, cx| {
                crate::chatter_appearance::restyle(
                    &self.chatter_appearance,
                    &mut p.timeline.borrow_mut(),
                );
                p.scroller.update(cx, |s, cx| s.remeasure(cx));
                cx.notify();
            });
        }
        self.schedule_save(cx);
    }
    fn open_ignores(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = false;
        let editor = if let Some(editor) = &self.ignores_editor {
            editor.update(cx, |e, cx| e.reset(cx));
            editor.clone()
        } else {
            let editor = cx.new(|cx| crate::ignores::Editor::new(self.ignores.clone(), window, cx));
            cx.subscribe(&editor, |this, _, _: &crate::ignores::Changed, cx| {
                this.schedule_save(cx);
                cx.notify();
            })
            .detach();
            self.ignores_editor = Some(editor.clone());
            editor
        };
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 500.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Ignores")
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(editor.clone())
        });
    }
    fn open_event_highlights(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = false;
        let editor =
            cx.new(|_| crate::event_highlights::Editor::new(self.event_highlights.clone()));
        cx.subscribe(
            &editor,
            |this, _, _: &crate::event_highlights::Changed, cx| {
                this.schedule_save(cx);
                cx.notify();
            },
        )
        .detach();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 500.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Event highlights")
                .child(editor.clone())
        });
    }
    fn open_custom_commands(
        &mut self,
        origin: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let channel = origin
            .or_else(|| self.selected_channel.clone())
            .or_else(|| {
                self.visible_panes(cx)
                    .first()
                    .map(|p| p.read(cx).name.to_string())
            })
            .unwrap_or_else(|| "channel".into());
        let editor = cx.new(|cx| {
            crate::custom_commands::Editor::new(self.custom_commands.clone(), channel, window, cx)
        });
        cx.subscribe(
            &editor,
            |this, _, _: &crate::custom_commands::Changed, cx| {
                for pane in this
                    .panes()
                    .iter()
                    .chain(this.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
                {
                    pane.update(cx, |p, cx| {
                        p.complete_query(false, cx);
                        cx.notify();
                    });
                }
                this.schedule_save(cx);
                cx.notify();
            },
        )
        .detach();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(180., 520.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title("Custom commands")
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(editor.clone())
        });
    }
    fn make_pane(
        &mut self,
        tab_id: u64,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ChannelPane> {
        let key = format!("{tab_id}:{name}");
        let draft = self.drafts.get(&key).cloned().unwrap_or_default();
        let saved_filter = self.view_filters.get(&key).cloned();
        let shared_state = self
            .panes()
            .iter()
            .find(|p| p.read(cx).name.as_ref() == name)
            .map(|p| {
                let p = p.read(cx);
                (p.connected, p.connection.clone(), p.room_settings.clone())
            });
        let pane = cx.new(|cx| {
            ChannelPane::new(
                name,
                self.media.clone(),
                self.catalog.clone(),
                &draft,
                self.font_size,
                self.history_limit,
                window,
                cx,
            )
        });
        if let Some(title) = crate::combined::special_title(name) {
            pane.update(cx, |p, _| {
                p.display_title = Some(title.into());
                p.connected = true;
                p.connection = if name == crate::combined::MENTIONS {
                    "Mentions, replies and highlights from all your open channels collect here."
                        .into()
                } else if name == crate::combined::LIVE {
                    "Channels you follow or have open show up here when they go live.".into()
                } else {
                    "Whispers you receive collect here. Type /w name message to whisper.".into()
                };
                // It only shows copies; the source channel already pinged.
                p.quiet_pings = true;
            });
        }
        pane.update(cx, |p, _| {
            p.timestamps = self.timestamps;
            p.pause_mode = self.pause_mode;
            p.custom_commands = self.custom_commands.clone();
            p.chatter_appearance = self.chatter_appearance.clone();
            p.ignores = self.ignores.clone();
        });
        if let Some((connected, connection, room)) = shared_state {
            pane.update(cx, |p, _| {
                p.connected = connected;
                p.connection = connection;
                p.room_settings = room;
            });
        }
        if let Some(query) = saved_filter {
            pane.update(cx, |p, cx| {
                let _ = p.set_view_filter(&query, cx);
            });
        }
        if let Some(hidden) = self.platform_filters.get(&key) {
            let hidden: Vec<_> = hidden
                .iter()
                .filter_map(|k| crate::combined::Platform::from_key(k))
                .collect();
            pane.update(cx, |p, _| p.hidden_platforms = hidden);
        }
        if let Some(target) = self
            .reply_drafts
            .get(&key)
            .and_then(crate::replies::Target::from_json)
        {
            pane.update(cx, |p, _| p.reply_target = Some(target));
        }
        let channel = name.to_owned();
        cx.subscribe_in(&pane, window, move |this, pane, event, window, cx| {
            if let PaneEvent::Send {
                request,
                text,
                reply_parent,
            } = event
            {
                let destination = this.destination(pane, reply_parent.as_deref(), &channel, cx);
                if crate::youtube::is_youtube(&destination) {
                    this.send_youtube(
                        pane.clone(),
                        destination,
                        *request,
                        text.clone(),
                        window,
                        cx,
                    );
                    return;
                }
                let as_identity = this.send_as_identity(&destination, cx);
                if let Err(error) = this.live.send(
                    *request,
                    destination,
                    text.clone(),
                    reply_parent.clone(),
                    as_identity,
                ) {
                    pane.update(cx, |p, cx| {
                        p.pending = None;
                        p.send_status = error;
                        cx.notify();
                    });
                }
                return;
            }
            if let PaneEvent::WantFollowAge(user_id) = event {
                let Some(identity) = this.account.read(cx).identity() else {
                    return;
                };
                // Your own channel: the subscription too (Twitch allows only the broadcaster).
                if this.account.read(cx).login().as_deref() == Some(channel.as_str())
                    && identity.has("channel:read:subscriptions")
                {
                    let (identity, user_id, pane) =
                        (identity.clone(), user_id.clone(), pane.clone());
                    let task = cx.background_spawn({
                        let user_id = user_id.clone();
                        async move { crate::twitch_actions::subscription(&identity, &user_id) }
                    });
                    cx.spawn(async move |_, cx| {
                        if let Ok(sub) = task.await {
                            let _ = pane.update(cx, |p, cx| {
                                p.sub_status.insert(user_id, sub);
                                cx.notify();
                            });
                        }
                    })
                    .detach();
                }
                let (channel, user_id, pane) = (channel.clone(), user_id.clone(), pane.clone());
                let task = cx.background_spawn({
                    let user_id = user_id.clone();
                    async move { crate::twitch_actions::followed_at(&identity, &channel, &user_id) }
                });
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = pane.update(cx, |p, cx| {
                        p.follow_ages.insert(user_id, result);
                        cx.notify();
                    });
                })
                .detach();
                return;
            }
            if let PaneEvent::EditChatterNote(target) = event {
                this.open_chatter_note(target.clone(), window, cx);
                return;
            }
            if let PaneEvent::EditChatter(target) = event {
                this.open_chatter_appearance(Some(target.clone()), window, cx);
                return;
            }
            if let PaneEvent::OpenCommands(channel) = event {
                this.open_custom_commands(Some(channel.clone()), window, cx);
                return;
            }
            if let PaneEvent::SearchAll(query) = event {
                this.open_workspace_search(Some(query.clone()), window, cx);
                return;
            }
            if matches!(event, PaneEvent::RequestClose) && this.is_popout(pane) {
                this.close_popout(pane, cx);
                return;
            }
            if matches!(event, PaneEvent::RequestClose) {
                this.confirm_close_channel(channel.clone(), window, cx);
                return;
            }
            if let PaneEvent::YouTubeModerate(source, action) = event {
                this.moderate_youtube(pane.clone(), source.clone(), action.clone(), window, cx);
                return;
            }
            if let PaneEvent::TwitchCommand { request, draft } = event {
                let destination = if crate::combined::is_special(&channel) {
                    this.account.read(cx).login().unwrap_or_default()
                } else {
                    this.destination(pane, None, &channel, cx)
                };
                this.run_twitch_command(
                    pane.clone(),
                    destination,
                    request.clone(),
                    draft.clone(),
                    window,
                    cx,
                );
                return;
            }
            if let PaneEvent::RunBotAction { action, args } = event {
                this.run_bot_action(action.clone(), args.clone(), cx);
                return;
            }
            if matches!(event, PaneEvent::ToggleModButtons) {
                this.mod_buttons = !this.mod_buttons;
                this.schedule_save(cx);
                cx.notify();
                return;
            }
            if matches!(event, PaneEvent::SetUpYouTube) {
                this.connect_youtube(window, cx);
                return;
            }
            if matches!(event, PaneEvent::PopOut) {
                this.open_popout(pane.read(cx).name.to_string(), cx);
                return;
            }
            if let PaneEvent::OpenChannel(target) = event {
                this.open_channel(target.clone(), window, cx);
                return;
            }
            // Live tab lines aren't messages in that chat: open the channel.
            if let PaneEvent::GoToMessage {
                channel: target, ..
            } = event
                && channel == crate::combined::LIVE
            {
                this.open_channel(target.clone(), window, cx);
                return;
            }
            if let PaneEvent::GoToMessage { channel, id, reply } = event {
                if let Some(target) = this.jump_activity(channel, id, window, cx)
                    && *reply
                {
                    target.update(cx, |p, cx| {
                        p.message_action(id, crate::message_actions::Action::Reply, window, cx)
                    });
                }
                return;
            }
            if let PaneEvent::OpenWhisper(login) = event {
                this.open_whispers(Some(login.clone()), window, cx);
                return;
            }
            if matches!(event, PaneEvent::IgnoresChanged) {
                for pane in this.panes() {
                    pane.update(cx, |p, cx| {
                        p.scroller.update(cx, |s, cx| s.remeasure(cx));
                        cx.notify();
                    });
                }
                this.schedule_save(cx);
                return;
            }
            if matches!(event, PaneEvent::ViewFilterChanged) {
                this.schedule_save(cx);
                return;
            }
            if let PaneEvent::Ping(_, id) | PaneEvent::SelfMention(id) = event {
                if let PaneEvent::Ping(ping, _) = event
                    && !this.muted_pings.contains(&channel)
                {
                    this.ping(*ping, id, window);
                }
                // Mentions tabs get a copy, tagged with the channel it came from.
                let found = if crate::combined::is_special(&channel) {
                    None
                } else {
                    let p = pane.read(cx);
                    let timeline = p.timeline.borrow();
                    timeline.messages().iter().find(|m| &m.id == id).cloned()
                };
                if let Some(mut message) = found {
                    if message.presentation.origin.is_none() {
                        message.presentation.origin = Some(message.channel_id.clone());
                    }
                    message.channel_id = crate::combined::MENTIONS.into();
                    for target in this.panes() {
                        if target.read(cx).name.as_ref() == crate::combined::MENTIONS {
                            let copy = message.clone();
                            target.update(cx, |p, cx| {
                                p.received(chat_core::Event::Message(copy), cx)
                            });
                        }
                    }
                }
                return;
            }
            if let PaneEvent::DismissAutoMod(id) = event {
                if let Some(queue) = this.automod.get_mut(channel.as_str()) {
                    std::rc::Rc::make_mut(queue).retain(|h| h.id != *id);
                    let queue = queue.clone();
                    this.share_automod(&channel, queue, cx);
                }
                return;
            }
            if matches!(event, PaneEvent::HideAutoMod) {
                this.automod_hidden.insert(channel.clone());
                this.automod.remove(channel.as_str());
                this.share_automod(&channel, Default::default(), cx);
                this.notice = Some((
                    format!("AutoMod hidden in #{channel}. Settings > Chat lists hidden channels."),
                    std::time::Instant::now(),
                ));
                this.schedule_save(cx);
                cx.notify();
                return;
            }
            if let PaneEvent::AutoMod(id, allow) = event {
                this.resolve_automod(channel.clone(), id.clone(), *allow, window, cx);
                return;
            }
            if let PaneEvent::SendAs(vault) = event {
                if this.account.read(cx).vault() == vault {
                    this.send_as.remove(channel.as_str());
                } else {
                    this.send_as.insert(channel.clone(), vault.clone());
                }
                this.schedule_save(cx);
                cx.notify();
                return;
            }
            if matches!(event, PaneEvent::OpenModTools) {
                this.open_mod_tools(channel.clone(), window, cx);
                return;
            }
            if matches!(event, PaneEvent::OpenStreamInfo) {
                this.open_stream_info(channel.clone(), window, cx);
                return;
            }
            if matches!(event, PaneEvent::OpenAccount) {
                this.open_accounts(window, cx);
                return;
            }
            if matches!(
                event,
                PaneEvent::EmotePreferencesChanged | PaneEvent::EmoteRecentsChanged
            ) {
                for pane in this
                    .panes()
                    .iter()
                    .chain(this.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
                {
                    pane.update(cx, |p, cx| {
                        if p.picker.open {
                            p.refresh_picker(cx);
                        }
                        cx.notify();
                    });
                }
                if matches!(event, PaneEvent::EmotePreferencesChanged) {
                    this.schedule_save(cx);
                }
                return;
            }
            if matches!(event, PaneEvent::DragStarted) {
                this.drop_edge = None;
                cx.notify();
                return;
            }
            let text = pane.read(cx).draft.read(cx).value().to_string();
            let owner = this
                .tabs
                .iter()
                .chain(this.closed_tabs.iter())
                .find(|t| t.panes.iter().any(|p| p == pane))
                .map(|t| t.id);
            let Some(owner) = owner else {
                return;
            };
            this.drafts.insert(format!("{owner}:{channel}"), text);
            let key = format!("{owner}:{channel}");
            if let Some(target) = &pane.read(cx).reply_target {
                this.reply_drafts.insert(key, target.json());
            } else {
                this.reply_drafts.remove(&key);
            }
            if matches!(event, PaneEvent::Close) {
                this.focus.focus(window, cx);
                if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == owner)
                    && let Some(ix) = tab.panes.iter().position(|p| p == pane)
                {
                    tab.panes.remove(ix);
                    tab.dock = tab.dock.take().and_then(|d| d.remove(&channel));
                    tab.dock_states.clear();
                }
            }
            this.schedule_save(cx);
            cx.notify();
        })
        .detach();
        pane
    }
    /// Show the channel: switch to a workspace that has it and select its tab.
    pub(super) fn reveal_channel(&mut self, channel: &str, cx: &mut Context<Self>) {
        let has = |t: &WorkspaceTab| t.panes.iter().any(|p| p.read(cx).name.as_ref() == channel);
        let index = if has(&self.tabs[self.active]) {
            Some(self.active)
        } else {
            self.tabs.iter().position(has)
        };
        let Some(index) = index else { return };
        self.select_tab(index, cx);
        if let Some(dock) = &mut self.tabs[index].dock {
            dock.select(channel);
        }
        self.selected_channel = Some(channel.to_owned());
        self.schedule_save(cx);
        cx.notify();
    }
    fn pump_live_alerts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.live_seeded && self.follows_live.list.is_some() {
            self.live_seeded = true;
            self.seed_live(cx);
        }
        // Followed channels that aren't open as tabs (open ones are watched below,
        // faster): alert if you follow-alert everyone or picked this channel.
        let open: std::collections::HashSet<String> = self
            .tab_panes()
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .collect();
        for live in self.follows_live.take_went_live() {
            let detail = [live.title.as_str(), live.game.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            if !open.contains(&live.login) {
                self.post_live(
                    &live.login,
                    &format!("{} went live", live.label()),
                    &detail,
                    cx,
                );
            }
            if open.contains(&live.login)
                || !(self.live_notify_follows || self.live_notify.contains(&live.login))
                || crate::streamer_mode::hides(crate::streamer_mode::LIVE_ALERTS)
            {
                continue;
            }
            let owner = cx.entity().downgrade();
            let target = live.login.clone();
            let detail = [live.title.as_str(), live.game.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            window.push_notification(
                Notification::new()
                    .title(format!("{} is live", live.label()))
                    .message(detail)
                    .in_app_and_system()
                    .on_click(move |_, window, cx| {
                        let _ = owner
                            .update(cx, |this, cx| this.open_channel(target.clone(), window, cx));
                    }),
                cx,
            );
        }
        for (channel, info) in self.streams.take_went_live() {
            let detail = [info.title.as_str(), info.category.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            self.post_live(&channel, &format!("{channel} went live"), &detail, cx);
            if !self.live_notify_all && !self.live_notify.contains(&channel) {
                continue;
            }
            if crate::streamer_mode::hides(crate::streamer_mode::LIVE_ALERTS) {
                continue;
            }
            let owner = cx.entity().downgrade();
            let target = channel.clone();
            let detail = [info.title.as_str(), info.category.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            window.push_notification(
                Notification::new()
                    .title(format!("{channel} is live"))
                    .message(detail)
                    .in_app_and_system()
                    .on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |this, cx| this.reveal_channel(&target, cx));
                    }),
                cx,
            );
        }
    }
    /// Live tabs: one line per channel going live, tagged with the channel.
    fn post_live(&mut self, login: &str, label: &str, detail: &str, cx: &mut Context<Self>) {
        let targets: Vec<_> = self
            .panes()
            .into_iter()
            .filter(|p| p.read(cx).name.as_ref() == crate::combined::LIVE)
            .collect();
        if targets.is_empty() {
            return;
        }
        let text = if detail.is_empty() {
            label.to_owned()
        } else {
            format!("{label} · {detail}")
        };
        let id = format!("system:live:{}", self.next_notice);
        self.next_notice += 1;
        let mut notice = chat_core::system_notice(
            crate::combined::LIVE,
            id,
            Some(chrono::Utc::now().timestamp()),
            text,
        );
        notice.presentation.origin = Some(login.to_owned());
        for pane in targets {
            let copy = notice.clone();
            pane.update(cx, |p, cx| p.received(chat_core::Event::Message(copy), cx));
        }
    }
    /// A new Live tab starts with who's live right now.
    pub(super) fn seed_live(&mut self, cx: &mut Context<Self>) {
        let Some(list) = self.follows_live.list.clone() else {
            return;
        };
        for live in list.iter().rev() {
            let detail = [live.title.as_str(), live.game.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let label = live.label();
            self.post_live(&live.login, &format!("{label} is live"), &detail, cx);
        }
    }
    pub(super) fn toggle_live_notify(&mut self, channel: &str, cx: &mut Context<Self>) {
        if let Some(at) = self.live_notify.iter().position(|c| c == channel) {
            self.live_notify.remove(at);
        } else {
            self.live_notify.push(channel.to_owned());
        }
        self.schedule_save(cx);
        cx.notify();
    }
    /// Request recent history once per opened channel (visible ones first) and
    /// fill panes that have not received anything yet.
    fn pump_recent_messages(&mut self, cx: &mut Context<Self>) {
        let open: Vec<String> = self
            .panes()
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .collect();
        self.recent_requested.retain(|c| open.contains(c));
        if self.recent_enabled {
            for channel in self.connection_priority(cx) {
                if self.recent_requested.insert(channel.clone()) {
                    self.recent.request(&channel);
                }
            }
        }
        while let Some((channel, result)) = self.recent.poll() {
            let Ok(messages) = result else { continue };
            for pane in &self.panes() {
                if pane.read(cx).name.as_ref() != channel
                    || !pane.read(cx).timeline.borrow().messages().is_empty()
                {
                    continue;
                }
                pane.update(cx, |p, cx| {
                    for message in &messages {
                        p.received(chat_core::Event::Message(message.clone()), cx);
                    }
                    p.scroller.update(cx, |s, cx| s.scroll_to_end(cx));
                });
            }
        }
    }
    /// Download the published installer, or start it once verified.
    fn update_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.updates.ready() {
            match self.updates.install() {
                // The installer waits for Jawjack to close; save first.
                Ok(_) => {
                    let _ = storage::save(&self.snapshot(cx));
                    cx.quit();
                }
                Err(e) => window.push_notification(Notification::info(e), cx),
            }
        } else {
            self.updates.start_download();
        }
        cx.notify();
    }
    fn render_updates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let available = self.updates.available();
        div()
            .v_flex()
            .gap_1()
            .child(
                div()
                    .h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(rgb(if available.is_some() {
                                theme::STREAM_LIVE
                            } else {
                                theme::MUTED
                            }))
                            .child(format!("v{}", self.updates.current_version())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(self.updates.status_label()),
                    )
                    .child(
                        Button::new("update-check")
                            .ghost()
                            .xsmall()
                            .icon(IconName::RefreshCw)
                            .tooltip(if self.updates.checking() {
                                "Checking for updates…"
                            } else {
                                "Check for updates"
                            })
                            .disabled(self.updates.busy())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.updates.check();
                                cx.notify();
                            })),
                    ),
            )
            .when_some(
                self.updates.install_label().filter(|_| available.is_some()),
                |el, label| {
                    el.child(
                        Button::new("update-install")
                            .small()
                            .when(self.updates.ready(), |b| b.primary())
                            .disabled(self.updates.busy())
                            .label(label)
                            .on_click(cx.listener(|this, _, w, cx| this.update_action(w, cx))),
                    )
                },
            )
            .when(available.is_some(), |el| {
                el.child(
                    Button::new("update-notes")
                        .ghost()
                        .xsmall()
                        .label("Release notes")
                        .on_click(|_, _, cx| cx.open_url(crate::updates::RELEASE_PAGE)),
                )
            })
    }
    /// Run a Twitch command after checking role and permission again.
    fn run_twitch_command(
        &mut self,
        pane: Entity<ChannelPane>,
        channel: String,
        request: crate::twitch_actions::Request,
        draft: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let check = self.capabilities.check(&channel, request.action());
        let identity = self.account.read(cx).identity();
        let (Some(identity), true) = (identity, check.available()) else {
            pane.update(cx, |p, cx| {
                p.command_feedback(
                    format!("{} Your draft is kept.", check.reason()),
                    window,
                    cx,
                )
            });
            return;
        };
        let blocking = matches!(request, crate::twitch_actions::Request::Block { .. });
        let task = cx.background_spawn(async move {
            crate::twitch_actions::run(&identity, &channel, &request)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            if blocking && result.is_ok() {
                let _ = view.update(cx, |this, _| this.blocks.refresh());
            }
            let _ = pane.update_in(cx, |p, window, cx| match result {
                Ok(done) => {
                    if p.draft.read(cx).value().as_ref() == draft {
                        p.command_draft("", window, cx);
                    }
                    p.command_feedback(done, window, cx);
                }
                Err(error) => {
                    p.command_feedback(format!("{error} Your draft is kept."), window, cx)
                }
            });
        })
        .detach();
    }
    /// Streamer tool: edit the own channel's title and category.
    fn open_stream_info(&mut self, channel: String, window: &mut Window, cx: &mut Context<Self>) {
        let check = self
            .capabilities
            .check(&channel, crate::capabilities::Action::EditStreamInfo);
        if !check.available() {
            window.push_notification(Notification::info(check.reason()), cx);
            // A missing grant is fixed in the account dialog, so take the owner there.
            if check == crate::capabilities::Check::NeedsPermission {
                self.open_accounts(window, cx);
            }
            return;
        }
        let (account, presets) = (self.account.clone(), self.stream_presets.clone());
        let editor = cx.new(|cx| crate::studio::StreamEditor::new(account, presets, window, cx));
        cx.subscribe(&editor, |this, _, _: &crate::studio::PresetsChanged, cx| {
            this.schedule_save(cx)
        })
        .detach();
        let width = (f32::from(window.viewport_size().width) - 24.).min(420.);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .w(px(width))
                .title(format!("Stream info · #{channel}"))
                // Enter belongs to the fields inside; it must not close the dialog.
                .on_ok(|_, _, _| false)
                .child(editor.clone())
        });
    }
    fn snapshot(&self, cx: &App) -> Value {
        let mut drafts = self.drafts.clone();
        let mut reply_drafts = self.reply_drafts.clone();
        for tab in self.tabs.iter().chain(self.closed_tabs.iter()) {
            for pane in &tab.panes {
                let pane = pane.read(cx);
                let key = format!("{}:{}", tab.id, pane.name);
                if let Some(target) = &pane.reply_target {
                    reply_drafts.insert(key, target.json());
                } else {
                    reply_drafts.remove(&key);
                }
                drafts.insert(
                    format!("{}:{}", tab.id, pane.name),
                    pane.draft.read(cx).value().to_string(),
                );
            }
        }
        let mut value = json!({"version":1,"chatter_notes":crate::chatter_notes::json(&self.chatter_notes),"chatter_appearance":crate::chatter_appearance::json(&self.chatter_appearance),"highlight_rules":crate::highlight_rules::json(&self.highlight_rules),"ignores":crate::ignores::json(&self.ignores),"event_highlights":crate::event_highlights::json(&self.event_highlights),"custom_commands":&*self.custom_commands.borrow(),"emote_favorites":self.catalog.borrow().favorites_json(),"next_id":self.next_id,"active":self.active,"sidebar":self.sidebar,"update_checks":self.updates.enabled,"experience":self.experience.key(),"streamer_bot":self.streamer_bot_json(),"studio_open":self.studio_open,"studio_page":self.studio_tab,"setup_offered":self.setup_offered,"recent_messages":self.recent_enabled,"live_notify":self.live_notify,"live_notify_all":self.live_notify_all,"live_notify_follows":self.live_notify_follows,"muted_pings":self.muted_pings,"seventv_activity":self.presence.enabled,"font_size":self.font_size,"timestamps":self.timestamps,"pause_mode":self.pause_mode,"history_limit":self.history_limit,"live_workspaces":self.live_workspaces,"live_channels":self.live_channels,"drafts":drafts,"reply_drafts":reply_drafts,"highlight_words":self.highlight_terms.join(", "),
            "tabs":self.tabs.iter().map(|t|json!({"id":t.id,"name":t.name,"vertical":t.vertical,"sizes":t.sizes,"dock":t.dock.as_ref().map(Dock::json),"pinned":t.pinned,"channels":t.panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>()});
        // Kept outside the macro above, which is at its recursion limit.
        let accounts = self.accounts_json(cx);
        value["accounts"] = accounts["accounts"].clone();
        value["active_account"] = accounts["active_account"].clone();
        value["send_as"] = json!(self.send_as);
        value["automod_hidden"] = json!(self.automod_hidden);
        value["popouts"] = json!(self.popout_channels(cx));
        value["hide_blocked"] = json!(self.hide_blocked);
        value["similar_messages"] = json!(self.similar_mode.key());
        value["spellcheck"] = json!(self.spellcheck);
        value["link_previews"] = json!(crate::link_preview::enabled());
        for (index, tab) in self.tabs.iter().enumerate() {
            value["tabs"][index]["locked"] = json!(tab.locked);
        }
        value["saved_filters"] = crate::chat_search::saved_json();
        value["header"] = json!(
            (0..crate::theme::HEADER.len())
                .map(crate::theme::header_shows)
                .collect::<Vec<_>>()
        );
        value["animate"] = json!(crate::media::ANIMATE.load(std::sync::atomic::Ordering::Relaxed));
        value["badges"] = json!(
            (0..crate::theme::BADGES.len())
                .map(crate::theme::badge_shown)
                .collect::<Vec<_>>()
        );
        value["shared_chat_look"] = json!(crate::shared_chat::style());
        value["on_top"] = json!(self.on_top);
        value["name_mode"] = json!(chat_core::NAME_MODE.load(std::sync::atomic::Ordering::Relaxed));
        value["separators"] = json!(crate::theme::flag(&crate::theme::SEPARATORS));
        value["hide_deleted"] = json!(crate::theme::flag(&crate::theme::HIDE_DELETED));
        value["chat_font"] = json!(crate::theme::chat_font());
        value["pronouns"] = json!(crate::pronouns::enabled());
        value["hotkeys"] = crate::hotkeys::json();
        value["emote_scale"] =
            json!(crate::inline_chat::EMOTE_SCALE.load(std::sync::atomic::Ordering::Relaxed));
        value["alternate_rows"] = json!(crate::theme::alternate_rows());
        let mut options = serde_json::Map::new();
        for (index, name) in crate::inline_chat::EMOTE_OPTION_NAMES.iter().enumerate() {
            options.insert(
                (*name).into(),
                json!(crate::inline_chat::emote_option(index)),
            );
        }
        options.insert(
            "unlisted_7tv".into(),
            json!(crate::catalog::SHOW_UNLISTED.load(std::sync::atomic::Ordering::Relaxed)),
        );
        value["emote_options"] = Value::Object(options);
        let mut inputs = serde_json::Map::new();
        for (index, name) in crate::replies::OPTION_NAMES.iter().enumerate() {
            inputs.insert((*name).into(), json!(crate::replies::option(index)));
        }
        value["input_options"] = Value::Object(inputs);
        value["mentions"] = json!({
            "color": crate::mention_names::COLOR_MENTIONS.load(std::sync::atomic::Ordering::Relaxed),
            "find_names": crate::mention_names::FIND_NAMES.load(std::sync::atomic::Ordering::Relaxed),
        });
        value["emote_providers"] = json!({
            "7tv": crate::catalog::on(&crate::catalog::SEVEN_TV),
            "bttv": crate::catalog::on(&crate::catalog::BTTV),
            "ffz": crate::catalog::on(&crate::catalog::FFZ),
        });
        let mut filters = serde_json::Map::new();
        for tab in &self.tabs {
            for pane in &tab.panes {
                if let Some((query, _)) = &pane.read(cx).view_filter {
                    filters.insert(format!("{}:{}", tab.id, pane.read(cx).name), json!(query));
                }
            }
        }
        value["view_filters"] = Value::Object(filters);
        let mut platforms = serde_json::Map::new();
        for tab in &self.tabs {
            for pane in &tab.panes {
                let hidden = &pane.read(cx).hidden_platforms;
                if !hidden.is_empty() {
                    platforms.insert(
                        format!("{}:{}", tab.id, pane.read(cx).name),
                        json!(hidden.iter().map(|p| p.key()).collect::<Vec<_>>()),
                    );
                }
            }
        }
        value["platform_filters"] = Value::Object(platforms);
        value["mod_buttons"] = json!(self.mod_buttons);
        value["custom_mod_buttons"] = crate::mod_buttons::json(&self.custom_mod_buttons);
        value["follows_open"] = json!(self.follows_open);
        value["live_sort"] = json!(self.live_sort.key());
        value["raid_sort"] = json!(self.raid_sort.key());
        value["stream_presets"] = crate::studio::presets_json(&self.stream_presets);
        value["obs"] = self.obs_json();
        value["checklist"] = json!(self.checklist_items);
        value["logs"] =
            json!({"enabled": self.logs_enabled, "folder": self.logs_folder.display().to_string()});
        value["streamer_mode"] = json!({"mode": self.streamer_mode.key(), "mute": self.streamer_mute, "hide": crate::streamer_mode::OPTIONS.iter().enumerate().map(|(index, (key, ..))| (key.to_string(), json!(crate::streamer_mode::HIDE[index].load(std::sync::atomic::Ordering::Relaxed)))).collect::<serde_json::Map<_, _>>()});
        value["sound"] = json!({
            "mentions": self.sound_mentions,
            "highlights": self.sound_highlights,
            "focused": self.sound_focused,
            "flash": self.flash_taskbar,
            "tone": self.sound_tone.key(),
            "volume": self.sound_volume,
        });
        value
    }
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_revision += 1;
        if !self.save_enabled {
            return;
        }
        let revision = self.save_revision;
        self.save_status = "Saving…".into();
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(350))
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.save_revision == revision {
                    this.save_status = match storage::save(&this.snapshot(cx)) {
                        Ok(()) => "Workspace saved locally".into(),
                        Err(e) => format!("Could not save: {e}"),
                    };
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let result = if self.save_enabled {
            storage::save(&self.snapshot(cx))
        } else if self.save_revision == 0 {
            Ok(())
        } else {
            Err(self.save_status.clone())
        };
        match result {
            Ok(()) => {
                // The main window is the app: popouts (already saved for next launch) close with it.
                if !self.popouts.is_empty() {
                    cx.defer(|cx| cx.quit());
                }
                true
            }
            Err(error) => {
                self.close_failed = true;
                self.save_status = format!("Save failed: {error}");
                window.push_notification(Notification::error("Workspace could not be saved. Window kept open. Workspace menu has Discard unsaved changes and quit."),cx);
                cx.notify();
                false
            }
        }
    }
    fn shown_tabs(&self, cx: &App) -> Vec<usize> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(ix, t)| {
                !self.live_workspaces
                    || *ix == self.active
                    || t.panes.is_empty()
                    || t.panes
                        .iter()
                        .any(|p| self.streams.get(p.read(cx).name.as_ref()) != Some(false))
            })
            .map(|(ix, _)| ix)
            .collect()
    }
    fn cycle_tab(&mut self, forward: bool, cx: &mut Context<Self>) {
        let shown = self.shown_tabs(cx);
        let ix = shown.iter().position(|i| *i == self.active).unwrap_or(0);
        let next = if forward {
            (ix + 1) % shown.len()
        } else {
            (ix + shown.len() - 1) % shown.len()
        };
        self.select_tab(shown[next], cx);
    }
    fn select_tab_id(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(ix) = self.tabs.iter().position(|t| t.id == id) {
            self.select_tab(ix, cx);
        }
    }
    fn select_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.tabs.len() {
            self.active = ix;
            let id = self.tabs[ix].id;
            self.recent_workspaces.retain(|t| *t != id);
            self.recent_workspaces.insert(0, id);
            self.adding = false;
            self.renaming = false;
            self.schedule_save(cx);
            cx.notify();
        }
    }
    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(WorkspaceTab {
            id,
            name: format!("Workspace {id}"),
            panes: vec![],
            vertical: false,
            sizes: vec![],
            split: cx.new(|_| ResizableState::default()),
            dock: None,
            dock_states: BTreeMap::new(),
            locked: false,
            pinned: Vec::new(),
        });
        self.select_tab(self.tabs.len() - 1, cx);
        self.open_add(false, window, cx);
    }
    /// A copy of a workspace: same channels, layout and pins, as a new workspace.
    fn duplicate_workspace(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let new_id = self.next_id;
        self.next_id += 1;
        let names: Vec<String> = self.tabs[source]
            .panes
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .collect();
        let panes: Vec<Entity<ChannelPane>> = names
            .iter()
            .map(|name| self.make_pane(new_id, name, window, cx))
            .collect();
        // Bring the current chat along (found by name before the copy exists).
        for pane in &panes {
            self.seed_popout(pane, cx);
        }
        let copy = WorkspaceTab {
            id: new_id,
            name: format!("{} copy", self.tabs[source].name),
            panes,
            vertical: self.tabs[source].vertical,
            sizes: self.tabs[source].sizes.clone(),
            split: cx.new(|_| ResizableState::default()),
            dock: self.tabs[source].dock.clone(),
            dock_states: BTreeMap::new(),
            locked: false,
            pinned: self.tabs[source].pinned.clone(),
        };
        self.tabs.insert(source + 1, copy);
        self.select_tab(source + 1, cx);
        self.schedule_save(cx);
        cx.notify();
    }
    fn confirm_close_tab(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_tab_pending.is_some() {
            return;
        }
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let title = format!("Close {}?", tab.name);
        let channels = tab
            .panes
            .iter()
            .map(|p| format!("#{}", crate::combined::title(&p.read(cx).name)))
            .collect::<Vec<_>>()
            .join(", ");
        let body = if channels.is_empty() {
            "This workspace has no channels.".to_string()
        } else {
            format!("Channels: {channels}")
        };
        self.close_tab_pending = Some(id);
        let owner = cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,_,_|{
            let cancel_owner=owner.clone(); let close_owner=owner.clone(); let dismiss_owner=owner.clone();
            dialog.title(title.clone()).close_button(false)
                .child(div().v_flex().gap_3().child(body.clone()).child("Drafts are kept. Reopen it with Ctrl+Shift+T (or Reopen closed workspace) during this session."))
                .on_ok(|_,_,_|true)
                .on_close(move|_,_,cx|{let _=dismiss_owner.update(cx,|this,cx|{this.close_tab_pending=None;cx.notify();});})
                .footer(div().h_flex().justify_end().gap_2()
                    .child(Button::new("cancel-tab-close").label("Cancel").on_click(move|_,window,cx|{let _=cancel_owner.update(cx,|this,cx|{this.close_tab_pending=None;cx.notify();});window.close_dialog(cx);}))
                    .child(Button::new("confirm-tab-close").danger().label("Close workspace").on_click(move|_,window,cx|{
                        let _=close_owner.update(cx,|this,cx|{if let Some(ix)=this.tabs.iter().position(|t|t.id==id){this.close_tab(ix,cx);}this.close_tab_pending=None;this.focus.focus(window,cx);});window.close_dialog(cx);
                    })))
        });
        cx.notify();
    }
    fn close_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        for pane in &self.tabs[ix].panes {
            let pane = pane.read(cx);
            self.drafts.insert(
                format!("{}:{}", self.tabs[ix].id, pane.name),
                pane.draft.read(cx).value().to_string(),
            );
        }
        self.closed_tabs.push(self.tabs.remove(ix));
        if self.closed_tabs.len() > 10 {
            self.closed_tabs.remove(0);
        }
        if self.tabs.is_empty() {
            let id = self.next_id;
            self.next_id += 1;
            self.tabs.push(WorkspaceTab {
                id,
                name: "Workspace".into(),
                panes: vec![],
                vertical: false,
                sizes: vec![],
                split: cx.new(|_| ResizableState::default()),
                dock: None,
                dock_states: BTreeMap::new(),
                locked: false,
                pinned: Vec::new(),
            });
        }
        self.active = if ix < self.active {
            self.active - 1
        } else {
            self.active.min(self.tabs.len() - 1)
        };
        self.adding = false;
        self.schedule_save(cx);
        cx.notify();
    }
    fn reopen_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.closed_tabs.pop() {
            // Closed workspaces can outlive account changes and catalog revocations.
            for pane in &tab.panes {
                pane.update(cx, |p, cx| {
                    if p.picker.open {
                        p.refresh_picker(cx);
                    }
                    cx.notify();
                });
            }
            self.tabs.push(tab);
            self.select_tab(self.tabs.len() - 1, cx);
        }
    }
    fn open_add(&mut self, rename: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.adding = true;
        self.renaming = rename;
        self.replacing = None;
        self.add_target = None;
        self.add_error = None;
        let value = if rename {
            self.tabs[self.active].name.clone()
        } else {
            String::new()
        };
        self.channel_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            input.focus(window, cx);
        });
        let tab_id = self.tabs[self.active].id;
        cx.on_next_frame(window, move |this, window, cx| {
            if this.adding && this.renaming == rename && this.tabs[this.active].id == tab_id {
                this.channel_input
                    .update(cx, |input, cx| input.focus(window, cx));
            }
        });
        cx.notify();
    }
    /// Where a pane's messages and commands go: the pane's channel, or for a
    /// combined feed the replied-to message's channel, else the chosen destination.
    fn destination(
        &self,
        pane: &Entity<ChannelPane>,
        reply_parent: Option<&str>,
        channel: &str,
        cx: &App,
    ) -> String {
        let p = pane.read(cx);
        if !crate::combined::is_combined(channel) && p.linked.is_empty() {
            return channel.to_owned();
        }
        reply_parent
            .and_then(|id| {
                p.timeline
                    .borrow()
                    .messages()
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| m.presentation.origin.clone())
            })
            .or_else(|| p.send_to.clone())
            .unwrap_or_else(|| crate::combined::sources(channel)[0].clone())
    }
    /// Shared Chat: look sessions up for channels showing messages from other
    /// channels, keep them fresh, and load the participants' names and avatars.
    /// Rich Twitch link cards use the signed-in session.
    fn sync_link_preview_identity(&self, cx: &App) {
        crate::link_preview::set_identity(self.account.read(cx).identity());
    }
    fn pump_shared_chat(
        &mut self,
        identity: Option<&crate::live::Identity>,
        cx: &mut Context<Self>,
    ) {
        if let Some(identity) = identity {
            for pane in self.panes() {
                let pane = pane.read(cx);
                let name = pane.name.to_string();
                if crate::combined::is_combined(&name) || crate::youtube::is_youtube(&name) {
                    continue;
                }
                let seen = pane
                    .timeline
                    .borrow()
                    .messages()
                    .iter()
                    .rev()
                    .take(30)
                    .any(|m| m.presentation.source_id.is_some());
                if let Some(id) = self.catalog.borrow().channel_id(&name) {
                    self.shared_chat.want(&name, &id, identity, seen);
                }
            }
        }
        if self.shared_chat.pump() {
            cx.notify();
        }
        for pane in self.panes() {
            let name = pane.read(cx).name.to_string();
            let session = self.shared_chat.get(&name).map(|s| s.participants.clone());
            let own = self.catalog.borrow().channel_id(&name);
            pane.update(cx, |p, cx| p.set_shared_session(session, own, cx));
        }
    }
    /// "+N" and the names for a channel's Shared Chat session, if it has one.
    fn shared_chat_label(&self, channel: &str) -> Option<(String, String)> {
        let session = self.shared_chat.get(channel)?;
        let own = self.catalog.borrow().channel_id(channel);
        let mut catalog = self.catalog.borrow_mut();
        let names: Vec<String> = session
            .participants
            .iter()
            .filter(|id| Some(*id) != own.as_ref())
            .map(|id| {
                catalog.profiles.request(id);
                catalog
                    .profiles
                    .get(id)
                    .map(|p| p.display_name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "…".into())
            })
            .collect();
        (!names.is_empty()).then(|| {
            (
                format!("+{}", names.len()),
                format!("Shared Chat with {}", names.join(", ")),
            )
        })
    }
    /// One chat for every channel and stream in a workspace (Twitch and YouTube),
    /// added as a tab there; platforms can then be shown or hidden in its header.
    fn combine_workspace(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.select_tab_id(id, cx);
        let mut sources: Vec<String> = Vec::new();
        for source in self.tabs[self.active]
            .panes
            .iter()
            .flat_map(|p| crate::combined::sources(&p.read(cx).name))
        {
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        if sources.len() < 2 {
            window.push_notification(
                Notification::info("Add at least two channels or streams to this workspace first."),
                cx,
            );
            return;
        }
        sources.truncate(8);
        let name = format!("+{}", sources.join(","));
        if !self.tabs[self.active]
            .panes
            .iter()
            .any(|p| p.read(cx).name.as_ref() == name)
        {
            self.add_channel_to_active(name.clone(), window, cx);
        }
        if let Some(dock) = self.tabs[self.active].dock.as_mut() {
            dock.select(&name);
        }
        self.schedule_save(cx);
        cx.notify();
    }
    /// Add a channel as a tab in the active workspace (Add channel, live follows).
    fn add_channel_to_active(
        &mut self,
        channel: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self.tabs[self.active].id;
        if channel == crate::combined::LIVE {
            self.live_seeded = false;
        }
        let pane = self.make_pane(id, &channel, window, cx);
        let tab = &mut self.tabs[self.active];
        tab.panes.push(pane);
        tab.dock = Some(match tab.dock.take() {
            Some(mut d) => {
                let mut active = Vec::new();
                d.active_names(&mut active);
                if let Some(target) = self.add_target.as_ref().or(active.first()) {
                    d.tabify(target, channel.clone());
                }
                d
            }
            None => Dock::Leaf(channel.clone()),
        });
        tab.dock_states.clear();
        tab.sizes.clear();
        tab.split = cx.new(|_| ResizableState::default());
        if tab.name.starts_with("Workspace") && tab.panes.len() == 1 {
            tab.name = channel;
        }
    }
    /// Jump to an open channel, or add it to the active workspace.
    pub(super) fn open_channel(
        &mut self,
        channel: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .tab_panes()
            .iter()
            .any(|p| p.read(cx).name.as_ref() == channel)
        {
            self.reveal_channel(&channel, cx);
        } else {
            self.add_channel_to_active(channel.clone(), window, cx);
            self.reveal_channel(&channel, cx);
        }
        self.focus.focus(window, cx);
        self.schedule_save(cx);
        cx.notify();
    }
    fn accept_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.channel_input.read(cx).value().trim().to_owned();
        if self.renaming {
            if value.is_empty() || value.chars().count() > 40 {
                self.add_error = Some("Use a workspace name of 1–40 characters".into());
                cx.notify();
                return;
            }
            self.tabs[self.active].name = value;
        } else if let Some(special) = [
            crate::combined::MENTIONS,
            crate::combined::WHISPERS,
            crate::combined::LIVE,
        ]
        .into_iter()
        .find(|s| value.eq_ignore_ascii_case(s))
        {
            if self.tabs[self.active]
                .panes
                .iter()
                .any(|p| p.read(cx).name.as_ref() == special)
            {
                self.add_error = Some("That tab is already in this workspace".into());
                cx.notify();
                return;
            }
            self.add_channel_to_active(special.to_owned(), window, cx);
        } else if let Some(video) = crate::youtube::parse(&value) {
            if self.tabs[self.active]
                .panes
                .iter()
                .any(|p| p.read(cx).name.as_ref() == video)
            {
                self.add_error = Some("This YouTube stream is already in the workspace".into());
                cx.notify();
                return;
            }
            self.add_channel_to_active(video, window, cx);
        } else if let Some(combined) = crate::combined::parse(&value, |s| {
            valid_channel(s).or_else(|| crate::youtube::parse(s))
        }) {
            if self.tabs[self.active]
                .panes
                .iter()
                .any(|p| p.read(cx).name.as_ref() == combined)
            {
                self.add_error = Some("This combined view is already in the workspace".into());
                cx.notify();
                return;
            }
            self.add_channel_to_active(combined, window, cx);
        } else {
            let Some(channel) = valid_channel(&value) else {
                self.add_error =
                    Some("Enter a Twitch channel name: letters, numbers and underscores".into());
                cx.notify();
                return;
            };
            if self.tabs[self.active]
                .panes
                .iter()
                .any(|p| p.read(cx).name.as_ref() == channel.as_str())
            {
                self.add_error = Some("This channel is already in the workspace".into());
                cx.notify();
                return;
            }
            self.add_channel_to_active(channel, window, cx);
        }
        // Change channel: the new tab takes the old one's place, then it closes.
        if let Some(old) = self.replacing.take()
            && let Some(new) = self.tabs[self.active]
                .panes
                .last()
                .map(|p| p.read(cx).name.to_string())
            && new != old
        {
            if let Some(dock) = &mut self.tabs[self.active].dock {
                dock.reorder_before(&new, Some(&old));
                dock.select(&new);
            }
            if let Some(pane) = self.tabs[self.active]
                .panes
                .iter()
                .find(|p| p.read(cx).name.as_ref() == old)
                .cloned()
            {
                pane.update(cx, |_, cx| cx.emit(PaneEvent::Close));
            }
        }
        self.adding = false;
        self.focus.focus(window, cx);
        self.schedule_save(cx);
        cx.notify();
    }
    fn move_tab(&mut self, direction: isize, cx: &mut Context<Self>) {
        let target = self.active as isize + direction;
        if target >= 0 && (target as usize) < self.tabs.len() {
            self.tabs.swap(self.active, target as usize);
            self.select_tab(target as usize, cx);
        }
    }
    fn reorder_tab(&mut self, source: u64, target: u64, cx: &mut Context<Self>) {
        if source == target {
            return;
        }
        let selected = self.tabs[self.active].id;
        let Some(from) = self.tabs.iter().position(|t| t.id == source) else {
            return;
        };
        let tab = self.tabs.remove(from);
        let to = self
            .tabs
            .iter()
            .position(|t| t.id == target)
            .unwrap_or(self.tabs.len());
        self.tabs.insert(to, tab);
        self.active = self.tabs.iter().position(|t| t.id == selected).unwrap_or(0);
        self.schedule_save(cx);
        cx.notify();
    }
    fn move_pane(
        &mut self,
        drag: &DraggedChannel,
        target: u64,
        edge: Option<DropEdge>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drop_edge = None;
        let Some(source) = self.tabs.iter().position(|t| t.panes.contains(&drag.pane)) else {
            return;
        };
        let Some(destination) = self.tabs.iter().position(|t| t.id == target) else {
            return;
        };
        if source != destination
            && self.tabs[destination]
                .panes
                .iter()
                .any(|p| p.read(cx).name.as_ref() == drag.name)
        {
            window.push_notification(
                Notification::info("This channel is already in the destination workspace"),
                cx,
            );
            return;
        }
        let mut target_channel = self.drop_target.take();
        if source == destination && target_channel.as_deref() == Some(drag.name.as_str()) {
            target_channel = self.tabs[source]
                .dock
                .as_ref()
                .and_then(|d| d.companion(&drag.name));
            if target_channel.is_none() {
                cx.notify();
                return;
            }
        }
        let from = self.tabs[source]
            .panes
            .iter()
            .position(|p| *p == drag.pane)
            .unwrap();
        let pane = self.tabs[source].panes.remove(from);
        self.tabs[source].dock = self.tabs[source]
            .dock
            .take()
            .and_then(|d| d.remove(&drag.name));
        self.tabs[source].dock_states.clear();
        let tab = &mut self.tabs[destination];
        tab.panes.push(pane);
        let docking_edge = edge;
        let edge = edge.unwrap_or(DropEdge::Right);
        tab.dock = Some(match tab.dock.take() {
            None => Dock::Leaf(drag.name.clone()),
            Some(mut dock) => {
                if !target_channel.as_ref().is_some_and(|target| {
                    if docking_edge.is_none() || docking_edge == Some(DropEdge::Center) {
                        dock.tabify(target, drag.name.clone())
                    } else {
                        dock.insert(target, drag.name.clone(), edge.vertical(), edge.first())
                    }
                }) {
                    dock = dock.append(drag.name.clone(), edge.vertical(), edge.first());
                }
                dock
            }
        });
        tab.dock_states.clear();
        self.active = destination;
        self.selected_channel = Some(drag.name.clone());
        self.focus.focus(window, cx);
        self.adding = false;
        self.schedule_save(cx);
        cx.notify();
    }
    fn drop_location(&self, position: Point<Pixels>) -> Option<(String, DropEdge, Option<String>)> {
        // Tab strips are dedicated merge/reorder targets, including wrapped rows.
        // The same resolver drives hover feedback and release behavior.
        if let Some((name, strip)) = self
            .strip_bounds
            .borrow()
            .iter()
            .find(|(_, b)| b.contains(&position))
        {
            let mut tabs = self
                .tab_bounds
                .borrow()
                .iter()
                .filter(|(_, b)| strip.contains(&b.origin))
                .map(|(n, b)| (n.clone(), *b))
                .collect::<Vec<_>>();
            tabs.sort_by(|(_, a), (_, b)| {
                f32::from(a.origin.y)
                    .total_cmp(&f32::from(b.origin.y))
                    .then(f32::from(a.origin.x).total_cmp(&f32::from(b.origin.x)))
            });
            let before = tabs
                .iter()
                .find(|(_, b)| {
                    position.y < b.origin.y
                        || (position.y < b.origin.y + b.size.height
                            && position.x < b.origin.x + b.size.width / 2.)
                })
                .map(|(n, _)| n.clone());
            return Some((name.clone(), DropEdge::Center, before));
        }
        let (name, bounds) = self
            .dock_bounds
            .borrow()
            .iter()
            .find(|(_, b)| b.contains(&position))
            .map(|(n, b)| (n.clone(), *b))?;
        let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width);
        let y = f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height);
        // Keep generous central space for merging; narrow edges are for splits.
        let edge = if x < 0.22 {
            DropEdge::Left
        } else if x > 0.78 {
            DropEdge::Right
        } else if y < 0.22 {
            DropEdge::Top
        } else if y > 0.78 {
            DropEdge::Bottom
        } else {
            DropEdge::Center
        };
        Some((name, edge, None))
    }
    fn drop_at_pointer(
        &mut self,
        drag: &DraggedChannel,
        tab_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(position) = self.release_position.take() else {
            return;
        };
        let Some((name, edge, before)) = self.drop_location(position) else {
            return;
        };
        if before.as_deref() == Some(drag.name.as_str()) {
            self.drop_edge = None;
            self.drop_target = None;
            cx.notify();
            return;
        }
        self.last_drop = Some(
            json!({"input":if self.release_from_control{"local-control"}else{"native-window"},"source":drag.name,"target":name,"edge":edge.label(),"x":f32::from(position.x),"y":f32::from(position.y)}),
        );
        self.drop_target = Some(name);
        self.move_pane(drag, tab_id, Some(edge), window, cx);
        if edge == DropEdge::Center {
            if let Some(dock) = self.tabs[self.active].dock.as_mut() {
                dock.reorder_before(&drag.name, before.as_deref());
            }
            self.schedule_save(cx);
        }
    }
    fn track_drop(&mut self, event: &DragMoveEvent<DraggedChannel>, cx: &mut Context<Self>) {
        let location = self.drop_location(event.event.position);
        let target = location.as_ref().map(|(name, _, _)| name.clone());
        let edge = location.map(|(_, edge, _)| edge);
        if self.drop_edge != edge || self.drop_target != target {
            self.drop_edge = edge;
            self.drop_target = target;
            cx.notify();
        }
    }
    fn drop_preview(edge: DropEdge) -> AnyElement {
        div()
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .border_2()
            .border_color(rgb(0xA99CF4))
            .bg(rgba(0xA99CF42A))
            .text_color(rgb(theme::TEXT))
            .when(edge.vertical(), |el| el.left_0().right_0().h(relative(0.5)))
            .when(!edge.vertical() && edge != DropEdge::Center, |el| {
                el.top_0().bottom_0().w(relative(0.5))
            })
            .when(edge == DropEdge::Center, |el| el.inset_0())
            .when(edge == DropEdge::Left, |el| el.left_0())
            .when(edge == DropEdge::Right, |el| el.right_0())
            .when(edge == DropEdge::Top, |el| el.top_0())
            .when(edge == DropEdge::Bottom, |el| el.bottom_0())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(px(5.))
                    .bg(rgb(theme::PANEL))
                    .child(edge.label()),
            )
            .with_animation(
                ("drop-edge-reveal", edge as usize),
                Animation::new(Duration::from_millis(80)),
                |el, progress| el.opacity(progress),
            )
            .into_any_element()
    }
    fn flip_split(&mut self, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active];
        tab.vertical = !tab.vertical;
        if let Some(dock) = &mut tab.dock {
            dock.rotate();
        }
        tab.dock_states.clear();
        tab.sizes.clear();
        tab.split = cx.new(|_| ResizableState::default());
        self.schedule_save(cx);
        cx.notify();
    }
    fn change_pause_mode(&mut self, mode: u8, window: &Window, cx: &mut Context<Self>) {
        self.pause_mode = mode.min(2);
        for pane in self
            .panes()
            .iter()
            .chain(self.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
        {
            pane.update(cx, |p, cx| {
                p.pause_mode = self.pause_mode;
                p.update_hover_pause(window, cx);
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
    }
    /// Re-lay out every chat row (after an appearance change).
    /// Refetch every open channel's 7TV/BTTV/FFZ emotes (an emote filter changed).
    pub(crate) fn reload_all_emotes(&mut self, cx: &mut Context<Self>) {
        let names: Vec<String> = self
            .panes()
            .iter()
            .flat_map(|p| crate::combined::sources(&p.read(cx).name))
            .filter(|n| !crate::combined::is_special(n))
            .collect();
        let mut catalog = self.catalog.borrow_mut();
        for name in names {
            catalog.reload_channel(&name);
        }
    }
    pub(crate) fn remeasure_all(&mut self, cx: &mut Context<Self>) {
        for pane in self
            .panes()
            .iter()
            .chain(self.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
        {
            pane.update(cx, |p, cx| {
                p.scroller.update(cx, |s, cx| s.remeasure(cx));
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
    }
    fn change_timestamps(&mut self, mode: u8, cx: &mut Context<Self>) {
        self.timestamps = mode.min(4);
        for pane in self
            .panes()
            .iter()
            .chain(self.closed_tabs.iter().flat_map(|tab| tab.panes.iter()))
        {
            pane.update(cx, |p, cx| {
                p.timestamps = self.timestamps;
                p.scroller.update(cx, |s, cx| s.remeasure(cx));
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
    }
    fn change_history_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        self.history_limit = limit.clamp(500, 10_000);
        for pane in self
            .panes()
            .iter()
            .chain(self.closed_tabs.iter().flat_map(|t| t.panes.iter()))
        {
            pane.update(cx, |p, cx| p.change_history_limit(self.history_limit, cx));
        }
        self.schedule_save(cx);
        cx.notify();
    }
    fn change_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.font_size = (self.font_size + delta).clamp(12., 24.);
        for pane in self
            .panes()
            .iter()
            .chain(self.closed_tabs.iter().flat_map(|t| t.panes.iter()))
        {
            pane.update(cx, |p, cx| {
                p.font_size = self.font_size;
                p.scroller.update(cx, |s, cx| s.remeasure(cx));
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
    }
    fn cycle_channel(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self
            .visible_panes(cx)
            .into_iter()
            .find(|p| {
                let p = p.read(cx);
                p.focus.contains_focused(window, cx)
                    || p.draft.read(cx).focus_handle(cx).is_focused(window)
                    || p.search.input.read(cx).focus_handle(cx).is_focused(window)
            })
            .map(|p| p.read(cx).name.to_string());
        let Some(dock) = &self.tabs[self.active].dock else {
            return;
        };
        let mut active = Vec::new();
        dock.active_names(&mut active);
        let current = focused
            .or_else(|| self.selected_channel.clone().filter(|n| active.contains(n)))
            .or_else(|| active.first().cloned());
        let Some(current) = current else {
            return;
        };
        let channels = dock
            .group(&current)
            .unwrap_or_default()
            .into_iter()
            .filter(|n| {
                !self.live_channels
                    || n == &current
                    || self.streams.get(n) != Some(false)
                    || self.is_pinned(n, cx)
            })
            .collect::<Vec<_>>();
        if channels.len() < 2 {
            return;
        }
        let index = channels.iter().position(|n| n == &current).unwrap_or(0);
        let index = if forward {
            (index + 1) % channels.len()
        } else {
            (index + channels.len() - 1) % channels.len()
        };
        let channel = channels[index].clone();
        if let Some(dock) = &mut self.tabs[self.active].dock {
            dock.select(&channel);
        }
        self.selected_channel = Some(channel.clone());
        if let Some(pane) = self.tabs[self.active]
            .panes
            .iter()
            .find(|p| p.read(cx).name.as_ref() == channel)
        {
            pane.read(cx)
                .draft
                .clone()
                .update(cx, |input, cx| input.focus(window, cx));
        }
        self.schedule_save(cx);
        cx.notify();
    }
    /// Tab menu → Close other tabs / Close tabs to the right: one confirmation for
    /// all of them. Pinned tabs (and your own in Streamer setup) are kept.
    fn confirm_close_group(
        &mut self,
        channel: String,
        right_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tabs[self.active].locked {
            window.push_notification(
                Notification::info("This workspace's layout is locked. Right-click it in the sidebar → Unlock layout."),
                cx,
            );
            return;
        }
        let Some(group) = self.tabs[self.active]
            .dock
            .as_ref()
            .and_then(|d| d.group(&channel))
        else {
            return;
        };
        // Only tabs you can see: with "Only show live channel tabs", offline tabs are
        // hidden from the strip and must not be closed by a bulk action.
        let group: Vec<String> = group
            .into_iter()
            .filter(|n| !self.live_channels || *n == channel || self.streams.get(n) != Some(false))
            .collect();
        let at = group.iter().position(|n| *n == channel).unwrap_or(0);
        let names: Vec<String> = group
            .iter()
            .enumerate()
            .filter(|(i, n)| **n != channel && (!right_only || *i > at) && !self.is_pinned(n, cx))
            .map(|(_, n)| n.clone())
            .collect();
        if names.is_empty() {
            window.push_notification(
                Notification::info("No other visible, unpinned tabs to close."),
                cx,
            );
            return;
        }
        let panes: Vec<Entity<ChannelPane>> = self.tabs[self.active]
            .panes
            .iter()
            .filter(|p| names.iter().any(|n| p.read(cx).name.as_ref() == n))
            .cloned()
            .collect();
        let count = panes.len();
        let shown = names
            .iter()
            .take(6)
            .map(|n| crate::combined::title(n))
            .collect::<Vec<_>>()
            .join(", ");
        let more = if names.len() > 6 {
            format!(" and {} more", names.len() - 6)
        } else {
            String::new()
        };
        window.open_dialog(cx, move |dialog, _, _| {
            let panes = panes.clone();
            dialog
                .title(if count == 1 {
                    "Close 1 tab?".to_owned()
                } else {
                    format!("Close {count} tabs?")
                })
                .child(format!(
                    "{shown}{more}. Drafts stay saved; pinned and hidden (offline) tabs stay open."
                ))
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-group-close")
                                .label("Cancel")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("confirm-group-close")
                                .danger()
                                .label(format!("Close {count}"))
                                .on_click(move |_, w, cx| {
                                    for pane in &panes {
                                        pane.update(cx, |_, cx| cx.emit(PaneEvent::Close));
                                    }
                                    w.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
    fn confirm_close_channel(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_channel_pending {
            return;
        }
        if self.tabs[self.active].locked {
            window.push_notification(
                Notification::info("This workspace's layout is locked. Right-click it in the sidebar → Unlock layout."),
                cx,
            );
            return;
        }
        if self.anchor(cx).as_deref() == Some(name.as_str()) {
            window.push_notification(
                Notification::info("Your channel stays pinned in Streamer setup. Gear menu → Choose your setup → Viewer unpins it."),
                cx,
            );
            return;
        }
        if self.is_pinned(&name, cx) {
            window.push_notification(
                Notification::info(format!(
                    "#{name} is pinned. Right-click the tab → Unpin tab to close it."
                )),
                cx,
            );
            return;
        }
        let Some(pane) = self.tabs[self.active]
            .panes
            .iter()
            .find(|p| p.read(cx).name.as_ref() == name)
            .cloned()
        else {
            return;
        };
        self.close_channel_pending = true;
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let cancel = owner.clone();
            let close = owner.clone();
            let dismiss = owner.clone();
            let pane = pane.clone();
            dialog
                .title(
                    if crate::youtube::is_youtube(&name) || crate::combined::is_combined(&name) {
                        format!("Close {}?", crate::combined::title(&name))
                    } else {
                        format!("Close #{name}?")
                    },
                )
                .close_button(false)
                .child("Your draft stays saved. Other channel tabs and splits remain open.")
                .on_ok(|_, _, _| true)
                .on_close(move |_, _, cx| {
                    let _ = dismiss.update(cx, |this, cx| {
                        this.close_channel_pending = false;
                        cx.notify();
                    });
                })
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-channel-close")
                                .label("Cancel")
                                .on_click(move |_, window, cx| {
                                    let _ = cancel.update(cx, |this, cx| {
                                        this.close_channel_pending = false;
                                        cx.notify();
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("confirm-channel-close")
                                .label("Close channel")
                                .on_click(move |_, window, cx| {
                                    pane.update(cx, |_, cx| cx.emit(PaneEvent::Close));
                                    let _ = close.update(cx, |this, cx| {
                                        this.close_channel_pending = false;
                                        cx.notify();
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
        cx.notify();
    }
    fn render_dock(
        &mut self,
        dock: &Dock,
        path: Vec<usize>,
        width: f32,
        height: f32,
        reading_available: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_id = self.tabs[self.active].id;
        match dock {
            Dock::Leaf(name) | Dock::Deck { active: name, .. } => {
                let pane = self.tabs[self.active]
                    .panes
                    .iter()
                    .find(|p| p.read(cx).name.as_ref() == name)
                    .cloned();
                let names = match dock {
                    Dock::Deck { channels, .. } => channels.clone(),
                    _ => vec![name.clone()],
                };
                let name = name.clone();
                let bounds_name = name.clone();
                let dock_bounds = self.dock_bounds.clone();
                let strip_name = name.clone();
                let strip_bounds = self.strip_bounds.clone();
                // The Streamer anchor and user-pinned tabs share the pin presentation.
                let pins: Vec<String> = self
                    .anchor(cx)
                    .into_iter()
                    .chain(self.tabs[self.active].pinned.iter().cloned())
                    .collect();
                let tabs = names
                    .iter()
                    .filter(|channel| {
                        !self.live_channels
                            || *channel == &name
                            || self.streams.get(channel) != Some(false)
                            || pins.contains(*channel)
                    })
                    .map(|channel| {
                        let selected = channel == &name;
                        let channel = channel.clone();
                        let drag_pane = self.tabs[self.active]
                            .panes
                            .iter()
                            .find(|p| p.read(cx).name.as_ref() == channel)
                            .cloned();
                        let locked = self.tabs[self.active].locked;
                        let counts = drag_pane
                            .as_ref()
                            .map(|p| self.displayed_attention(p, reading_available, cx))
                            .unwrap_or_default();
                        let tab_name = channel.clone();
                        let tab_bounds = self.tab_bounds.clone();
                        let item = div()
                            .id(SharedString::from(format!(
                                "channel-tab-{tab_id}-{channel}"
                            )))
                            .h(px(28.))
                            .px_2()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(rgb(if selected {
                                theme::CONTROL
                            } else {
                                theme::PANEL
                            }))
                            .border_b_2()
                            .border_color(rgb(if selected { 0xA99CF4 } else { theme::PANEL }))
                            .on_prepaint(move |bounds, _, _| {
                                tab_bounds.borrow_mut().insert(tab_name.clone(), bounds);
                            })
                            .hover(|s| s.bg(rgb(theme::HOVER)))
                            .child(stream_marker(&channel, self.streams.get(&channel)))
                            .when(pins.contains(&channel), |el| {
                                el.child(
                                    div()
                                        .mr_1()
                                        .text_color(rgb(0xA99CF4))
                                        .child(Icon::new(gpui_kit::assets::IconName::Pin).xsmall()),
                                )
                            })
                            .when(crate::youtube::is_youtube(&channel), |el| {
                                el.child(
                                    div().mr_1().child(
                                        crate::assets::platform_icon(
                                            crate::combined::Platform::YouTube,
                                        )
                                        .xsmall(),
                                    ),
                                )
                            })
                            .child(if crate::youtube::is_youtube(&channel) {
                                let title = self.tabs[self.active]
                                    .panes
                                    .iter()
                                    .find(|p| p.read(cx).name.as_ref() == channel)
                                    .and_then(|p| p.read(cx).display_title.clone())
                                    .map(|t| t.split(" · ").next().unwrap_or("").to_owned())
                                    .filter(|t| !t.is_empty())
                                    .unwrap_or_else(|| "YouTube".into());
                                title
                            } else if let Some(title) = crate::combined::special_title(&channel) {
                                title.to_owned()
                            } else {
                                format!("#{}", crate::combined::title(&channel))
                            })
                            // Shared Chat: "+N" with who it's shared with on hover.
                            .when_some(self.shared_chat_label(&channel), |el, (plus, who)| {
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("shared-tab-{channel}")))
                                        .ml_1()
                                        .px_1()
                                        .rounded(px(3.))
                                        .text_size(px(10.))
                                        .bg(rgb(0x2A2340))
                                        .text_color(rgb(theme::ACCENT))
                                        .child(plus)
                                        .tooltip(move |w, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(who.clone())
                                                .build(w, cx)
                                        }),
                                )
                            })
                            .when(counts.0 > 0, |el| {
                                let tip = if counts.1 > 0 {
                                    format!(
                                        "{} {} you or {} your highlights, among {} unread.\nClick to jump to {}.",
                                        crate::attention::count(counts.1),
                                        if counts.1 == 1 { "message mentions" } else { "messages mention" },
                                        if counts.1 == 1 { "matches" } else { "match" },
                                        crate::attention::count(counts.0),
                                        if counts.1 == 1 { "it" } else { "the first one" },
                                    )
                                } else {
                                    format!(
                                        "{} unread {}. Click to jump to the first.",
                                        crate::attention::count(counts.0),
                                        if counts.0 == 1 { "message" } else { "messages" },
                                    )
                                };
                                let jump_channel = channel.clone();
                                let mentions = counts.1 > 0;
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("unread-chip-{channel}")))
                                        .cursor_pointer()
                                        .tooltip(move |w, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            if let Some(d) = &mut this.tabs[this.active].dock {
                                                d.select(&jump_channel);
                                            }
                                            let pane = this.tabs[this.active]
                                                .panes
                                                .iter()
                                                .find(|p| p.read(cx).name.as_ref() == jump_channel)
                                                .cloned();
                                            if let Some(pane) = pane {
                                                pane.update(cx, |p, cx| {
                                                    p.navigate_attention(if mentions { 2 } else { 0 }, window, cx)
                                                });
                                            }
                                            cx.notify();
                                        }))
                                        .ml_1()
                                        .px_1()
                                        .rounded(px(3.))
                                        .text_size(px(10.))
                                        .bg(rgb(if counts.1 > 0 { 0x403250 } else { 0x2B3038 }))
                                        .text_color(rgb(if counts.1 > 0 {
                                            0xE4BCFA
                                        } else {
                                            theme::TEXT
                                        }))
                                        .child(if counts.1 > 0 {
                                            format!("@{}", crate::attention::count(counts.1))
                                        } else {
                                            crate::attention::count(counts.0)
                                        }),
                                )
                            })
                            .on_mouse_down(
                                MouseButton::Middle,
                                cx.listener({
                                    let channel = channel.clone();
                                    move |this, _, window, cx| {
                                        window.prevent_default();
                                        cx.stop_propagation();
                                        this.confirm_close_channel(channel.clone(), window, cx);
                                    }
                                }),
                            )
                            .on_click(cx.listener({
                                let channel = channel.clone();
                                move |this, _, window, cx| {
                                    if let Some(d) = &mut this.tabs[this.active].dock {
                                        d.select(&channel);
                                    }
                                    this.selected_channel = Some(channel.clone());
                                    this.focus.focus(window, cx);
                                    this.schedule_save(cx);
                                    cx.notify();
                                }
                            }));
                        let owner = cx.entity().downgrade();
                        let menu_channel = channel.clone();
                        let menu_pane = name.clone();
                        let menu = move |menu: gpui_kit::component::menu::PopupMenu,
                                             window: &mut Window,
                                             cx: &mut Context<
                                gpui_kit::component::menu::PopupMenu,
                            >| {
                                let live_only = owner
                                    .upgrade()
                                    .is_some_and(|owner| owner.read(cx).live_channels);
                                let add = owner.clone();
                                let add_channel = menu_channel.clone();
                                let close = owner.clone();
                                let close_channel = menu_channel.clone();
                                let filter = owner.clone();
                                let left = owner.clone();
                                let left_channel = menu_channel.clone();
                                let right = owner.clone();
                                let right_channel = menu_channel.clone();
                                let open_url = format!("https://www.twitch.tv/{menu_channel}");
                                let copy_url = open_url.clone();
                                let read = owner.clone();
                                let read_channel = menu_channel.clone();
                                let details = owner.clone();
                                let details_channel = menu_channel.clone();
                                let equal = owner.clone();
                                let equal_channel = menu_channel.clone();
                                let equal_all = owner.clone();
                                let next_pane = owner.clone();
                                let next_origin = menu_pane.clone();
                                let nav_owner = owner.clone();
                                let nav_channel = menu_channel.clone();
                                let pin_owner = owner.clone();
                                let pin_channel = menu_channel.clone();
                                let pinned_now = owner
                                    .upgrade()
                                    .is_some_and(|o| o.read(cx).is_pinned(&menu_channel, cx));
                                let menu_special = crate::combined::is_special(&menu_channel);
                                let notify_owner = owner.clone();
                                let notify_channel = menu_channel.clone();
                                let notify_now = owner.upgrade().is_some_and(|o| {
                                    o.read(cx).live_notify.contains(&menu_channel)
                                });
                                let mute_owner = owner.clone();
                                let mute_channel = menu_channel.clone();
                                let muted_now = owner.upgrade().is_some_and(|o| {
                                    o.read(cx).muted_pings.contains(&menu_channel)
                                });
                                menu.item(
                                    PopupMenuItem::new(if pinned_now {
                                        "Unpin tab"
                                    } else {
                                        "Pin tab"
                                    })
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = pin_owner.update(cx, |this, cx| {
                                                this.toggle_pin(&pin_channel, cx)
                                            });
                                        },
                                    ),
                                )
                                .item_if(!menu_special,
                                    PopupMenuItem::new("Notify when live")
                                        .checked(notify_now)
                                        .on_click(move |_, _, cx| {
                                            let _ = notify_owner.update(cx, |this, cx| {
                                                this.toggle_live_notify(&notify_channel, cx)
                                            });
                                        }),
                                )
                                .item_if(!menu_special,
                                    PopupMenuItem::new("Mute ping sounds here")
                                        .checked(muted_now)
                                        .on_click(move |_, _, cx| {
                                            let _ = mute_owner.update(cx, |this, cx| {
                                                if let Some(at) = this.muted_pings.iter().position(|c| *c == mute_channel) {
                                                    this.muted_pings.remove(at);
                                                } else {
                                                    this.muted_pings.push(mute_channel.clone());
                                                }
                                                this.schedule_save(cx);
                                                cx.notify();
                                            });
                                        }),
                                )
                                .separator()
                                .submenu(
                                    "Reading & highlights",
                                    window,
                                    cx,
                                    move |mut menu, _, _| {
                                        for (label, direction) in [
                                            ("First unread / last read · Ctrl+Home", 0i8),
                                            ("Previous highlight · Alt+PageUp", -1),
                                            ("Next highlight · Alt+PageDown", 1),
                                        ] {
                                            let owner = nav_owner.clone();
                                            let channel = nav_channel.clone();
                                            menu = menu.item(PopupMenuItem::new(label).on_click(
                                                move |_, w, cx| {
                                                    let _ = owner.update(cx, |this, cx| {
                                                        let Some(tab) = this
                                                            .tabs
                                                            .iter()
                                                            .position(|tab| tab.id == tab_id)
                                                        else {
                                                            return;
                                                        };
                                                        let Some(pane) = this.tabs[tab]
                                                            .panes
                                                            .iter()
                                                            .find(|p| {
                                                                p.read(cx).name.as_ref() == channel
                                                            })
                                                            .cloned()
                                                        else {
                                                            return;
                                                        };
                                                        this.select_tab(tab, cx);
                                                        if let Some(dock) = &mut this.tabs[tab].dock
                                                        {
                                                            dock.select(&channel);
                                                        }
                                                        this.selected_channel =
                                                            Some(channel.clone());
                                                        this.schedule_save(cx);
                                                        pane.update(cx, |p, cx| {
                                                            p.navigate_attention(direction, w, cx)
                                                        });
                                                        cx.notify();
                                                    });
                                                },
                                            ));
                                        }
                                        menu
                                    },
                                )
                                .item_if(!menu_special, PopupMenuItem::new("Channel details · Ctrl+I").on_click(
                                    move |_, w, cx| {
                                        let _ = details.update(cx, |this, cx| {
                                            this.open_channel_details(
                                                details_channel.clone(),
                                                w,
                                                cx,
                                            )
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::new("Mark channel read").on_click(
                                    move |_, _, cx| {
                                        let _ = read.update(cx, |this, cx| {
                                            if let Some(pane) = this.tabs[this.active]
                                                .panes
                                                .iter()
                                                .find(|p| p.read(cx).name.as_ref() == read_channel)
                                            {
                                                pane.update(cx, |p, cx| {
                                                    p.attention.mark_all();
                                                    p.scroller.update(cx, |s, cx| s.remeasure(cx));
                                                    cx.notify();
                                                });
                                            }
                                            cx.notify();
                                        });
                                    },
                                ))
                                .item_if(!menu_special, PopupMenuItem::new("Reconnect").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            for source in crate::combined::sources(&channel) {
                                                if !crate::youtube::is_youtube(&source) {
                                                    this.live.reconnect(&source);
                                                }
                                            }
                                            cx.notify();
                                        });
                                        window.push_notification(
                                            Notification::info(format!("Reconnecting #{channel}…")),
                                            cx,
                                        );
                                    }
                                }))
                                .item(PopupMenuItem::new("Change channel…").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            if this.tabs[this.active].locked {
                                                window.push_notification(
                                                    Notification::info("This workspace's layout is locked. Right-click it in the sidebar → Unlock layout."),
                                                    cx,
                                                );
                                                return;
                                            }
                                            this.open_add(false, window, cx);
                                            this.replacing = Some(channel.clone());
                                            this.add_target = Some(channel.clone());
                                            cx.notify();
                                        });
                                    }
                                }))
                                .item_if(!menu_special, PopupMenuItem::new("Reload emotes").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            let mut catalog = this.catalog.borrow_mut();
                                            for source in crate::combined::sources(&channel) {
                                                catalog.reload_channel(&source);
                                            }
                                            drop(catalog);
                                            cx.notify();
                                        });
                                        window.push_notification(
                                            Notification::info("Reloading emotes…"),
                                            cx,
                                        );
                                    }
                                }))
                                .item(PopupMenuItem::new("Clear messages locally").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, _, cx| {
                                        let pane = owner.upgrade().and_then(|o| {
                                            let o = o.read(cx);
                                            o.tabs[o.active]
                                                .panes
                                                .iter()
                                                .find(|p| p.read(cx).name.as_ref() == channel)
                                                .cloned()
                                        });
                                        if let Some(pane) = pane {
                                            pane.update(cx, |p, cx| p.clear_locally(cx));
                                        }
                                    }
                                }))
                                .separator()
                                .item_if(!menu_special,
                                    PopupMenuItem::new("Open stream in browser")
                                        .on_click(move |_, _, cx| cx.open_url(&open_url)),
                                )
                                .item_if(!menu_special, PopupMenuItem::new("Open player pop-out").on_click({
                                    let url = crate::streamlink::player_url(&menu_channel);
                                    move |_, _, cx| cx.open_url(&url)
                                }))
                                .item_if(!menu_special, PopupMenuItem::new("Open in Streamlink").on_click({
                                    let channel = menu_channel.clone();
                                    move |_, window, cx| {
                                        let note = match crate::streamlink::open(&channel) {
                                            Ok(()) => format!("Starting #{channel} in Streamlink…"),
                                            Err(problem) => problem,
                                        };
                                        window.push_notification(Notification::info(note), cx);
                                    }
                                }))
                                .item_if(!menu_special, PopupMenuItem::new("Copy channel URL").on_click(
                                    move |_, window, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copy_url.clone(),
                                        ));
                                        window.push_notification(
                                            Notification::info("Channel URL copied"),
                                            cx,
                                        );
                                    },
                                ))
                                .separator()
                                .item(PopupMenuItem::new("Add channel tab").on_click(
                                    move |_, window, cx| {
                                        let _ = add.update(cx, |this, cx| {
                                            this.open_add(false, window, cx);
                                            this.add_target = Some(add_channel.clone());
                                        });
                                    },
                                ))
                                .item(
                                    PopupMenuItem::new("Only show live channel tabs")
                                        .checked(live_only)
                                        .on_click(move |_, _, cx| {
                                            let _ = filter.update(cx, |this, cx| {
                                                this.live_channels = !this.live_channels;
                                                this.schedule_save(cx);
                                                cx.notify();
                                            });
                                        }),
                                )
                                .separator()
                                .item(PopupMenuItem::new("Sort tabs A–Z").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, _, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.tabs[this.active].dock {
                                                d.sort_group(&channel);
                                            }
                                            let pinned = this.tabs[this.active].pinned.clone();
                                            if let Some(d) = &mut this.tabs[this.active].dock {
                                                d.pin_front(&pinned);
                                            }
                                            this.schedule_save(cx);
                                            cx.notify();
                                        });
                                    }
                                }))
                                .item(PopupMenuItem::new("Move tab left").on_click(
                                    move |_, _, cx| {
                                        let _ = left.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.tabs[this.active].dock {
                                                d.shift_tab(&left_channel, false);
                                            }
                                            this.schedule_save(cx);
                                            cx.notify();
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::new("Move tab right").on_click(
                                    move |_, _, cx| {
                                        let _ = right.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.tabs[this.active].dock {
                                                d.shift_tab(&right_channel, true);
                                            }
                                            this.schedule_save(cx);
                                            cx.notify();
                                        });
                                    },
                                ))
                                .separator()
                                .item(PopupMenuItem::new("Filter view…").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, window, cx| {
                                        let pane = owner.upgrade().and_then(|o| {
                                            let o = o.read(cx);
                                            o.tabs[o.active]
                                                .panes
                                                .iter()
                                                .find(|p| p.read(cx).name.as_ref() == channel)
                                                .cloned()
                                        });
                                        if let Some(pane) = pane {
                                            pane.update(cx, |p, cx| p.open_view_filter(window, cx));
                                        }
                                    }
                                }))
                                .item(PopupMenuItem::new("Pop out to its own window").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, _, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            this.open_popout(channel.clone(), cx)
                                        });
                                    }
                                }))
                                .item(PopupMenuItem::new("Focus next pane · F6").on_click(
                                    move |_, w, cx| {
                                        let _ = next_pane.update(cx, |this, cx| {
                                            this.cycle_pane_from(
                                                true,
                                                Some(next_origin.clone()),
                                                w,
                                                cx,
                                            )
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::new("Equalize this split").on_click(
                                    move |_, w, cx| {
                                        let _ = equal.update(cx, |this, cx| {
                                            this.equalize_panes(
                                                Some(equal_channel.clone()),
                                                false,
                                                w,
                                                cx,
                                            )
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::new("Equalize all splits").on_click(
                                    move |_, w, cx| {
                                        let _ = equal_all.update(cx, |this, cx| {
                                            this.equalize_panes(None, true, w, cx)
                                        });
                                    },
                                ))
                                .separator()
                                .item(PopupMenuItem::new("Close other tabs…").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, w, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            this.confirm_close_group(channel.clone(), false, w, cx)
                                        });
                                    }
                                }))
                                .item(PopupMenuItem::new("Close tabs to the right…").on_click({
                                    let owner = owner.clone();
                                    let channel = menu_channel.clone();
                                    move |_, w, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            this.confirm_close_group(channel.clone(), true, w, cx)
                                        });
                                    }
                                }))
                                .item(
                                    PopupMenuItem::new("Close channel…").on_click(
                                        move |_, window, cx| {
                                            let _ = close.update(cx, |this, cx| {
                                                this.confirm_close_channel(
                                                    close_channel.clone(),
                                                    window,
                                                    cx,
                                                )
                                            });
                                        },
                                    ),
                                )
                            };
                        if let Some(pane) = drag_pane.filter(|_| !locked) {
                            item.on_drag(
                                DraggedChannel {
                                    pane,
                                    name: channel.clone(),
                                },
                                move |drag, _, _, cx| cx.new(|_| DragPreview(drag.name.clone())),
                            )
                            .context_menu(menu)
                            .into_any_element()
                        } else {
                            item.context_menu(menu).into_any_element()
                        }
                    })
                    .collect::<Vec<_>>();
                let preview = if self.drop_target.as_ref() == Some(&name) {
                    self.drop_edge
                } else {
                    None
                };
                div()
                    .id(SharedString::from(format!("dock-{tab_id}-{name}")))
                    .relative()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .on_drag_move(cx.listener(
                        move |this, event: &DragMoveEvent<DraggedChannel>, _, cx| {
                            this.track_drop(event, cx);
                        },
                    ))
                    .on_prepaint(move |bounds, _, _| {
                        dock_bounds.borrow_mut().insert(bounds_name.clone(), bounds);
                    })
                    .on_drop(cx.listener(move |this, drag: &DraggedChannel, w, cx| {
                        cx.stop_propagation();
                        this.drop_at_pointer(drag, tab_id, w, cx);
                    }))
                    .v_flex()
                    .child(
                        div()
                            .id(SharedString::from(format!("channel-strip-{tab_id}-{name}")))
                            .h_flex()
                            .flex_wrap()
                            .min_h(px(28.))
                            .flex_shrink_0()
                            .bg(rgb(theme::PANEL))
                            .on_prepaint(move |bounds, _, _| {
                                strip_bounds.borrow_mut().insert(strip_name.clone(), bounds);
                            })
                            .children(tabs)
                            .child(
                                Button::new(SharedString::from(format!("add-tab-{tab_id}-{name}")))
                                    .ghost()
                                    .xsmall()
                                    .label("+")
                                    .tooltip("Add channel tab · Ctrl+K")
                                    .on_click(cx.listener({
                                        let target = name.clone();
                                        move |this, _, window, cx| {
                                            this.open_add(false, window, cx);
                                            this.add_target = Some(target.clone());
                                        }
                                    })),
                            ),
                    )
                    .child(div().flex_1().min_h_0().children(pane))
                    .when(cx.has_active_drag(), |el| {
                        el.children(preview.map(Self::drop_preview))
                    })
                    .into_any_element()
            }
            Dock::Split {
                vertical,
                weights,
                children,
            } => {
                let key = format!("{tab_id}:{path:?}");
                // While a divider reset eases, sizes come from interpolated weights.
                let mut weights = weights.clone();
                if let Some((tab, at, from, started)) = &self.equalizing
                    && *tab == tab_id
                    && *at == path
                    && from.len() == weights.len()
                {
                    let t = (started.elapsed().as_secs_f32() / 0.2).clamp(0., 1.);
                    let eased = 1. - (1. - t).powi(3);
                    let total = from.iter().sum::<f32>().max(f32::EPSILON);
                    let equal = 1. / from.len() as f32;
                    weights = from
                        .iter()
                        .map(|w| w / total + (equal - w / total) * eased)
                        .collect();
                    self.tabs[self.active].dock_states.remove(&key);
                    if t >= 1. {
                        if let Some(dock) = &mut self.tabs[self.active].dock {
                            dock.resize(&path, vec![1.; from.len()]);
                        }
                        self.equalizing = None;
                        self.schedule_save(cx);
                    }
                }
                let weights = &weights;
                let state = self.tabs[self.active]
                    .dock_states
                    .entry(key.clone())
                    .or_insert_with(|| cx.new(|_| ResizableState::default()))
                    .clone();
                let total = weights.iter().copied().sum::<f32>();
                let mut panels = Vec::new();
                for (ix, child) in children.iter().enumerate() {
                    let share = if total > 0. && weights.len() == children.len() {
                        weights[ix] / total
                    } else {
                        1. / children.len() as f32
                    };
                    let min = child.minimum();
                    let mut subpath = path.clone();
                    subpath.push(ix);
                    let (cw, ch) = if *vertical {
                        (width, height * share)
                    } else {
                        (width * share, height)
                    };
                    let content = self.render_dock(child, subpath, cw, ch, reading_available, cx);
                    panels.push(
                        resizable_panel()
                            .size(px(if *vertical { ch } else { cw }))
                            .size_range(px(if *vertical { min.1 } else { min.0 })..Pixels::MAX)
                            .child(content),
                    );
                }
                let group = if *vertical {
                    v_resizable(SharedString::from(key))
                } else {
                    h_resizable(SharedString::from(key))
                };
                // Double-clicking a divider evens out this split (like resetting a grid).
                // Dividers occlude their ancestors, so listen at window level within our bounds.
                let shares: Vec<f32> = weights
                    .iter()
                    .map(|w| w / total.max(f32::EPSILON))
                    .collect();
                let (vertical, reset_path, owner) =
                    (*vertical, path.clone(), cx.entity().downgrade());
                let group = group.with_state(&state).children(panels);
                div()
                    .relative()
                    .size_full()
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |b, _, window, _| {
                                let (shares, reset_path, owner) =
                                    (shares.clone(), reset_path.clone(), owner.clone());
                                window.on_mouse_event(
                                    move |event: &MouseDownEvent, phase, _, cx| {
                                        if !phase.capture()
                                            || event.button != MouseButton::Left
                                            || event.click_count != 2
                                            || shares.len() < 2
                                            || !b.contains(&event.position)
                                        {
                                            return;
                                        }
                                        let (offset, length) = if vertical {
                                            (
                                                f32::from(event.position.y - b.origin.y),
                                                f32::from(b.size.height),
                                            )
                                        } else {
                                            (
                                                f32::from(event.position.x - b.origin.x),
                                                f32::from(b.size.width),
                                            )
                                        };
                                        let mut edge = 0.;
                                        let on_divider =
                                            shares[..shares.len() - 1].iter().any(|share| {
                                                edge += share * length;
                                                (offset - edge).abs() <= 5.
                                            });
                                        if on_divider {
                                            cx.stop_propagation();
                                            let (path, shares) =
                                                (reset_path.clone(), shares.clone());
                                            let _ = owner.update(cx, |this, cx| {
                                                this.equalize_split(tab_id, path, shares, cx)
                                            });
                                        }
                                    },
                                );
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(group.on_resize(cx.listener(
                        move |this, state: &Entity<ResizableState>, _, cx| {
                            if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == tab_id)
                                && let Some(dock) = &mut tab.dock
                            {
                                dock.resize(
                                    &path,
                                    state
                                        .read(cx)
                                        .sizes()
                                        .iter()
                                        .map(|p| f32::from(*p))
                                        .collect(),
                                );
                            }
                            this.schedule_save(cx);
                        },
                    )))
                    .into_any_element()
            }
        }
    }
    /// Ease a split to equal sizes (instantly with reduced motion).
    fn equalize_split(
        &mut self,
        tab_id: u64,
        path: Vec<usize>,
        from: Vec<f32>,
        cx: &mut Context<Self>,
    ) {
        if cx.reduce_motion() {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                if let Some(dock) = &mut tab.dock {
                    dock.resize(&path, vec![1.; from.len()]);
                }
                tab.dock_states.clear();
            }
            self.schedule_save(cx);
            cx.notify();
            return;
        }
        self.equalizing = Some((tab_id, path, from, std::time::Instant::now()));
        cx.spawn(async move |view, cx| {
            for _ in 0..16 {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if view.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn render_sidebar(&self, reading_available: bool, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .w(px(190.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(theme::PANEL))
            .border_r_1()
            .border_color(rgb(theme::BORDER))
            .p_3()
            .gap_3()
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(theme::MUTED))
                    .child("YOUR WORKSPACES"),
            )
            .child(
                Button::new("new-workspace-sidebar")
                    .ghost()
                    .small()
                    .label("+ New workspace")
                    .tooltip("Ctrl+T")
                    .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx))),
            )
            .when(self.live_workspaces, |el| {
                el.child(
                    Button::new("show-offline-workspaces")
                        .ghost()
                        .xsmall()
                        .label("Live only · show all")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.live_workspaces = false;
                            this.schedule_save(cx);
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .id("workspace-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(self.shown_tabs(cx).into_iter().map(|ix| {
                        let tab = &self.tabs[ix];
                        let counts = tab
                            .panes
                            .iter()
                            .filter(|p| !crate::combined::is_special(&p.read(cx).name))
                            .map(|p| self.displayed_attention(p, reading_available, cx))
                            .fold((0, 0), |(u, h), (a, b)| (u + a, h + b));
                        div()
                            .id(("sidebar-tab", tab.id))
                            .v_flex()
                            .p_2()
                            .gap_1()
                            .rounded(px(5.))
                            .cursor_pointer()
                            .bg(rgb(if ix == self.active {
                                theme::CONTROL
                            } else {
                                theme::PANEL
                            }))
                            .hover(|s| s.bg(rgb(theme::HOVER)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_tab(ix, cx);
                                this.focus.focus(window, cx);
                            }))
                            .on_mouse_down(
                                MouseButton::Middle,
                                cx.listener({
                                    let id = tab.id;
                                    move |this, _, window, cx| {
                                        window.prevent_default();
                                        cx.stop_propagation();
                                        this.confirm_close_tab(id, window, cx);
                                    }
                                }),
                            )
                            .on_drag(
                                DraggedTab {
                                    id: tab.id,
                                    name: tab.name.clone(),
                                },
                                move |drag, _, _, cx| cx.new(|_| DragPreview(drag.name.clone())),
                            )
                            .drag_over::<DraggedTab>(|style, _, _, _| {
                                style.border_t_2().border_color(rgb(0xA99CF4))
                            })
                            .on_drop(cx.listener({
                                let id = tab.id;
                                move |this, drag: &DraggedTab, _, cx| {
                                    this.reorder_tab(drag.id, id, cx)
                                }
                            }))
                            .drag_over::<DraggedChannel>(|style, _, _, _| {
                                style.bg(rgb(theme::HOVER))
                            })
                            .on_drop(cx.listener({
                                let id = tab.id;
                                move |this, drag: &DraggedChannel, window, cx| {
                                    this.drop_target = None;
                                    this.move_pane(drag, id, None, window, cx);
                                }
                            }))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .child(tab.name.clone()),
                                    )
                                    .child(
                                        Button::new(("close-sidebar-workspace", tab.id))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Close)
                                            .tooltip("Close workspace · Middle-click / Ctrl+W")
                                            .on_click(cx.listener({
                                                let id = tab.id;
                                                move |this, _, window, cx| {
                                                    cx.stop_propagation();
                                                    this.confirm_close_tab(id, window, cx);
                                                }
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::MUTED))
                                    .child(format!(
                                        "{} channels{}",
                                        tab.panes.len(),
                                        if counts.0 > 0 {
                                            format!(
                                                " · {} unread{}",
                                                crate::attention::count(counts.0),
                                                match counts.1 {
                                                    0 => String::new(),
                                                    1 => " · 1 mention".into(),
                                                    n => format!(
                                                        " · {} mentions",
                                                        crate::attention::count(n)
                                                    ),
                                                }
                                            )
                                        } else {
                                            String::new()
                                        }
                                    )),
                            )
                            .context_menu({
                                let owner = cx.entity().downgrade();
                                let (id, last) = (tab.id, self.tabs.len() - 1);
                                let can_reopen = !self.closed_tabs.is_empty();
                                let locked = tab.locked;
                                let live_only = self.live_workspaces;
                                move |menu, _, _| {
                                    // Each action applies to the workspace that was right-clicked.
                                    let act = |f: fn(
                                        &mut Workbench,
                                        u64,
                                        &mut Window,
                                        &mut Context<Workbench>,
                                    )| {
                                        let owner = owner.clone();
                                        move |_: &ClickEvent, w: &mut Window, cx: &mut App| {
                                            let _ = owner.update(cx, |this, cx| f(this, id, w, cx));
                                        }
                                    };
                                    menu.item(PopupMenuItem::new("Rename workspace").on_click(act(
                                        |this, id, w, cx| {
                                            this.select_tab_id(id, cx);
                                            this.open_add(true, w, cx);
                                        },
                                    )))
                                    .item(PopupMenuItem::new("Combine into one chat").on_click(
                                        act(|this, id, w, cx| this.combine_workspace(id, w, cx)),
                                    ))
                                    .item(PopupMenuItem::new("Duplicate workspace").on_click(
                                        act(|this, id, w, cx| this.duplicate_workspace(id, w, cx)),
                                    ))
                                    .item(
                                        PopupMenuItem::new("Lock layout")
                                            .checked(locked)
                                            .on_click(act(|this, id, w, cx| {
                                                if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == id) {
                                                    tab.locked = !tab.locked;
                                                    let note = if tab.locked {
                                                        "Layout locked: tabs can't be dragged or closed"
                                                    } else {
                                                        "Layout unlocked"
                                                    };
                                                    w.push_notification(Notification::info(note), cx);
                                                }
                                                this.schedule_save(cx);
                                                cx.notify();
                                            })),
                                    )
                                    .item(PopupMenuItem::new("Move up").disabled(ix == 0).on_click(
                                        act(|this, id, _, cx| {
                                            this.select_tab_id(id, cx);
                                            this.move_tab(-1, cx);
                                        }),
                                    ))
                                    .item(
                                        PopupMenuItem::new("Move down")
                                            .disabled(ix == last)
                                            .on_click(act(|this, id, _, cx| {
                                                this.select_tab_id(id, cx);
                                                this.move_tab(1, cx);
                                            })),
                                    )
                                    .item(
                                        PopupMenuItem::new("Only show live workspaces")
                                            .checked(live_only)
                                            .on_click(act(|this, _, _, cx| {
                                                this.live_workspaces = !this.live_workspaces;
                                                this.schedule_save(cx);
                                                cx.notify();
                                            })),
                                    )
                                    .separator()
                                    .item(
                                        PopupMenuItem::new("Reopen closed workspace")
                                            .disabled(!can_reopen)
                                            .on_click(act(|this, _, _, cx| this.reopen_tab(cx))),
                                    )
                                    .item(
                                        PopupMenuItem::new("Close workspace…").on_click(act(
                                            |this, id, w, cx| this.confirm_close_tab(id, w, cx),
                                        )),
                                    )
                                }
                            })
                    })),
            )
            .children(self.render_live_follows(cx))
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .p_2()
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .text_size(px(11.))
                    .text_color(rgb(theme::MUTED))
                    .child(self.render_updates(cx)),
            )
            .into_any_element()
    }
    /// Sidebar LIVE NOW: followed channels that are live, sortable (most viewers
    /// first by default) and filterable; hover one for its stream card.
    fn render_live_follows(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use crate::follows_live::SortBy;
        use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu};
        let all = self.follows_live.list.as_ref()?;
        let query = if self.live_filter_open {
            self.live_filter.read(cx).value().to_string()
        } else {
            String::new()
        };
        let list = crate::follows_live::arrange(all, self.live_sort, &query, None);
        let open: std::collections::HashSet<String> = self
            .tab_panes()
            .iter()
            .map(|p| p.read(cx).name.to_string())
            .collect();
        let hide_counts = crate::streamer_mode::hides(crate::streamer_mode::VIEWER_COUNTS);
        let owner = cx.entity().downgrade();
        let sort = self.live_sort;
        let mut rows = Vec::new();
        let mut last_game: Option<String> = None;
        for (index, live) in list.iter().enumerate() {
            // "By category": a small heading above each category.
            if sort.by == SortBy::Category && last_game.as_deref() != Some(live.game.as_str()) {
                last_game = Some(live.game.clone());
                rows.push(
                    div()
                        .flex_shrink_0()
                        .pt_1()
                        .px_1()
                        .text_size(px(10.))
                        .text_color(rgb(0xC5B8E6))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(if live.game.is_empty() {
                            "No category".to_owned()
                        } else {
                            live.game.clone()
                        })
                        .into_any_element(),
                );
            }
            let avatar = {
                let mut catalog = self.catalog.borrow_mut();
                if !live.user_id.is_empty() {
                    catalog.profiles.request(&live.user_id);
                }
                catalog
                    .profiles
                    .get(&live.user_id)
                    .and_then(|p| p.avatar.clone())
            }
            .and_then(|key| self.media.borrow_mut().get(&key, cx));
            let login = live.login.clone();
            let is_open = open.contains(&live.login);
            let detail = [
                Some(live.game.clone()).filter(|g| !g.is_empty()),
                (!hide_counts)
                    .then(|| format!("{} viewers", crate::follows_live::viewers(live.viewers))),
                live.uptime()
                    .filter(|_| !hide_counts)
                    .map(|u| format!("live {u}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            let card = crate::media_hover::Content::Stream {
                name: live.label().to_owned(),
                title: live.title.clone(),
                detail,
                avatar: avatar.clone(),
            };
            rows.push(
                div()
                    .id(("live-follow", index))
                    .flex_shrink_0()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px_1()
                    .py(px(3.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::HOVER)))
                    .tooltip(move |_, cx| cx.new(|_| crate::media_hover::Card(card.clone())).into())
                    .child(
                        div()
                            .relative()
                            .size(px(22.))
                            .flex_shrink_0()
                            .child(match avatar {
                                Some(image) => crate::emote_picker::avatar(image, 22.),
                                None => div()
                                    .size(px(22.))
                                    .rounded_full()
                                    .bg(rgb(0x3A3350))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.))
                                    .child(
                                        live.label()
                                            .chars()
                                            .next()
                                            .unwrap_or('?')
                                            .to_uppercase()
                                            .to_string(),
                                    )
                                    .into_any_element(),
                            })
                            // Live dot on the avatar's corner.
                            .child(
                                div()
                                    .absolute()
                                    .right(px(-1.))
                                    .bottom(px(-1.))
                                    .size(px(8.))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(rgb(theme::PANEL))
                                    .bg(rgb(theme::STREAM_LIVE)),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(12.))
                                    .when(is_open, |el| el.text_color(rgb(theme::ACCENT)))
                                    .child(live.label().to_owned()),
                            )
                            .when(sort.by != SortBy::Category, |el| {
                                el.child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(10.))
                                        .text_color(rgb(theme::MUTED))
                                        .child(live.game.clone()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(10.))
                            .text_color(rgb(theme::MUTED))
                            .when(!hide_counts, |el| {
                                el.child(match sort.by {
                                    SortBy::LiveTime => live.uptime().unwrap_or_default(),
                                    _ => crate::follows_live::viewers(live.viewers),
                                })
                            }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_channel(login.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
        }
        let shown = list.len();
        let total = all.len();
        Some(
            div()
                .v_flex()
                .flex_shrink_0()
                .max_h(px(320.))
                .gap_1()
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id("live-follows-header")
                                .flex_1()
                                .h_flex()
                                .items_center()
                                .gap_1()
                                .cursor_pointer()
                                .text_size(px(10.))
                                .text_color(rgb(theme::MUTED))
                                .child(
                                    Icon::new(if self.follows_open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .xsmall(),
                                )
                                .child(if shown == total {
                                    format!("LIVE NOW · {total}")
                                } else {
                                    format!("LIVE NOW · {shown} of {total}")
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.follows_open = !this.follows_open;
                                    this.schedule_save(cx);
                                    cx.notify();
                                })),
                        )
                        .when(self.follows_open, |el| {
                            el.child(
                                Button::new("live-filter")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(gpui_kit::assets::IconName::ListFilter))
                                    .when(self.live_filter_open, |b| {
                                        gpui_kit::component::Selectable::selected(b, true)
                                    })
                                    .tooltip("Filter by name, game or title")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.live_filter_open = !this.live_filter_open;
                                        if this.live_filter_open {
                                            this.live_filter
                                                .update(cx, |i, cx| i.focus(window, cx));
                                        } else {
                                            this.live_filter
                                                .update(cx, |i, cx| i.set_value("", window, cx));
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("live-sort")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(
                                        gpui_kit::assets::IconName::ArrowDownWideNarrow,
                                    ))
                                    .tooltip(format!(
                                        "Sorted by {} · click a field again to flip it",
                                        sort.by.label().to_lowercase()
                                    ))
                                    .dropdown_menu(move |mut menu: PopupMenu, _, _| {
                                        for by in SortBy::ALL {
                                            let owner = owner.clone();
                                            let mut item = PopupMenuItem::new(by.label());
                                            // The arrow shows the direction; picking it again flips it.
                                            if by == sort.by {
                                                item = item.icon(Icon::new(if sort.descending {
                                                    IconName::ArrowDown
                                                } else {
                                                    IconName::ArrowUp
                                                }));
                                            }
                                            menu = menu.item(item.on_click(move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.live_sort = this.live_sort.pick(by);
                                                    this.schedule_save(cx);
                                                    cx.notify();
                                                });
                                            }));
                                        }
                                        menu
                                    }),
                            )
                        }),
                )
                .when(self.follows_open && self.live_filter_open, |el| {
                    el.child(Input::new(&self.live_filter).xsmall().cleanable(true))
                })
                .when(self.follows_open && total == 0, |el| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(theme::MUTED))
                            .child("Nobody you follow is live."),
                    )
                })
                .when(self.follows_open && total > 0 && shown == 0, |el| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(theme::MUTED))
                            .child("No live channels match."),
                    )
                })
                .when(self.follows_open && shown > 0, |el| {
                    el.child(
                        div()
                            .id("live-follows")
                            .v_flex()
                            .min_h_0()
                            .overflow_y_scroll()
                            .children(rows),
                    )
                })
                .into_any_element(),
        )
    }
}
fn valid_channel(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_start_matches("twitch.tv/")
        .trim_matches('/')
        .trim_start_matches('#')
        .to_ascii_lowercase();
    (!value.is_empty()
        && value.len() <= 25
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    .then_some(value)
}
impl Workbench {
    /// A banner nobody can miss while Twitch is signed out: chat can't connect.
    fn render_signed_out(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let reason = self.account.read(cx).signed_out_reason()?;
        let detail = if reason.starts_with("Sign in with Twitch") {
            "Chat can't connect until you sign in.".to_owned()
        } else {
            format!("{reason} Chat can't connect until you sign in.")
        };
        Some(
            div()
                .id("signed-out-banner")
                .mx(px(2.))
                .mt(px(2.))
                .px_3()
                .py_2()
                .h_flex()
                .gap_3()
                .items_center()
                .rounded(px(6.))
                .bg(rgb(0x2A1F3D))
                .border_1()
                .border_color(rgb(0x6D5BC7))
                .child(Icon::new(gpui_kit::assets::IconName::Lock).text_color(rgb(theme::ACCENT)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .v_flex()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::TEXT))
                                .child("You're signed out of Twitch"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::MUTED))
                                .child(detail),
                        ),
                )
                .child(
                    Button::new("signed-out-sign-in")
                        .small()
                        .primary()
                        .label("Sign in")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_accounts(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
}
impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.streamer_live = match self.streamer_mode {
            crate::streamer_mode::Mode::On => true,
            crate::streamer_mode::Mode::Off => false,
            // OBS saying it's streaming is the surest signal; otherwise broadcasting software running.
            crate::streamer_mode::Mode::Automatic => {
                self.obs.state.streaming || self.streamer_detector.detected()
            }
        };
        for channel in std::mem::take(&mut self.restore_popouts) {
            self.open_popout(channel, cx);
        }
        let reading_available = window.is_window_active() && !window.has_active_dialog(cx);
        // Retarget from the current sampled width so rapid reversals stay smooth.
        // GPUI's motion primitive honors the OS reduced-motion preference.
        let sidebar_width = gpui_kit::base::motion::transition(
            "jawjack-sidebar-width",
            if self.sidebar { 190.0_f32 } else { 0.0_f32 },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(180)),
            window,
            cx,
        );
        let studio_width = gpui_kit::base::motion::transition(
            "jawjack-studio-width",
            if self.studio_open && self.anchor(cx).is_some() {
                roles::STUDIO_WIDTH
            } else {
                0.0_f32
            },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(200)),
            window,
            cx,
        );
        let panes = self.panes();
        let connected = panes.iter().filter(|p| p.read(cx).connected).count();
        let live_status = if panes.is_empty() {
            "○ No channels open".to_owned()
        } else if connected == panes.len() {
            format!("● Live · {connected} connected panes")
        } else if connected > 0 {
            format!("◐ Live · {connected}/{} connected panes", panes.len())
        } else {
            "○ Live chat disconnected".to_owned()
        };
        let counts = self.attention_counts(reading_available, cx);
        let pointer_guard = cx.entity().downgrade();
        let tab = &self.tabs[self.active];
        let dock = tab.dock.clone();
        self.dock_bounds.borrow_mut().clear();
        self.strip_bounds.borrow_mut().clear();
        self.tab_bounds.borrow_mut().clear();
        let content = if let Some(dock) = dock {
            let min = dock.minimum();
            let available = window.viewport_size();
            let width = (f32::from(available.width) - sidebar_width - 4.).max(min.0);
            let height = (f32::from(available.height) - 60.).max(min.1);
            let body = self.render_dock(&dock, Vec::new(), width, height, reading_available, cx);
            div()
                .id("dock-overflow")
                .size_full()
                .overflow_scroll()
                .child(
                    div()
                        .size_full()
                        .min_w(px(min.0))
                        .min_h(px(min.1))
                        .child(body),
                )
                .into_any_element()
        } else {
            div()
                .v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .text_size(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Make room for your channels."),
                )
                .child(
                    div()
                        .text_color(rgb(theme::MUTED))
                        .child("Drag a channel to any panel edge to split your workspace."),
                )
                .child(
                    Button::new("empty-add").label("+ Add a channel").on_click(
                        cx.listener(|this, _, window, cx| this.open_add(false, window, cx)),
                    ),
                )
                .into_any_element()
        };
        div()
            .id("workspace")
            .relative()
            .track_focus(&self.focus)
            .key_context("ChatWorkspace")
            .v_flex()
            .size_full()
            .font_family("Segoe UI")
            .text_size(px(13.))
            .bg(rgb(theme::SHELL))
            .text_color(rgb(theme::TEXT))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    cx.stop_active_drag(window);
                    this.pointer_owner = Some(this.control_pointer_active);
                    this.release_position = None;
                }
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &MouseUpEvent, window, cx| {
                if event.button == MouseButton::Left {
                    let valid = this.pointer_owner.take() == Some(this.control_pointer_active);
                    this.release_position = valid.then_some(event.position);
                    this.release_from_control = this.control_pointer_active;
                    if !valid {
                        cx.stop_active_drag(window);
                        this.drop_edge = None;
                        this.drop_target = None;
                        cx.notify();
                    }
                }
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let zoom = pointer_guard.clone();
                        window.on_mouse_event(
                            move |event: &ScrollWheelEvent, phase, window, cx| {
                                if !phase.capture()
                                    || !event.modifiers.control
                                    || window.has_active_dialog(cx)
                                {
                                    return;
                                }
                                let delta = match event.delta {
                                    ScrollDelta::Lines(p) => p.y.signum(),
                                    ScrollDelta::Pixels(p) => f32::from(p.y) / 40.,
                                };
                                let _ = zoom.update(cx, |this, cx| {
                                    this.zoom_accumulator += delta;
                                    let steps = this.zoom_accumulator.trunc().clamp(-4., 4.);
                                    if steps != 0. {
                                        this.zoom_accumulator -= steps;
                                        this.font_feedback(steps, false, window, cx);
                                    }
                                });
                                window.prevent_default();
                                cx.stop_propagation();
                            },
                        );
                        let owner = pointer_guard.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if !phase.capture() {
                                return;
                            }
                            let _ = owner.update(cx, |this, cx| {
                                let crossed = this
                                    .pointer_owner
                                    .is_some_and(|source| source != this.control_pointer_active);
                                if crossed || event.pressed_button != Some(MouseButton::Left) {
                                    this.pointer_owner = None;
                                    this.release_position = None;
                                    this.drop_edge = None;
                                    this.drop_target = None;
                                    if cx.stop_active_drag(window) {
                                        cx.notify();
                                    }
                                    // Also clear a framework drag created later in this event from a stale down.
                                    window.defer(cx, |window, cx| {
                                        cx.stop_active_drag(window);
                                    });
                                }
                            });
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .on_action(cx.listener(|this, _: &crate::FindChat, w, cx| {
                let panes = this.visible_panes(cx);
                let selected = this.selected_channel.as_ref();
                if let Some(pane) = panes
                    .iter()
                    .find(|p| Some(&p.read(cx).name.to_string()) == selected)
                    .or(panes.first())
                {
                    pane.update(cx, |p, cx| p.open_search(w, cx));
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &FocusNextPane, w, cx| this.cycle_pane(true, w, cx)))
            .on_action(
                cx.listener(|this, _: &FocusPreviousPane, w, cx| this.cycle_pane(false, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPaneLeft, w, cx| this.focus_neighbor(-1, 0, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPaneRight, w, cx| this.focus_neighbor(1, 0, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPaneUp, w, cx| this.focus_neighbor(0, -1, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPaneDown, w, cx| this.focus_neighbor(0, 1, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &EqualizeSplit, w, cx| {
                    this.equalize_panes(None, false, w, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &EqualizeAllSplits, w, cx| {
                this.equalize_panes(None, true, w, cx)
            }))
            .on_action(cx.listener(|this, _: &EditChatterAppearance, w, cx| {
                this.open_chatter_appearance(None, w, cx)
            }))
            .on_action(cx.listener(|this, _: &EditCustomCommands, w, cx| {
                this.open_custom_commands(None, w, cx)
            }))
            .on_action(cx.listener(|this, _: &EditIgnores, w, cx| this.open_ignores(w, cx)))
            .on_action(
                cx.listener(|this, _: &EditEventHighlights, w, cx| {
                    this.open_event_highlights(w, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ChooseSetup, w, cx| this.open_setup(w, cx)))
            .on_action(cx.listener(|this, _: &OpenWhispers, w, cx| this.open_whispers(None, w, cx)))
            .on_action(cx.listener(|this, _: &ToggleStudio, _, cx| {
                this.studio_open = !this.studio_open;
                this.schedule_save(cx);
                cx.notify();
            }))
            .on_action(
                cx.listener(|this, _: &ImportChatterino, w, cx| this.open_chatterino_import(w, cx)),
            )
            .on_action(cx.listener(|this, _: &SearchOpenChannels, w, cx| {
                this.open_workspace_search(None, w, cx)
            }))
            .on_action(cx.listener(|this, _: &SwitchChannel, w, cx| this.open_switcher(w, cx)))
            .on_action(cx.listener(|this, _: &ChannelDetails, w, cx| {
                this.open_focused_channel_details(w, cx)
            }))
            .on_action(cx.listener(|this, _: &OpenActivity, w, cx| this.open_activity(w, cx)))
            .on_action(cx.listener(|this, _: &MarkAllRead, _, cx| this.mark_all_read(cx)))
            .on_action(
                cx.listener(|this, _: &FontLarger, w, cx| this.font_feedback(1., false, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FontSmaller, w, cx| this.font_feedback(-1., false, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ResetFont, w, cx| this.font_feedback(0., true, w, cx)),
            )
            .on_action(cx.listener(|this, _: &NewTab, w, cx| this.new_tab(w, cx)))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.confirm_close_tab(this.tabs[this.active].id, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReopenTab, window, cx| {
                this.reopen_tab(cx);
                this.focus.focus(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextTab, w, cx| {
                this.cycle_tab(true, cx);
                this.focus.focus(w, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousTab, w, cx| {
                this.cycle_tab(false, cx);
                this.focus.focus(w, cx);
            }))
            .on_action(cx.listener(|this, _: &NextChannel, w, cx| this.cycle_channel(true, w, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousChannel, w, cx| this.cycle_channel(false, w, cx)),
            )
            .on_action(cx.listener(|this, _: &AddChannel, w, cx| this.open_add(false, w, cx)))
            .on_action(cx.listener(|this, _: &RenameWorkspace, w, cx| this.open_add(true, w, cx)))
            .on_action(cx.listener(|this, _: &MoveTabLeft, _, cx| this.move_tab(-1, cx)))
            .on_action(cx.listener(|this, _: &MoveTabRight, _, cx| this.move_tab(1, cx)))
            .on_action(cx.listener(|this, _: &ToggleStreamerMode, window, cx| {
                // On if it's off now, otherwise Off (Settings has Automatic).
                this.streamer_mode = if this.streamer_live {
                    crate::streamer_mode::Mode::Off
                } else {
                    crate::streamer_mode::Mode::On
                };
                window.push_notification(
                    Notification::info(if this.streamer_live {
                        "Streamer mode off"
                    } else {
                        "Streamer mode on"
                    }),
                    cx,
                );
                this.schedule_save(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar = !this.sidebar;
                this.schedule_save(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleAppearance, w, cx| this.toggle_settings(w, cx)))
            // Esc closes Settings, unless an editor opened from it is on top.
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if this.settings && e.keystroke.key == "escape" && !w.has_active_dialog(cx) {
                    this.settings = false;
                    this.focus.focus(w, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleOrientation, _, cx| this.flip_split(cx)))
            .on_action(cx.listener(|this, _: &ToggleLiveWorkspaces, _, cx| {
                this.live_workspaces = !this.live_workspaces;
                this.schedule_save(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleLiveChannels, _, cx| {
                this.live_channels = !this.live_channels;
                this.schedule_save(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &QuitWithoutSaving, _, cx| {
                if this.close_failed {
                    cx.quit();
                }
            }))
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        Button::new("sidebar")
                            .ghost()
                            .small()
                            .icon(if self.sidebar {
                                IconName::PanelLeftClose
                            } else {
                                IconName::PanelLeftOpen
                            })
                            .tooltip("Workspaces")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sidebar = !this.sidebar;
                                this.schedule_save(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .h_flex()
                            .px_2()
                            .gap_1()
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .text_color(rgb(0xA99CF4))
                                    .font_weight(FontWeight::BOLD)
                                    .child("//"),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(11.))
                                    .child("JAWJACK"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(8.))
                            .h_full()
                            .window_control_area(WindowControlArea::Drag),
                    )
                    .when(self.streamer_live, |el| {
                        el.child(
                            Button::new("streamer-mode-pill")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(gpui_kit::assets::IconName::Tv))
                                .label("Streamer mode")
                                .tooltip(
                                    "Private notes, whisper text and stream thumbnails are hidden",
                                )
                                .on_click(cx.listener(|this, _, w, cx| {
                                    this.settings_section = 5;
                                    if !this.settings {
                                        this.toggle_settings(w, cx);
                                    }
                                })),
                        )
                    })
                    .child({
                        let unread = self.whispers.unread();
                        Button::new("whispers-menu")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(gpui_kit::assets::IconName::MessageCircle))
                            .when(unread > 0, |b| b.label(crate::attention::count(unread)))
                            .tooltip("Whispers")
                            .on_click(cx.listener(|this, _, w, cx| this.open_whispers(None, w, cx)))
                    })
                    .child(
                        Button::new("activity-menu")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Bell)
                            .when(counts.1 > 0 || counts.0 > 0, |b| {
                                b.label(if counts.1 > 0 {
                                    format!("@{}", crate::attention::count(counts.1))
                                } else {
                                    crate::attention::count(counts.0)
                                })
                            })
                            .tooltip("Activity: mentions, highlights and unread messages across your channels (\"@\" = messages that mention you or match your highlights) · Ctrl+Shift+M")
                            .on_click(cx.listener(|this, _, w, cx| this.open_activity(w, cx))),
                    )
                    .child(
                        Button::new("account-menu")
                            .xsmall()
                            .when(self.account.read(cx).signed_out_reason().is_some(), |b| {
                                b.primary()
                            })
                            .when(self.account.read(cx).signed_out_reason().is_none(), |b| {
                                b.ghost()
                            })
                            .label(if self.account.read(cx).signed_out_reason().is_some() {
                                "Sign in to Twitch".to_owned()
                            } else if self.account.read(cx).missing_scopes().is_empty() {
                                self.account.read(cx).label()
                            } else {
                                format!("{} •", self.account.read(cx).label())
                            })
                            .tooltip(if self.account.read(cx).missing_scopes().is_empty() {
                                "Twitch account"
                            } else {
                                "Twitch account · permissions can be updated"
                            })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_accounts(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("settings-menu")
                            .ghost()
                            .small()
                            .icon(IconName::Settings)
                            .selected(self.settings)
                            .tooltip("Settings")
                            .on_click(cx.listener(|this, _, w, cx| this.toggle_settings(w, cx))),
                    )
                    .when(self.experience == roles::Experience::Streamer, |el| {
                        el.child(
                            Button::new("studio-toggle")
                                .ghost()
                                .small()
                                .icon(if self.studio_open {
                                    IconName::PanelRightClose
                                } else {
                                    IconName::PanelRightOpen
                                })
                                .tooltip(if self.studio_open {
                                    "Hide Studio panel · Ctrl+Shift+S"
                                } else {
                                    "Show Studio panel · Ctrl+Shift+S"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.studio_open = !this.studio_open;
                                    this.schedule_save(cx);
                                    cx.notify();
                                })),
                        )
                    })
                    .child(caption_control(
                        "minimize",
                        IconName::WindowMinimize,
                        WindowControlArea::Min,
                        false,
                    ))
                    .child(caption_control(
                        "maximize",
                        if window.is_maximized() {
                            IconName::WindowRestore
                        } else {
                            IconName::WindowMaximize
                        },
                        WindowControlArea::Max,
                        false,
                    ))
                    .child(caption_control(
                        "close",
                        IconName::WindowClose,
                        WindowControlArea::Close,
                        true,
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .relative()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    // No account: a sign-in screen instead of tabs that can't connect.
                    .when(
                        self.account.read(cx).login().is_none()
                            && !self.guest_reading
                            && !self.account.read(cx).restoring(),
                        |el| el.child(deferred(self.render_sign_in_screen(cx)).with_priority(1)),
                    )
                    // Startup: a splash while the saved sign-in is restored, fading out.
                    .children(self.render_splash(cx).map(|splash| deferred(splash).with_priority(2)))
                    .when(sidebar_width > 0.1, |el| {
                        el.child(
                            div()
                                .w(px(sidebar_width))
                                .h_full()
                                .flex_shrink_0()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .relative()
                                        .left(px(sidebar_width - 190.))
                                        .w(px(190.))
                                        .h_full()
                                        .child(self.render_sidebar(reading_available, cx)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .when(self.adding, |el| {
                                el.child(
                                    div()
                                        .v_flex()
                                        .p_3()
                                        .gap_2()
                                        .bg(rgb(theme::ELEVATED))
                                        .border_b_1()
                                        .border_color(rgb(theme::BORDER))
                                        .child(div().text_size(px(12.)).child(if self.renaming {
                                            "Rename workspace".to_owned()
                                        } else if let Some(old) = &self.replacing {
                                            format!("Change {} to another channel", crate::combined::special_title(old).map(str::to_owned).unwrap_or_else(|| format!("#{}", crate::combined::title(old))))
                                        } else {
                                            "Add a channel to this workspace".to_owned()
                                        }))
                                        .child(
                                            div()
                                                .h_flex()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .child(Input::new(&self.channel_input)),
                                                )
                                                .child(
                                                    Button::new("accept-channel")
                                                        .label(if self.renaming {
                                                            "Save"
                                                        } else if self.replacing.is_some() {
                                                            "Change"
                                                        } else {
                                                            "Add channel"
                                                        })
                                                        .on_click(cx.listener(|this, _, w, cx| {
                                                            this.accept_input(w, cx)
                                                        })),
                                                )
                                                .child(
                                                    Button::new("cancel-add")
                                                        .ghost()
                                                        .label("Cancel")
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.adding = false;
                                                                this.focus.focus(window, cx);
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        )
                                        .when_some(self.add_error.clone(), |el, error| {
                                            el.child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(rgb(0xF29D9D))
                                                    .child(error),
                                            )
                                        }),
                                )
                            })
                            .children(self.render_signed_out(cx))
                            .child(
                                div()
                                    .h_flex()
                                    .items_stretch()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .p(px(2.))
                                    .child(
                                        div()
                                            .id("channel-dock")
                                            .relative()
                                            .flex_1()
                                            .h_full()
                                            .min_w_0()
                                            .min_h_0()
                                            .child(content),
                                    )
                                    // Studio slides in from the right edge and fades with its width.
                                    .when(studio_width > 0.1, |el| {
                                        el.child(
                                            div()
                                                .w(px(studio_width))
                                                .h_full()
                                                .flex_shrink_0()
                                                .overflow_hidden()
                                                .opacity(
                                                    (studio_width / roles::STUDIO_WIDTH)
                                                        .clamp(0., 1.),
                                                )
                                                .child(self.render_studio(window, cx)),
                                        )
                                    }),
                            ),
                    ),
            )
            .when(self.settings, |el| {
                el.child(self.render_settings(window, cx))
            })
            .child(
                div()
                    .h_flex()
                    .h(px(20.))
                    .flex_shrink_0()
                    .px_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .text_size(px(10.))
                    .text_color(rgb(theme::MUTED))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(live_status),
                    )
                    .child(
                        self.notice
                            .as_ref()
                            .filter(|(_, at)| at.elapsed() < Duration::from_secs(12))
                            .map(|(text, _)| text.clone())
                            .unwrap_or_else(|| self.save_status.clone()),
                    ),
            )
    }
}

fn stream_marker(channel: &str, status: Option<bool>) -> impl IntoElement {
    let (color, label) = match status {
        Some(true) => (theme::STREAM_LIVE, "Stream live"),
        Some(false) => (
            theme::STREAM_OFFLINE,
            "Stream offline · chat may still be connected",
        ),
        None => (
            theme::STREAM_UNKNOWN,
            "Stream status unknown · not checked yet or unavailable",
        ),
    };
    div()
        .id(SharedString::from(format!("stream-status-{channel}")))
        .w(px(12.))
        .h(px(16.))
        .mr_1()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(label).build(w, cx))
        .child(match status {
            Some(live) => div()
                .size(px(8.))
                .rounded(px(4.))
                .border_1()
                .border_color(rgb(color))
                .when(live, |el| el.bg(rgb(color)))
                .into_any_element(),
            None => div()
                .text_size(px(13.))
                .line_height(px(16.))
                .child("◇")
                .into_any_element(),
        })
}

/// Menu items that only apply sometimes (e.g. not on Mentions/Whispers/Live).
trait ItemIf {
    fn item_if(self, show: bool, item: PopupMenuItem) -> Self;
}
impl ItemIf for gpui_kit::component::menu::PopupMenu {
    fn item_if(self, show: bool, item: PopupMenuItem) -> Self {
        if show { self.item(item) } else { self }
    }
}
