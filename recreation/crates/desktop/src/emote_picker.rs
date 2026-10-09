//! Channel-aware picker and source-text completion. Insertion never sends chat.
use crate::{
    ChannelPane, ComposerFeedback, PaneEvent, media::DecodedMedia, theme, twitch_assets::Choice,
};
use gpui_kit::{
    component::{
        Disableable, Sizable, StyledExt, WindowExt,
        button::Button,
        input::Textarea,
        notification::{Notification, NotificationDelivery},
    },
    prelude::FluentBuilder,
    *,
};
use std::{ops::Range, sync::Arc};
const PAGE: usize = 40;
#[derive(Default)]
pub struct Picker {
    pub open: bool,
    pub generation: usize,
    pub choices: Vec<Choice>,
    pub page: usize,
    pub suggestions: Vec<Choice>,
    pub selected: usize,
    pub token: Option<(Range<usize>, String)>,
}
pub fn preview(image: Option<Arc<DecodedMedia>>, label: String) -> AnyElement {
    icon(image, label, 34., 30.)
}
pub fn icon(
    image: Option<Arc<DecodedMedia>>,
    label: String,
    width: f32,
    height: f32,
) -> AnyElement {
    if let Some(media) = image {
        canvas(
            |bounds, _, _| bounds,
            move |bounds, _, window, cx| {
                let frame = media.frame(cx.reduce_motion());
                if media.image.frame_count() > 1 && !cx.reduce_motion() {
                    window.request_animation_frame();
                }
                let fit = ObjectFit::Contain.get_bounds(bounds, media.image.size(frame));
                let _ = window.paint_image(
                    bounds,
                    fit,
                    Corners::all(px(0.)),
                    media.image.clone(),
                    frame,
                    false,
                );
            },
        )
        .w(px(width))
        .h(px(height))
        .into_any_element()
    } else {
        div()
            .w(px(width))
            .h(px(height))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.))
            .text_color(rgb(theme::MUTED))
            .child(label.chars().take(3).collect::<String>())
            .into_any_element()
    }
}
impl ChannelPane {
    pub fn refresh_picker(&mut self, cx: &mut Context<Self>) {
        let query = self.emote_search.read(cx).value().to_string();
        self.picker.choices = self.catalog.borrow().choices(&self.name, &query, 10000);
        self.picker.page = self
            .picker
            .page
            .min(self.picker.choices.len().saturating_sub(1) / PAGE);
    }
    pub fn complete_query(&mut self, forced: bool, cx: &mut Context<Self>) {
        self.picker.suggestions.clear();
        self.picker.selected = 0;
        self.picker.token = None;
        if self.picker.open {
            return;
        }
        let draft = self.draft.read(cx);
        let value = draft.value().to_string();
        let range = draft.selected_range();
        if range.start != range.end || range.end > value.len() || !value.is_char_boundary(range.end)
        {
            return;
        }
        let start = value[..range.end]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let raw = &value[start..range.end];
        if raw.is_empty() || (!forced && !raw.starts_with(':')) {
            return;
        }
        let query = raw.strip_prefix(':').unwrap_or(raw);
        if query.chars().count() < 2 && !forced {
            return;
        }
        self.picker.suggestions = self.catalog.borrow().choices(&self.name, query, 8);
        if !self.picker.suggestions.is_empty() {
            self.picker.token = Some((start..range.end, raw.into()));
        }
    }
    pub fn insert_emote(
        &mut self,
        choice: &Choice,
        completion: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.draft.read(cx);
        let value = input.value().to_string();
        let mut range = input.selected_range();
        if completion {
            let Some((saved, raw)) = &self.picker.token else {
                return;
            };
            // A moved cursor or intervening edit cannot replace an unrelated word.
            if range.start != range.end
                || range.end != saved.end
                || value.get(saved.clone()) != Some(raw.as_str())
            {
                self.picker.suggestions.clear();
                self.picker.token = None;
                cx.notify();
                return;
            }
            range = saved.clone();
        }
        let before = &value[..range.start];
        let after = &value[range.end..];
        let left = if before.chars().last().is_some_and(|c| !c.is_whitespace()) {
            " "
        } else {
            ""
        };
        let right = if after.chars().next().is_none_or(|c| !c.is_whitespace()) {
            " "
        } else {
            ""
        };
        let replacement = format!("{left}{}{right}", choice.label);
        if before.chars().count() + replacement.chars().count() + after.chars().count() > 500 {
            window.push_notification(
                Notification::info(
                    "That emote would exceed 500 characters. Your draft is unchanged.",
                )
                .id::<ComposerFeedback>()
                .delivery(NotificationDelivery::InApp),
                cx,
            );
            return;
        }
        self.draft.update(cx, |input, cx| {
            input.set_selected_range(range, cx);
            input.replace(replacement, window, cx);
            input.focus(window, cx);
        });
        self.picker.open = false;
        self.picker.suggestions.clear();
        self.picker.token = None;
        cx.emit(PaneEvent::DraftChanged);
        cx.notify();
    }
    pub fn toggle_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker.open = !self.picker.open;
        self.picker.generation = self.picker.generation.wrapping_add(1);
        self.picker.suggestions.clear();
        self.picker.token = None;
        if self.picker.open {
            self.refresh_picker(cx);
            self.emote_search.update(cx, |s, cx| s.focus(window, cx));
        } else {
            self.draft.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }
    pub fn completion_action(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.picker.open {return false;}
        if key == "tab" && self.picker.suggestions.is_empty() {
            self.complete_query(true, cx);
        }
        if self.picker.suggestions.is_empty() {
            return false;
        }
        match key {
            "down" => {
                self.picker.selected = (self.picker.selected + 1) % self.picker.suggestions.len()
            }
            "up" => {
                self.picker.selected = (self.picker.selected + self.picker.suggestions.len() - 1)
                    % self.picker.suggestions.len()
            }
            "enter" | "tab" => {
                let choice = self.picker.suggestions[self.picker.selected].clone();
                self.insert_emote(&choice, true, window, cx);
            }
            "escape" => {
                self.picker.suggestions.clear();
                self.picker.token = None;
            }
            _ => return false,
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
        true
    }
    pub fn render_suggestions(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let items = self.picker.suggestions.clone();
        div()
            .v_flex()
            .gap_1()
            .p_2()
            .rounded(px(6.))
            .bg(rgb(theme::CANVAS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme::MUTED))
                    .child("Emotes · ↑ ↓ choose · Enter / Tab inserts · Esc dismisses"),
            )
            .children(items.into_iter().enumerate().map(|(i, choice)| {
                let image = self.media.borrow_mut().get(&choice.key, cx);
                div()
                    .id(("emote-suggest", i))
                    .h_flex()
                    .gap_2()
                    .px_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(i == self.picker.selected, |el| el.bg(rgb(theme::HOVER)))
                    .hover(|s| s.bg(rgb(theme::HOVER)))
                    .child(preview(image, choice.label.clone()))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .child(choice.label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme::MUTED))
                            .child(choice.provider),
                    )
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.insert_emote(&choice, true, window, cx)
                    }))
            }))
            .into_any_element()
    }
    pub fn render_picker(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let reduced = cx.reduce_motion();
        let total = self.picker.choices.len();
        let start = self.picker.page * PAGE;
        let choices: Vec<_> = self
            .picker
            .choices
            .iter()
            .skip(start)
            .take(PAGE)
            .cloned()
            .collect();
        let mut rows = Vec::new();
        for (row, chunk) in choices.chunks(5).enumerate() {
            let mut cells = Vec::new();
            for (col, choice) in chunk.iter().cloned().enumerate() {
                let image = self.media.borrow_mut().get(&choice.key, cx);
                let tooltip = format!(
                    "{} · {} · Insert into #{}",
                    choice.label, choice.provider, self.name
                );
                let clicked = choice.clone();
                cells.push(
                    div()
                        .id(("emote-choice", row * 5 + col))
                        .v_flex()
                        .flex_1()
                        .min_w_0()
                        .h(px(54.))
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::HOVER)))
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tooltip.clone())
                                .build(window, cx)
                        })
                        .child(preview(image, choice.label.clone()))
                        .child(
                            div()
                                .text_size(px(10.))
                                .overflow_hidden()
                                .child(choice.label),
                        )
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.insert_emote(&clicked, false, window, cx)
                        })),
                );
            }
            rows.push(div().h_flex().gap_1().children(cells));
        }
        div()
            .v_flex()
            .gap_2()
            .p_3()
            .bg(rgb(theme::CANVAS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .rounded(px(6.))

            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.))
                            .child(format!("Emotes · #{}", self.name)),
                    )
                    .child(
                        Button::new("emotes-close")
                            .xsmall()
                            .label("×")
                            .tooltip("Close emotes")
                            .on_click(cx.listener(|this, _, w, cx| this.toggle_picker(w, cx))),
                    ),
            )
            .child(Textarea::new(&self.emote_search))
            .child(
                div()
                    .id("emote-grid")
                    .v_flex()
                    .max_h(px(228.))
                    .overflow_y_scroll()
                    .gap_1()
                    .children(rows)
                    .when(total == 0, |el| {
                        el.child(
                            div()
                                .p_3()
                                .text_size(px(12.))
                                .text_color(rgb(theme::MUTED))
                                .child("No matching emotes yet. Catalogs load with your channel."),
                        )
                    }),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .text_size(px(10.))
                    .text_color(rgb(theme::MUTED))
                    .child(format!("{} emotes · Twitch · 7TV · BTTV · FFZ", total))
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .child(
                                Button::new("emotes-prev")
                                    .xsmall()
                                    .label("‹")
                                    .disabled(self.picker.page == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.picker.page = this.picker.page.saturating_sub(1);
                                        cx.notify();
                                    })),
                            )
                            .child(format!(
                                "{} / {}",
                                self.picker.page + 1,
                                total.div_ceil(PAGE).max(1)
                            ))
                            .child(
                                Button::new("emotes-next")
                                    .xsmall()
                                    .label("›")
                                    .disabled(start + PAGE >= total)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.picker.page += 1;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .relative()
            .with_animation(
                ("picker-reveal", self.picker.generation),
                Animation::new(std::time::Duration::from_millis(if reduced {
                    1
                } else {
                    160
                })),
                move |el, p| {
                    if reduced {
                        el
                    } else {
                        let eased = 1. - (1. - p).powi(3);
                        el.opacity(eased).top(px(6. * (1. - eased)))
                    }
                },
            )
            .into_any_element()
    }
}
