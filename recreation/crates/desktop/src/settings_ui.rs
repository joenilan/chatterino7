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

thread_local! {
    /// The settings search, lowercase; rows that don't mention it are hidden.
    static SEARCH: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// Rows shown since the count was last reset (to skip empty sections).
    static SHOWN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn searching() -> bool {
    SEARCH.with_borrow(|q| !q.is_empty())
}
fn matches_search(title: &str, detail: &str) -> bool {
    let shown = SEARCH.with_borrow(|q| {
        q.split_whitespace()
            .all(|word| title.to_lowercase().contains(word) || detail.to_lowercase().contains(word))
    });
    if shown {
        SHOWN.set(SHOWN.get() + 1);
    }
    shown
}
/// A setting whose controls are too wide to sit beside the text: they wrap
/// on their own line underneath.
pub(super) fn stacked_row(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
) -> Div {
    let (title, detail) = (title.into(), detail.into());
    if !matches_search(&title, &detail) {
        return div().hidden();
    }
    div()
        .v_flex()
        .gap_2()
        .py(px(10.))
        .border_b_1()
        .border_color(rgb(theme::BORDER))
        .child(
            div()
                .v_flex()
                .gap(px(2.))
                .child(div().text_size(px(13.)).child(title))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child(detail),
                ),
        )
        .child(control)
}
pub(super) fn row(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
) -> Div {
    let (title, detail) = (title.into(), detail.into());
    if !matches_search(&title, &detail) {
        return div().hidden();
    }
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
                .child(div().text_size(px(13.)).child(title))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .child(detail),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}
