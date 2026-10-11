//! Settings window: section list on the left, one page of settings on the right.
//! Each row is a title, a short explanation and its control, so pages read the same.
use super::*;
use gpui_kit::assets::IconName as Kit;
use gpui_kit::component::switch::Switch;

const SECTIONS: [(&str, Kit); 8] = [
    ("General", Kit::SlidersHorizontal),
    ("Appearance", Kit::Palette),
    ("Chat", Kit::MessageSquareText),
    ("Highlights", Kit::Highlighter),
    ("Notifications", Kit::BellRing),
    ("Connections", Kit::Plug),
    ("Shortcuts", Kit::Keyboard),
    ("Updates", Kit::Download),
];
const SHORTCUTS: [(&str, &str); 18] = [
    ("Ctrl+P", "Switch channel"),
    ("Ctrl+K", "Add a channel"),
    ("Ctrl+Tab", "Next workspace"),
    ("Ctrl+PageDown / PageUp", "Next / previous channel"),
    ("Ctrl+I", "Channel details"),
    ("Ctrl+F", "Search this channel"),
    ("Ctrl+Shift+F", "Search open channels"),
    ("Ctrl+Shift+M", "Highlights & unread"),
    ("Ctrl+Shift+R", "Mark all read"),
    ("Ctrl+Shift+S", "Show or hide the Studio panel"),
    ("Ctrl+Shift+H", "Sent message history"),
    ("Alt+Up / Down", "Previous / next sent message"),
    ("Ctrl+T", "New workspace"),
    ("Ctrl+W", "Close workspace"),
    ("Ctrl+Shift+T", "Reopen workspace"),
    ("Ctrl+Space", "Pause or resume scrolling"),
    ("Ctrl+plus / minus", "Text size (Ctrl+0 resets)"),
    ("Double-click a divider", "Make the splits equal"),
];

fn row(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap_4()
        .py(px(10.))
        .border_b_1()
        .border_color(rgb(theme::BORDER))
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(div().text_size(px(13.)).child(title.into()))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child(detail.into()),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}
fn heading(text: &'static str) -> Div {
    div()
        .pt_4()
        .pb_1()
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::MUTED))
        .child(text.to_uppercase())
}
fn open_button(id: &'static str, label: &'static str) -> Button {
    Button::new(id).small().label(label)
}

