//! Channel-aware picker and source-text completion. Insertion never sends chat.
use crate::{
    ChannelPane, ComposerFeedback, PaneEvent, media::DecodedMedia, theme, twitch_assets::Choice,
};
use gpui_kit::base::ElementExt;
use gpui_kit::{
    component::{
        Sizable, StyledExt, WindowExt,
        button::Button,
        input::Textarea,
        notification::{Notification, NotificationDelivery},
    },
    prelude::FluentBuilder,
    *,
};
use std::{ops::Range, sync::Arc};
// Track visible popup bounds across panes so transcript capture handlers respect overlays.
#[derive(Default)]
struct BrowserOverlays(Vec<(WeakEntity<ChannelPane>, WindowId, Bounds<Pixels>)>);
impl Global for BrowserOverlays {}
pub fn covers(position: Point<Pixels>, window: &Window, cx: &App) -> bool {
    cx.try_global::<BrowserOverlays>().is_some_and(|overlays| overlays.0.iter().any(|(owner, id, bounds)| {
        *id == window.window_handle().window_id() && bounds.contains(&position)
            && owner.upgrade().is_some_and(|pane| pane.read(cx).picker.open)
    }))
}
#[derive(Clone)]
pub struct BrowserChoice {
    pub choice: Choice,
    pub origin: String,
    pub available: bool,
    pub saved_key: Option<[String; 3]>,
}
#[derive(Clone)]
pub struct Collection {
    pub id: String,
    pub title: String,
    pub user: Option<String>,
    pub items: Vec<BrowserChoice>,
}
#[derive(Clone)]
enum GridRow {
    Heading(String),
    Cells(Range<usize>),
}
#[derive(Default)]
pub struct Picker {
    pub open: bool,
    pub generation: usize,
    pub button_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    pub suggestions: Vec<Choice>,
    pub selected: usize,
    pub token: Option<(Range<usize>, String)>,
    collections: Vec<Collection>,
    collection: String,
    service: String,
    items: Vec<BrowserChoice>,
    rows: Vec<GridRow>,
    columns: usize,
    current: usize,
    hover_text: String,
    scroll: UniformListScrollHandle,
    tabs_scroll: ScrollHandle,
}
impl Picker {
    pub fn inspection(&self) -> serde_json::Value {
        serde_json::json!({"open":self.open,"collection":self.collection,"service":self.service,"matches":self.items.len(),"columns":self.columns,"virtual_rows":self.rows.len(),"selected":self.current,"collections":self.collections.iter().map(|c|serde_json::json!({"id":c.id,"title":c.title,"emotes":c.items.len()})).collect::<Vec<_>>()})
    }
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
    smooth_icon(image, label, width, height, true)
}
fn smooth_icon(image: Option<Arc<DecodedMedia>>, label: String, width: f32, height: f32, reduced: bool) -> AnyElement {
    if let Some(media) = image {
        let reveal = media.clone();
        let image_id = media.image.id.0;
        let element = canvas(
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
        .h(px(height));
        if !reduced && reveal.arrival_opacity() < 1. {
            element.with_animation(("emote-arrival", image_id), Animation::new(std::time::Duration::from_millis(180)), move |el, _| el.opacity(reveal.arrival_opacity())).into_any_element()
        } else { element.into_any_element() }
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
        self.picker.collections = self.catalog.borrow_mut().browser_collections(&self.name);
        if !self
            .picker
            .collections
            .iter()
            .any(|c| c.id == self.picker.collection)
        {
            self.picker.collection = "all".into();
        }
        self.rebuild_browser(false, cx);
    }
    pub fn reset_browser_search(&mut self, cx: &mut Context<Self>) {
        self.rebuild_browser(true, cx);
    }
    fn rebuild_browser(&mut self, reset: bool, cx: &mut Context<Self>) {
        let query = self.emote_search.read(cx).value().to_lowercase();
        let old = self.picker.items.get(self.picker.current).map(|i| {
            (
                i.choice.label.clone(),
                i.choice.provider,
                i.origin.clone(),
                i.choice.key.clone(),
                i.saved_key.clone(),
            )
        });
        self.picker.items = self
            .picker
            .collections
            .iter()
            .find(|c| c.id == self.picker.collection)
            .into_iter()
            .flat_map(|c| c.items.iter())
            .filter(|i| {
                (self.picker.service.is_empty() || i.choice.provider == self.picker.service)
                    && i.choice.label.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        self.picker.items.sort_by(|a, b| {
            (a.choice.provider, &a.origin, a.choice.label.to_lowercase()).cmp(&(
                b.choice.provider,
                &b.origin,
                b.choice.label.to_lowercase(),
            ))
        });
        self.picker.rows.clear();
        let columns = self.picker.columns.max(1);
        let mut start = 0;
        while start < self.picker.items.len() {
            let first = &self.picker.items[start];
            let mut end = start + 1;
            while end < self.picker.items.len()
                && self.picker.items[end].choice.provider == first.choice.provider
                && self.picker.items[end].origin == first.origin
            {
                end += 1;
            }
            self.picker.rows.push(GridRow::Heading(format!(
                "{} · {} · {}",
                first.choice.provider,
                first.origin,
                end - start
            )));
            for row in (start..end).step_by(columns) {
                self.picker
                    .rows
                    .push(GridRow::Cells(row..(row + columns).min(end)));
            }
            start = end;
        }
        self.picker.current = if reset {
            0
        } else {
            old.map(|key| {
                self.picker
                    .items
                    .iter()
                    .position(|i| {
                        (
                            i.choice.label.clone(),
                            i.choice.provider,
                            i.origin.clone(),
                            i.choice.key.clone(),
                i.saved_key.clone(),
                        ) == key
                    })
                    .unwrap_or(usize::MAX)
            })
            .unwrap_or(0)
        };
        if reset {
            self.picker.hover_text.clear();
            self.picker
                .scroll
                .scroll_to_item_strict(0, ScrollStrategy::Top);
        }
    }
    pub fn picker_action(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.picker.open {
            return false;
        }
        let len = self.picker.items.len();
        match key {
            "down" | "up" => {
                if len > 0 {
                    if let Some((row, range)) =
                        self.picker.rows.iter().enumerate().find_map(|(row, r)| {
                            if let GridRow::Cells(range) = r {
                                range
                                    .contains(&self.picker.current)
                                    .then_some((row, range.clone()))
                            } else {
                                None
                            }
                        })
                    {
                        let column = self.picker.current - range.start;
                        let target = if key == "down" {
                            self.picker.rows.iter().skip(row + 1).find_map(|r| {
                                if let GridRow::Cells(range) = r {
                                    Some(range.clone())
                                } else {
                                    None
                                }
                            })
                        } else {
                            self.picker.rows[..row].iter().rev().find_map(|r| {
                                if let GridRow::Cells(range) = r {
                                    Some(range.clone())
                                } else {
                                    None
                                }
                            })
                        };
                        if let Some(target) = target {
                            self.picker.current = target.start + column.min(target.len() - 1);
                        }
                    } else {
                        self.picker.current = if key == "down" { 0 } else { len - 1 };
                    }
                }
            }
            "right" => {
                if len > 0 {
                    self.picker.current = if self.picker.current >= len {
                        0
                    } else {
                        (self.picker.current + 1).min(len - 1)
                    };
                }
            }
            "left" => {
                self.picker.current = if self.picker.current >= len {
                    len.saturating_sub(1)
                } else {
                    self.picker.current.saturating_sub(1)
                };
            }
            "enter" => {
                if let Some(item) = self.picker.items.get(self.picker.current).cloned() {
                    self.insert_browser_item(&item, window, cx);
                }
            }
            _ => return false,
        }
        if let Some(row) =
            self.picker.rows.iter().position(
                |r| matches!(r,GridRow::Cells(range) if range.contains(&self.picker.current)),
            )
        {
            self.picker
                .scroll
                .scroll_to_item(row, ScrollStrategy::Nearest);
        }
        self.picker.hover_text.clear();
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
        true
    }
    fn toggle_browser_favorite(&mut self, item: &BrowserChoice, window: &mut Window, cx: &mut Context<Self>) {
        let result = if let Some(key) = &item.saved_key {
            self.catalog.borrow_mut().remove_saved_favorite(key);
            Ok(false)
        } else { self.catalog.borrow_mut().toggle_favorite(&item.choice) };
        let message = match result {
            Ok(true) => format!("{} added to favorites", item.choice.label),
            Ok(false) => format!("{} removed from favorites", item.choice.label),
            Err(error) => error.to_owned(),
        };
        if result.is_ok() {
            self.refresh_picker(cx);
            cx.emit(PaneEvent::EmotePreferencesChanged);
        }
        window.push_notification(Notification::info(message).id::<ComposerFeedback>().delivery(NotificationDelivery::InApp),cx);
        cx.notify();
    }
    fn insert_browser_item(
        &mut self,
        item: &BrowserChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = item.saved_key.is_none() && self
            .catalog
            .borrow()
            .choices(&self.name, &item.choice.label, usize::MAX)
            .into_iter()
            .any(|c| c.label == item.choice.label && c.key == item.choice.key);
        if current {
            self.insert_emote(&item.choice, false, window, cx);
        } else {
            self.refresh_picker(cx);
            window.push_notification(Notification::info("That emote is unavailable or its alias changed in this channel. Your draft is unchanged.").id::<ComposerFeedback>().delivery(NotificationDelivery::InApp),cx);
            cx.notify();
        }
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
        if raw.is_empty() || (!forced && !raw.starts_with(':') && !raw.starts_with('@')) {
            return;
        }
        if let Some(query) = raw.strip_prefix('@') {
            let query = query.to_ascii_lowercase();
            let mut seen = std::collections::HashSet::new();
            // Suggestions describe recent speakers, never a complete viewer roster.
            for message in self.timeline.borrow().messages().iter().rev() {
                let Some(login) = message.login.as_deref().and_then(chat_core::twitch_login) else {
                    continue;
                };
                if login.starts_with(&query) && seen.insert(login.clone()) {
                    self.picker.suggestions.push(Choice {
                        label: format!("@{login}"),
                        provider: "Recent chatter",
                        key: None,
                    });
                    if self.picker.suggestions.len() == 8 {
                        break;
                    }
                }
            }
            if !self.picker.suggestions.is_empty() {
                self.picker.token = Some((start..range.end, raw.into()));
            }
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
        let keep_open = self.picker.open && !completion;
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
                    "That insertion would exceed 500 characters. Your draft is unchanged.",
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
        self.picker.open = keep_open;
        if keep_open {
            self.emote_search.update(cx, |s, cx| s.focus(window, cx));
        }
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
    pub fn completion_action(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.picker.open {
            return self.picker_action(key, window, cx);
        }
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
                    .child("Suggestions · ↑ ↓ choose · Enter / Tab inserts · Esc dismisses"),
            )
            .children(
                items
                    .into_iter()
                    .enumerate()
                    .skip((self.picker.selected / 4) * 4)
                    .take(4)
                    .map(|(i, choice)| {
                        let image = choice
                            .key
                            .as_ref()
                            .and_then(|key| self.media.borrow_mut().get(key, cx));
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
                    }),
            )
            .into_any_element()
    }
    pub fn render_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selected_item=self.picker.items.get(self.picker.current).cloned();
        let selected_favorite=selected_item.as_ref().is_some_and(|i|i.saved_key.is_some() || self.catalog.borrow().is_favorite(&i.choice));
        let reduced = cx.reduce_motion();
        let owned_status=self.catalog.borrow().owned.status.clone();
        let provider_status=self.catalog.borrow().browser_status(&self.picker.service);
        let can_retry=self.catalog.borrow().can_retry_public();
        let empty_message=if self.picker.collection=="favorites" && self.emote_search.read(cx).value().trim().is_empty(){"Right-click an emote to favorite it, or select one and use the star button. Unavailable saved entries can still be removed. Channel availability always applies.".to_owned()}else if !self.emote_search.read(cx).value().trim().is_empty(){"No matching emotes. Try a different search or service.".to_owned()}else{provider_status};
        let height = (f32::from(window.viewport_size().height) - 70.).clamp(180., 390.);
        let width = self
            .viewport
            .borrow()
            .map(|b| f32::from(b.size.width))
            .unwrap_or(320.)
            .clamp(260., 460.)
            .min(f32::from(window.viewport_size().width) - 16.);
        let compact = height < 260.;
        let owner = cx.entity().downgrade();
        let popup_owner = owner.clone();
        let columns = self.picker.columns;
        let mut tabs = Vec::new();
        for collection in self
            .picker
            .collections
            .iter()
            .map(|c| Collection {
                id: c.id.clone(),
                title: c.title.clone(),
                user: c.user.clone(),
                items: Vec::new(),
            })
            .collect::<Vec<_>>()
        {
            let selected = self.picker.collection == collection.id;
            let avatar = collection.user.as_deref().and_then(|id| {
                self.catalog
                    .borrow()
                    .profiles
                    .get(id)
                    .and_then(|p| p.avatar.clone())
            });
            let image = avatar
                .as_ref()
                .and_then(|key| self.media.borrow_mut().get(key, cx));
            let fallback = match collection.id.as_str() {
                "all" => "All".into(),
                "global" => "◎".into(),
                "personal" => "P".into(),
                "favorites" => "★".into(),
                _ => collection
                    .title
                    .trim_start_matches('#')
                    .chars()
                    .take(2)
                    .collect::<String>()
                    .to_uppercase(),
            };
            let title = collection.title.clone();
            let profile_user = collection.user.clone();
            let id = collection.id.clone();
            tabs.push(
                div()
                    .id(SharedString::from(format!(
                        "emote-origin-{}",
                        collection.id
                    )))
                    .size(px(32.))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(if selected {
                        rgb(0xB9A1FF)
                    } else {
                        rgba(0x00000000)
                    })
                    .bg(if selected {
                        rgb(theme::HOVER)
                    } else {
                        rgb(theme::PANEL)
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::HOVER)))
                    .tooltip(move |w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(title.clone()).build(w, cx)
                    })
                    .child(icon(image, fallback, 30., 30.))
                    .on_mouse_down(MouseButton::Left, |_, w, cx| {
                        w.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.picker.collection = id.clone();
                        if let Some(user)=&profile_user {this.catalog.borrow_mut().profiles.request(user);}
                        this.rebuild_browser(true, cx);
                        cx.notify();
                    })),
            );
        }
        let mut services = Vec::new();
        for (id, label, mark, color) in [
            ("", "All services", "≡", theme::MUTED),
            ("Twitch", "Twitch", "T", 0xB9A1FF),
            ("7TV", "7TV", "7TV", 0xA7DBCE),
            ("BTTV", "BetterTTV", "BTTV", 0xF1B4BA),
            ("FFZ", "FrankerFaceZ", "FFZ", 0xE9C99C),
        ] {
            let selected = self.picker.service == id;
            services.push(
                div()
                    .id(SharedString::from(format!("emote-service-{id}")))
                    .w(px(34.))
                    .h(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(color))
                    .bg(if selected {
                        rgb(theme::HOVER)
                    } else {
                        rgb(theme::CANVAS)
                    })
                    .border_b_2()
                    .border_color(if selected {
                        rgb(color)
                    } else {
                        rgba(0x00000000)
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::HOVER)))
                    .tooltip(move |w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(label).build(w, cx)
                    })
                    .child(mark)
                    .on_mouse_down(MouseButton::Left, |_, w, cx| {
                        w.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.picker.service = id.into();
                        this.rebuild_browser(true, cx);
                        cx.notify();
                    })),
            );
        }
        let (compact_services, bottom_services) = if compact {
            (services, Vec::new())
        } else {
            (Vec::new(), services)
        };
        let title = self
            .picker
            .collections
            .iter()
            .find(|c| c.id == self.picker.collection)
            .map(|c| c.title.clone())
            .unwrap_or_default();
        let details = if !self.picker.hover_text.is_empty() {
            self.picker.hover_text.clone()
        } else {
            self.picker
                .items
                .get(self.picker.current)
                .map(|i| {
                    format!(
                        "{} · {}",
                        i.choice.label,
                        if i.available {
                            "Enter inserts · Alt ← → selects"
                        } else {
                            "Unavailable or shadowed here"
                        }
                    )
                })
                .unwrap_or_else(|| "Scroll to browse · Esc closes".into())
        };
        div().id("emote-browser").occlude().v_flex().h(px(height)).w(px(width)).min_w_0().overflow_hidden().p_2().gap_1().bg(rgb(theme::CANVAS)).border_1().border_color(rgb(theme::BORDER)).rounded(px(6.))
            .on_prepaint(move|bounds,window,cx|{
                let mut entries=cx.try_global::<BrowserOverlays>().map(|v|v.0.clone()).unwrap_or_default();
                entries.retain(|(owner,_,_)|owner!=&popup_owner && owner.upgrade().is_some_and(|p|p.read(cx).picker.open));
                entries.push((popup_owner.clone(),window.window_handle().window_id(),bounds));
                cx.set_global(BrowserOverlays(entries));
            })
            .on_mouse_down(MouseButton::Left,|_,_,cx|cx.stop_propagation())
            .on_scroll_wheel(|_,_,cx|cx.stop_propagation())
            .when(!compact,|el|el.child(div().h(px(18.)).flex_shrink_0().text_size(px(11.)).overflow_hidden().child(format!("{title} · {} emotes",self.picker.items.len()))))
            .child(div().h_flex().min_w_0().gap_1().flex_shrink_0().child(div().flex_1().min_w_0().child(Textarea::new(&self.emote_search))).when_some(selected_item,|el,item|el.child(Button::new("emote-favorite-selected").xsmall().label(if selected_favorite{"★"}else{"☆"}).tooltip(if selected_favorite{"Remove selected emote from favorites"}else{"Favorite selected emote · right-click also works"}).on_click(cx.listener(move|this,_,w,cx|this.toggle_browser_favorite(&item,w,cx)))))
                .child(Button::new("emotes-close").xsmall().label("×").tooltip("Close emotes · Esc").on_click(cx.listener(|this,_,w,cx|this.toggle_picker(w,cx)))))
            .child(div().id("emote-origin-tabs").h_flex().h(px(34.)).flex_shrink_0().gap_1().overflow_x_scroll().track_scroll(&self.picker.tabs_scroll).children(compact_services).children(tabs))
            .when(self.picker.service.is_empty()||self.picker.service=="Twitch",|el|el.child(div().h_flex().min_w_0().gap_1().flex_shrink_0()
                .child(div().flex_1().min_w_0().overflow_hidden().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(owned_status))
                .child(Button::new("emote-account-settings").xsmall().label("Account").tooltip("Manage Twitch account and subscription emote permission")
                    .on_click(cx.listener(|this,_,_,cx|{this.picker.open=false;cx.emit(PaneEvent::OpenAccount);cx.notify();})))))
            .child(div().flex_1().min_h_0().w_full().overflow_hidden().on_prepaint(move|bounds,_,cx|{
                let next=((f32::from(bounds.size.width)+4.)/44.).floor().max(1.)as usize;
                if next!=columns{let owner=owner.clone();cx.defer(move|cx|{let _=owner.update(cx,|this,cx|{if this.picker.columns!=next{this.picker.columns=next;this.rebuild_browser(false,cx);if let Some(row)=this.picker.rows.iter().position(|r|matches!(r,GridRow::Cells(range)if range.contains(&this.picker.current))){this.picker.scroll.scroll_to_item(row,ScrollStrategy::Nearest);}cx.notify();}});});}
            }).when(self.picker.items.is_empty(),|el|el.child(div().p_2().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(empty_message).when(can_retry,|el|el.child(Button::new("retry-public-emotes").xsmall().label("Retry public emotes").on_click(cx.listener(|this,_,_,cx|{if this.catalog.borrow_mut().retry_public(){this.refresh_picker(cx);cx.notify();}}))))))
            .when(!self.picker.items.is_empty(),|el|el.child(uniform_list("emote-browser-grid",self.picker.rows.len(),cx.processor(|this,range:Range<usize>,_,cx|{
                let ahead = range.end..(range.end + 2).min(this.picker.rows.len());
                let elements: Vec<_> = range.map(|row|{
                    match this.picker.rows[row].clone(){
                        GridRow::Heading(title)=>div().h(px(44.)).w_full().flex().items_center().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(title).into_any_element(),
                        GridRow::Cells(range)=>{
                            let mut cells=Vec::new();for index in range{
                                let item=this.picker.items[index].clone();let image=item.choice.key.as_ref().and_then(|key|this.media.borrow_mut().get(key,cx));let loading=image.is_none() && item.choice.key.as_ref().is_some_and(|key|!this.media.borrow().failed(key));let clicked=item.clone();let favorited=item.saved_key.is_some() || this.catalog.borrow().is_favorite(&item.choice);let favorite_item=item.clone();
                                let tooltip=format!("{} · {} · {}{}",item.choice.label,item.choice.provider,item.origin,if item.available{""}else{" · Not available with this alias in the current channel"});let tooltip=format!("{tooltip} · {}",if favorited{"Favorite · right-click to remove"}else{"Right-click to favorite"});let hover=tooltip.clone();
                                cells.push(div().id(("emote-browser-cell",index)).size(px(40.)).flex_shrink_0().flex().items_center().justify_center().rounded(px(4.)).border_1().border_color(if index==this.picker.current{rgb(0xB9A1FF)}else{rgba(0x00000000)}).opacity(if item.available{1.}else{0.4}).cursor_pointer().hover(|s|s.bg(rgb(theme::HOVER))).tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(w,cx)).relative().child(if loading { div().size(px(34.)).flex().items_center().justify_center().child(div().size(px(20.)).rounded(px(5.)).bg(rgb(theme::HOVER))).into_any_element() } else { smooth_icon(image,item.choice.label,34.,34.,cx.reduce_motion()) }).when(favorited,|el|el.child(div().absolute().top_0().right_0().text_size(px(9.)).text_color(rgb(0xE9C99C)).child("★"))).on_hover(cx.listener(move|this,over,_,cx|{if *over{this.picker.hover_text=hover.clone();this.picker.current=index;cx.notify();}})).on_mouse_down(MouseButton::Left,|_,w,cx|{w.prevent_default();cx.stop_propagation();}).on_mouse_down(MouseButton::Right,cx.listener(move|this,_,w,cx|{w.prevent_default();cx.stop_propagation();this.toggle_browser_favorite(&favorite_item,w,cx);})).on_click(cx.listener(move|this,_,w,cx|{this.insert_browser_item(&clicked,w,cx);})));
                            }
                            div().h(px(44.)).h_flex().gap_1().children(cells).into_any_element()
                        }
                    }
                }).collect();
                for row in ahead {
                    if let GridRow::Cells(indices) = this.picker.rows[row].clone() {
                        for index in indices {
                            if let Some(key) = &this.picker.items[index].choice.key { this.media.borrow_mut().prefetch(key,cx); }
                        }
                    }
                }
                elements
            })).track_scroll(&self.picker.scroll).h_full().w_full())))
            .when(!compact,|el|el.child(div().id("emote-services").h_flex().h(px(28.)).flex_shrink_0().gap_1().overflow_x_scroll().children(bottom_services)))
            .when(!compact,|el|el.child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).overflow_hidden().h(px(14.)).flex_shrink_0().child(details)))
            .relative().with_animation(("picker-reveal",self.picker.generation),Animation::new(std::time::Duration::from_millis(if reduced{1}else{140})),move|el,p|if reduced{el}else{el.opacity(p).top(px(5.*(1.-p)))})
            .into_any_element()
    }
}
