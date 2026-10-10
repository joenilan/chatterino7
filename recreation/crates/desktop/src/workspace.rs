#[path="pane_navigation.rs"]
mod pane_navigation;
#[path="activity.rs"]
mod activity;
#[path="channel_details.rs"]
mod channel_details;
#[path="channel_switcher.rs"]
mod channel_switcher;
use gpui_kit::base::ElementExt;
use crate::dock::Dock;
use crate::{ChannelPane, DragPreview, DraggedChannel, PaneEvent, caption_control, storage, theme};
use gpui_kit::component::{
    Disableable,
    IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu,ContextMenuExt,PopupMenuItem},
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
        FocusNextPane, FocusPreviousPane, FocusPaneLeft, FocusPaneRight, FocusPaneUp, FocusPaneDown, EqualizeSplit, EqualizeAllSplits,
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
        OpenActivity, MarkAllRead, FontLarger, FontSmaller, ResetFont, ChannelDetails, SwitchChannel
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
        KeyBinding::new("ctrl-alt-down", FocusPaneDown, Some("ChatWorkspace > Input")),
        KeyBinding::new("ctrl-p", SwitchChannel, Some("ChatWorkspace")),
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
}
pub struct Workbench {
    account: Entity<crate::auth::TwitchAccount>,
    live: crate::live::LiveChat,
    streams: crate::stream_status::Streams,
    live_workspaces: bool,
    live_channels: bool,
    catalog: std::rc::Rc<std::cell::RefCell<crate::catalog::Catalog>>,
    media: std::rc::Rc<std::cell::RefCell<crate::media::MediaCache>>,
    drop_edge: Option<DropEdge>,
    drop_target: Option<String>,
    dock_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String,Bounds<Pixels>>>>,
    strip_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String,Bounds<Pixels>>>>,
    tab_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String,Bounds<Pixels>>>>,
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
    add_target: Option<String>,
    selected_channel: Option<String>,
    add_error: Option<String>,
    sidebar: bool,
    settings: bool,
    font_size: f32,
    timestamps:u8,
    history_limit: usize,
    highlight_input: Entity<InputState>,
    highlight_terms: Vec<String>,
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
    pub fn control_pointer(&mut self, active:bool){self.control_pointer_active=active;}
    pub fn focus_workspace(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }
    pub fn visible_panes(&self,cx:&App) -> Vec<Entity<ChannelPane>> {
        let tab=&self.tabs[self.active];let mut names=Vec::new();if let Some(dock)=&tab.dock{dock.active_names(&mut names);}
        names.iter().filter_map(|n|tab.panes.iter().find(|p|p.read(cx).name.as_ref()==n).cloned()).collect()
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
                let panes=self.visible_panes(cx);
                let pane=panes.get(pane).ok_or("No such visible pane")?;
                let draft = pane.read(cx).draft.clone();
                draft.update(cx, |input, cx| input.focus(window, cx));
            }
            "search" => {
                let panes=self.visible_panes(cx);let pane=panes.get(pane).ok_or("No such visible pane")?;
                pane.update(cx,|p,cx|p.open_search(window,cx));
            }
            "transcript" => {
                let panes=self.visible_panes(cx);
                let pane=panes.get(pane).ok_or("No such visible pane")?;
                let focus = pane.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            "workspace" => self.focus.focus(window, cx),
            _ => return Err("Target is unavailable".into()),
        }
        Ok(())
    }
    pub fn media_inspection(&self) -> Value { json!({"cache":self.media.borrow().inspection(),"7tv":self.catalog.borrow().inspection()}) }
    pub fn inspection(&self, cx: &App) -> Value {
        json!({"last_drop":self.last_drop,"pointer_owner":self.pointer_owner.map(|control|if control{"local-control"}else{"native-window"}),"control_enabled":self.control_enabled,"active":self.active,"tabs":self.tabs.iter().map(|t|json!({"id":t.id,"name":t.name,"vertical":t.vertical,"dock":t.dock.as_ref().map(Dock::json),"channels":t.panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>(),"adding_channel":self.adding,"appearance_open":self.settings,"font_size":self.font_size,"timestamps":self.timestamps,"history_limit":self.history_limit,"save_status":self.save_status,"close_tab_pending":self.close_tab_pending,"live_workspace_filter":self.live_workspaces,"live_channel_filter":self.live_channels,"streams":self.panes().iter().map(|p|{let n=p.read(cx).name.to_string();(n.clone(),self.streams.get(&n))}).collect::<BTreeMap<_,_>>()})
    }
    pub fn panes(&self) -> Vec<Entity<ChannelPane>> {
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
        let input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Channel name or twitch.tv/channel"));
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
        let highlight_input=cx.new(|cx|{let mut input=InputState::new(window,cx).placeholder("Words or phrases, comma separated").validate(|text,_|text.chars().count()<=400);input.set_value(state["highlight_words"].as_str().unwrap_or(""),window,cx);input});
        cx.subscribe_in(&highlight_input,window,|this:&mut Self,_,event,_,cx|{if matches!(event,InputEvent::Change){this.highlight_terms=crate::attention::terms(&this.highlight_input.read(cx).value());this.schedule_save(cx);cx.notify();}}).detach();
        let mut this = Self {
            highlight_terms: crate::attention::terms(state["highlight_words"].as_str().unwrap_or("")),
            highlight_input, activity_all:false, zoom_accumulator:0.,
            live: crate::live::LiveChat::new(),
            streams: crate::stream_status::Streams::new(),
            live_workspaces: state["live_workspaces"].as_bool().unwrap_or(false),
            live_channels: state["live_channels"].as_bool().unwrap_or(false),
            catalog: std::rc::Rc::new(std::cell::RefCell::new(crate::catalog::Catalog::new())),
            media: std::rc::Rc::new(std::cell::RefCell::new(crate::media::MediaCache::new())),
            account: cx.new(|cx| crate::auth::TwitchAccount::new(cx)),
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
            add_target: None,
            selected_channel: None,
            add_error: None,
            sidebar: state["sidebar"].as_bool().unwrap_or(false),
            settings: false,
            font_size: state["font_size"]
                .as_f64()
                .filter(|n| n.is_finite())
                .unwrap_or(14.)
                .clamp(12., 24.) as f32,
            timestamps:state["timestamps"].as_u64().unwrap_or(0).min(2)as u8,
            history_limit: state["history_limit"].as_u64().unwrap_or(10_000).clamp(500,10_000) as usize,
            drafts,
            reply_drafts: state["reply_drafts"].as_object().map(|v|v.iter().map(|(k,v)|(k.clone(),v.clone())).collect()).unwrap_or_default(),
            save_revision: 0,
            save_enabled,
            save_status,
            close_failed: false,
            close_tab_pending: None,
            close_channel_pending: false,
        };
        this.catalog.borrow_mut().restore_favorites(&state["emote_favorites"]);
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
                        if let Some(name) = channel.as_str().and_then(valid_channel) {
                            if !panes.iter().any(|p: &Entity<ChannelPane>| {
                                p.read(cx).name.as_ref() == name.as_str()
                            }) {
                                panes.push(this.make_pane(id, &name, window, cx));
                            }
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
                let names=panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>();
                let dock=Dock::from_json(&item["dock"]).filter(|d|{let mut actual=Vec::new();d.names(&mut actual);let mut expected=names.clone();actual.sort();expected.sort();actual==expected})
                    .or_else(||Dock::legacy(&names,item["vertical"].as_bool().unwrap_or(false),&sizes));
                this.tabs.push(WorkspaceTab {
                    id,
                    name,
                    panes,
                    vertical: item["vertical"].as_bool().unwrap_or(false),
                    sizes,
                    split: cx.new(|_| ResizableState::default()),
                    dock,
                    dock_states: BTreeMap::new(),
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
        this
    }
    fn pump_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let identity = self.account.read(cx).identity();
        let panes = self.panes();
        for pane in panes.iter().chain(self.closed_tabs.iter().flat_map(|tab|tab.panes.iter())) {pane.update(cx,|p,_|p.input_history.account(identity.as_ref().map(|i|i.user_id.as_str())));}
        self.sync_attention(window,cx);
        if self.streams.pump(identity.clone(),panes.iter().map(|p|p.read(cx).name.to_string()).collect()){cx.notify();}
        let profiles_changed=self.catalog.borrow_mut().profiles.pump(identity.clone());
        if profiles_changed {for pane in &panes {pane.update(cx,|p,cx|{if p.picker.open {p.refresh_picker(cx);}cx.notify();});}cx.notify();}
        if self.catalog.borrow_mut().pump(panes.iter().map(|p|p.read(cx).name.to_string()).collect(), identity.clone()) {
            for pane in &panes { pane.update(cx, |p,cx| {
                p.search.dirty=true;
                p.timeline.borrow_mut().enrich(|message| message.fragments = self.catalog.borrow().expand(&p.name, &message.user_id, &message.fragments));
                p.attention.reconcile(&p.timeline.borrow(),(p.next_id-p.timeline.borrow().messages().len()) as u64);
                p.scroller.update(cx, |s,cx|s.remeasure(cx)); if p.picker.open {p.refresh_picker(cx);}else{p.complete_query(false,cx);} cx.notify();
            }); }
            cx.notify();
        }
        if self.media.borrow_mut().pump(cx) { for pane in &panes { pane.update(cx, |p, cx| { p.scroller.update(cx, |s, cx| s.remeasure(cx)); cx.notify(); }); } }
        if self.live.configure(
            identity.clone(),
            panes.iter().map(|p| p.read(cx).name.to_string()).collect(),
        ) {
            for pane in &panes {
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
        for _ in 0..500 {
            let Ok((version, event)) = self.live.events.try_recv() else {
                break;
            };
            if version != self.live.version() && !matches!(event, crate::live::Event::Sent(..)) {
                continue;
            }
            match event {
                crate::live::Event::RoomSettings(channel,settings,initial)=>{
                    for pane in &panes{if pane.read(cx).name.as_ref()==channel{pane.update(cx,|p,cx|{if !initial||p.room_settings.is_none(){p.room_settings=Some(settings.clone());cx.notify();}});}}
                    cx.notify();
                }
                crate::live::Event::Resolved(channel, id) => self.catalog.borrow_mut().channel(&channel, &id),
                crate::live::Event::State(channel, status, ready) => {
                    for pane in &panes {
                        if pane.read(cx).name.as_ref() == channel {
                            pane.update(cx, |p, cx| {
                                p.connection = status.clone();
                                p.connected = ready;
                                if !ready {p.room_settings=None;}
                                cx.notify();
                            });
                        }
                    }
                    cx.notify();
                }
                crate::live::Event::Chat(channel, event) => {
                    cx.notify();
                    let moderation=!matches!(&event,chat_core::Event::Message(_));
                    for pane in panes.iter().chain(self.closed_tabs.iter().flat_map(|tab|tab.panes.iter()).filter(|_|moderation)) {
                        if pane.read(cx).name.as_ref() == channel {
                            pane.update(cx, |p, cx| p.received(event.clone(), cx));
                        }
                    }
                }
                crate::live::Event::Sent(request, result) => {
                    for pane in panes
                        .iter()
                        .chain(self.closed_tabs.iter().flat_map(|t| t.panes.iter()))
                    {
                        pane.update(cx, |p, cx| p.sent(request, &result, version == self.live.version(), window, cx));
                    }
                }
            }
        }
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
        let shared_state=self.panes().iter().find(|p|p.read(cx).name.as_ref()==name).map(|p|{let p=p.read(cx);(p.connected,p.connection.clone(),p.room_settings.clone())});
        let pane = cx.new(|cx| ChannelPane::new(name, self.media.clone(), self.catalog.clone(), &draft, self.font_size, self.history_limit, window, cx));
        pane.update(cx,|p,_|p.timestamps=self.timestamps);
        if let Some((connected,connection,room))=shared_state {pane.update(cx,|p,_|{p.connected=connected;p.connection=connection;p.room_settings=room;});}
        if let Some(target)=self.reply_drafts.get(&key).and_then(crate::replies::Target::from_json){pane.update(cx,|p,_|p.reply_target=Some(target));}
        let channel = name.to_owned();
        cx.subscribe_in(&pane, window, move |this, pane, event, window, cx| {
            if let PaneEvent::Send { request, text, reply_parent } = event {
                if let Err(error) = this.live.send(*request, channel.clone(), text.clone(), reply_parent.clone()) {
                    pane.update(cx, |p, cx| {
                        p.pending = None;
                        p.send_status = error;
                        cx.notify();
                    });
                }
                return;
            }
            if matches!(event, PaneEvent::RequestClose) {
                this.confirm_close_channel(channel.clone(),window,cx);
                return;
            }
            if matches!(event, PaneEvent::OpenAccount) {
                let account=this.account.clone();
                let width=(f32::from(window.viewport_size().width)-24.).min(380.);
                window.open_dialog(cx,move|dialog,_,_|dialog.w(px(width)).title("Twitch account").child(account.clone()));
                return;
            }
            if matches!(event, PaneEvent::EmotePreferencesChanged) {
                for pane in this.panes().iter().chain(this.closed_tabs.iter().flat_map(|tab|tab.panes.iter())) { pane.update(cx, |p, cx| {
                    if p.picker.open { p.refresh_picker(cx); }
                    cx.notify();
                }); }
                this.schedule_save(cx);
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
                .iter().chain(this.closed_tabs.iter())
                .find(|t| t.panes.iter().any(|p| p == pane))
                .map(|t| t.id);
            let Some(owner) = owner else {
                return;
            };
            this.drafts.insert(format!("{owner}:{channel}"), text);
            let key=format!("{owner}:{channel}");
            if let Some(target)=&pane.read(cx).reply_target {this.reply_drafts.insert(key,target.json());}else{this.reply_drafts.remove(&key);}
            if matches!(event, PaneEvent::Close) {
                this.focus.focus(window,cx);
                if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == owner) {
                    if let Some(ix) = tab.panes.iter().position(|p| p == pane) {
                        tab.panes.remove(ix);
                        tab.dock=tab.dock.take().and_then(|d|d.remove(&channel));
                        tab.dock_states.clear();
                    }
                }
            }
            this.schedule_save(cx);
            cx.notify();
        })
        .detach();
        pane
    }
    fn snapshot(&self, cx: &App) -> Value {
        let mut drafts = self.drafts.clone();
        let mut reply_drafts=self.reply_drafts.clone();
        for tab in self.tabs.iter().chain(self.closed_tabs.iter()) {
            for pane in &tab.panes {
                let pane = pane.read(cx);
                let key=format!("{}:{}",tab.id,pane.name);
                if let Some(target)=&pane.reply_target {reply_drafts.insert(key,target.json());}else{reply_drafts.remove(&key);}
                drafts.insert(
                    format!("{}:{}", tab.id, pane.name),
                    pane.draft.read(cx).value().to_string(),
                );
            }
        }
        json!({"version":1,"emote_favorites":self.catalog.borrow().favorites_json(),"next_id":self.next_id,"active":self.active,"sidebar":self.sidebar,"font_size":self.font_size,"timestamps":self.timestamps,"history_limit":self.history_limit,"live_workspaces":self.live_workspaces,"live_channels":self.live_channels,"drafts":drafts,"reply_drafts":reply_drafts,"highlight_words":self.highlight_terms.join(", "),
            "tabs":self.tabs.iter().map(|t|json!({"id":t.id,"name":t.name,"vertical":t.vertical,"sizes":t.sizes,"dock":t.dock.as_ref().map(Dock::json),"channels":t.panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>()})
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
            Ok(()) => true,
            Err(error) => {
                self.close_failed = true;
                self.save_status = format!("Save failed: {error}");
                window.push_notification(Notification::error("Workspace could not be saved. Window kept open. Workspace menu has Discard unsaved changes and quit."),cx);
                cx.notify();
                false
            }
        }
    }
    fn shown_tabs(&self,cx:&App)->Vec<usize>{self.tabs.iter().enumerate().filter(|(ix,t)|!self.live_workspaces||*ix==self.active||t.panes.is_empty()||t.panes.iter().any(|p|self.streams.get(p.read(cx).name.as_ref())!=Some(false))).map(|(ix,_)|ix).collect()}
    fn cycle_tab(&mut self,forward:bool,cx:&mut Context<Self>){let shown=self.shown_tabs(cx);let ix=shown.iter().position(|i|*i==self.active).unwrap_or(0);let next=if forward{(ix+1)%shown.len()}else{(ix+shown.len()-1)%shown.len()};self.select_tab(shown[next],cx);}
    fn select_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.tabs.len() {
            self.active = ix;
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
        });
        self.select_tab(self.tabs.len() - 1, cx);
        self.open_add(false, window, cx);
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
            .map(|p| format!("#{}", p.read(cx).name))
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
                .child(div().v_flex().gap_3().child(body.clone()).child("Drafts are kept. You can reopen this tab with Ctrl+Shift+T during this session."))
                .on_ok(|_,_,_|true)
                .on_close(move|_,_,cx|{let _=dismiss_owner.update(cx,|this,cx|{this.close_tab_pending=None;cx.notify();});})
                .footer(div().h_flex().justify_end().gap_2()
                    .child(Button::new("cancel-tab-close").label("Cancel").on_click(move|_,window,cx|{let _=cancel_owner.update(cx,|this,cx|{this.close_tab_pending=None;cx.notify();});window.close_dialog(cx);}))
                    .child(Button::new("confirm-tab-close").label("Close tab").on_click(move|_,window,cx|{
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
            self.tabs.push(tab);
            self.select_tab(self.tabs.len() - 1, cx);
        }
    }
    fn open_add(&mut self, rename: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.adding = true;
        self.renaming = rename;
        self.add_target=None;
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
    fn accept_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.channel_input.read(cx).value().trim().to_owned();
        if self.renaming {
            if value.is_empty() || value.chars().count() > 40 {
                self.add_error = Some("Use a workspace name of 1–40 characters".into());
                cx.notify();
                return;
            }
            self.tabs[self.active].name = value;
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
            let id = self.tabs[self.active].id;
            let pane = self.make_pane(id, &channel, window, cx);
            let tab = &mut self.tabs[self.active];
            tab.panes.push(pane);
            tab.dock=Some(match tab.dock.take(){Some(mut d)=>{let mut active=Vec::new();d.active_names(&mut active);if let Some(target)=self.add_target.as_ref().or(active.first()){d.tabify(target,channel.clone());}d},None=>Dock::Leaf(channel.clone())});
            tab.dock_states.clear();
            tab.sizes.clear();
            tab.split = cx.new(|_| ResizableState::default());
            if tab.name.starts_with("Workspace") && tab.panes.len() == 1 {
                tab.name = channel;
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
        let Some(source) = self
            .tabs
            .iter()
            .position(|t| t.panes.iter().any(|p| *p == drag.pane))
        else {
            return;
        };
        let Some(destination) = self.tabs.iter().position(|t| t.id == target) else {
            return;
        };
        if source != destination && self.tabs[destination].panes.iter().any(|p|p.read(cx).name.as_ref()==drag.name) {
            window.push_notification(Notification::info("This channel is already in the destination workspace"),cx);return;
        }
        let mut target_channel=self.drop_target.take();
        if source==destination && target_channel.as_deref()==Some(drag.name.as_str()) {
            target_channel=self.tabs[source].dock.as_ref().and_then(|d|d.companion(&drag.name));
            if target_channel.is_none(){cx.notify();return;}
        }
        let from=self.tabs[source].panes.iter().position(|p|*p==drag.pane).unwrap();
        let pane=self.tabs[source].panes.remove(from);
        self.tabs[source].dock=self.tabs[source].dock.take().and_then(|d|d.remove(&drag.name));
        self.tabs[source].dock_states.clear();
        let tab=&mut self.tabs[destination];
        tab.panes.push(pane);
        let docking_edge=edge;
        let edge=edge.unwrap_or(DropEdge::Right);
        tab.dock=Some(match tab.dock.take(){
            None=>Dock::Leaf(drag.name.clone()),
            Some(mut dock)=>{if !target_channel.as_ref().is_some_and(|target|if docking_edge.is_none()||docking_edge==Some(DropEdge::Center){dock.tabify(target,drag.name.clone())}else{dock.insert(target,drag.name.clone(),edge.vertical(),edge.first())}){dock=dock.append(drag.name.clone(),edge.vertical(),edge.first());}dock}
        });
        tab.dock_states.clear();
        self.active = destination;
        self.selected_channel=Some(drag.name.clone());
        self.focus.focus(window,cx);
        self.adding = false;
        self.schedule_save(cx);
        cx.notify();
    }
    fn drop_location(&self, position: Point<Pixels>) -> Option<(String, DropEdge, Option<String>)> {
        // Tab strips are dedicated merge/reorder targets, including wrapped rows.
        // The same resolver drives hover feedback and release behavior.
        if let Some((name, strip)) = self.strip_bounds.borrow().iter().find(|(_, b)| b.contains(&position)) {
            let mut tabs = self.tab_bounds.borrow().iter()
                .filter(|(_, b)| strip.contains(&b.origin))
                .map(|(n,b)| (n.clone(), *b)).collect::<Vec<_>>();
            tabs.sort_by(|(_,a),(_,b)| f32::from(a.origin.y).total_cmp(&f32::from(b.origin.y))
                .then(f32::from(a.origin.x).total_cmp(&f32::from(b.origin.x))));
            let before = tabs.iter().find(|(_,b)| position.y < b.origin.y
                || (position.y < b.origin.y + b.size.height && position.x < b.origin.x + b.size.width / 2.))
                .map(|(n,_)| n.clone());
            return Some((name.clone(), DropEdge::Center, before));
        }
        let (name, bounds) = self.dock_bounds.borrow().iter()
            .find(|(_, b)| b.contains(&position)).map(|(n,b)| (n.clone(), *b))?;
        let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width);
        let y = f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height);
        // Keep generous central space for merging; narrow edges are for splits.
        let edge = if x < 0.22 { DropEdge::Left } else if x > 0.78 { DropEdge::Right }
            else if y < 0.22 { DropEdge::Top } else if y > 0.78 { DropEdge::Bottom }
            else { DropEdge::Center };
        Some((name, edge, None))
    }
    fn drop_at_pointer(&mut self, drag: &DraggedChannel, tab_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(position) = self.release_position.take() else { return; };
        let Some((name, edge, before)) = self.drop_location(position) else { return; };
        if before.as_deref() == Some(drag.name.as_str()) { self.drop_edge = None; self.drop_target = None; cx.notify(); return; }
        self.last_drop = Some(json!({"input":if self.release_from_control{"local-control"}else{"native-window"},"source":drag.name,"target":name,"edge":edge.label(),"x":f32::from(position.x),"y":f32::from(position.y)}));
        self.drop_target = Some(name);
        self.move_pane(drag, tab_id, Some(edge), window, cx);
        if edge == DropEdge::Center {
            if let Some(dock) = self.tabs[self.active].dock.as_mut() { dock.reorder_before(&drag.name, before.as_deref()); }
            self.schedule_save(cx);
        }
    }
    fn track_drop(&mut self, event: &DragMoveEvent<DraggedChannel>, cx: &mut Context<Self>) {
        let location = self.drop_location(event.event.position);
        let target = location.as_ref().map(|(name,_,_)| name.clone());
        let edge = location.map(|(_,edge,_)| edge);
        if self.drop_edge != edge || self.drop_target != target {
            self.drop_edge = edge; self.drop_target = target; cx.notify();
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
            .when(!edge.vertical() && edge!=DropEdge::Center, |el| {
                el.top_0().bottom_0().w(relative(0.5))
            })
            .when(edge == DropEdge::Center, |el|el.inset_0())
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
        if let Some(dock)=&mut tab.dock {dock.rotate();}
        tab.dock_states.clear();
        tab.sizes.clear();
        tab.split = cx.new(|_| ResizableState::default());
        self.schedule_save(cx);
        cx.notify();
    }
    fn change_timestamps(&mut self,mode:u8,cx:&mut Context<Self>){
        self.timestamps=mode.min(2);
        for pane in self.panes().iter().chain(self.closed_tabs.iter().flat_map(|tab|tab.panes.iter())){pane.update(cx,|p,cx|{p.timestamps=self.timestamps;p.scroller.update(cx,|s,cx|s.remeasure(cx));cx.notify();});}
        self.schedule_save(cx);cx.notify();
    }
    fn change_history_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        self.history_limit=limit.clamp(500,10_000);
        for pane in self.panes().iter().chain(self.closed_tabs.iter().flat_map(|t|t.panes.iter())) {
            pane.update(cx,|p,cx|p.change_history_limit(self.history_limit,cx));
        }
        self.schedule_save(cx);cx.notify();
    }
    fn change_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.font_size = (self.font_size + delta).clamp(12., 24.);
        for pane in self.panes().iter().chain(self.closed_tabs.iter().flat_map(|t|t.panes.iter())) {
            pane.update(cx, |p, cx| {
                p.font_size = self.font_size;
                p.scroller.update(cx,|s,cx|s.remeasure(cx));
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
    }
    fn cycle_channel(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.visible_panes(cx).into_iter().find(|p| {
            let p = p.read(cx);
            p.focus.contains_focused(window,cx) || p.draft.read(cx).focus_handle(cx).is_focused(window) || p.search.input.read(cx).focus_handle(cx).is_focused(window)
        }).map(|p|p.read(cx).name.to_string());
        let Some(dock) = &self.tabs[self.active].dock else { return; };
        let mut active = Vec::new(); dock.active_names(&mut active);
        let current = focused.or_else(|| self.selected_channel.clone().filter(|n| active.contains(n))).or_else(||active.first().cloned());
        let Some(current) = current else { return; };
        let channels = dock.group(&current).unwrap_or_default().into_iter()
            .filter(|n| !self.live_channels || n == &current || self.streams.get(n) != Some(false)).collect::<Vec<_>>();
        if channels.len() < 2 { return; }
        let index = channels.iter().position(|n|n == &current).unwrap_or(0);
        let index = if forward {(index + 1) % channels.len()} else {(index + channels.len() - 1) % channels.len()};
        let channel = channels[index].clone();
        if let Some(dock) = &mut self.tabs[self.active].dock { dock.select(&channel); }
        self.selected_channel = Some(channel.clone());
        if let Some(pane) = self.tabs[self.active].panes.iter().find(|p|p.read(cx).name.as_ref()==channel) {
            pane.read(cx).draft.clone().update(cx,|input,cx|input.focus(window,cx));
        }
        self.schedule_save(cx); cx.notify();
    }
    fn confirm_close_channel(&mut self,name:String,window:&mut Window,cx:&mut Context<Self>){
        if self.close_channel_pending{return;}
        let Some(pane)=self.tabs[self.active].panes.iter().find(|p|p.read(cx).name.as_ref()==name).cloned()else{return;};
        self.close_channel_pending=true;let owner=cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,_,_|{let cancel=owner.clone();let close=owner.clone();let dismiss=owner.clone();let pane=pane.clone();
            dialog.title(format!("Close #{name}?" )).close_button(false).child("Your draft stays saved. Other channel tabs and splits remain open.")
                .on_ok(|_,_,_|true).on_close(move|_,_,cx|{let _=dismiss.update(cx,|this,cx|{this.close_channel_pending=false;cx.notify();});})
                .footer(div().h_flex().justify_end().gap_2()
                    .child(Button::new("cancel-channel-close").label("Cancel").on_click(move|_,window,cx|{let _=cancel.update(cx,|this,cx|{this.close_channel_pending=false;cx.notify();});window.close_dialog(cx);}))
                    .child(Button::new("confirm-channel-close").label("Close channel").on_click(move|_,window,cx|{pane.update(cx,|_,cx|cx.emit(PaneEvent::Close));let _=close.update(cx,|this,cx|{this.close_channel_pending=false;cx.notify();});window.close_dialog(cx);})))
        });cx.notify();
    }
    fn render_dock(&mut self, dock:&Dock, path:Vec<usize>, width:f32,height:f32,cx:&mut Context<Self>)->AnyElement {
        let tab_id=self.tabs[self.active].id;
        match dock {
            Dock::Leaf(name)|Dock::Deck{active:name,..}=> {
                let pane=self.tabs[self.active].panes.iter().find(|p|p.read(cx).name.as_ref()==name).cloned();
                let names=match dock{Dock::Deck{channels,..}=>channels.clone(),_=>vec![name.clone()]};
                let name=name.clone();let bounds_name=name.clone();let dock_bounds=self.dock_bounds.clone();
                let strip_name=name.clone();let strip_bounds=self.strip_bounds.clone();
                let tabs=names.iter().filter(|channel|!self.live_channels||*channel==&name||self.streams.get(channel)!=Some(false)).map(|channel|{let selected=channel==&name;let channel=channel.clone();
                    let drag_pane=self.tabs[self.active].panes.iter().find(|p|p.read(cx).name.as_ref()==channel).cloned();
                    let counts=drag_pane.as_ref().map(|p|p.read(cx).attention.counts()).unwrap_or_default();
                    let tab_name=channel.clone();let tab_bounds=self.tab_bounds.clone();
                    let stream_tip=self.streams.summary(&channel);
                    let item=div().id(SharedString::from(format!("channel-tab-{tab_id}-{channel}"))).h(px(28.)).px_2().flex().items_center().cursor_pointer()
                        .bg(rgb(if selected{theme::CONTROL}else{theme::PANEL})).border_b_2().border_color(rgb(if selected{0xA99CF4}else{theme::PANEL}))
                        .on_prepaint(move|bounds,_,_|{tab_bounds.borrow_mut().insert(tab_name.clone(),bounds);})
                        .hover(|s|s.bg(rgb(theme::HOVER)))
                        .tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(stream_tip.clone()).build(w,cx))
                        .child(stream_marker(&channel,self.streams.get(&channel)))
                        .child(format!("#{channel}"))
                        .when(counts.0>0,|el|el.child(div().ml_1().px_1().rounded(px(3.)).text_size(px(10.)).bg(rgb(if counts.1>0{0x403250}else{0x2B3038})).text_color(rgb(if counts.1>0{0xE4BCFA}else{theme::TEXT})).child(if counts.1>0{format!("@{}",crate::attention::count(counts.1))}else{crate::attention::count(counts.0)})))
                        .on_mouse_down(MouseButton::Middle,cx.listener({let channel=channel.clone();move|this,_,window,cx|{window.prevent_default();cx.stop_propagation();this.confirm_close_channel(channel.clone(),window,cx);}}))
                        .on_click(cx.listener({let channel=channel.clone();move|this,_,window,cx|{if let Some(d)=&mut this.tabs[this.active].dock{d.select(&channel);}this.selected_channel=Some(channel.clone());this.focus.focus(window,cx);this.schedule_save(cx);cx.notify();}}));
                    let owner=cx.entity().downgrade();let menu_channel=channel.clone();let menu_pane=name.clone();
                    let menu=move |menu:gpui_kit::component::menu::PopupMenu,_:&mut Window,_:&mut Context<gpui_kit::component::menu::PopupMenu>|{
                        let add=owner.clone();let add_channel=menu_channel.clone();let close=owner.clone();let close_channel=menu_channel.clone();let filter=owner.clone();let left=owner.clone();let left_channel=menu_channel.clone();let right=owner.clone();let right_channel=menu_channel.clone();let open_url=format!("https://www.twitch.tv/{menu_channel}");let copy_url=open_url.clone();let read=owner.clone();let read_channel=menu_channel.clone();
                        let details=owner.clone();let details_channel=menu_channel.clone();
                        let equal=owner.clone();let equal_channel=menu_channel.clone();let equal_all=owner.clone();let next_pane=owner.clone();let next_origin=menu_pane.clone();
                        menu.item(PopupMenuItem::new("Channel details · Ctrl+I").on_click(move|_,w,cx|{let _=details.update(cx,|this,cx|this.open_channel_details(details_channel.clone(),w,cx));}))
                            .item(PopupMenuItem::new("Mark channel read").on_click(move|_,_,cx|{let _=read.update(cx,|this,cx|{if let Some(pane)=this.tabs[this.active].panes.iter().find(|p|p.read(cx).name.as_ref()==read_channel){pane.update(cx,|p,cx|{p.attention.mark_all();cx.notify();});}cx.notify();});}))
                            .item(PopupMenuItem::new("Open stream in browser").on_click(move|_,_,cx|cx.open_url(&open_url)))
                            .item(PopupMenuItem::new("Copy channel URL").on_click(move|_,window,cx|{cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));window.push_notification(Notification::info("Channel URL copied"),cx);}))
                            .separator().item(PopupMenuItem::new("Add channel tab").on_click(move|_,window,cx|{let _=add.update(cx,|this,cx|{this.open_add(false,window,cx);this.add_target=Some(add_channel.clone());});}))
                            .item(PopupMenuItem::new("Toggle live-only channel tabs").on_click(move|_,_,cx|{let _=filter.update(cx,|this,cx|{this.live_channels=!this.live_channels;this.schedule_save(cx);cx.notify();});}))
                            .separator().item(PopupMenuItem::new("Move tab left").on_click(move|_,_,cx|{let _=left.update(cx,|this,cx|{if let Some(d)=&mut this.tabs[this.active].dock{d.shift_tab(&left_channel,false);}this.schedule_save(cx);cx.notify();});}))
                            .item(PopupMenuItem::new("Move tab right").on_click(move|_,_,cx|{let _=right.update(cx,|this,cx|{if let Some(d)=&mut this.tabs[this.active].dock{d.shift_tab(&right_channel,true);}this.schedule_save(cx);cx.notify();});}))
                            .separator().item(PopupMenuItem::new("Focus next pane · F6").on_click(move|_,w,cx|{let _=next_pane.update(cx,|this,cx|this.cycle_pane_from(true,Some(next_origin.clone()),w,cx));}))
                            .item(PopupMenuItem::new("Equalize this split").on_click(move|_,w,cx|{let _=equal.update(cx,|this,cx|this.equalize_panes(Some(equal_channel.clone()),false,w,cx));}))
                            .item(PopupMenuItem::new("Equalize all splits").on_click(move|_,w,cx|{let _=equal_all.update(cx,|this,cx|this.equalize_panes(None,true,w,cx));}))
                            .separator().item(PopupMenuItem::new("Close channel…").on_click(move|_,window,cx|{let _=close.update(cx,|this,cx|this.confirm_close_channel(close_channel.clone(),window,cx));}))
                    };
                    if let Some(pane)=drag_pane{item.on_drag(DraggedChannel{pane,name:channel.clone()},move|drag,_,_,cx|cx.new(|_|DragPreview(drag.name.clone()))).context_menu(menu).into_any_element()}else{item.context_menu(menu).into_any_element()}
                }).collect::<Vec<_>>();
                let preview=if self.drop_target.as_ref()==Some(&name){self.drop_edge}else{None};
                div().id(SharedString::from(format!("dock-{tab_id}-{name}"))).relative().size_full().min_w_0().min_h_0()
                    .on_drag_move(cx.listener(move|this,event:&DragMoveEvent<DraggedChannel>,_,cx|{this.track_drop(event,cx);}))
                    .on_prepaint(move|bounds,_,_|{dock_bounds.borrow_mut().insert(bounds_name.clone(),bounds);})
                    .on_drop(cx.listener(move|this,drag:&DraggedChannel,w,cx|{cx.stop_propagation();this.drop_at_pointer(drag,tab_id,w,cx);}))
                    .v_flex().child(div().id(SharedString::from(format!("channel-strip-{tab_id}-{name}"))).h_flex().flex_wrap().min_h(px(28.)).flex_shrink_0().bg(rgb(theme::PANEL))
                        .on_prepaint(move|bounds,_,_|{strip_bounds.borrow_mut().insert(strip_name.clone(),bounds);}).children(tabs)
                        .child(Button::new(SharedString::from(format!("add-tab-{tab_id}-{name}"))).ghost().xsmall().label("+").tooltip("Add channel tab · Ctrl+K")
                            .on_click(cx.listener({let target=name.clone();move|this,_,window,cx|{this.open_add(false,window,cx);this.add_target=Some(target.clone());}}))))
                    .child(div().flex_1().min_h_0().children(pane)).when(cx.has_active_drag(),|el|el.children(preview.map(Self::drop_preview))).into_any_element()
            }
            Dock::Split {vertical,weights,children}=> {
                let key=format!("{tab_id}:{path:?}");
                let state=self.tabs[self.active].dock_states.entry(key.clone()).or_insert_with(||cx.new(|_|ResizableState::default())).clone();
                let total=weights.iter().copied().sum::<f32>();
                let mut panels=Vec::new();
                for (ix,child) in children.iter().enumerate() {
                    let share=if total>0. && weights.len()==children.len(){weights[ix]/total}else{1./children.len() as f32};
                    let min=child.minimum();
                    let mut subpath=path.clone();subpath.push(ix);
                    let (cw,ch)=if *vertical{(width,height*share)}else{(width*share,height)};
                    let content=self.render_dock(child,subpath,cw,ch,cx);
                    panels.push(resizable_panel().size(px(if *vertical{ch}else{cw})).size_range(px(if *vertical{min.1}else{min.0})..Pixels::MAX).child(content));
                }
                let group=if *vertical{v_resizable(SharedString::from(key))}else{h_resizable(SharedString::from(key))};
                group.with_state(&state).children(panels).on_resize(cx.listener(move|this,state:&Entity<ResizableState>,_,cx|{
                    if let Some(tab)=this.tabs.iter_mut().find(|t|t.id==tab_id){if let Some(dock)=&mut tab.dock {dock.resize(&path,state.read(cx).sizes().iter().map(|p|f32::from(*p)).collect());}}
                    this.schedule_save(cx);
                })).into_any_element()
            }
        }
    }
    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
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
            .child(Button::new("new-workspace-sidebar").ghost().small().label("+ New workspace").tooltip("Ctrl+T").on_click(cx.listener(|this,_,window,cx|this.new_tab(window,cx))))
            .when(self.live_workspaces,|el|el.child(Button::new("show-offline-workspaces").ghost().xsmall().label("Live only · show all").on_click(cx.listener(|this,_,_,cx|{this.live_workspaces=false;this.schedule_save(cx);cx.notify();}))))
            .child(
                div()
                    .id("workspace-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(self.shown_tabs(cx).into_iter().map(|ix| {
                        let tab=&self.tabs[ix];
                        let counts=tab.panes.iter().map(|p|p.read(cx).attention.counts()).fold((0,0),|(u,h),(a,b)|(u+a,h+b));
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
                            .on_mouse_down(MouseButton::Middle,cx.listener({let id=tab.id;move|this,_,window,cx|{window.prevent_default();cx.stop_propagation();this.confirm_close_tab(id,window,cx);}}))
                            .on_drag(DraggedTab{id:tab.id,name:tab.name.clone()},move|drag,_,_,cx|cx.new(|_|DragPreview(drag.name.clone())))
                            .drag_over::<DraggedTab>(|style,_,_,_|style.border_t_2().border_color(rgb(0xA99CF4)))
                            .on_drop(cx.listener({let id=tab.id;move|this,drag:&DraggedTab,_,cx|this.reorder_tab(drag.id,id,cx)}))
                            .drag_over::<DraggedChannel>(|style,_,_,_|style.bg(rgb(theme::HOVER)))
                            .on_drop(cx.listener({let id=tab.id;move|this,drag:&DraggedChannel,window,cx|{this.drop_target=None;this.move_pane(drag,id,None,window,cx);}}))
                            .child(div().h_flex().gap_1().child(div().flex_1().min_w_0().overflow_hidden().child(tab.name.clone()))
                                .child(Button::new(("close-sidebar-workspace",tab.id)).ghost().xsmall().label("×").tooltip("Close workspace · Middle-click / Ctrl+W")
                                    .on_click(cx.listener({let id=tab.id;move|this,_,window,cx|{cx.stop_propagation();this.confirm_close_tab(id,window,cx);}}))))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::MUTED))
                                    .child(format!("{} channels{}", tab.panes.len(),if counts.0>0{format!(" · {} unread · @{}",crate::attention::count(counts.0),crate::attention::count(counts.1))}else{String::new()})),
                            )
                    })),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .p_2()
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .text_size(px(11.))
                    .text_color(rgb(theme::MUTED))
                    .child("LOCAL WORKSPACES")
                    .child("Channels and drafts stay on this PC."),
            )
            .into_any_element()
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
impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Retarget from the current sampled width so rapid reversals stay smooth.
        // GPUI's motion primitive honors the OS reduced-motion preference.
        let sidebar_width = gpui_kit::base::motion::transition(
            "jawjack-sidebar-width",
            if self.sidebar { 190.0_f32 } else { 0.0_f32 },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(180)),
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
        } else { "○ Live chat disconnected".to_owned() };
        let counts=self.attention_counts(cx);
        let view_focus = self.focus.clone();
        let pointer_guard = cx.entity().downgrade();
        let live_workspaces=self.live_workspaces;let live_channels=self.live_channels;
        let can_reopen = !self.closed_tabs.is_empty();
        let can_discard = self.close_failed;
        let can_left = self.active > 0;
        let can_right = self.active + 1 < self.tabs.len();
        let tab = &self.tabs[self.active];
        let dock=tab.dock.clone();
        self.dock_bounds.borrow_mut().clear();
        self.strip_bounds.borrow_mut().clear();
        self.tab_bounds.borrow_mut().clear();
        let content=if let Some(dock)=dock {
            let min=dock.minimum();
            let available=window.viewport_size();
            let width=(f32::from(available.width)-sidebar_width-4.).max(min.0);
            let height=(f32::from(available.height)-60.).max(min.1);
            let body=self.render_dock(&dock,Vec::new(),width,height,cx);
            div().id("dock-overflow").size_full().overflow_scroll().child(div().size_full().min_w(px(min.0)).min_h(px(min.1)).child(body)).into_any_element()
        } else {
            div().v_flex().size_full().items_center().justify_center().gap_4()
                .child(div().text_size(px(30.)).font_weight(FontWeight::SEMIBOLD).child("Make room for your channels."))
                .child(div().text_color(rgb(theme::MUTED)).child("Drag a channel to any panel edge to split your workspace."))
                .child(Button::new("empty-add").label("+ Add a channel").on_click(cx.listener(|this,_,window,cx|this.open_add(false,window,cx))))
                .into_any_element()
        };
        div().id("workspace").relative().track_focus(&self.focus).key_context("ChatWorkspace").v_flex().size_full().font_family("Segoe UI").text_size(px(13.)).bg(rgb(theme::SHELL)).text_color(rgb(theme::TEXT))
            .capture_any_mouse_down(cx.listener(|this,event:&MouseDownEvent,window,cx|{if event.button==MouseButton::Left{
                cx.stop_active_drag(window); this.pointer_owner=Some(this.control_pointer_active); this.release_position=None;
            }}))
            .capture_any_mouse_up(cx.listener(|this,event:&MouseUpEvent,window,cx|{if event.button==MouseButton::Left{
                let valid=this.pointer_owner.take()==Some(this.control_pointer_active);
                this.release_position=valid.then_some(event.position);this.release_from_control=this.control_pointer_active;
                if !valid{cx.stop_active_drag(window);this.drop_edge=None;this.drop_target=None;cx.notify();}
            }}))
            .child(canvas(|_,_,_|(),move|_,_,window,_|{
                let zoom=pointer_guard.clone();
                window.on_mouse_event(move|event:&ScrollWheelEvent,phase,window,cx|{
                    if !phase.capture()||!event.modifiers.control||window.has_active_dialog(cx){return;}
                    let delta=match event.delta{ScrollDelta::Lines(p)=>p.y.signum(),ScrollDelta::Pixels(p)=>f32::from(p.y)/40.};
                    let _=zoom.update(cx,|this,cx|{this.zoom_accumulator+=delta;let steps=this.zoom_accumulator.trunc().clamp(-4.,4.);if steps!=0.{this.zoom_accumulator-=steps;this.font_feedback(steps,false,window,cx);}});
                    window.prevent_default();cx.stop_propagation();
                });
                let owner=pointer_guard.clone();
                window.on_mouse_event(move|event:&MouseMoveEvent,phase,window,cx|{
                    if !phase.capture(){return;}
                    let _=owner.update(cx,|this,cx|{
                        let crossed=this.pointer_owner.is_some_and(|source|source!=this.control_pointer_active);
                        if crossed || event.pressed_button!=Some(MouseButton::Left){
                            this.pointer_owner=None;this.release_position=None;this.drop_edge=None;this.drop_target=None;
                            if cx.stop_active_drag(window){cx.notify();}
                            // Also clear a framework drag created later in this event from a stale down.
                            window.defer(cx,|window,cx|{cx.stop_active_drag(window);});
                        }
                    });
                });
            }).absolute().size_0())
            .on_action(cx.listener(|this,_:&crate::FindChat,w,cx|{let panes=this.visible_panes(cx);let selected=this.selected_channel.as_ref();if let Some(pane)=panes.iter().find(|p|Some(&p.read(cx).name.to_string())==selected).or(panes.first()){pane.update(cx,|p,cx|p.open_search(w,cx));}cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&FocusNextPane,w,cx|this.cycle_pane(true,w,cx)))
            .on_action(cx.listener(|this,_:&FocusPreviousPane,w,cx|this.cycle_pane(false,w,cx)))
            .on_action(cx.listener(|this,_:&FocusPaneLeft,w,cx|this.focus_neighbor(-1,0,w,cx)))
            .on_action(cx.listener(|this,_:&FocusPaneRight,w,cx|this.focus_neighbor(1,0,w,cx)))
            .on_action(cx.listener(|this,_:&FocusPaneUp,w,cx|this.focus_neighbor(0,-1,w,cx)))
            .on_action(cx.listener(|this,_:&FocusPaneDown,w,cx|this.focus_neighbor(0,1,w,cx)))
            .on_action(cx.listener(|this,_:&EqualizeSplit,w,cx|this.equalize_panes(None,false,w,cx)))
            .on_action(cx.listener(|this,_:&EqualizeAllSplits,w,cx|this.equalize_panes(None,true,w,cx)))
            .on_action(cx.listener(|this,_:&SwitchChannel,w,cx|this.open_switcher(w,cx)))
            .on_action(cx.listener(|this,_:&ChannelDetails,w,cx|this.open_focused_channel_details(w,cx)))
            .on_action(cx.listener(|this,_:&OpenActivity,w,cx|this.open_activity(w,cx)))
            .on_action(cx.listener(|this,_:&MarkAllRead,_,cx|this.mark_all_read(cx)))
            .on_action(cx.listener(|this,_:&FontLarger,w,cx|this.font_feedback(1.,false,w,cx)))
            .on_action(cx.listener(|this,_:&FontSmaller,w,cx|this.font_feedback(-1.,false,w,cx)))
            .on_action(cx.listener(|this,_:&ResetFont,w,cx|this.font_feedback(0.,true,w,cx)))
            .on_action(cx.listener(|this,_:&NewTab,w,cx|this.new_tab(w,cx)))
            .on_action(cx.listener(|this,_:&CloseTab,window,cx|{this.confirm_close_tab(this.tabs[this.active].id,window,cx);}))
            .on_action(cx.listener(|this,_:&ReopenTab,window,cx|{this.reopen_tab(cx);this.focus.focus(window,cx);}))
            .on_action(cx.listener(|this,_:&NextTab,w,cx|{this.cycle_tab(true,cx);this.focus.focus(w,cx);}))
            .on_action(cx.listener(|this,_:&PreviousTab,w,cx|{this.cycle_tab(false,cx);this.focus.focus(w,cx);}))
            .on_action(cx.listener(|this,_:&NextChannel,w,cx|this.cycle_channel(true,w,cx)))
            .on_action(cx.listener(|this,_:&PreviousChannel,w,cx|this.cycle_channel(false,w,cx)))
            .on_action(cx.listener(|this,_:&AddChannel,w,cx|this.open_add(false,w,cx)))
            .on_action(cx.listener(|this,_:&RenameWorkspace,w,cx|this.open_add(true,w,cx)))
            .on_action(cx.listener(|this,_:&MoveTabLeft,_,cx|this.move_tab(-1,cx)))
            .on_action(cx.listener(|this,_:&MoveTabRight,_,cx|this.move_tab(1,cx)))
            .on_action(cx.listener(|this,_:&ToggleSidebar,_,cx|{this.sidebar=!this.sidebar;this.schedule_save(cx);cx.notify();}))
            .on_action(cx.listener(|this,_:&ToggleAppearance,_,cx|{this.settings=!this.settings;cx.notify();}))
            .on_action(cx.listener(|this,_:&ToggleOrientation,_,cx|this.flip_split(cx)))
            .on_action(cx.listener(|this,_:&ToggleLiveWorkspaces,_,cx|{this.live_workspaces=!this.live_workspaces;this.schedule_save(cx);cx.notify();}))
            .on_action(cx.listener(|this,_:&ToggleLiveChannels,_,cx|{this.live_channels=!this.live_channels;this.schedule_save(cx);cx.notify();}))
            .on_action(cx.listener(|this,_:&QuitWithoutSaving,_,cx|{if this.close_failed {cx.quit();}}))
            .child(div().h_flex().h(px(32.)).flex_shrink_0().border_b_1().border_color(rgb(theme::BORDER))
                .child(Button::new("sidebar").ghost().small().label("☰").tooltip("Workspaces").on_click(cx.listener(|this,_,_,cx|{this.sidebar=!this.sidebar;this.schedule_save(cx);cx.notify();})))
                .child(div().h_flex().px_2().gap_1().window_control_area(WindowControlArea::Drag).child(div().text_color(rgb(0xA99CF4)).font_weight(FontWeight::BOLD).child("//"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(11.)).child("JAWJACK")))
                .child(div().flex_1().min_w(px(8.)).h_full().window_control_area(WindowControlArea::Drag))
                .child(Button::new("activity-menu").ghost().xsmall().label(if counts.1>0{format!("@{}",crate::attention::count(counts.1))}else if counts.0>0{format!("• {}",crate::attention::count(counts.0))}else{"@".into()}).tooltip("Highlights & unread · Ctrl+Shift+M").on_click(cx.listener(|this,_,w,cx|this.open_activity(w,cx))))
                .child(Button::new("account-menu").ghost().xsmall().label(self.account.read(cx).label()).tooltip("Twitch account")
                    .on_click(cx.listener(|this,_,window,cx|{let account=this.account.clone();let width=(f32::from(window.viewport_size().width)-24.).min(380.);window.open_dialog(cx,move|dialog,_,_|dialog.w(px(width)).title("Twitch account").child(account.clone()));})))
                .child(Button::new("settings-menu").ghost().small().label("⚙").tooltip("Settings and workspace actions").dropdown_menu(move|menu,_,_|menu.action_context(view_focus.clone())
                    .menu("Switch channel · Ctrl+P",Box::new(SwitchChannel))
                    .menu("Highlights & unread",Box::new(OpenActivity))
                    .menu("Mark all read",Box::new(MarkAllRead))
                    .menu("Appearance & memory",Box::new(ToggleAppearance))
                    .menu(if live_channels{"Show all channel tabs"}else{"Only show live channel tabs"},Box::new(ToggleLiveChannels))
                    .separator().menu("New workspace",Box::new(NewTab)).menu("Rename workspace",Box::new(RenameWorkspace))
                    .menu("Close workspace…",Box::new(CloseTab)).menu_with_enable("Reopen workspace",Box::new(ReopenTab),can_reopen)
                    .menu_with_enable("Move workspace up",Box::new(MoveTabLeft),can_left).menu_with_enable("Move workspace down",Box::new(MoveTabRight),can_right)
                    .menu(if live_workspaces{"Show all workspaces"}else{"Only show live workspaces"},Box::new(ToggleLiveWorkspaces))
                    .when(can_discard,|menu|menu.separator().menu("Discard unsaved changes and quit",Box::new(QuitWithoutSaving)))))
                .child(caption_control("minimize",IconName::WindowMinimize,WindowControlArea::Min,false))
                .child(caption_control("maximize",if window.is_maximized(){IconName::WindowRestore}else{IconName::WindowMaximize},WindowControlArea::Max,false))
                .child(caption_control("close",IconName::WindowClose,WindowControlArea::Close,true)))
            .child(div().h_flex().items_stretch().flex_1().min_h_0().overflow_hidden()
                .when(sidebar_width > 0.1, |el| el.child(
                    div().w(px(sidebar_width)).h_full().flex_shrink_0().overflow_hidden()
                        .child(div().relative().left(px(sidebar_width - 190.)).w(px(190.)).h_full()
                            .child(self.render_sidebar(cx)))))
                .child(div().v_flex().flex_1().h_full().min_w_0().min_h_0()
                    .when(self.adding,|el|el.child(div().v_flex().p_3().gap_2().bg(rgb(theme::ELEVATED)).border_b_1().border_color(rgb(theme::BORDER))
                        .child(div().text_size(px(12.)).child(if self.renaming{"Rename workspace"}else{"Add a Twitch channel to this workspace"}))
                        .child(div().h_flex().gap_2().child(div().flex_1().child(Input::new(&self.channel_input)))
                            .child(Button::new("accept-channel").label(if self.renaming{"Save"}else{"Add channel"}).on_click(cx.listener(|this,_,w,cx|this.accept_input(w,cx))))
                            .child(Button::new("cancel-add").ghost().label("Cancel").on_click(cx.listener(|this,_,window,cx|{this.adding=false;this.focus.focus(window,cx);cx.notify();}))))
                        .when_some(self.add_error.clone(),|el,error|el.child(div().text_size(px(12.)).text_color(rgb(0xF29D9D)).child(error)))))
                    .child(div().h_flex().items_stretch().flex_1().min_h_0().overflow_hidden().p(px(2.)).child(div().id("channel-dock").relative().flex_1().h_full().min_w_0().min_h_0()
                        .child(content)
                        )
                        .when(self.settings,|el|el.child(div().absolute().top(px(34.)).right(px(4.)).w(px((f32::from(window.viewport_size().width)-8.).min(300.))).max_h(px(f32::from(window.viewport_size().height)-60.)).id("settings-card").occlude().overflow_hidden().v_flex().bg(rgb(theme::PANEL)).border_1().border_color(rgb(theme::BORDER)).rounded(px(6.))
                            .child(div().h_flex().flex_shrink_0().p_2().gap_2().border_b_1().border_color(rgb(theme::BORDER))
                                .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("Appearance & memory"))
                                .child(Button::new("close-settings").ghost().small().label("Done").on_click(cx.listener(|this,_,w,cx|{this.settings=false;this.focus.focus(w,cx);cx.notify();}))))
                            .child(div().id("settings-scroll").min_h_0().overflow_y_scroll().v_flex().p_3().gap_2()
                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Dark Studio · Segoe UI"))
                            .child(div().h_flex().gap_2().child(Button::new("font-minus").small().label("A−").on_click(cx.listener(|this,_,_,cx|this.change_font(-1.,cx))))
                                .child(format!("{} px",self.font_size as u32)).child(Button::new("font-reset").small().label("Reset").tooltip("Ctrl+0").on_click(cx.listener(|this,_,w,cx|this.font_feedback(0.,true,w,cx)))).child(Button::new("font-plus").small().label("A+").on_click(cx.listener(|this,_,_,cx|this.change_font(1.,cx)))))
                            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Ctrl+wheel or Ctrl+plus/minus · Ctrl+0 resets"))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Timestamps"))
                            .child(div().h_flex().flex_wrap().gap_1().children([(0u8,"Off"),(1,"Hours/minutes"),(2,"With seconds")].into_iter().map(|(mode,label)|Button::new(("timestamps",mode as usize)).small().label(label).disabled(self.timestamps==mode).on_click(cx.listener(move|this,_,_,cx|this.change_timestamps(mode,cx))))))
                            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Twitch event time in your local timezone. Hover for date and UTC offset."))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Highlights"))
                            .child(Input::new(&self.highlight_input).small())
                            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Your mentions and replies are included. Add up to 8 words/phrases (40 characters each). Whole-word, case-insensitive; no alerts or sounds."))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Chat memory"))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Newest messages per channel. Older history is released; drafts stay saved."))
                            .child(div().h_flex().flex_wrap().gap_1().children([500usize,1000,2000,5000,10_000].into_iter().map(|limit|
                                Button::new(("history-limit",limit)).small().label(if limit<1000 {limit.to_string()}else{format!("{}k",limit/1000)})
                                    .disabled(limit==self.history_limit).on_click(cx.listener(move|this,_,_,cx|this.change_history_limit(limit,cx)))
                            )))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Image cache is separately bounded to 48 MiB. Visible chat rows are drawn on demand."))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Tabs, splits, channel drafts and appearance are saved locally."))
                            ))))))
            .child(div().h_flex().h(px(20.)).flex_shrink_0().px_2().gap_2().border_t_1().border_color(rgb(theme::BORDER)).text_size(px(10.)).text_color(rgb(theme::MUTED))
                .child(div().flex_1().min_w_0().overflow_hidden().child(live_status)).child(self.save_status.clone()))
    }
}


fn stream_marker(channel:&str,status:Option<bool>)->impl IntoElement {
    let (color,label)=match status {
        Some(true)=>(theme::STREAM_LIVE,"Stream live"),
        Some(false)=>(theme::STREAM_OFFLINE,"Stream offline · chat may still be connected"),
        None=>(theme::STREAM_UNKNOWN,"Stream status unknown · not checked yet or unavailable"),
    };
    div().id(SharedString::from(format!("stream-status-{channel}"))).w(px(12.)).h(px(16.)).mr_1().flex_shrink_0().flex().items_center().justify_center().text_color(rgb(color))
        .tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(label).build(w,cx))
        .child(match status {
            Some(live)=>div().size(px(8.)).rounded(px(4.)).border_1().border_color(rgb(color)).when(live,|el|el.bg(rgb(color))).into_any_element(),
            None=>div().text_size(px(13.)).line_height(px(16.)).child("◇").into_any_element(),
        })
}