pub(super) fn heading(text: &'static str) -> Div {
    if searching() {
        return div().hidden();
    }
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
                    crate::theme::Segmented::new("settings-experience")
                        .selected_index(roles::Experience::ALL.iter().position(|e| *e == experience).unwrap_or(0))
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            this.set_experience(roles::Experience::ALL[*index], cx)
                        }))
                        .children(roles::Experience::ALL.map(|e| crate::theme::segment(e.label()))),
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
                    "Keep Jawjack on top",
                    "The main window stays above other windows. Pop-outs have their own pin.",
                    Switch::new("settings-on-top").checked(self.on_top).on_click(cx.listener(|this, on: &bool, w, cx| {
                        if super::sound_settings::set_on_top(w, *on) {
                            this.on_top = *on;
                            this.schedule_save(cx);
                        }
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Start with Windows",
                    "Open Jawjack when you sign in to Windows.",
                    Switch::new("settings-autorun").checked(*self.autorun.get_or_insert_with(crate::autorun::enabled)).on_click(cx.listener(|this, on: &bool, w, cx| {
                        match crate::autorun::set(*on) {
                            Ok(()) => this.autorun = Some(*on),
                            Err(problem) => w.push_notification(Notification::info(problem), cx),
                        }
                        cx.notify();
                    })),
                ))
                .child({
                    let backups = crate::storage::backups();
                    stacked_row(
                        "Restore a backup",
                        "Jawjack keeps a copy of your settings each day (the last 7). Restoring one replaces your current settings and closes Jawjack; open it again to continue.",
                        div().h_flex().flex_wrap().gap_1()
                            .when(backups.is_empty(), |el| el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("No backups yet; the first is made the next time Jawjack starts.")))
                            .children(backups.into_iter().enumerate().map(|(index, (date, file))| {
                                let label = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").map(|d| d.format("%b %-d").to_string()).unwrap_or(date.clone());
                                Button::new(("settings-backup", index)).small().label(label.clone()).on_click(cx.listener(move |_, _, window, cx| {
                                    let file = file.clone();
                                    let owner = cx.entity().downgrade();
                                    let label = label.clone();
                                    window.open_dialog(cx, move |dialog, _, _| {
                                        let (file, owner) = (file.clone(), owner.clone());
                                        dialog.title(format!("Restore settings from {label}?"))
                                            .child(div().text_size(px(12.)).text_color(rgb(theme::MUTED)).child("Your current workspaces, channels and settings are replaced by that day's copy, and Jawjack closes. Sign-ins are unaffected."))
                                            .footer(div().h_flex().justify_end().gap_2()
                                                .child(Button::new("backup-cancel").small().label("Cancel").on_click(|_, w, cx| w.close_dialog(cx)))
                                                .child(Button::new("backup-restore").small().danger().label("Restore and close").on_click(move |_, w, cx| {
                                                    w.close_dialog(cx);
                                                    let file = file.clone();
                                                    let _ = owner.update(cx, |this, cx| this.restore_backup(&file, w, cx));
                                                })))
                                    });
                                }))
                            })),
                    )
                })
                .child(row(
                    "Data folder",
                    "Where Jawjack keeps your workspace and settings (sign-ins are in Windows Credential Manager).",
                    open_button("settings-data-folder", "Open").on_click(|_, _, cx| {
                        if let Some(folder) = crate::storage::folder() {
                            cx.open_with_system(&folder);
                        }
                    }),
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
                    crate::theme::Segmented::new("settings-timestamps")
                        .selected_index(self.timestamps as usize)
                        .on_click(cx.listener(|this, index: &usize, _, cx| this.change_timestamps(*index as u8, cx)))
                        .children(["Off", "13:34", "13:34:56", "1:34 PM", "1:34:56 PM"].map(|l| crate::theme::segment(l))),
                ))
                .child(row(
                    "Pause scrolling",
                    "When chat stops moving so you can read. Ctrl+Space toggles.",
                    crate::theme::Segmented::new("settings-pause")
                        .selected_index(self.pause_mode as usize)
                        .on_click(cx.listener(|this, index: &usize, w, cx| this.change_pause_mode(*index as u8, w, cx)))
                        .children(["Manual", "Hover", "Alt+hover"].map(|l| crate::theme::segment(l))),
                ))
                .child(row(
                    "Scrolling",
                    "How far the mouse wheel moves chat, and whether each step glides instead of jumping.",
                    div().h_flex().gap_3().items_center()
                        .child(
                            crate::theme::Segmented::new("settings-scroll-speed")
                                .selected_index(crate::message_scroller::SCROLL_SPEED.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, index: &usize, _, cx| {
                                    crate::message_scroller::SCROLL_SPEED.store(*index, std::sync::atomic::Ordering::Relaxed);
                                    this.schedule_save(cx);
                                    cx.notify();
                                }))
                                .children(crate::message_scroller::SPEEDS.map(|(l, _)| crate::theme::segment(l))),
                        )
                        .child(
                            gpui_kit::component::checkbox::Checkbox::new("settings-smooth-scroll")
                                .label("Smooth")
                                .checked(crate::message_scroller::SMOOTH_SCROLL.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    crate::message_scroller::SMOOTH_SCROLL.store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.schedule_save(cx);
                                    cx.notify();
                                })),
                        ),
                ))
                .child(row(
                    "Chat font",
                    "The font for chat messages. Click to pick one installed on this PC.",
                    div()
                        .h_flex()
                        .gap_1()
                        .when(crate::theme::chat_font().is_some(), |el| {
                            el.child(Button::new("chat-font-reset").small().ghost().label("Default").on_click(cx.listener(|this, _, _, cx| {
                                if let Ok(mut font) = crate::theme::CHAT_FONT.lock() {
                                    *font = None;
                                }
                                this.remeasure_all(cx);
                            })))
                        })
                        .child(Button::new("chat-font").small().label(crate::theme::chat_font().unwrap_or_else(|| "Default".into())).on_click(cx.listener(|this, _, w, cx| this.choose_chat_font(w, cx)))),
                ))
                .child(row(
                    "Names",
                    "Show chatters by display name, login, or both when they differ (e.g. 日本語 names).",
                    crate::theme::Segmented::new("settings-names")
                        .selected_index(chat_core::NAME_MODE.load(std::sync::atomic::Ordering::Relaxed) as usize)
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            chat_core::NAME_MODE.store((*index).min(2) as u8, std::sync::atomic::Ordering::Relaxed);
                            this.remeasure_all(cx);
                        }))
                        .children(["Display name", "Login", "Both"].map(|l| crate::theme::segment(l))),
                ))
                .child(row(
                    "Channel header",
                    "While a channel is live, show these beside its name.",
                    div().h_flex().flex_wrap().justify_end().gap_x_3().gap_y_1().children(crate::theme::HEADER_PARTS.iter().enumerate().map(|(index, label)| {
                        gpui_kit::component::checkbox::Checkbox::new(("settings-header-part", index))
                            .label(*label)
                            .checked(crate::theme::header_shows(index))
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                crate::theme::HEADER[index].store(*on, std::sync::atomic::Ordering::Relaxed);
                                this.remeasure_all(cx);
                            }))
                    })),
                ))
                .child(row(
                    "Badges",
                    "Which badges show before names.",
                    div().h_flex().flex_wrap().justify_end().gap_x_3().gap_y_1().max_w(px(300.)).children(crate::theme::BADGE_KINDS.iter().enumerate().map(|(index, label)| {
                        gpui_kit::component::checkbox::Checkbox::new(("settings-badge-kind", index))
                            .label(*label)
                            .checked(crate::theme::badge_shown(index))
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                crate::theme::BADGES[index].store(*on, std::sync::atomic::Ordering::Relaxed);
                                this.remeasure_all(cx);
                            }))
                    })),
                ))
                .child(row(
                    "7TV name paints",
                    "Gradient and image name colours from 7TV, and the glow around them.",
                    div().h_flex().gap_3().children([(0usize, "Paints"), (1, "Shadows")].map(|(index, label)| {
                        gpui_kit::component::checkbox::Checkbox::new(("settings-paint-option", index))
                            .label(label)
                            .checked(crate::name_paint::paint_option(index))
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                crate::name_paint::PAINT_OPTIONS[index].store(*on, std::sync::atomic::Ordering::Relaxed);
                                this.remeasure_all(cx);
                            }))
                    })),
                ))
                .child(row(
                    "Lines between messages",
                    "A thin separator under every message.",
                    Switch::new("settings-separators").checked(crate::theme::flag(&crate::theme::SEPARATORS)).on_click(cx.listener(|this, on: &bool, _, cx| {
                        crate::theme::SEPARATORS.store(*on, std::sync::atomic::Ordering::Relaxed);
                        this.remeasure_all(cx);
                    })),
                ))
                .child(row(
                    "Hide deleted messages",
                    "Off: deleted and timed-out messages stay readable, greyed out. On: they disappear.",
                    Switch::new("settings-hide-deleted").checked(crate::theme::flag(&crate::theme::HIDE_DELETED)).on_click(cx.listener(|this, on: &bool, _, cx| {
                        crate::theme::HIDE_DELETED.store(*on, std::sync::atomic::Ordering::Relaxed);
                        this.remeasure_all(cx);
                    })),
                ))
                .child(row(
                    "Animated emotes",
                    "Animated emotes, badges and name paints. \"When focused\" freezes them while Jawjack is in the background, which saves CPU while you're in a game.",
                    crate::theme::Segmented::new("settings-animate")
                        .selected_index(crate::media::ANIMATE.load(std::sync::atomic::Ordering::Relaxed) as usize)
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            crate::media::ANIMATE.store((*index).min(2) as u8, std::sync::atomic::Ordering::Relaxed);
                            this.schedule_save(cx);
                            cx.notify();
                        }))
                        .children(["Always", "When focused", "Never"].map(crate::theme::segment)),
                ))
                .child(row(
                    "Emote size",
                    "How big emotes are in chat.",
                    crate::theme::Segmented::new("settings-emote-size")
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
                        .children(["Small", "Normal", "Large", "Huge"].map(|l| crate::theme::segment(l))),
                ))
                .child(stacked_row(
                    "Input & replies",
                    "The Send button (Enter always sends), the \"replying to\" line above replies, a reply button when you hover a message, and hiding the @name Twitch puts at the start of replies.",
                    div().h_flex().flex_wrap().gap_x_4().gap_y_1()
                        .children([(0usize, "Send button"), (1, "Reply context"), (2, "Reply on hover"), (3, "Hide leading @mention")].map(|(index, label)| {
                            gpui_kit::component::checkbox::Checkbox::new(("settings-input-option", index))
                                .label(label)
                                .checked(crate::replies::option(index))
                                .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                    crate::replies::OPTIONS[index].store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.remeasure_all(cx);
                                }))
                        })),
                ))
                .child(stacked_row(
                    "Mentions",
                    "Colour @mentions like the chatter's name (once they've chatted). Finding names without @ makes a chatter's bare name work like a mention: coloured, and Ctrl+click opens their card.",
                    div().h_flex().flex_wrap().gap_x_4().gap_y_1()
                        .child(
                            gpui_kit::component::checkbox::Checkbox::new("settings-color-mentions")
                                .label("Colour @mentions")
                                .checked(crate::mention_names::COLOR_MENTIONS.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    crate::mention_names::COLOR_MENTIONS.store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.remeasure_all(cx);
                                })),
                        )
                        .child(
                            gpui_kit::component::checkbox::Checkbox::new("settings-find-names")
                                .label("Find names without @")
                                .checked(crate::mention_names::FIND_NAMES.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    crate::mention_names::FIND_NAMES.store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.remeasure_all(cx);
                                })),
                        ),
                ))
                .child(stacked_row(
                    "Emote options",
                    "Images off shows emote names as text. Zero-width emotes (7TV overlays) stack on the emote before them. Bits combines repeated cheers into one total.",
                    div().h_flex().flex_wrap().gap_x_4().gap_y_1()
                        .children([(0usize, "Images"), (1, "Stack zero-width"), (2, "No spaces between emotes"), (3, "Combine bits")].map(|(index, label)| {
                            gpui_kit::component::checkbox::Checkbox::new(("settings-emote-option", index))
                                .label(label)
                                .checked(crate::inline_chat::emote_option(index))
                                .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                    crate::inline_chat::EMOTE_OPTIONS[index].store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.remeasure_all(cx);
                                }))
                        }))
                        .child(
                            gpui_kit::component::checkbox::Checkbox::new("settings-unlisted-7tv")
                                .label("Unlisted 7TV")
                                .checked(crate::catalog::SHOW_UNLISTED.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    crate::catalog::SHOW_UNLISTED.store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.reload_all_emotes(cx);
                                    this.remeasure_all(cx);
                                })),
                        ),
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
                    crate::theme::Segmented::new("settings-history")
                        .selected_index([500usize, 1000, 2000, 5000, 10_000].iter().position(|l| *l == self.history_limit).unwrap_or(1))
                        .on_click(cx.listener(|this, index: &usize, _, cx| this.change_history_limit([500usize, 1000, 2000, 5000, 10_000][*index], cx)))
                        .children(["500", "1k", "2k", "5k", "10k"].map(|l| crate::theme::segment(l))),
                ))
                .child(row(
                    "Load recent messages",
                    format!("Recent chat when a channel opens, from {} (as Chatterino does). It sees the channel names you open.", crate::recent_messages::SERVICE),
                    div().h_flex().gap_3().items_center()
                        .when(self.recent_enabled, |el| {
                            el.child(
                                crate::theme::Segmented::new("settings-recent-limit")
                                    .selected_index(crate::recent_messages::LIMIT.load(std::sync::atomic::Ordering::Relaxed))
                                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                                        crate::recent_messages::LIMIT.store(*index, std::sync::atomic::Ordering::Relaxed);
                                        this.schedule_save(cx);
                                        cx.notify();
                                    }))
                                    .children(crate::recent_messages::LIMITS.map(|n| crate::theme::segment(n.to_string()))),
                            )
                        })
                        .child(Switch::new("settings-recent").checked(self.recent_enabled).on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.recent_enabled = *on;
                            this.schedule_save(cx);
                            cx.notify();
                        }))),
                ))
                .child(stacked_row(
                    "Completion",
                    "Tab completes emotes and chatter names; @ always completes names. A comma after a name that starts the message reads like \"name, hi\".",
                    div().h_flex().flex_wrap().gap_x_4().gap_y_1()
                        .children([(0usize, "Names only after @"), (1, "Comma after a leading name"), (2, "Add @ to names"), (3, "Include the streamer")].map(|(index, label)| {
                            gpui_kit::component::checkbox::Checkbox::new(("settings-completion", index))
                                .label(label)
                                .checked(crate::emote_picker::COMPLETION_OPTIONS[index].load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                    crate::emote_picker::COMPLETION_OPTIONS[index].store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.schedule_save(cx);
                                    cx.notify();
                                }))
                        })),
                ))
                .child(stacked_row(
                    "Clicking names",
                    "Click a name to open the chatter's card (off: Ctrl+click), and what right-clicking a name does. Shift+right-click always opens the message menu.",
                    div().h_flex().gap_3().items_center()
                        .child(
                            gpui_kit::component::checkbox::Checkbox::new("settings-name-click")
                                .label("Click opens card")
                                .checked(crate::message_actions::PLAIN_NAME_CLICK.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    crate::message_actions::PLAIN_NAME_CLICK.store(*on, std::sync::atomic::Ordering::Relaxed);
                                    this.schedule_save(cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            crate::theme::Segmented::new("settings-name-right-click")
                                .selected_index(crate::message_actions::RIGHT_CLICK_NAME.load(std::sync::atomic::Ordering::Relaxed))
                                .on_click(cx.listener(|this, index: &usize, _, cx| {
                                    crate::message_actions::RIGHT_CLICK_NAME.store(*index, std::sync::atomic::Ordering::Relaxed);
                                    this.schedule_save(cx);
                                    cx.notify();
                                }))
                                .children(crate::message_actions::RIGHT_CLICK_NAMES.map(crate::theme::segment)),
                        ),
                ))
                .child(row(
                    "Web search",
                    "Select text in chat, then Shift+right-click → Search the web. (Plain right-click copies the selection.)",
                    crate::theme::Segmented::new("settings-search-engine")
                        .selected_index(crate::message_actions::SEARCH_ENGINE.load(std::sync::atomic::Ordering::Relaxed))
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            crate::message_actions::SEARCH_ENGINE.store(*index, std::sync::atomic::Ordering::Relaxed);
                            this.schedule_save(cx);
                            cx.notify();
                        }))
                        .children(crate::message_actions::SEARCH_ENGINES.map(|(l, _)| crate::theme::segment(l))),
                ))
                .child(row(
                    "Whispers in chat",
                    "Also show each whisper you get as a line in the chats on screen, like Chatterino's inline whispers. Hidden in streamer mode.",
                    Switch::new("settings-inline-whispers")
                        .checked(crate::whispers::INLINE.load(std::sync::atomic::Ordering::Relaxed))
                        .on_click(cx.listener(|this, on: &bool, _, cx| {
                            crate::whispers::INLINE.store(*on, std::sync::atomic::Ordering::Relaxed);
                            this.schedule_save(cx);
                            cx.notify();
                        })),
                ))
                .child(row(
                    "Joins and parts",
                    "Chatters arriving and leaving, listed every few seconds. Twitch only reports these in channels with fewer than 1,000 chatters.",
                    Switch::new("settings-joins-parts")
                        .checked(crate::anon_chat::SHOW_MEMBERSHIP.load(std::sync::atomic::Ordering::Relaxed))
                        .on_click(cx.listener(|this, on: &bool, _, cx| {
                            crate::anon_chat::SHOW_MEMBERSHIP.store(*on, std::sync::atomic::Ordering::Relaxed);
                            this.membership.clear();
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
                        "Log timestamps",
                        "The time written at the start of each line.",
                        crate::theme::Segmented::new("settings-log-time")
                            .selected_index(crate::chat_logs::TIME_FORMAT.load(std::sync::atomic::Ordering::Relaxed))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                crate::chat_logs::TIME_FORMAT.store(*index, std::sync::atomic::Ordering::Relaxed);
                                this.schedule_save(cx);
                                cx.notify();
                            }))
                            .children(crate::chat_logs::TIME_FORMATS.map(|(l, _)| crate::theme::segment(l))),
                    ))
                    .child(row(
                        "Which channels",
                        "Only chosen channels: right-click a tab → Log this channel.",
                        crate::theme::Segmented::new("settings-log-which")
                            .selected_index(usize::from(crate::chat_logs::ONLY_CHOSEN.load(std::sync::atomic::Ordering::Relaxed)))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                crate::chat_logs::ONLY_CHOSEN.store(*index == 1, std::sync::atomic::Ordering::Relaxed);
                                this.schedule_save(cx);
                                cx.notify();
                            }))
                            .children(["All channels", "Only chosen"].map(crate::theme::segment)),
                    ))
                    .child(row(
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
                    "Shared chat",
                    "When streamers share chat, messages from other channels get a small bar and an avatar ring in that streamer's colour, matching the bubbles at the top of the chat. Your own chat stays plain.",
                    crate::theme::Segmented::new("settings-shared-chat")
                        .selected_index(crate::shared_chat::style() as usize)
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            crate::shared_chat::STYLE.store((*index).min(3) as u8, std::sync::atomic::Ordering::Relaxed);
                            this.remeasure_all(cx);
                        }))
                        .children(crate::shared_chat::STYLES.map(|l| crate::theme::segment(l))),
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
                    "When someone posts nearly the same thing again, like Chatterino's similar-message filter.",
                    crate::theme::Segmented::new("settings-similar")
                        .selected_index(crate::similarity::Mode::ALL.iter().position(|m| *m == self.similar_mode).unwrap_or(0))
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            this.similar_mode = crate::similarity::Mode::ALL[*index];
                            this.schedule_save(cx);
                            cx.notify();
                        }))
                        .children(crate::similarity::Mode::ALL.map(|m| crate::theme::segment(m.label()))),
                ))
                .when(self.similar_mode != crate::similarity::Mode::Show, |el| {
                    use std::sync::atomic::Ordering::Relaxed;
                    let choice = |id: &'static str, value: &'static std::sync::atomic::AtomicUsize, labels: Vec<String>| {
                        crate::theme::Segmented::new(id)
                            .selected_index(value.load(Relaxed))
                            .on_click(cx.listener(move |this, index: &usize, _, cx| {
                                value.store(*index, Relaxed);
                                this.schedule_save(cx);
                                cx.notify();
                            }))
                            .children(labels.into_iter().map(crate::theme::segment))
                    };
                    el.child(row(
                        "How alike",
                        "How close a message must be to an earlier one to count as a repeat.",
                        choice("settings-similar-percent", &crate::similarity::PERCENT, crate::similarity::PERCENTS.iter().map(|p| format!("{p}%")).collect()),
                    ))
                    .child(stacked_row(
                        "Within",
                        "Only messages this recent are compared, up to this many of them.",
                        div().h_flex().gap_2()
                            .child(choice("settings-similar-window", &crate::similarity::WINDOW, ["10 s", "30 s", "1 min", "5 min"].map(String::from).to_vec()))
                            .child(choice("settings-similar-count", &crate::similarity::COUNT, crate::similarity::COUNTS.iter().map(|n| format!("{n} msg")).collect())),
                    ))
                    .child(stacked_row(
                        "Repeat options",
                        "Same chatter only compares a chatter with themselves; off catches copy-paste across chatters. Repeats normally never highlight or ping.",
                        div().h_flex().flex_wrap().gap_x_4().gap_y_1()
                            .children([(0usize, "Same chatter only"), (1, "Include my messages"), (2, "Repeats can highlight")].map(|(index, label)| {
                                gpui_kit::component::checkbox::Checkbox::new(("settings-similar-flag", index))
                                    .label(label)
                                    .checked(crate::similarity::flag(index))
                                    .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                        crate::similarity::FLAGS[index].store(*on, Relaxed);
                                        this.schedule_save(cx);
                                        cx.notify();
                                    }))
                            })),
                    ))
                })
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
                    "Live alerts for everyone you follow",
                    "A Windows notification when anyone you follow goes live, even if their chat isn't open. Click it to open their chat. Checked every two minutes.",
                    Switch::new("settings-live-follows").checked(self.live_notify_follows).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.live_notify_follows = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "Live alerts for open channels",
                    format!(
                        "A Windows notification when any channel you have open goes live. Otherwise only channels set to Notify when live in their tab menu ({} now).",
                        self.live_notify.len()
                    ),
                    Switch::new("settings-live-all").checked(self.live_notify_all).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.live_notify_all = *on;
                        this.schedule_save(cx);
                        cx.notify();
                    })),
                ))
                .child(row(
                    "With each live alert",
                    "Also play your ping sound, and flash Jawjack's taskbar button until you look.",
                    div().h_flex().gap_3()
                        .child(gpui_kit::component::checkbox::Checkbox::new("settings-live-sound").label("Sound").checked(self.live_alert_sound).on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.live_alert_sound = *on;
                            if *on {
                                crate::sound::play(&this.sound_tone, this.sound_volume);
                            }
                            this.schedule_save(cx);
                            cx.notify();
                        })))
                        .child(gpui_kit::component::checkbox::Checkbox::new("settings-live-flash").label("Flash taskbar").checked(self.live_alert_flash).on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.live_alert_flash = *on;
                            this.schedule_save(cx);
                            cx.notify();
                        }))),
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
                let active = index == self.settings_section
                    && self.settings_search.read(cx).value().trim().is_empty();
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
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.settings_section = index;
                        this.settings_search
                            .update(cx, |s, cx| s.set_value("", window, cx));
                        cx.notify();
                    }))
            }));
        let nav = if narrow {
            nav
        } else {
            div()
                .v_flex()
                .h_full()
                .bg(rgb(theme::PANEL))
                .child(
                    div()
                        .w(px(184.))
                        .p_2()
                        .pb_0()
                        .child(Input::new(&self.settings_search).xsmall().cleanable(true)),
                )
                .child(nav)
        };
        // Searching shows matching rows from every section, each under its name.
        let query = self.settings_search.read(cx).value().trim().to_lowercase();
        SEARCH.with_borrow_mut(|q| *q = query.clone());
        let title = if query.is_empty() {
            SECTIONS[self.settings_section.min(SECTIONS.len() - 1)].0
        } else {
            "Search results"
        };
        let page = if query.is_empty() {
            self.settings_page(window, cx)
        } else {
            let current = self.settings_section;
            let mut all = div().v_flex();
            let mut found = 0;
            for index in 0..SECTIONS.len() {
                self.settings_section = index;
                SHOWN.set(0);
                let section = self.settings_page(window, cx);
                if SHOWN.get() == 0 {
                    continue;
                }
                found += SHOWN.get();
                all = all
                    .child(
                        div()
                            .pt_3()
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::ACCENT))
                            .child(SECTIONS[index].0.to_uppercase()),
                    )
                    .child(section);
            }
            if found == 0 {
                all = all.child(
                    div()
                        .py_4()
                        .text_size(px(12.))
                        .text_color(rgb(theme::MUTED))
                        .child("No settings match. Try another word."),
                );
            }
            self.settings_section = current;
            all
        };
        SEARCH.with_borrow_mut(String::clear);
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
                "Shown after delete, timeout and ban in moderator mode. {user}, {msg-id} and {channel} are filled from the message. Any /command, including your custom commands.",
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

