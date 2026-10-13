//! Local-profile notes. Never included in chat, copied messages, or provider requests.
use crate::{chatter_appearance::Target, theme};
use gpui_kit::{
    component::{
        Disableable, Sizable, StyledExt,
        button::Button,
        input::{InputEvent, Textarea, TextareaState},
    },
    prelude::FluentBuilder,
    *,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
#[derive(Clone, Default)]
pub struct Note {
    pub text: String,
    pub login: String,
    pub name: String,
}
pub type Notes = Rc<RefCell<BTreeMap<String, Note>>>;
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 255 && !id.chars().any(char::is_control)
}
fn valid_text(text: &str) -> bool {
    text.chars().count() <= 2000
        && !text.chars().any(|c| {
            (c.is_control() && !matches!(c, '\n' | '\t'))
                || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
}
pub fn load(value: &serde_json::Value) -> Notes {
    let mut notes = BTreeMap::new();
    if let Some(map) = value.as_object() {
        for (id, item) in map.iter().take(256) {
            let text = item["text"].as_str().unwrap_or("");
            if !valid_id(id) || text.trim().is_empty() || !valid_text(text) {
                continue;
            }
            let login =
                chat_core::twitch_login(item["login"].as_str().unwrap_or("")).unwrap_or_default();
            let name = item["name"]
                .as_str()
                .unwrap_or(&login)
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect();
            notes.insert(
                id.clone(),
                Note {
                    text: text.into(),
                    login,
                    name,
                },
            );
            if stored_size(&notes) > 192 * 1024 {
                notes.remove(id);
            }
        }
    }
    Rc::new(RefCell::new(notes))
}
fn stored(notes: &BTreeMap<String, Note>) -> serde_json::Value {
    serde_json::Value::Object(
        notes
            .iter()
            .map(|(id, n)| {
                (
                    id.clone(),
                    serde_json::json!({"text":n.text,"login":n.login,"name":n.name}),
                )
            })
            .collect(),
    )
}
fn stored_size(notes: &BTreeMap<String, Note>) -> usize {
    serde_json::to_vec_pretty(&stored(notes)).map_or(usize::MAX, |bytes| bytes.len())
}
pub fn json(notes: &Notes) -> serde_json::Value {
    stored(&notes.borrow())
}
pub struct Changed;
pub struct Editor {
    notes: Notes,
    input: Entity<TextareaState>,
    target: Option<Target>,
    pending: Option<Target>,
    delete_armed: bool,
    status: String,
}
impl EventEmitter<Changed> for Editor {}
impl Editor {
    pub fn new(notes: Notes, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(4, 10)
                .placeholder("A reminder about this chatter…")
        });
        cx.subscribe_in(&input, window, |this: &mut Self, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.delete_armed = false;
                this.pending = None;
                this.status.clear();
                cx.notify();
            }
        })
        .detach();
        Self {
            notes,
            input,
            target: None,
            pending: None,
            delete_armed: false,
            status: String::new(),
        }
    }
    pub fn open(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        if !valid_id(&target.id) {
            return;
        }
        self.delete_armed = false;
        self.pending = None;
        self.status.clear();
        if self.target.as_ref().is_some_and(|t| t.id == target.id) {
            return;
        }
        let original = self
            .target
            .as_ref()
            .and_then(|t| self.notes.borrow().get(&t.id).map(|n| n.text.clone()))
            .unwrap_or_default();
        if self.target.is_some() && self.input.read(cx).value().as_ref() != original {
            self.pending = Some(target);
            self.status = "Unsaved note kept. Save it, or discard edits to switch chatters.".into();
            cx.notify();
            return;
        }
        self.select(target, window, cx);
    }
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }
    fn select(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .notes
            .borrow()
            .get(&target.id)
            .map(|n| n.text.clone())
            .unwrap_or_default();
        self.input.update(cx, |input, cx| {
            input.set_value(text, window, cx);
        });
        self.target = Some(target);
        self.pending = None;
        self.delete_armed = false;
        self.status.clear();
        cx.notify();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = &self.target else {
            return;
        };
        let raw = self.input.read(cx).value().to_string();
        let text = raw.replace("\r\n", "\n");
        if !valid_text(&text) {
            self.status = "Use at most 2,000 characters, without hidden control characters.".into();
            cx.notify();
            return;
        }
        if text.trim().is_empty() {
            self.status = "Use Delete note to remove a saved note.".into();
            cx.notify();
            return;
        }
        let mut notes = self.notes.borrow_mut();
        if !notes.contains_key(&target.id) && notes.len() >= 256 {
            self.status = "256 notes are saved. Remove an unused note first.".into();
            cx.notify();
            return;
        }
        let previous = notes.insert(
            target.id.clone(),
            Note {
                text: text.clone(),
                login: chat_core::twitch_login(&target.login).unwrap_or_default(),
                name: target
                    .name
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(80)
                    .collect(),
            },
        );
        if stored_size(&notes) > 192 * 1024 {
            if let Some(previous) = previous {
                notes.insert(target.id.clone(), previous);
            } else {
                notes.remove(&target.id);
            }
            self.status =
                "The local notes storage budget is full. Shorten a note or delete an unused one."
                    .into();
            cx.notify();
            return;
        }
        drop(notes);
        let next = self.pending.take();
        if raw != text {
            self.input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
        self.delete_armed = false;
        self.status = "Local save queued. Never sent to Twitch or shown in chat.".into();
        cx.emit(Changed);
        if let Some(next) = next {
            self.select(next, window, cx);
        }
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (f32::from(window.viewport_size().height) - 150.).clamp(100., 560.);
        let count = self.input.read(cx).value().chars().count();
        let saved = self
            .target
            .as_ref()
            .is_some_and(|t| self.notes.borrow().contains_key(&t.id));
        let entries = self
            .notes
            .borrow()
            .iter()
            .map(|(id, n)| Target {
                id: id.clone(),
                login: n.login.clone(),
                name: n.name.clone(),
            })
            .collect::<Vec<_>>();
        div().id("private-chatter-notes").v_flex().gap_2().max_h(px(height)).overflow_y_scroll()
            .capture_key_down(cx.listener(|this,e:&KeyDownEvent,w,cx|{if e.keystroke.modifiers.control&&!e.keystroke.modifiers.alt&&e.keystroke.key=="s"{this.save(w,cx);w.prevent_default();cx.stop_propagation();}}))
            .child(div().text_size(px(11.)).text_color(rgb(theme::muted())).child("Private to this Jawjack profile, stored locally as plain text. Hidden from ordinary chatter cards and transcripts. Anyone who can access this profile’s files can read them."))
            .when_some(self.target.clone(),|el,t|el.child(div().text_size(px(12.)).child(format!("{} · @{}",t.name,t.login))))
            .child(Textarea::new(&self.input))
            .child(div().text_size(px(11.)).text_color(rgb(if count>2000{0xE69DCD}else{theme::muted()})).child(format!("{count}/2,000 characters")))
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("save-private-note").small().label("Save · Ctrl+S").disabled(self.target.is_none()).on_click(cx.listener(|this,_,w,cx|this.save(w,cx))))
                .child(Button::new("delete-private-note").small().label(if self.delete_armed{"Confirm delete"}else{"Delete note…"}).disabled(!saved).on_click(cx.listener(|this,_,w,cx|{
                    if !this.delete_armed{this.delete_armed=true;this.status="Confirm to delete this locally saved note.".into();cx.notify();return;}
                    if let Some(target)=&this.target{this.notes.borrow_mut().remove(&target.id);}
                    this.input.update(cx,|input,cx|input.set_value("",w,cx));this.delete_armed=false;this.pending=None;this.status="Note removed; local save queued.".into();cx.emit(Changed);cx.notify();
                })))
                .when(self.pending.is_some(),|el|el.child(Button::new("discard-note-switch").small().label("Discard edits & switch").on_click(cx.listener(|this,_,w,cx|{if let Some(next)=this.pending.take(){this.select(next,w,cx);}})))))
            .when(!self.status.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(crate::theme::tone(0xE9C99C))).child(self.status.clone())))
            // Saved-chatter navigation sits last, so saving or deleting never shifts the controls above.
            .when(!entries.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(theme::muted())).child("Saved notes")).child(div().id("saved-note-chatters").h_flex().flex_wrap().gap_1().max_h(px(90.)).overflow_y_scroll().children(entries.into_iter().map(|target|{
                let label=if target.login.is_empty(){target.name.clone()}else{format!("@{}",target.login)};
                Button::new(SharedString::from(format!("note-{}",target.id))).xsmall().label(label).on_click(cx.listener(move|this,_,w,cx|{this.open(target.clone(),w,cx);this.focus(w,cx);}))
            }))))
    }
}
