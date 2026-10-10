//! Session-local, successful sends only. Navigation never submits chat.
use crate::{ChannelPane, PaneEvent};
use gpui_kit::*;
#[derive(Default)]
pub struct History {
    account: Option<String>,
    items: Vec<String>,
    cursor: Option<usize>,
    draft: String,
    shown: String,
}
impl History {
    pub fn account(&mut self, id: Option<&str>) {
        if self.account.as_deref() != id {
            *self = Self {
                account: id.map(str::to_owned),
                ..Default::default()
            };
        }
    }
    pub fn record(&mut self, text: &str) {
        if !text.is_empty() && self.items.last().is_none_or(|last| last != text) {
            self.items.push(text.into());
            if self.items.len() > 100 {
                // Keep the item currently being recalled stable while an async send finishes.
                let remove = if self.cursor == Some(0) { 1 } else { 0 };
                self.items.remove(remove);
                if let Some(index) = self.cursor {
                    if index > remove {
                        self.cursor = Some(index - 1);
                    }
                }
            }
        }
        if self.cursor.is_none() {
            self.draft.clear();
            self.shown.clear();
        }
    }
    fn navigate(&mut self, current: &str, older: bool) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        if self.cursor.is_some() && current != self.shown {
            self.cursor = None;
        }
        let next = match self.cursor {
            None if older => {
                self.draft = current.into();
                self.items.len() - 1
            }
            None => return None,
            Some(index) if older => index.saturating_sub(1),
            Some(index) if index + 1 < self.items.len() => index + 1,
            Some(_) => {
                self.cursor = None;
                self.shown = self.draft.clone();
                return Some(self.draft.clone());
            }
        };
        self.cursor = Some(next);
        self.shown = self.items[next].clone();
        Some(self.shown.clone())
    }
}
impl ChannelPane {
    pub fn recall_input(&mut self, older: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.read(cx).focus_handle(cx).is_focused(window) {
            return;
        }
        let current = self.draft.read(cx).value().to_string();
        if let Some(value) = self.input_history.navigate(&current, older) {
            self.compose_revision = self.compose_revision.wrapping_add(1);
            self.draft.update(cx, |input, cx| {
                input.set_value(value.clone(), window, cx);
                input.set_selected_range(value.len()..value.len(), cx);
            });
            self.picker.open = false;
            self.picker.suggestions.clear();
            self.picker.token = None;
            cx.emit(PaneEvent::DraftChanged);
            cx.notify();
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}

use gpui_kit::{
    component::{
        Sizable, StyledExt, WindowExt,
        input::{Input, InputEvent, InputState},
    },
    prelude::FluentBuilder,
};
struct Browser {
    owner: WeakEntity<ChannelPane>,
    query: Entity<InputState>,
    items: Vec<String>,
    selected: usize,
    scroll: ScrollHandle,
    account: Option<String>,
}
impl Browser {
    fn matches(&self, cx: &App) -> Vec<usize> {
        if self
            .owner
            .upgrade()
            .is_none_or(|p| p.read(cx).input_history.account != self.account)
        {
            return vec![];
        }
        let query = self.query.read(cx).value().to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query
                    .split_whitespace()
                    .all(|q| s.to_lowercase().contains(q))
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn choose(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.items.get(index).cloned() else {
            return;
        };
        let applied = self
            .owner
            .update(cx, |pane, cx| {
                // Revalidate against current session/account history before changing a draft.
                if pane.input_history.account != self.account {
                    return false;
                }
                let Some(at) = pane.input_history.items.iter().rposition(|s| s == &text) else {
                    return false;
                };
                let current = pane.draft.read(cx).value().to_string();
                if pane.input_history.cursor.is_none() || current != pane.input_history.shown {
                    pane.input_history.draft = current;
                }
                pane.input_history.cursor = Some(at);
                pane.input_history.shown = text.clone();
                pane.compose_revision = pane.compose_revision.wrapping_add(1);
                pane.draft.update(cx, |input, cx| {
                    input.set_value(text.clone(), window, cx);
                    input.set_selected_range(text.len()..text.len(), cx);
                    input.focus(window, cx);
                });
                pane.picker.open = false;
                pane.picker.suggestions.clear();
                pane.picker.token = None;
                cx.emit(PaneEvent::DraftChanged);
                cx.notify();
                true
            })
            .unwrap_or(false);
        if applied {
            window.close_dialog(cx);
            let _ = self.owner.update(cx, |pane, cx| {
                pane.draft.update(cx, |input, cx| input.focus(window, cx))
            });
        } else {
            window.push_notification(
                gpui_kit::component::notification::Notification::info(
                    "That sent message is no longer available for this account",
                ),
                cx,
            );
        }
    }
}
impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = self.matches(cx);
        self.selected = self.selected.min(matches.len().saturating_sub(1));
        let height = (f32::from(window.viewport_size().height) - 235.).clamp(65., 360.);
        let selected = self.selected;
        div().v_flex().gap_2().min_w_0()
            .capture_key_down(cx.listener(|this,event:&KeyDownEvent,w,cx|{
                if event.keystroke.modifiers.shift||event.keystroke.modifiers.control||event.keystroke.modifiers.alt||event.keystroke.modifiers.platform{return;}
                let found=this.matches(cx);
                match event.keystroke.key.as_str(){
                    "up"=>{this.selected=this.selected.saturating_sub(1);},
                    "down"=>{this.selected=(this.selected+1).min(found.len().saturating_sub(1));},
                    "enter"=>{if let Some(index)=found.get(this.selected){this.choose(*index,w,cx);}},
                    _=>return,
                }
                this.scroll.scroll_to_item(this.selected);w.prevent_default();cx.stop_propagation();cx.notify();
            }))
            .child(Input::new(&self.query).small())
            .child(div().text_size(px(11.)).text_color(rgb(crate::theme::MUTED)).child(format!("{} matches · newest first · Enter restores draft, never sends",matches.len())))
            .child(div().id("sent-history-results").v_flex().gap_1().max_h(px(height)).overflow_y_scroll().track_scroll(&self.scroll)
                .when(matches.is_empty(),|el|el.child(div().p_3().text_color(rgb(crate::theme::MUTED)).child(if self.items.is_empty(){"No successful sends in this channel yet."}else{"No sent messages match that search."})))
                .children(matches.into_iter().enumerate().map(|(row,index)|{
                    let text=self.items[index].clone();let preview=text.chars().take(160).collect::<String>();
                    div().id(("sent-history",index)).p_2().rounded(px(4.)).min_w_0().cursor_pointer()
                        .bg(rgb(if row==selected{0x303A50}else{crate::theme::CONTROL}))
                        .border_1().border_color(rgb(if row==selected{0x91C7E8}else{crate::theme::BORDER}))
                        .hover(|s|s.bg(rgb(crate::theme::HOVER)))
                        .child(div().text_size(px(12.)).child(preview))
                        .child(div().text_size(px(10.)).text_color(rgb(crate::theme::MUTED)).child(format!("{} characters",text.chars().count())))
                        .on_click(cx.listener(move|this,_,w,cx|this.choose(index,w,cx)))
                })))
            .child(div().text_size(px(11.)).text_color(rgb(crate::theme::MUTED)).child("Session-only history. Alt+Down returns through newer entries to your previous draft."))
    }
}
impl ChannelPane {
    pub fn open_input_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let account = self.input_history.account.clone();
        let items = self.input_history.items.iter().rev().cloned().collect();
        let owner = cx.entity().downgrade();
        let browser = cx.new(|cx| {
            let query =
                cx.new(|cx| InputState::new(window, cx).placeholder("Search sent messages…"));
            cx.subscribe(&query, |this: &mut Browser, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.selected = 0;
                    this.scroll.scroll_to_top_of_item(0);
                    cx.notify();
                }
            })
            .detach();
            Browser {
                owner,
                query,
                items,
                selected: 0,
                scroll: ScrollHandle::new(),
                account,
            }
        });
        let focus = browser.read(cx).query.read(cx).focus_handle(cx);
        let channel = self.name.to_string();
        window.open_dialog(cx, move |dialog, w, _| {
            dialog
                .title(format!("Sent history · #{channel}"))
                .w(px(
                    (f32::from(w.viewport_size().width) - 24.).clamp(180., 500.)
                ))
                .child(browser.clone())
        });
        focus.focus(window, cx);
    }
}
