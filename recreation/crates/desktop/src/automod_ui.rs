//! The AutoMod strip above a channel's message box.
use crate::{ChannelPane, PaneEvent, theme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Disableable, Icon, Sizable, StyledExt,
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
