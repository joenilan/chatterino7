//! Settings window: section list on the left, one page of settings on the right.
//! Each row is a title, a short explanation and its control, so pages read the same.
use super::*;
use gpui_kit::assets::IconName as Kit;
use gpui_kit::component::switch::Switch;

const SECTIONS: [(&str, Kit); 9] = [
    ("General", Kit::SlidersHorizontal),
    ("Appearance", Kit::Palette),
    ("Chat", Kit::MessageSquareText),
    ("Highlights", Kit::Highlighter),
    ("Notifications", Kit::BellRing),
    ("Streamer mode", Kit::Tv),
    ("Connections", Kit::Plug),
    ("Shortcuts", Kit::Keyboard),
    ("Updates", Kit::Download),
];
/// Fixed shortcuts shown under the customisable ones.
const SHORTCUTS: [(&str, &str); 3] = [
    (
        "Up / Down (or Alt+Up / Down)",
        "Previous / next sent message",
    ),
    ("Ctrl+plus / minus", "Text size (Ctrl+0 resets)"),
    ("Double-click a divider", "Make the splits equal"),
];

pub(super) fn row(
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
pub(super) fn heading(text: &'static str) -> Div {
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
    fn settings_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
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
                    "Channels you moderate",
                    match self.capabilities.moderated() {
                        Some(m) => format!("Open any of the {} channels you moderate in a new workspace.", m.len()),
                        None => "Checking which channels you moderate…".into(),
                    },
                    open_button("settings-mod-channels", "Choose…").on_click(cx.listener(|this, _, w, cx| {
                        this.settings = false;
                        this.offer_moderated_channels(w, cx);
                    })),
                ))
                .child(row(
                    "Twitch accounts",
                    format!("Active: {}. Add a bot or alt to send as it from any message box.", self.account.read(cx).label()),
                    open_button("settings-accounts", "Manage").on_click(cx.listener(|this, _, w, cx| this.open_accounts(w, cx))),
                ))
                .child(row(
                    "Moderator buttons on messages",
                    "Delete, 10-minute timeout and ban on each message in channels you moderate (the crossed-swords button in a channel header toggles this too).",
                    Switch::new("settings-mod-buttons").checked(self.mod_buttons).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.mod_buttons = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(self.render_mod_button_editor(window, cx))
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
                    "Emote size",
                    "How big emotes are in chat.",
                    gpui_kit::component::tab::TabBar::new("settings-emote-size")
                        .segmented()
                        .selected_index(
                            [75u32, 100, 125, 150]
                                .iter()
                                .position(|v| *v == crate::inline_chat::EMOTE_SCALE.load(std::sync::atomic::Ordering::Relaxed))
                                .unwrap_or(1),
                        )
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            crate::inline_chat::EMOTE_SCALE.store([75, 100, 125, 150][(*index).min(3)], std::sync::atomic::Ordering::Relaxed);
                            this.remeasure_all(cx);
                        }))
                        .children(["Small", "Normal", "Large", "Huge"].map(|l| gpui_kit::component::tab::Tab::new().label(l))),
                ))
                .child(row(
                    "Alternate row backgrounds",
                    "Every other message gets a slightly lighter background, easier to follow in busy chats.",
                    Switch::new("settings-alternate-rows").checked(crate::theme::alternate_rows()).on_click(cx.listener(|this, on: &bool, _, cx| {
                        crate::theme::ALTERNATE_ROWS.store(*on, std::sync::atomic::Ordering::Relaxed);
                        this.remeasure_all(cx);
                    })),
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
                    "Save chat logs",
                    "A text file per channel per day, like Chatterino's logs. Off by default.",
                    Switch::new("settings-logs").checked(self.logs_enabled).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.logs_enabled = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .when(self.logs_enabled, |el| {
                    el.child(row(
                        "Log folder",
                        self.logs_folder.display().to_string(),
                        div()
                            .h_flex()
                            .gap_1()
                            .child(open_button("settings-logs-open", "Open").on_click(cx.listener(|this, _, _, cx| {
                                let _ = std::fs::create_dir_all(&this.logs_folder);
                                cx.open_with_system(&this.logs_folder);
                            })))
                            .child(open_button("settings-logs-change", "Change…").on_click(cx.listener(|this, _, w, cx| this.choose_logs_folder(w, cx)))),
                    ))
                })
                .child(row(
                    "Custom commands",
                    "Your own /commands: text to send, or another command such as /timeout {1} 600.",
                    open_button("settings-commands", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_custom_commands(None, w, cx))),
                ))
                .child(row(
                    "Spellcheck",
                    "English. Misspellings show above the message box with fixes; emotes, names and chat words like lol or pog are skipped.",
                    Switch::new("settings-spellcheck").checked(self.spellcheck).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.spellcheck = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Link previews",
                    "Hovering a link shows its title, where it really goes, and warnings for lookalike sites. Jawjack loads the page from your PC only when you hover.",
                    Switch::new("settings-link-previews").checked(crate::link_preview::enabled()).on_click(cx.listener(|this, on: &bool, _, cx| {
                        crate::link_preview::ENABLED.store(*on, std::sync::atomic::Ordering::Relaxed);
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Show pronouns",
                    format!("On user cards, from {} (as Chatterino does). It sees the names of chatters whose cards you open.", crate::pronouns::SERVICE),
                    Switch::new("settings-pronouns").checked(crate::pronouns::enabled()).on_click(cx.listener(|this, on: &bool, _, cx| {
                        crate::pronouns::ENABLED.store(*on, std::sync::atomic::Ordering::Relaxed);
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Emote providers",
                    "Show emotes from these services in chat, the emote picker and Tab completion. Twitch emotes always show.",
                    div().h_flex().gap_3().children(
                        [
                            ("settings-emotes-7tv", "7TV", &crate::catalog::SEVEN_TV),
                            ("settings-emotes-bttv", "BTTV", &crate::catalog::BTTV),
                            ("settings-emotes-ffz", "FFZ", &crate::catalog::FFZ),
                        ]
                        .map(|(id, label, flag)| {
                            gpui_kit::component::checkbox::Checkbox::new(id)
                                .label(label)
                                .checked(crate::catalog::on(flag))
                                .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                    flag.store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.schedule_save(cx);
                                    cx.notify();
                                }))
                        }),
                    ),
                ))
                .child(row(
                    "Repeated messages",
                    "When someone posts nearly the same thing again within a minute (90% alike), like Chatterino's similar-message filter.",
                    gpui_kit::component::tab::TabBar::new("settings-similar")
                        .segmented()
                        .selected_index(crate::similarity::Mode::ALL.iter().position(|m| *m == self.similar_mode).unwrap_or(0))
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            this.similar_mode = crate::similarity::Mode::ALL[*index];
                            this.schedule_save(cx);
                            cx.notify();
                        }))
                        .children(crate::similarity::Mode::ALL.map(|m| gpui_kit::component::tab::Tab::new().label(m.label()))),
                ))
                .when(!self.automod_hidden.is_empty(), |el| {
                    el.child(row(
                        "AutoMod hidden in",
                        self.automod_hidden.iter().map(|c| format!("#{c}")).collect::<Vec<_>>().join(", "),
                        Button::new("settings-automod-unhide").small().label("Show all again").on_click(cx.listener(|this, _, _, cx| {
                            this.automod_hidden.clear();
                            this.schedule_save(cx);
                            cx.notify();
                        })),
                    ))
                })
                .child(row(
                    "Hide blocked users",
                    match &self.blocks.ids {
                        Some(ids) if ids.len() >= crate::blocks::MAX_IDS => format!(
                            "New messages from people you've blocked on Twitch stay hidden, as on twitch.tv ({}+ blocked; the first {} are used).",
                            crate::blocks::MAX_IDS, crate::blocks::MAX_IDS
                        ),
                        Some(ids) => format!("New messages from the {} people you've blocked on Twitch stay hidden, as on twitch.tv.", ids.len()),
                        None => "New messages from people you've blocked on Twitch stay hidden, as on twitch.tv.".into(),
                    },
                    Switch::new("settings-hide-blocked").checked(self.hide_blocked).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.hide_blocked = *on;
                        this.apply_blocks();
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Ignores",
                    "Hide chatters or phrases locally. Twitch blocks are separate.",
                    open_button("settings-ignores", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_ignores(w, cx))),
                )),
            3 => page
                .child(row(
                    "Highlight words",
                    "Up to 8 words or phrases, comma-separated. Whole-word, any case. Your mentions are always included. Add !name to never highlight or ping for that chatter (e.g. !streamelements, !nightbot).",
                    div().w(px(220.)).child(Input::new(&self.highlight_input).small()),
                ))
                .child(row(
                    "Highlight rules",
                    "Rules for words, chatters or badges, each with its own color and sound.",
                    open_button("settings-rules", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_highlight_rules(w, cx))),
                ))
                .child(row(
                    "Event highlights",
                    "Colors for mentions, raids, subs, first messages and more.",
                    open_button("settings-events", "Edit").on_click(cx.listener(|this, _, w, cx| this.open_event_highlights(w, cx))),
                )),
            4 => self
                .sound_rows(page, cx)
                .child(heading("Alerts"))
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
            5 => self.streamer_rows(page, cx),
            6 => {
                let bot: SharedString = if self.bot_settings.enabled && !self.streamer_bot.status.is_empty() {
                    self.streamer_bot.status.clone().into()
                } else {
                    "Run your actions from Studio and see YouTube, Kick, Ko-fi and other events in Activity.".into()
                };
                let page = self.youtube_rows(page, window, cx);
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
                        open_button("settings-bot", if self.bot_settings.enabled { "Settings…" } else { "Set up…" }).on_click(cx.listener(|this, _, w, cx| {
                            this.settings = false;
                            if this.bot_settings.enabled {
                                this.open_bot_settings(w, cx)
                            } else {
                                this.open_guide(super::guides::Guide::StreamerBot, w, cx)
                            }
                        })),
                    ))
            }
            7 => page
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).pb_1().child("Click a shortcut to change it, then press the new keys. Esc cancels."))
                .children(crate::hotkeys::HOTKEYS.iter().map(|hotkey| {
                    let keys = crate::hotkeys::keys(hotkey);
                    let custom = keys != hotkey.default;
                    let id = hotkey.id;
                    div()
                        .h_flex()
                        .items_center()
                        .gap_1()
                        .py(px(5.))
                        .border_b_1()
                        .border_color(rgb(theme::BORDER))
                        .child(div().flex_1().text_size(px(13.)).child(hotkey.label))
                        .when(custom, |el| {
                            el.child(Button::new(SharedString::from(format!("hotkey-reset-{id}"))).xsmall().ghost().label("Reset").tooltip(format!("Back to {}", crate::hotkeys::label(hotkey.default))).on_click(cx.listener(move |this, _, _, cx| {
                                crate::hotkeys::set(id, None, cx);
                                this.schedule_save(cx);
                                cx.notify();
                            })))
                        })
                        .child(Button::new(SharedString::from(format!("hotkey-{id}"))).small().label(crate::hotkeys::label(&keys)).when(custom, |b| b.primary()).on_click(cx.listener(move |this, _, w, cx| this.record_hotkey(id, w, cx))))
                }))
                .children(SHORTCUTS.iter().map(|(keys, what)| {
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
        let page = self.settings_page(window, cx);
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

impl Workbench {
    /// Settings > General: your own moderator buttons.
    fn render_mod_button_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if self.mod_button_inputs.is_none() {
            let label = cx.new(|cx| InputState::new(window, cx).placeholder("Label, e.g. 1h"));
            let command =
                cx.new(|cx| InputState::new(window, cx).placeholder("/timeout {user} 3600 spam"));
            self.mod_button_inputs = Some((label, command));
        }
        let (label, command) = self.mod_button_inputs.clone().expect("created above");
        let buttons = self.custom_mod_buttons.clone();
        div()
            .v_flex()
            .gap_1()
            .py(px(10.))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(div().text_size(px(13.)).child("Your moderator buttons"))
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                "Shown after delete, timeout and ban in moderator mode. {user}, {msg-id} and {channel} are filled from the message. Moderation commands only.",
            ))
            .children(buttons.iter().enumerate().map(|(index, b)| {
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .child(div().w(px(60.)).font_weight(FontWeight::SEMIBOLD).child(b.label.clone()))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(rgb(theme::MUTED)).child(b.command.clone()))
                    .child(Button::new(("mod-button-remove", index)).xsmall().ghost().icon(IconName::Close).on_click(cx.listener(move |this, _, _, cx| {
                        let mut list = (*this.custom_mod_buttons).clone();
                        if index < list.len() {
                            list.remove(index);
                        }
                        this.custom_mod_buttons = std::rc::Rc::new(list);
                        this.schedule_save(cx);
                        cx.notify();
                    })))
            }))
            .when(buttons.len() < crate::mod_buttons::MAX, |el| {
                el.child(
                    div()
                        .h_flex()
                        .gap_1()
                        .pt_1()
                        .child(div().w(px(80.)).child(Input::new(&label).small()))
                        .child(div().flex_1().child(Input::new(&command).small()))
                        .child(Button::new("mod-button-add").small().label("Add").on_click(cx.listener(move |this, _, window, cx| {
                            let (l, c) = (label.read(cx).value().to_string(), command.read(cx).value().to_string());
                            match crate::mod_buttons::validate(&l, &c) {
                                Ok(button) => {
                                    let mut list = (*this.custom_mod_buttons).clone();
                                    list.push(button);
                                    this.custom_mod_buttons = std::rc::Rc::new(list);
                                    label.update(cx, |i, cx| i.set_value("", window, cx));
                                    command.update(cx, |i, cx| i.set_value("", window, cx));
                                    this.schedule_save(cx);
                                }
                                Err(problem) => window.push_notification(Notification::info(problem), cx),
                            }
                            cx.notify();
                        }))),
                )
            })
    }
}