impl Workbench {
    /// Settings → Appearance → Chat font: a menu of the fonts installed on this PC.
    fn choose_chat_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut fonts = cx.text_system().all_font_names();
        fonts.retain(|f| !f.starts_with('.') && !f.starts_with('@'));
        fonts.sort_by_key(|f| f.to_lowercase());
        fonts.dedup();
        let owner = cx.entity().downgrade();
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(240., 380.);
        let height = (f32::from(window.viewport_size().height) - 200.).clamp(160., 480.);
        window.open_dialog(cx, move |dialog, _, _| {
            let current = crate::theme::chat_font();
            dialog.w(px(width)).title("Chat font").child(
                div()
                    .id("chat-font-list")
                    .v_flex()
                    .max_h(px(height))
                    .overflow_y_scroll()
                    .children(fonts.iter().enumerate().map(|(index, font)| {
                        let (owner, chosen) = (owner.clone(), font.clone());
                        let selected = current.as_deref() == Some(font.as_str());
                        div()
                            .id(("chat-font-option", index))
                            .px_2()
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .font_family(font.clone())
                            .when(selected, |el| {
                                el.bg(rgb(theme::CONTROL)).text_color(rgb(theme::ACCENT))
                            })
                            .hover(|s| s.bg(rgb(theme::HOVER)))
                            .child(font.clone())
                            .on_click(move |_, w, cx| {
                                if let Ok(mut font) = crate::theme::CHAT_FONT.lock() {
                                    *font = Some(chosen.clone());
                                }
                                w.close_dialog(cx);
                                let _ = owner.update(cx, |this, cx| this.remeasure_all(cx));
                            })
                    })),
            )
        });
    }
}
