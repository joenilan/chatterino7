//! The AutoMod strip above a channel's message box.
use crate::{ChannelPane, PaneEvent, theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Disableable, Icon, Sizable, StyledExt, WindowExt,
        button::{Button, ButtonVariants},
    },
    *,
};

impl ChannelPane {
    /// Held AutoMod messages, newest first, each with Allow and Deny.
    pub fn render_automod(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        const SHOWN: usize = 3;
        if self.automod.is_empty() {
            return None;
        }
        let hidden = self.automod.len().saturating_sub(SHOWN);
        Some(
            div()
                .v_flex()
                .mb_1()
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(0x5A4A1E))
                .bg(rgb(0x1E1A10))
                .overflow_hidden()
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .pt_1()
                        .text_size(px(10.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xE8C468))
                        .child(Icon::new(gpui_kit::assets::IconName::Shield).xsmall())
                        .child(format!("AUTOMOD · {} HELD", self.automod.len()))
                        .when(hidden > 0, |el| {
                            el.child(
                                div()
                                    .text_color(rgb(theme::MUTED))
                                    .child(format!("· {hidden} older not shown")),
                            )
                        })
                        .child(div().flex_1())
                        .child(
                            Button::new("automod-hide-channel")
                                .xsmall()
                                .ghost()
                                .label("Hide here")
                                .tooltip("Stop showing AutoMod in this channel (Settings > Chat brings it back)")
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(PaneEvent::HideAutoMod))),
                        ),
                )
                .children(
                    self.automod
                        .iter()
                        .rev()
                        .take(SHOWN)
                        .enumerate()
                        .map(|(index, held)| {
                            let (allow_id, deny_id, dismiss_id) =
                                (held.id.clone(), held.id.clone(), held.id.clone());
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
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
                                                .child(format!("{}: {}", held.name, held.text)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(rgb(theme::MUTED))
                                                .child(format!(
                                                    "{} · {}",
                                                    held.reason,
                                                    held.at.format("%H:%M")
                                                )),
                                        ),
                                )
                                .child(
                                    Button::new(("automod-allow", index))
                                        .xsmall()
                                        .primary()
                                        .label("Allow")
                                        .disabled(held.pending)
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(PaneEvent::AutoMod(allow_id.clone(), true))
                                        })),
                                )
                                .child(
                                    Button::new(("automod-deny", index))
                                        .xsmall()
                                        .ghost()
                                        .label("Deny")
                                        .disabled(held.pending)
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(PaneEvent::AutoMod(deny_id.clone(), false))
                                        })),
                                )
                                .child(
                                    Button::new(("automod-dismiss", index))
                                        .xsmall()
                                        .ghost()
                                        .icon(gpui_kit::component::IconName::Close)
                                        .tooltip("Dismiss: hide it here without allowing or denying")
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(PaneEvent::DismissAutoMod(dismiss_id.clone()))
                                        })),
                                )
                        }),
                )
                .into_any_element(),
        )
    }
}

impl ChannelPane {
    /// Label a message from a flagged chatter, now or when it arrives.
    pub fn mark_suspicious(&mut self, id: String, label: String, cx: &mut Context<Self>) {
        let mut found = false;
        self.timeline.borrow_mut().enrich(|m| {
            if m.id == id {
                m.presentation.suspicious = Some(label.clone());
                found = true;
            }
        });
        if found {
            self.scroller.update(cx, |s, cx| s.remeasure(cx));
            cx.notify();
        } else {
            if self.suspicious_pending.len() >= 200 {
                self.suspicious_pending.clear();
            }
            self.suspicious_pending.insert(id, label);
        }
    }
}