impl Workbench {
    pub(super) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = !self.settings;
        // Focus stays in the workbench so Esc reaches it.
        self.focus.focus(window, cx);
        if self.settings {
            self.settings_opened = std::time::Instant::now();
            self.animate_for(Duration::from_millis(180), cx);
        }
        cx.notify();
    }
    fn settings_page(&mut self, cx: &mut Context<Self>) -> Div {
        let page = div().v_flex();
        match self.settings_section {
            0 => {
                let experience = self.experience;
                page.child(row(
                    "Layout",
                    "What the window is set up for. Moderator adds mod tools; Streamer adds the Studio panel.",
                    gpui_kit::component::tab::TabBar::new("settings-experience")
                        .segmented()
                        .selected_index(roles::Experience::ALL.iter().position(|e| *e == experience).unwrap_or(0))
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            this.set_experience(roles::Experience::ALL[*index], cx)
                        }))
                        .children(roles::Experience::ALL.map(|e| gpui_kit::component::tab::Tab::new().label(e.label()))),
                ))
                .when(experience == roles::Experience::Streamer, |el| {
                    el.child(row(
                        "Studio panel",
                        "Stream info, activity and tools on the right. Ctrl+Shift+S.",
                        Switch::new("settings-studio").checked(self.studio_open).on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.studio_open = *on;
                            this.schedule_save(cx);
                            cx.notify();
                        })),
                    ))
                })
                .child(row(
                    "Only show live channel tabs",
                    "Offline channels stay open but hide from the tab strip.",
                    Switch::new("settings-live-channels").checked(self.live_channels).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.live_channels = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Only show live workspaces",
                    "Workspaces with no live channel hide from the sidebar.",
                    Switch::new("settings-live-workspaces").checked(self.live_workspaces).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.live_workspaces = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Twitch accounts",
                    format!("Active: {}. Add a bot or alt to send as it from any message box.", self.account.read(cx).label()),
                    open_button("settings-accounts", "Manage").on_click(cx.listener(|this, _, w, cx| this.open_accounts(w, cx))),
                ))
                .child(row(
                    "Import from Chatterino",
                    "Brings in your Chatterino tabs, channels and highlights alongside what's here.",
                    open_button("settings-import", "Import").on_click(cx.listener(|this, _, w, cx| this.open_chatterino_import(w, cx))),
                ))
                .when(self.close_failed, |el| {
                    el.child(row(
                        "Discard unsaved changes and quit",
                        "Saving failed. Quits without writing the workspace file.",
                        Button::new("settings-discard").small().danger().label("Quit").on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(QuitWithoutSaving), cx))),
                    ))
                })
            }
            1 => {
                let font = self.font_size as u32;
                page.child(row(
                    "Text size",
                    "Ctrl+wheel or Ctrl+plus/minus. Ctrl+0 resets.",
                    div()
                        .h_flex()
                        .items_center()
                        .gap_1()
                        .child(Button::new("font-minus").small().ghost().label("A−").on_click(cx.listener(|this, _, _, cx| this.change_font(-1., cx))))
                        .child(div().w(px(44.)).text_center().text_size(px(12.)).child(format!("{font} px")))
                        .child(Button::new("font-plus").small().ghost().label("A+").on_click(cx.listener(|this, _, _, cx| this.change_font(1., cx))))
                        .child(Button::new("font-reset").small().ghost().label("Reset").on_click(cx.listener(|this, _, w, cx| this.font_feedback(0., true, w, cx)))),
                ))
                .child(row(
                    "Timestamps",
                    "Twitch's event time in your timezone. Hover one for the date.",
                    gpui_kit::component::tab::TabBar::new("settings-timestamps")
                        .segmented()
                        .selected_index(self.timestamps as usize)
                        .on_click(cx.listener(|this, index: &usize, _, cx| this.change_timestamps(*index as u8, cx)))
                        .children(["Off", "12:34", "12:34:56"].map(|l| gpui_kit::component::tab::Tab::new().label(l))),
                ))
                .child(row(
                    "Pause scrolling",
                    "When chat stops moving so you can read. Ctrl+Space toggles.",
                    gpui_kit::component::tab::TabBar::new("settings-pause")
                        .segmented()
                        .selected_index(self.pause_mode as usize)
                        .on_click(cx.listener(|this, index: &usize, w, cx| this.change_pause_mode(*index as u8, w, cx)))
                        .children(["Manual", "Hover", "Alt+hover"].map(|l| gpui_kit::component::tab::Tab::new().label(l))),
                ))
                .child(row(
                    "Chatter nicknames & colors",
                    "Rename or recolor people locally. Nobody else sees it.",
                    open_button("settings-nicknames", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_chatter_appearance(None, w, cx))),
                ))
            }
            2 => page
                .child(row(
                    "Chat memory",
                    "Newest messages kept per channel. Images are capped at 48 MiB separately.",
                    gpui_kit::component::tab::TabBar::new("settings-history")
                        .segmented()
                        .selected_index([500usize, 1000, 2000, 5000, 10_000].iter().position(|l| *l == self.history_limit).unwrap_or(1))
                        .on_click(cx.listener(|this, index: &usize, _, cx| this.change_history_limit([500usize, 1000, 2000, 5000, 10_000][*index], cx)))
                        .children(["500", "1k", "2k", "5k", "10k"].map(|l| gpui_kit::component::tab::Tab::new().label(l))),
                ))
                .child(row(
                    "Load recent messages",
                    format!("Recent chat when a channel opens, from {} (as Chatterino does). It sees the channel names you open.", crate::recent_messages::SERVICE),
                    Switch::new("settings-recent").checked(self.recent_enabled).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.recent_enabled = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Custom commands",
                    "Your own /commands that expand into text.",
                    open_button("settings-commands", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_custom_commands(None, w, cx))),
                ))
                .child(row(
                    "Ignores",
                    "Hide chatters or phrases locally. Twitch blocks are separate.",
                    open_button("settings-ignores", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_ignores(w, cx))),
                )),
            3 => page
                .child(row(
                    "Highlight words",
                    "Up to 8 words or phrases. Whole-word, any case. Your mentions are always included.",
                    div().w(px(220.)).child(Input::new(&self.highlight_input).small()),
                ))
                .child(row(
                    "Highlight rules",
                    "Per-channel and per-user rules with their own colors.",
                    open_button("settings-rules", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_highlight_rules(w, cx))),
                ))
                .child(row(
                    "Event highlights",
                    "Colors for mentions, raids, subs, first messages and more.",
                    open_button("settings-events", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_event_highlights(w, cx))),
                )),
            4 => page
                .child(row(
                    "Live alerts for every channel",
                    format!(
                        "A Windows notification when any open channel goes live. Otherwise only channels set to Notify when live in their tab menu ({} now).",
                        self.live_notify.len()
                    ),
                    Switch::new("settings-live-all").checked(self.live_notify_all).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.live_notify_all = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Whispers",
                    "A notification for each whisper unless that conversation is open.",
                    div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Always on"),
                )),
            5 => {
                let bot: SharedString = if self.bot_settings.enabled && !self.streamer_bot.status.is_empty() {
                    self.streamer_bot.status.clone().into()
                } else {
                    "Run your actions from Studio and see YouTube, Kick, Ko-fi and other events in Activity.".into()
                };
                page.child(heading("7TV"))
                    .child(row(
                        "Share 7TV activity",
                        "After you chat, tells 7TV you're here so other 7TV users see your paint, badge and personal emotes (as Chatterino7 does).",
                        Switch::new("settings-7tv").checked(self.presence.enabled).on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.presence.enabled = *on;
                            this.schedule_save(cx);
                            cx.notify();
                        })),
                    ))
                    .child(heading("Streamer.bot"))
                    .child(row(
                        "Streamer.bot",
                        bot,
                        open_button("settings-bot", "Set up").on_click(cx.listener(|this, _, w, cx| this.open_bot_settings(w, cx))),
                    ))
            }
            6 => page.children(SHORTCUTS.iter().map(|(keys, what)| {
                div()
                    .h_flex()
                    .items_center()
                    .py(px(7.))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(div().flex_1().text_size(px(13.)).child(*what))
                    .child(
                        div()
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(4.))
                            .bg(rgb(theme::CONTROL))
                            .border_1()
                            .border_color(rgb(theme::BORDER))
                            .text_size(px(11.))
                            .child(*keys),
                    )
            })),
            _ => page
                .child(row(
                    format!("Jawjack {}", env!("CARGO_PKG_VERSION")),
                    self.updates.detail(),
                    div().text_size(px(12.)).child(self.render_updates(cx)),
                ))
                .child(row(
                    "Check automatically",
                    "From apps.zombie.digital. Nothing installs until you click.",
                    Switch::new("settings-updates").checked(self.updates.enabled).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.updates.enabled = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                )),
        }
    }
    pub(super) fn render_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let viewport = window.viewport_size();
        let width = (f32::from(viewport.width) - 32.).min(760.);
        let height = (f32::from(viewport.height) - 80.).min(560.);
        // Fade and rise in over 180 ms.
        let t = if cx.reduce_motion() {
            1.
        } else {
            (self.settings_opened.elapsed().as_secs_f32() / 0.18).clamp(0., 1.)
        };
        let ease = 1. - (1. - t).powi(3);
        let narrow = width < 560.;
        let nav = div()
            .v_flex()
            .w(px(if narrow { 52. } else { 184. }))
            .flex_shrink_0()
            .h_full()
            .p_2()
            .gap(px(2.))
            .bg(rgb(theme::PANEL))
            .border_r_1()
            .border_color(rgb(theme::BORDER))
            .children(SECTIONS.iter().enumerate().map(|(index, (label, icon))| {
                let active = index == self.settings_section;
                div()
                    .id(("settings-section", index))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .h(px(30.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .when(active, |el| {
                        el.bg(rgb(theme::CONTROL)).text_color(rgb(theme::TEXT))
                    })
                    .when(!active, |el| {
                        el.text_color(rgb(theme::MUTED))
                            .hover(|s| s.bg(rgb(theme::HOVER)).text_color(rgb(theme::TEXT)))
                    })
                    .child(
                        Icon::new(*icon)
                            .small()
                            .when(active, |i| i.text_color(rgb(theme::ACCENT))),
                    )
                    .when(!narrow, |el| el.child(*label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_section = index;
                        cx.notify();
                    }))
            }));
        let title = SECTIONS[self.settings_section.min(SECTIONS.len() - 1)].0;
        let page = self.settings_page(cx);
        div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.45 * ease))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, w, cx| {
                    this.settings = false;
                    this.focus.focus(w, cx);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("settings-window")
                    .occlude()
                    .w(px(width))
                    .h(px(height))
                    .mt(px(12. * (1. - ease)))
                    .opacity(ease)
                    .h_flex()
                    .overflow_hidden()
                    .bg(rgb(theme::ELEVATED))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .rounded(px(10.))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(nav)
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .flex_shrink_0()
                                    .h(px(48.))
                                    .px_5()
                                    .border_b_1()
                                    .border_color(rgb(theme::BORDER))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .child(
                                        Button::new("close-settings")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Close)
                                            .tooltip("Close · Esc")
                                            .on_click(cx.listener(|this, _, w, cx| {
                                                this.settings = false;
                                                this.focus.focus(w, cx);
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .id("settings-scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .px_5()
                                    .pb_4()
                                    .child(page),
                            ),
                    ),
            )
            .into_any_element()
    }
}
