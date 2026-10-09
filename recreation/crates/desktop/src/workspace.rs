use crate::{ChannelPane, PaneEvent, caption_control, storage, theme};
use gpui_kit::component::{
    IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::DropdownMenu,
    notification::Notification,
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
    tab::{Tab, TabBar},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};

gpui_kit::actions!(
    workspace,
    [
        NewTab,
        CloseTab,
        ReopenTab,
        NextTab,
        PreviousTab,
        AddChannel,
        RenameWorkspace,
        MoveTabLeft,
        MoveTabRight,
        ToggleSidebar,
        ToggleAppearance,
        ToggleOrientation,
        QuitWithoutSaving
    ]
);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-t", NewTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-w", CloseTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-t", ReopenTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-tab", NextTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, Some("ChatWorkspace")),
        KeyBinding::new("ctrl-k", AddChannel, Some("ChatWorkspace")),
    ]);
}
struct WorkspaceTab {
    id: u64,
    name: String,
    panes: Vec<Entity<ChannelPane>>,
    vertical: bool,
    sizes: Vec<f32>,
    split: Entity<ResizableState>,
}
pub struct Workbench {
    account: Entity<crate::auth::TwitchAccount>,
    control_enabled: bool,
    tabs: Vec<WorkspaceTab>,
    closed_tabs: Vec<WorkspaceTab>,
    active: usize,
    next_id: u64,
    focus: FocusHandle,
    tab_scroll: ScrollHandle,
    channel_input: Entity<InputState>,
    adding: bool,
    renaming: bool,
    add_error: Option<String>,
    sidebar: bool,
    settings: bool,
    font_size: f32,
    drafts: BTreeMap<String, String>,
    save_revision: u64,
    save_enabled: bool,
    save_status: String,
    close_failed: bool,
}
impl Workbench {
    pub fn focus_workspace(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }
    pub fn visible_panes(&self) -> Vec<Entity<ChannelPane>> {
        self.tabs[self.active].panes.clone()
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
                let pane = self.tabs[self.active]
                    .panes
                    .get(pane)
                    .ok_or("No such visible pane")?;
                let draft = pane.read(cx).draft.clone();
                draft.update(cx, |input, cx| input.focus(window, cx));
            }
            "transcript" => {
                let pane = self.tabs[self.active]
                    .panes
                    .get(pane)
                    .ok_or("No such visible pane")?;
                let focus = pane.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            "workspace" => self.focus.focus(window, cx),
            _ => return Err("Target is unavailable".into()),
        }
        Ok(())
    }
    pub fn inspection(&self, cx: &App) -> Value {
        json!({"active":self.active,"tabs":self.tabs.iter().map(|t|json!({"id":t.id,"name":t.name,"vertical":t.vertical,"channels":t.panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>(),"adding_channel":self.adding,"appearance_open":self.settings,"font_size":self.font_size,"save_status":self.save_status})
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
        let mut this = Self {
            account: cx.new(|cx| crate::auth::TwitchAccount::new(!control_enabled, cx)),
            control_enabled,
            tabs: vec![],
            closed_tabs: vec![],
            active: 0,
            next_id: state["next_id"]
                .as_u64()
                .filter(|id| *id > 0 && *id < 1_000_000)
                .unwrap_or(1),
            focus: cx.focus_handle(),
            tab_scroll: ScrollHandle::new(),
            channel_input: input,
            adding: false,
            renaming: false,
            add_error: None,
            sidebar: state["sidebar"].as_bool().unwrap_or(false),
            settings: false,
            font_size: state["font_size"]
                .as_f64()
                .filter(|n| n.is_finite())
                .unwrap_or(14.)
                .clamp(12., 24.) as f32,
            drafts,
            save_revision: 0,
            save_enabled,
            save_status,
            close_failed: false,
        };
        if let Some(tabs) = state["tabs"].as_array() {
            for item in tabs.iter().take(16) {
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
                    for channel in channels.iter().take(2) {
                        if let Some(name) = channel.as_str().and_then(valid_channel) {
                            if !panes.iter().any(|p: &Entity<ChannelPane>| {
                                p.read(cx).name.as_ref() == name.as_str()
                            }) {
                                panes.push(this.make_pane(id, &name, window, cx));
                            }
                        }
                    }
                }
                let sizes = item["sizes"]
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
                this.tabs.push(WorkspaceTab {
                    id,
                    name,
                    panes,
                    vertical: item["vertical"].as_bool().unwrap_or(false),
                    sizes,
                    split: cx.new(|_| ResizableState::default()),
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
            });
            this.next_id += 1;
        }
        this.active = (state["active"].as_u64().unwrap_or(0) as usize).min(this.tabs.len() - 1);
        this
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
        let pane = cx.new(|cx| ChannelPane::new(name, &draft, self.font_size, window, cx));
        let channel = name.to_owned();
        cx.subscribe(&pane, move |this, pane, event, cx| {
            let text = pane.read(cx).draft.read(cx).value().to_string();
            this.drafts.insert(format!("{tab_id}:{channel}"), text);
            if matches!(event, PaneEvent::Close) {
                if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == tab_id) {
                    if let Some(ix) = tab.panes.iter().position(|p| *p == pane) {
                        tab.panes.remove(ix);
                        tab.split.update(cx, |state, cx| state.remove_panel(ix, cx));
                        tab.sizes = tab
                            .split
                            .read(cx)
                            .sizes()
                            .iter()
                            .map(|p| f32::from(*p))
                            .collect();
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
        for tab in &self.tabs {
            for pane in &tab.panes {
                let pane = pane.read(cx);
                drafts.insert(
                    format!("{}:{}", tab.id, pane.name),
                    pane.draft.read(cx).value().to_string(),
                );
            }
        }
        json!({"version":1,"next_id":self.next_id,"active":self.active,"sidebar":self.sidebar,"font_size":self.font_size,"drafts":drafts,
            "tabs":self.tabs.iter().map(|t|json!({"id":t.id,"name":t.name,"vertical":t.vertical,"sizes":t.sizes,"channels":t.panes.iter().map(|p|p.read(cx).name.to_string()).collect::<Vec<_>>()})).collect::<Vec<_>>()})
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
    fn select_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.tabs.len() {
            self.active = ix;
            self.tab_scroll.scroll_to_item(ix);
            self.adding = false;
            self.renaming = false;
            self.schedule_save(cx);
            cx.notify();
        }
    }
    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() >= 16 {
            window.push_notification(Notification::info("Keep up to 16 workspaces open"), cx);
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(WorkspaceTab {
            id,
            name: format!("Workspace {id}"),
            panes: vec![],
            vertical: false,
            sizes: vec![],
            split: cx.new(|_| ResizableState::default()),
        });
        self.select_tab(self.tabs.len() - 1, cx);
        self.open_add(false, window, cx);
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
        if self.tabs.len() >= 16 {
            return;
        }
        if let Some(tab) = self.closed_tabs.pop() {
            self.tabs.push(tab);
            self.select_tab(self.tabs.len() - 1, cx);
        }
    }
    fn open_add(&mut self, rename: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.adding = true;
        self.renaming = rename;
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
            if self.tabs[self.active].panes.len() >= 2 {
                self.add_error = Some(
                    "Two splits per workspace in this build. Open another tab for more channels."
                        .into(),
                );
                cx.notify();
                return;
            }
            let id = self.tabs[self.active].id;
            let pane = self.make_pane(id, &channel, window, cx);
            let tab = &mut self.tabs[self.active];
            tab.panes.push(pane);
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
    fn flip_split(&mut self, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active];
        tab.vertical = !tab.vertical;
        tab.sizes.clear();
        tab.split = cx.new(|_| ResizableState::default());
        self.schedule_save(cx);
        cx.notify();
    }
    fn change_font(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.font_size = (self.font_size + delta).clamp(12., 24.);
        for pane in self.panes() {
            pane.update(cx, |p, cx| {
                p.font_size = self.font_size;
                cx.notify();
            });
        }
        self.schedule_save(cx);
        cx.notify();
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
            .child(
                div()
                    .id("workspace-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(self.tabs.iter().enumerate().map(|(ix, tab)| {
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
                            .child(tab.name.clone())
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::MUTED))
                                    .child(format!("{} channels", tab.panes.len())),
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
        let menu_focus = self.focus.clone();
        let view_focus = self.focus.clone();
        let can_reopen = !self.closed_tabs.is_empty();
        let can_discard = self.close_failed;
        let can_left = self.active > 0;
        let can_right = self.active + 1 < self.tabs.len();
        let tab = &self.tabs[self.active];
        let tab_id = tab.id;
        let panels = tab
            .panes
            .iter()
            .enumerate()
            .map(|(ix, pane)| {
                resizable_panel()
                    .size(px(tab.sizes.get(ix).copied().unwrap_or(if tab.vertical {
                        300.
                    } else {
                        500.
                    })))
                    .size_range(px(if tab.vertical { 180. } else { 240. })..Pixels::MAX)
                    .child(pane.clone())
            })
            .collect::<Vec<_>>();
        let content = if tab.panes.is_empty() {
            div().v_flex().size_full().items_center().justify_center().gap_4()
            .child(div().text_size(px(30.)).font_weight(FontWeight::SEMIBOLD).child("Make room for your channels."))
            .child(div().text_size(px(14.)).text_color(rgb(theme::MUTED)).child("Keep streams together in tabs. Split a workspace when one channel isn’t enough."))
            .child(Button::new("empty-add").label("+ Add a channel").on_click(cx.listener(|this,_,window,cx|this.open_add(false,window,cx))))
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Ctrl+K  add channel       Ctrl+T  new workspace"))
            .into_any_element()
        } else {
            let group = if tab.vertical {
                v_resizable(("split", tab_id))
            } else {
                h_resizable(("split", tab_id))
            };
            div()
                .size_full()
                .child(
                    group
                        .with_state(&tab.split)
                        .children(panels)
                        .on_resize(cx.listener(
                            move |this, state: &Entity<ResizableState>, _, cx| {
                                if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == tab_id) {
                                    tab.sizes = state
                                        .read(cx)
                                        .sizes()
                                        .iter()
                                        .map(|p| f32::from(*p))
                                        .collect();
                                }
                                this.schedule_save(cx);
                            },
                        )),
                )
                .into_any_element()
        };
        div().id("workspace").track_focus(&self.focus).key_context("ChatWorkspace").v_flex().size_full().font_family("Segoe UI").text_size(px(13.)).bg(rgb(theme::SHELL)).text_color(rgb(theme::TEXT))
            .on_action(cx.listener(|this,_:&NewTab,w,cx|this.new_tab(w,cx)))
            .on_action(cx.listener(|this,_:&CloseTab,window,cx|{this.close_tab(this.active,cx);this.focus.focus(window,cx);}))
            .on_action(cx.listener(|this,_:&ReopenTab,window,cx|{this.reopen_tab(cx);this.focus.focus(window,cx);}))
            .on_action(cx.listener(|this,_:&NextTab,w,cx|{this.select_tab((this.active+1)%this.tabs.len(),cx);this.focus.focus(w,cx);}))
            .on_action(cx.listener(|this,_:&PreviousTab,w,cx|{this.select_tab((this.active+this.tabs.len()-1)%this.tabs.len(),cx);this.focus.focus(w,cx);}))
            .on_action(cx.listener(|this,_:&AddChannel,w,cx|this.open_add(false,w,cx)))
            .on_action(cx.listener(|this,_:&RenameWorkspace,w,cx|this.open_add(true,w,cx)))
            .on_action(cx.listener(|this,_:&MoveTabLeft,_,cx|this.move_tab(-1,cx)))
            .on_action(cx.listener(|this,_:&MoveTabRight,_,cx|this.move_tab(1,cx)))
            .on_action(cx.listener(|this,_:&ToggleSidebar,_,cx|{this.sidebar=!this.sidebar;this.schedule_save(cx);cx.notify();}))
            .on_action(cx.listener(|this,_:&ToggleAppearance,_,cx|{this.settings=!this.settings;cx.notify();}))
            .on_action(cx.listener(|this,_:&ToggleOrientation,_,cx|this.flip_split(cx)))
            .on_action(cx.listener(|this,_:&QuitWithoutSaving,_,cx|{if this.close_failed {cx.quit();}}))
            .child(div().h_flex().h(px(40.)).flex_shrink_0().border_b_1().border_color(rgb(theme::BORDER))
                .child(Button::new("sidebar").ghost().small().label("☰").tooltip("Toggle workspace sidebar").on_click(cx.listener(|this,_,_,cx|{this.sidebar=!this.sidebar;this.schedule_save(cx);cx.notify();})))
                .child(div().h_flex().px_4().gap_2().child(div().text_color(rgb(0xA99CF4)).font_weight(FontWeight::BOLD).child("//"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(12.)).child("JAWJACK")))

                .child(Button::new("workspace-menu").ghost().small().label("Workspace").dropdown_menu(move|menu,_,_|menu.action_context(menu_focus.clone())
                    .menu("New workspace",Box::new(NewTab)).menu("Add channel",Box::new(AddChannel)).menu("Rename workspace",Box::new(RenameWorkspace))
                    .separator().menu_with_enable("Move tab left",Box::new(MoveTabLeft),can_left).menu_with_enable("Move tab right",Box::new(MoveTabRight),can_right)
                    .separator().menu("Close workspace",Box::new(CloseTab)).menu_with_enable("Reopen closed workspace",Box::new(ReopenTab),can_reopen)
                    .when(can_discard,|menu|menu.separator().menu("Discard unsaved changes and quit",Box::new(QuitWithoutSaving)))))
                .child(Button::new("view-menu").ghost().small().label("View").dropdown_menu(move|menu,_,_|menu.action_context(view_focus.clone())
                    .menu("Toggle sidebar",Box::new(ToggleSidebar)).menu("Appearance",Box::new(ToggleAppearance)).menu("Rotate split layout",Box::new(ToggleOrientation))))
                .child(div().flex_1().mt(px(6.)).h(px(34.)).window_control_area(WindowControlArea::Drag))
                .when(self.control_enabled,|el|el.child(div().px_3().text_size(px(10.)).text_color(rgb(theme::MUTED)).child("LOCAL CONTROL")))
                .child(caption_control("minimize",IconName::WindowMinimize,WindowControlArea::Min,false))
                .child(caption_control("maximize",if window.is_maximized(){IconName::WindowRestore}else{IconName::WindowMaximize},WindowControlArea::Max,false))
                .child(caption_control("close",IconName::WindowClose,WindowControlArea::Close,true)))
            .child(div().h_flex().items_stretch().flex_1().min_h_0().overflow_hidden()
                .when(sidebar_width > 0.1, |el| el.child(
                    div().w(px(sidebar_width)).h_full().flex_shrink_0().overflow_hidden()
                        .child(div().relative().left(px(sidebar_width - 190.)).w(px(190.)).h_full()
                            .child(self.render_sidebar(cx)))))
                .child(div().v_flex().flex_1().h_full().min_w_0().min_h_0()
                    .child(div().h_flex().h(px(42.)).flex_shrink_0().bg(rgb(theme::PANEL)).border_b_1().border_color(rgb(theme::BORDER))
                        .child(div().flex_1().min_w_0().child(TabBar::new("workspace-tabs").selected_index(self.active).max_width(px(180.)).menu(true).track_scroll(&self.tab_scroll)
                            .on_click(cx.listener(|this,ix:&usize,window,cx|{this.select_tab(*ix,cx);this.focus.focus(window,cx);}))
                            .children(self.tabs.iter().enumerate().map(|(ix,tab)|Tab::new().label(tab.name.clone()).suffix(Button::new(("close-tab",tab.id)).ghost().xsmall().label("×").tooltip("Close workspace · Ctrl+W")
                                .on_click(cx.listener(move|this,_,window,cx|{cx.stop_propagation();this.close_tab(ix,cx);this.focus.focus(window,cx);})))))))
                        .child(Button::new("new-tab").ghost().small().label("+").tooltip("New workspace · Ctrl+T").on_click(cx.listener(|this,_,w,cx|this.new_tab(w,cx)))))
                    .child(div().h_flex().h(px(42.)).flex_shrink_0().px_3().gap_2().border_b_1().border_color(rgb(theme::BORDER))

                        .child(Button::new("add-channel").small().label("+ Channel").tooltip("Add a split · Ctrl+K").on_click(cx.listener(|this,_,w,cx|this.open_add(false,w,cx))))
                        .child(Button::new("orientation").ghost().small().label(if tab.vertical{"Stacked"}else{"Side by side"}).tooltip("Change split orientation").on_click(cx.listener(|this,_,_,cx|this.flip_split(cx))))
                        .child(Button::new("rename").ghost().small().label("Rename").on_click(cx.listener(|this,_,w,cx|this.open_add(true,w,cx))))
                        .when(!self.closed_tabs.is_empty(), |el|el.child(Button::new("reopen").ghost().small().label("Reopen closed").tooltip("Ctrl+Shift+T").on_click(cx.listener(|this,_,_,cx|this.reopen_tab(cx)))))
                        .child(div().flex_1())
                        .child(Button::new("preferences").ghost().small().label("Appearance").on_click(cx.listener(|this,_,_,cx|{this.settings=!this.settings;cx.notify();}))))
                    .child(self.account.clone())
                    .when(self.adding,|el|el.child(div().v_flex().p_3().gap_2().bg(rgb(theme::ELEVATED)).border_b_1().border_color(rgb(theme::BORDER))
                        .child(div().text_size(px(12.)).child(if self.renaming{"Rename workspace"}else{"Add a Twitch channel to this workspace"}))
                        .child(div().h_flex().gap_2().child(div().flex_1().child(Input::new(&self.channel_input)))
                            .child(Button::new("accept-channel").label(if self.renaming{"Save"}else{"Add channel"}).on_click(cx.listener(|this,_,w,cx|this.accept_input(w,cx))))
                            .child(Button::new("cancel-add").ghost().label("Cancel").on_click(cx.listener(|this,_,window,cx|{this.adding=false;this.focus.focus(window,cx);cx.notify();}))))
                        .when_some(self.add_error.clone(),|el,error|el.child(div().text_size(px(12.)).text_color(rgb(0xF29D9D)).child(error)))))
                    .child(div().h_flex().items_stretch().flex_1().min_h_0().overflow_hidden().p_2().gap_2().child(div().flex_1().h_full().min_w_0().min_h_0().child(content))
                        .when(self.settings,|el|el.child(div().v_flex().w(px(220.)).p_4().gap_3().bg(rgb(theme::PANEL)).rounded(px(6.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Appearance"))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Dark Studio · Segoe UI"))
                            .child(div().h_flex().gap_2().child(Button::new("font-minus").small().label("A−").on_click(cx.listener(|this,_,_,cx|this.change_font(-1.,cx))))
                                .child(format!("{} px",self.font_size as u32)).child(Button::new("font-plus").small().label("A+").on_click(cx.listener(|this,_,_,cx|this.change_font(1.,cx)))))
                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Tabs, splits, channel drafts and appearance are saved locally."))
                            .child(Button::new("close-settings").ghost().label("Done").on_click(cx.listener(|this,_,_,cx|{this.settings=false;cx.notify();}))))))))
            .child(div().h_flex().h(px(26.)).flex_shrink_0().px_3().gap_3().border_t_1().border_color(rgb(theme::BORDER)).text_size(px(10.)).text_color(rgb(theme::MUTED))
                .child("○ Live chat disconnected").child(div().flex_1().child(self.save_status.clone())).child("Ctrl+K channels · Ctrl+T tabs"))
    }
}