impl ChannelPane {
    /// Re-check the draft; emotes, chatter names and slash commands are skipped.
    pub fn check_spelling(&mut self, cx: &mut Context<Self>) {
        if !self.spellcheck {
            if !self.spelling.is_empty() {
                self.spelling.clear();
            }
            return;
        }
        let text = self.draft.read(cx).value().to_string();
        if text.trim_start().starts_with('/') {
            self.spelling.clear();
            return;
        }
        let channel = self.name.to_string();
        let catalog = self.catalog.clone();
        let timeline = self.timeline.clone();
        let known = move |word: &str| {
            let lower = word.to_lowercase();
            timeline
                .borrow()
                .messages()
                .iter()
                .rev()
                .take(300)
                .any(|m| {
                    m.login.as_deref() == Some(lower.as_str())
                        || m.display_name.eq_ignore_ascii_case(word)
                })
                || catalog
                    .borrow()
                    .expand(&channel, "", &[chat_core::Fragment::Text(word.to_owned())])
                    .iter()
                    .any(|f| matches!(f, chat_core::Fragment::Emote { .. }))
        };
        self.spelling = crate::spell::check(&text, &known, 3);
    }
    /// "Spelling: teh → the" above the message box; click a suggestion to fix it.
    pub fn render_spelling(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.spelling.is_empty() {
            return None;
        }
        Some(
            div()
                .h_flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .mb_1()
                .text_size(px(11.))
                .text_color(rgb(theme::MUTED))
                .child("Spelling:")
                .children(self.spelling.iter().enumerate().flat_map(
                    |(index, (word, suggestions))| {
                        let mut items: Vec<AnyElement> = vec![
                            div()
                                .text_color(rgb(0xF29D9D))
                                .child(word.clone())
                                .into_any_element(),
                        ];
                        if suggestions.is_empty() {
                            items.push(div().child("(no suggestions)").into_any_element());
                        }
                        for (n, suggestion) in suggestions.iter().enumerate() {
                            let (word, suggestion) = (word.clone(), suggestion.clone());
                            items.push(
                                Button::new(("spell-fix", index * 10 + n))
                                    .xsmall()
                                    .ghost()
                                    .label(format!("→ {suggestion}"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let text = this.draft.read(cx).value().to_string();
                                        let fixed =
                                            crate::spell::replace_word(&text, &word, &suggestion);
                                        this.draft.update(cx, |input, cx| {
                                            input.set_value(fixed, window, cx)
                                        });
                                        this.check_spelling(cx);
                                        cx.emit(PaneEvent::DraftChanged);
                                        cx.notify();
                                    }))
                                    .into_any_element(),
                            );
                        }
                        items
                    },
                ))
                .into_any_element(),
        )
    }
}