impl Workbench {
    /// Settings → Shortcuts: press the new keys for a hotkey. The keystroke is
    /// intercepted before any binding runs, so it can't trigger its old action.
    fn record_hotkey(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hotkey) = crate::hotkeys::HOTKEYS.iter().find(|h| h.id == id) else {
            return;
        };
        let owner = cx.entity().downgrade();
        let label = hotkey.label;
        let status: std::rc::Rc<std::cell::RefCell<String>> = Default::default();
        let shown = status.clone();
        self.hotkey_capture = Some(cx.intercept_keystrokes(move |event, window, cx| {
            let key = event.keystroke.key.as_str();
            if matches!(
                key,
                "shift" | "control" | "alt" | "platform" | "function" | "capslock"
            ) {
                return;
            }
            cx.stop_propagation();
            let done = if key == "escape" && !event.keystroke.modifiers.modified() {
                true
            } else {
                match crate::hotkeys::validate(&event.keystroke.unparse()) {
                    Ok(keys) => {
                        crate::hotkeys::set(id, Some(keys), cx);
                        true
                    }
                    Err(problem) => {
                        *status.borrow_mut() = problem;
                        window.refresh();
                        false
                    }
                }
            };
            if done {
                window.close_dialog(cx);
                let _ = owner.update(cx, |this, cx| {
                    this.hotkey_capture = None;
                    this.schedule_save(cx);
                    cx.notify();
                });
            }
        }));
        let closing = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let closing = closing.clone();
            dialog
                .title(format!("Shortcut for {label}"))
                .on_close(move |_, _, cx| {
                    let _ = closing.update(cx, |this, _| this.hotkey_capture = None);
                })
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(13.))
                                .child("Press the new keys, e.g. Ctrl+Shift+K or Alt+J."),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(theme::MUTED))
                                .child("Use Ctrl or Alt with a key, or an F-key. Esc cancels."),
                        )
                        .when(!shown.borrow().is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(0xE9C99C))
                                    .child(shown.borrow().clone()),
                            )
                        }),
                )
        });
    }
}
