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
                        }),
                )
                .children(
                    self.automod
                        .iter()
                        .rev()
                        .take(SHOWN)
                        .enumerate()
                        .map(|(index, held)| {
                            let (allow_id, deny_id) = (held.id.clone(), held.id.clone());
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
                        }),
                )
                .into_any_element(),
        )
    }
}