impl ChannelPane {
    /// Set or clear the view filter; returns a problem with the query, if any.
    pub fn set_view_filter(&mut self, query: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let query = query.trim();
        if query.is_empty() {
            self.view_filter = None;
        } else {
            let parsed = crate::chat_search::Query::parse(query);
            if let Some(problem) = parsed.error() {
                return Err(problem.to_owned());
            }
            self.view_filter = Some((query.to_owned(), std::rc::Rc::new(parsed)));
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
        cx.emit(PaneEvent::ViewFilterChanged);
        cx.notify();
        Ok(())
    }
    pub fn open_view_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("e.g. is:mod   -is:notice   from:bob   regex:\\bgg\\b")
        });
        if let Some((query, _)) = &self.view_filter {
            let query = query.clone();
            input.update(cx, |i, cx| i.set_value(query, window, cx));
        }
        let focus = input.clone();
        let owner = cx.entity().downgrade();
        let channel = crate::combined::title(&self.name);
        let width = (f32::from(window.viewport_size().width) - 24.).clamp(240., 420.);
        window.open_dialog(cx, move |dialog, _, _| {
            let (apply_input, apply_owner, clear_owner) = (input.clone(), owner.clone(), owner.clone());
            dialog
                .w(px(width))
                .title(format!("Filter #{channel}"))
                .on_ok(|_, _, _| false)
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(
                            "Show only messages matching a search: is:mod, is:sub, is:vip, is:first, from:name, has:link, badge:…, subtier:2, regex:…, -is:notice to exclude. Nothing is removed; clear the filter to see everything.",
                        ))
                        .child(gpui_kit::component::input::Input::new(&input).small()),
                )
                .footer(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("filter-clear").small().ghost().label("Show everything").on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = clear_owner.update(cx, |p, cx| p.set_view_filter("", cx));
                        }))
                        .child(Button::new("filter-apply").small().primary().label("Apply").on_click(move |_, w, cx| {
                            let query = apply_input.read(cx).value().to_string();
                            let result = apply_owner.update(cx, |p, cx| p.set_view_filter(&query, cx));
                            match result {
                                Ok(Err(problem)) => w.push_notification(gpui_kit::component::notification::Notification::info(problem), cx),
                                _ => w.close_dialog(cx),
                            }
                        })),
                )
        });
        focus.update(cx, |i, cx| i.focus(window, cx));
    }
    /// "Filtered: is:mod ×" in the header while a view filter is on.
    /// Every source shown here: the pane's own (or a combined feed's) plus linked ones.
    pub fn all_sources(&self) -> Vec<String> {
        let mut sources = crate::combined::sources(&self.name);
        sources.extend(
            self.linked
                .iter()
                .filter(|s| !sources.contains(s))
                .cloned()
                .collect::<Vec<_>>(),
        );
        sources
    }
    /// Link extra sources (your own YouTube live chat into your own channel).
    pub fn set_linked(&mut self, linked: Vec<String>, cx: &mut Context<Self>) {
        self.linked = linked;
        if !crate::combined::is_combined(&self.name) {
            self.send_to = (!self.linked.is_empty()).then(|| self.name.to_string());
        }
        let platforms = crate::combined::platforms_of(&self.all_sources());
        self.hidden_platforms.retain(|p| platforms.contains(p));
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
        cx.notify();
    }
    /// Show or hide one platform in a combined feed (at least one stays shown).
    pub fn toggle_platform(&mut self, platform: crate::combined::Platform, cx: &mut Context<Self>) {
        if let Some(at) = self.hidden_platforms.iter().position(|p| *p == platform) {
            self.hidden_platforms.remove(at);
        } else if crate::combined::platforms_of(&self.all_sources())
            .iter()
            .any(|p| *p != platform && !self.hidden_platforms.contains(p))
        {
            self.hidden_platforms.push(platform);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
        cx.emit(PaneEvent::ViewFilterChanged);
        cx.notify();
    }
    /// Twitch / YouTube chips on a combined feed that mixes platforms.
    pub fn render_platform_toggles(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let platforms = crate::combined::platforms_of(&self.all_sources());
        if platforms.len() < 2 {
            return None;
        }
        Some(
            div()
                .h_flex()
                .gap_1()
                .children(platforms.into_iter().map(|platform| {
                    let shown = !self.hidden_platforms.contains(&platform);
                    Button::new(SharedString::from(format!("platform-{}", platform.key())))
                        .xsmall()
                        .label(platform.label())
                        .when(shown, |b| b.primary())
                        .when(!shown, |b| b.ghost())
                        .tooltip(if shown {
                            format!("Showing {} chat · click to hide it", platform.label())
                        } else {
                            format!("{} chat hidden · click to show it", platform.label())
                        })
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_platform(platform, cx)),
                        )
                }))
                .into_any_element(),
        )
    }
    pub fn render_filter_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (query, _) = self.view_filter.as_ref()?;
        Some(
            div()
                .id("view-filter-chip")
                .h_flex()
                .items_center()
                .gap_1()
                .px(px(6.))
                .h(px(20.))
                .max_w(px(180.))
                .rounded(px(10.))
                .bg(rgb(0x272237))
                .text_size(px(10.))
                .text_color(rgb(theme::ACCENT))
                .cursor_pointer()
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(format!("Filtered: {query}")),
                )
                .child(
                    div()
                        .id("view-filter-clear")
                        .child("×")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            let _ = this.set_view_filter("", cx);
                        })),
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_view_filter(window, cx)))
                .into_any_element(),
        )
    }
}
