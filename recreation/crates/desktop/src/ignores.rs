//! Local ignores: hide messages from chosen chatters, or hide/replace phrases.
//! Applied as messages arrive; nothing is sent to Twitch.
use crate::highlight_rules::{Pattern, compile};
use crate::theme;
use chat_core::{Fragment, Message};
use gpui_kit::component::{
    Sizable, StyledExt,
    button::Button,
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc};

const MAX_USERS: usize = 256;
const MAX_PHRASES: usize = 128;

#[derive(Clone, PartialEq, Eq)]
pub struct Phrase {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    /// `None` hides the whole message; `Some` replaces each match.
    pub replace: Option<String>,
    compiled: Pattern,
}
impl Phrase {
    pub fn new(
        pattern: &str,
        regex: bool,
        case_sensitive: bool,
        replace: Option<String>,
    ) -> Result<Self, String> {
        let pattern = pattern.trim().to_owned();
        if pattern.is_empty()
            || pattern.chars().count() > 200
            || pattern.chars().any(char::is_control)
        {
            return Err("Enter 1–200 characters on one line.".into());
        }
        if replace
            .as_ref()
            .is_some_and(|r| r.chars().count() > 100 || r.chars().any(char::is_control))
        {
            return Err("Replacement text must be one line of up to 100 characters.".into());
        }
        let source = if regex {
            pattern.clone()
        } else {
            regex::escape(&pattern)
        };
        Ok(Self {
            compiled: compile(&source, case_sensitive)?,
            pattern,
            regex,
            case_sensitive,
            replace,
        })
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Ignores {
    /// Twitch logins, lowercase.
    pub users: Vec<String>,
    pub phrases: Vec<Phrase>,
    /// Twitch-blocked user IDs (session only, from the account's block list);
    /// empty when hiding blocked users is turned off.
    pub blocked: std::collections::BTreeSet<String>,
}
pub type Shared = Rc<RefCell<Ignores>>;

pub enum Outcome {
    Keep,
    Hide,
}
impl Ignores {
    pub fn add_user(&mut self, login: &str) -> Result<(), String> {
        let login = chat_core::twitch_login(login.trim().trim_start_matches('@'))
            .ok_or("Enter a Twitch login.")?;
        if self.users.contains(&login) {
            return Ok(());
        }
        if self.users.len() >= MAX_USERS {
            return Err(format!(
                "{MAX_USERS} chatters are ignored. Remove one first."
            ));
        }
        self.users.push(login);
        Ok(())
    }
    pub fn add_phrase(&mut self, phrase: Phrase) -> Result<(), String> {
        if self
            .phrases
            .iter()
            .any(|p| p.pattern == phrase.pattern && p.regex == phrase.regex)
        {
            return Ok(());
        }
        if self.phrases.len() >= MAX_PHRASES {
            return Err(format!(
                "{MAX_PHRASES} phrases are ignored. Remove one first."
            ));
        }
        self.phrases.push(phrase);
        Ok(())
    }
    /// Hide the message, or rewrite matched text in place.
    pub fn apply(&self, message: &mut Message) -> Outcome {
        if self.blocked.contains(&message.user_id) {
            return Outcome::Hide;
        }
        if message
            .login
            .as_deref()
            .is_some_and(|login| self.users.iter().any(|u| u.eq_ignore_ascii_case(login)))
        {
            return Outcome::Hide;
        }
        for phrase in &self.phrases {
            let regex = &phrase.compiled.0;
            let Some(replacement) = &phrase.replace else {
                // Hiding considers the whole original message, emote codes included.
                if regex.is_match(&message.body()) {
                    return Outcome::Hide;
                }
                continue;
            };
            // Replacement edits text only, so emotes and mentions stay intact.
            for fragment in &mut message.fragments {
                if let Fragment::Text(text) = fragment {
                    *text = regex
                        .replace_all(text, regex::NoExpand(replacement))
                        .into_owned();
                }
            }
        }
        Outcome::Keep
    }
}
pub fn load(value: &serde_json::Value) -> Shared {
    let mut ignores = Ignores::default();
    for user in value["users"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_USERS)
    {
        if let Some(user) = user.as_str() {
            let _ = ignores.add_user(user);
        }
    }
    for item in value["phrases"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_PHRASES)
    {
        if let Ok(phrase) = Phrase::new(
            item["pattern"].as_str().unwrap_or(""),
            item["regex"].as_bool().unwrap_or(false),
            item["case_sensitive"].as_bool().unwrap_or(false),
            item["replace"].as_str().map(str::to_owned),
        ) {
            let _ = ignores.add_phrase(phrase);
        }
    }
    Rc::new(RefCell::new(ignores))
}
pub fn json(ignores: &Shared) -> serde_json::Value {
    let ignores = ignores.borrow();
    serde_json::json!({
        "users": ignores.users,
        "phrases": ignores.phrases.iter().map(|p| serde_json::json!({
            "pattern": p.pattern,
            "regex": p.regex,
            "case_sensitive": p.case_sensitive,
            "replace": p.replace,
        })).collect::<Vec<_>>(),
    })
}

pub struct Changed;
impl EventEmitter<Changed> for Editor {}
pub struct Editor {
    ignores: Shared,
    entry: Entity<InputState>,
    replacement: Entity<InputState>,
    user: bool,
    regex: bool,
    case_sensitive: bool,
    replace: bool,
    status: String,
}
impl Editor {
    pub fn new(ignores: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let entry = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Twitch login or phrase")
                .validate(|v, _| v.chars().count() <= 200 && !v.chars().any(char::is_control))
        });
        let replacement = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Replace with (empty removes the text)")
                .validate(|v, _| v.chars().count() <= 100 && !v.chars().any(char::is_control))
        });
        cx.subscribe_in(
            &entry,
            window,
            |this: &mut Self, _, event, window, cx| match event {
                InputEvent::Change => {
                    this.status.clear();
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.add(window, cx),
                _ => {}
            },
        )
        .detach();
        Self {
            ignores,
            entry,
            replacement,
            user: true,
            regex: false,
            case_sensitive: false,
            replace: false,
            status: String::new(),
        }
    }
    /// Clear transient feedback when the dialog is shown again.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.status.clear();
        cx.notify();
    }
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.entry.read(cx).value().to_string();
        let result = if self.user {
            self.ignores.borrow_mut().add_user(&value)
        } else {
            let replace = self
                .replace
                .then(|| self.replacement.read(cx).value().to_string());
            Phrase::new(&value, self.regex, self.case_sensitive, replace)
                .and_then(|p| self.ignores.borrow_mut().add_phrase(p))
        };
        match result {
            Ok(()) => {
                self.entry.update(cx, |i, cx| i.set_value("", window, cx));
                self.status = "Applies to new messages. Saved locally.".into();
                cx.emit(Changed);
            }
            Err(e) => self.status = e,
        }
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (f32::from(window.viewport_size().height) - 150.).clamp(100., 560.);
        let ignores = self.ignores.borrow().clone();
        let row = |label: String,
                   detail: String,
                   id: SharedString,
                   remove: Box<dyn Fn(&mut Ignores)>,
                   cx: &mut Context<Self>| {
            div()
                .h_flex()
                .gap_2()
                .items_center()
                .min_w_0()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme::MUTED))
                        .whitespace_nowrap()
                        .child(detail),
                )
                .child(
                    Button::new(id)
                        .xsmall()
                        .label("Remove")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            remove(&mut this.ignores.borrow_mut());
                            this.status = "Removed. Applies to new messages.".into();
                            cx.emit(Changed);
                            cx.notify();
                        })),
                )
        };
        let mut rows = Vec::new();
        for (index, user) in ignores.users.iter().enumerate() {
            let login = user.clone();
            rows.push(row(
                format!("@{user}"),
                "hide messages".into(),
                SharedString::from(format!("ignore-user-{index}")),
                Box::new(move |i: &mut Ignores| i.users.retain(|u| *u != login)),
                cx,
            ));
        }
        for (index, phrase) in ignores.phrases.iter().enumerate() {
            let (pattern, regex) = (phrase.pattern.clone(), phrase.regex);
            let mut detail = match &phrase.replace {
                None => "hide message".to_string(),
                Some(r) if r.is_empty() => "remove text".into(),
                Some(r) => format!("→ {r}"),
            };
            if phrase.regex {
                detail.push_str(" · regex");
            }
            rows.push(row(
                phrase.pattern.clone(),
                detail,
                SharedString::from(format!("ignore-phrase-{index}")),
                Box::new(move |i: &mut Ignores| {
                    i.phrases
                        .retain(|p| !(p.pattern == pattern && p.regex == regex))
                }),
                cx,
            ));
        }
        div().id("ignores-editor").v_flex().gap_2().max_h(px(height)).overflow_y_scroll()
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Hidden locally as messages arrive; already shown messages stay. Nothing is sent to Twitch, and Twitch blocks are separate."))
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("ignore-kind").small().label(if self.user { "Chatter" } else { "Phrase" }).on_click(cx.listener(|this, _, _, cx| { this.user = !this.user; cx.notify(); })))
                .when(!self.user, |el| el
                    .child(Button::new("ignore-regex").small().label(if self.regex { "Regex" } else { "Plain text" }).on_click(cx.listener(|this, _, _, cx| { this.regex = !this.regex; cx.notify(); })))
                    .child(Button::new("ignore-case").small().label(if self.case_sensitive { "Match case" } else { "Ignore case" }).on_click(cx.listener(|this, _, _, cx| { this.case_sensitive = !this.case_sensitive; cx.notify(); })))
                    .child(Button::new("ignore-action").small().label(if self.replace { "Replace text" } else { "Hide message" }).on_click(cx.listener(|this, _, _, cx| { this.replace = !this.replace; cx.notify(); })))))
            .child(div().h_flex().gap_1()
                .child(div().flex_1().child(Input::new(&self.entry).small()))
                .child(Button::new("ignore-add").small().label("Add").on_click(cx.listener(|this, _, w, cx| this.add(w, cx)))))
            .when(!self.user && self.replace, |el| el.child(Input::new(&self.replacement).small()))
            .when(!self.status.is_empty(), |el| el.child(div().text_size(px(11.)).text_color(rgb(0xE9C99C)).child(self.status.clone())))
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("{} chatters · {} phrases", ignores.users.len(), ignores.phrases.len())))
            .children(rows)
    }
}
