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
