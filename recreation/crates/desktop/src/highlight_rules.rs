//! Bounded local highlight rules. Never changes message identity or sends anything.
use crate::theme;
use gpui_kit::component::Selectable;
use gpui_kit::component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc};
use unicode_segmentation::UnicodeSegmentation;

pub const COLORS: [(&str, u32, u32); 6] = [
    ("Lilac", 0xA99CF4, 0x272237),
    ("Mint", 0x91D7BA, 0x192D27),
    ("Sky", 0x91C7E8, 0x1B2934),
    ("Rose", 0xE69DCD, 0x332333),
    ("Peach", 0xE8C68A, 0x30291D),
    ("Red", 0xF07A7A, 0x3A1D20),
];
/// A rule's colour is an index into COLORS, or CUSTOM_COLOR | 0xRRGGBB for a
/// colour of your own. Read either with `colors`.
pub const CUSTOM_COLOR: usize = 1 << 24;
/// Accent and row background for a highlight colour (palette or custom).
pub fn colors(color: usize) -> (u32, u32) {
    if color & CUSTOM_COLOR != 0 {
        let accent = (color & 0xFF_FFFF) as u32;
        (accent, tint(accent))
    } else {
        let (_, accent, background) = COLORS[color.min(COLORS.len() - 1)];
        (accent, background)
    }
}
/// A dark row background carrying a little of `accent`, like the palette's.
fn tint(accent: u32) -> u32 {
    let base = [0x12u32, 0x13, 0x16];
    let mut out = 0;
    for (i, shift) in [16u32, 8, 0].into_iter().enumerate() {
        let channel = (accent >> shift) & 0xFF;
        out |= ((base[i] * 82 + channel * 18) / 100) << shift;
    }
    out
}
/// "#ff8800", "ff8800" or "#f80" as 0xRRGGBB.
pub fn parse_hex(text: &str) -> Option<u32> {
    let hex = text.trim().trim_start_matches('#');
    let full: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.into(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}
pub type Rules = Rc<RefCell<Vec<Rule>>>;
/// Regular expressions stay small so a pasted pattern cannot exhaust memory.
pub const REGEX_LIMIT: usize = 256 * 1024;
/// A compiled pattern compares by its source, keeping rules comparable.
#[derive(Clone)]
pub struct Pattern(pub std::sync::Arc<regex::Regex>);
impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}
impl Eq for Pattern {}
pub fn compile(pattern: &str, case_sensitive: bool) -> Result<Pattern, String> {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(REGEX_LIMIT)
        .dfa_size_limit(REGEX_LIMIT)
        .build()
        .map(|r| Pattern(std::sync::Arc::new(r)))
        .map_err(|_| "That regular expression is invalid or too complex.".into())
}
#[derive(Clone, PartialEq, Eq)]
pub struct Rule {
    folded: String,
    compiled: Option<Pattern>,
    pub pattern: String,
    pub regex: bool,
    pub channel: String,
    pub author: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub enabled: bool,
    pub color: usize,
    /// Match a chat badge set (moderator, vip, subscriber…) instead of text.
    pub badge: bool,
    /// Index into SOUNDS: what plays when this rule pings.
    pub sound: u8,
    /// The .wav played when `sound` is CUSTOM_SOUND.
    pub sound_file: String,
    /// Copy matches into Mentions tabs (Chatterino's "show in mentions").
    pub mentions: bool,
    /// Flash the taskbar (when that's on in Notifications).
    pub flash: bool,
}
/// Per-rule ping sound: the Highlights sound setting, nothing, or a fixed tone.
pub const SOUNDS: [&str; 5] = ["Default sound", "No sound", "Chime", "Soft", "Your file…"];
/// SOUNDS index for a rule's own .wav (`sound_file`).
pub const CUSTOM_SOUND: u8 = 4;
/// Badge sets offered as examples in the editor; any Twitch badge set ID works.
pub const BADGE_EXAMPLES: &str =
    "broadcaster, moderator, vip, subscriber, founder, partner, artist-badge";
impl Default for Rule {
    fn default() -> Self {
        Self {
            folded: String::new(),
            compiled: None,
            pattern: String::new(),
            regex: false,
            channel: String::new(),
            author: false,
            case_sensitive: false,
            whole_word: true,
            enabled: true,
            color: 0,
            badge: false,
            sound: 0,
            sound_file: String::new(),
            mentions: true,
            flash: true,
        }
    }
}
impl Rule {
    pub fn validate(mut self) -> Result<Self, String> {
        self.pattern = self.pattern.trim().to_owned();
        let limit = if self.regex { 200 } else { 80 };
        if self.pattern.is_empty()
            || self.pattern.chars().count() > limit
            || self.pattern.chars().any(char::is_control)
        {
            return Err(format!("Enter 1–{limit} characters on one line."));
        }
        self.compiled = None;
        if self.badge {
            self.author = false;
            self.regex = false;
            self.pattern = self.pattern.to_ascii_lowercase();
            if self.pattern.len() > 40
                || !self
                    .pattern
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(format!("Enter a badge such as {BADGE_EXAMPLES}."));
            }
        } else if self.regex {
            self.compiled = Some(compile(&self.pattern, self.case_sensitive)?);
        } else if self.author {
            self.pattern = chat_core::twitch_login(self.pattern.trim_start_matches('@'))
                .ok_or("Enter a Twitch login, not a display name.")?;
        }
        let channel = self.channel.trim();
        self.channel = if channel.is_empty() {
            String::new()
        } else {
            chat_core::twitch_login(channel.strip_prefix('#').unwrap_or(channel))
                .ok_or("Enter a channel login, or leave it empty for every channel.")?
        };
        self.folded = self.pattern.to_lowercase();
        if self.color & CUSTOM_COLOR == 0 {
            self.color = self.color.min(COLORS.len() - 1);
        } else {
            self.color &= CUSTOM_COLOR | 0xFF_FFFF;
        }
        self.sound = self.sound.min(SOUNDS.len() as u8 - 1);
        if self.sound == CUSTOM_SOUND && self.sound_file.is_empty() {
            self.sound = 0;
        }
        Ok(self)
    }
    pub fn matches(&self, text: &str, login: Option<&str>, channel: &str) -> bool {
        if !self.enabled
            || (!self.channel.is_empty() && !self.channel.eq_ignore_ascii_case(channel))
        {
            return false;
        }
        if self.badge {
            // The editor preview has no badges; treat the sample login as a badge list.
            return login.is_some_and(|l| l.split([',', ' ']).any(|b| b == self.pattern));
        }
        if let Some(regex) = &self.compiled {
            let subject = if self.author {
                login.unwrap_or("")
            } else {
                text
            };
            return regex.0.is_match(subject);
        }
        if self.author {
            return login.is_some_and(|login| login.eq_ignore_ascii_case(&self.pattern));
        }
        text_matches(text, &self.pattern, self.case_sensitive, self.whole_word)
    }
}
fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
pub fn text_matches(text: &str, needle: &str, case_sensitive: bool, whole: bool) -> bool {
    if needle.is_empty() {
        return false;
    }
    let (text, needle) = if case_sensitive {
        (text.to_owned(), needle.to_owned())
    } else {
        (text.to_lowercase(), needle.to_lowercase())
    };
    normalized_match(&text, &needle, whole)
}
pub fn normalized_match(text: &str, needle: &str, whole: bool) -> bool {
    if needle.is_empty() {
        return false;
    }
    if !whole {
        return text.contains(needle);
    }
    let Some(mut at) = text.find(needle) else {
        return false;
    };
    let boundaries = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    loop {
        if let (Ok(start), Ok(end)) = (
            boundaries.binary_search(&at),
            boundaries.binary_search(&(at + needle.len())),
        ) {
            let before = start == 0 || !text[boundaries[start - 1]..at].chars().any(word);
            let after = end + 1 >= boundaries.len()
                || !text[boundaries[end]..boundaries[end + 1]].chars().any(word);
            if before && after {
                return true;
            }
        }
        let next = at + text[at..].chars().next().unwrap().len_utf8();
        let Some(relative) = text[next..].find(needle) else {
            return false;
        };
        at = next + relative;
    }
}
pub fn first_color(rules: &[Rule], message: &chat_core::Message, channel: &str) -> Option<usize> {
    first_match(rules, message, channel).map(|rule| rule.color)
}
/// The first enabled rule that matches `message` in `channel` (rules are in priority order).
pub fn first_match<'a>(
    rules: &'a [Rule],
    message: &chat_core::Message,
    channel: &str,
) -> Option<&'a Rule> {
    if message.deleted || rules.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    let mut text = String::new();
    for fragment in &message.fragments {
        if let chat_core::Fragment::Text(part) | chat_core::Fragment::Mention { label: part, .. } =
            fragment
        {
            text.push_str(part);
        } else if !text.is_empty() {
            segments.push(std::mem::take(&mut text));
        }
    }
    if !text.is_empty() {
        segments.push(text);
    }
    let folded = if rules
        .iter()
        .any(|r| r.enabled && !r.author && !r.case_sensitive)
    {
        segments
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    rules.iter().find(|rule| {
        if !rule.enabled
            || (!rule.channel.is_empty() && !rule.channel.eq_ignore_ascii_case(channel))
        {
            return false;
        }
        if rule.badge {
            return message.badges.iter().any(|b| b.set_id == rule.pattern);
        }
        if let Some(regex) = &rule.compiled {
            // Like Chatterino, patterns see the whole original message,
            // emote codes included, not only the text between emotes.
            return if rule.author {
                message
                    .login
                    .as_deref()
                    .is_some_and(|l| regex.0.is_match(l))
            } else {
                regex.0.is_match(&message.body())
            };
        }
        if rule.author {
            return message
                .login
                .as_deref()
                .is_some_and(|login| login.eq_ignore_ascii_case(&rule.pattern));
        }
        let (segments, needle) = if rule.case_sensitive {
            (&segments, &rule.pattern)
        } else {
            (&folded, &rule.folded)
        };
        segments
            .iter()
            .any(|text| normalized_match(text, needle, rule.whole_word))
    })
}

pub fn load(value: &serde_json::Value) -> Rules {
    let mut rules = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items.iter().take(24) {
            let rule = Rule {
                folded: String::new(),
                compiled: None,
                pattern: item["pattern"].as_str().unwrap_or("").into(),
                regex: item["regex"].as_bool().unwrap_or(false),
                channel: item["channel"].as_str().unwrap_or("").into(),
                author: item["author"].as_bool().unwrap_or(false),
                case_sensitive: item["case_sensitive"].as_bool().unwrap_or(false),
                whole_word: item["whole_word"].as_bool().unwrap_or(true),
                enabled: item["enabled"].as_bool().unwrap_or(true),
                color: item["color"].as_u64().unwrap_or(0) as usize,
                badge: item["badge"].as_bool().unwrap_or(false),
                sound: item["sound"]
                    .as_u64()
                    .unwrap_or(0)
                    .min(SOUNDS.len() as u64 - 1) as u8,
                mentions: item["mentions"].as_bool().unwrap_or(true),
                flash: item["flash"].as_bool().unwrap_or(true),
                sound_file: item["sound_file"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(1024)
                    .collect(),
            };
            if let Ok(rule) = rule.validate() {
                rules.push(rule);
            }
        }
    }
    Rc::new(RefCell::new(rules))
}
pub fn json(rules: &Rules) -> serde_json::Value {
    serde_json::Value::Array(rules.borrow().iter().map(|r|{let mut v=serde_json::json!({"pattern":r.pattern,"regex":r.regex,"channel":r.channel,"author":r.author,"case_sensitive":r.case_sensitive,"whole_word":r.whole_word,"enabled":r.enabled,"color":r.color,"badge":r.badge,"sound":r.sound,"mentions":r.mentions,"flash":r.flash}); v["sound_file"]=serde_json::json!(r.sound_file); v}).collect())
}

pub struct Changed;
impl EventEmitter<Changed> for Editor {}
pub struct Editor {
    rules: Rules,
    pattern: Entity<InputState>,
    channel: Entity<InputState>,
    sample: Entity<InputState>,
    login: Entity<InputState>,
    /// "#RRGGBB" for a custom colour.
    hex: Entity<InputState>,
    draft: Rule,
    selected: Option<usize>,
    switch_pending: Option<Option<usize>>,
    delete_armed: bool,
    status: String,
    preview_channel: String,
}
impl Editor {
    pub fn new(rules: Rules, channel: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pattern = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("A phrase or Twitch login")
                .validate(|value, _| {
                    value.chars().count() <= 200 && !value.chars().any(char::is_control)
                })
        });
        let scope = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Every channel (optional #channel)")
                .validate(|value, _| value.len() <= 26 && !value.chars().any(char::is_control))
        });
        let sample = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Try a sample message locally")
                .validate(|value, _| {
                    value.chars().count() <= 500 && !value.chars().any(char::is_control)
                })
        });
        let login = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Sample sender login")
                .validate(|value, _| value.len() <= 25 && !value.chars().any(char::is_control))
        });
        let hex = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("#RRGGBB")
                .validate(|value, _| {
                    value.len() <= 7 && value.chars().all(|c| c == '#' || c.is_ascii_hexdigit())
                })
        });
        cx.subscribe_in(&hex, window, |this: &mut Self, input, event, _, cx| {
            if matches!(event, InputEvent::Change)
                && let Some(rgb) = parse_hex(&input.read(cx).value())
            {
                this.draft.color = CUSTOM_COLOR | rgb as usize;
                this.edited(cx);
            }
        })
        .detach();
        for input in [&pattern, &scope, &sample, &login] {
            cx.subscribe_in(input, window, |this: &mut Self, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.status.clear();
                    this.delete_armed = false;
                    this.switch_pending = None;
                    cx.notify();
                }
            })
            .detach();
        }
        Self {
            rules,
            pattern,
            channel: scope,
            sample,
            login,
            hex,
            draft: Rule::default(),
            selected: None,
            switch_pending: None,
            delete_armed: false,
            status: String::new(),
            preview_channel: channel,
        }
    }
    pub fn set_channel(&mut self, channel: String, cx: &mut Context<Self>) {
        self.preview_channel = channel;
        self.delete_armed = false;
        self.switch_pending = None;
        self.status.clear();
        cx.notify();
    }
    fn current(&self, cx: &App) -> Rule {
        let mut r = self.draft.clone();
        r.pattern = self.pattern.read(cx).value().to_string();
        r.channel = self.channel.read(cx).value().to_string();
        r
    }
    /// "Your file…": pick a .wav for this rule, check it plays, and use it.
    fn choose_sound(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use this sound".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update(cx, |this, cx| match crate::sound::check_custom(&path) {
                Ok(()) => {
                    crate::sound::play(&crate::sound::Tone::Custom(path.clone()), 0.6);
                    this.draft.sound_file = path.display().to_string();
                    this.draft.sound = CUSTOM_SOUND;
                    this.edited(cx);
                }
                Err(error) => {
                    this.status = error;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn select(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let original = self
            .selected
            .and_then(|i| self.rules.borrow().get(i).cloned())
            .unwrap_or_default();
        if self.current(cx) != original && self.switch_pending != Some(index) {
            self.switch_pending = Some(index);
            self.status =
                "Unsaved edits. Save first, or select that choice again to discard them.".into();
            cx.notify();
            return;
        }
        self.draft = index
            .and_then(|i| self.rules.borrow().get(i).cloned())
            .unwrap_or_default();
        self.pattern.update(cx, |p, cx| {
            p.set_value(self.draft.pattern.clone(), window, cx)
        });
        self.channel.update(cx, |p, cx| {
            p.set_value(self.draft.channel.clone(), window, cx)
        });
        let hex = if self.draft.color & CUSTOM_COLOR != 0 {
            format!("#{:06x}", self.draft.color & 0xFF_FFFF)
        } else {
            String::new()
        };
        self.hex.update(cx, |p, cx| p.set_value(hex, window, cx));
        self.selected = index;
        self.switch_pending = None;
        self.delete_armed = false;
        self.status.clear();
        cx.notify();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rule = match self.current(cx).validate() {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                cx.notify();
                return;
            }
        };
        let mut rules = self.rules.borrow_mut();
        if let Some(index) = self.selected {
            if let Some(target) = rules.get_mut(index) {
                *target = rule.clone();
            } else {
                self.status = "Rule no longer exists. Start a new rule.".into();
                cx.notify();
                return;
            }
        } else {
            if rules.len() >= 24 {
                self.status = "24 rules are already saved. Edit or remove one first.".into();
                cx.notify();
                return;
            }
            self.selected = Some(rules.len());
            rules.push(rule.clone());
        }
        drop(rules);
        self.draft = rule;
        self.pattern.update(cx, |p, cx| {
            p.set_value(self.draft.pattern.clone(), window, cx)
        });
        self.channel.update(cx, |p, cx| {
            p.set_value(self.draft.channel.clone(), window, cx)
        });
        self.status =
            "Applied across workspaces; local save queued. Retained messages are re-evaluated."
                .into();
        self.delete_armed = false;
        self.switch_pending = None;
        cx.emit(Changed);
        cx.notify();
    }
    fn moved(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(index) = self.selected else {
            return;
        };
        let len = self.rules.borrow().len();
        let next = if up {
            index.checked_sub(1)
        } else {
            index.checked_add(1).filter(|i| *i < len)
        };
        if let Some(next) = next {
            self.rules.borrow_mut().swap(index, next);
            self.selected = Some(next);
            self.delete_armed = false;
            self.switch_pending = None;
            self.status = "Priority updated. First matching rule chooses the color.".into();
            cx.emit(Changed);
            cx.notify();
        }
    }
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.delete_armed = false;
        self.switch_pending = None;
        self.status.clear();
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rules = self.rules.borrow().clone();
        let current = self.current(cx).validate();
        let preview = current
            .as_ref()
            .map(|r| {
                r.matches(
                    &self.sample.read(cx).value(),
                    Some(&self.login.read(cx).value()),
                    &self.preview_channel,
                )
            })
            .unwrap_or(false);
        let (accent, background) = colors(self.draft.color);
        let custom = self.draft.color & CUSTOM_COLOR != 0;
        let max_height = (f32::from(window.viewport_size().height) - 150.).clamp(100., 650.);
        div().id("highlight-rule-editor")
            .capture_key_down(cx.listener(|this,event:&KeyDownEvent,w,cx|{if event.keystroke.modifiers.control&&!event.keystroke.modifiers.alt&&event.keystroke.key=="s"{this.save(w,cx);w.prevent_default();cx.stop_propagation();}})).v_flex().gap_2().max_h(px(max_height)).overflow_y_scroll()
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Local highlights across all workspaces. First matching rule chooses the color. Mentions, replies and your simple highlight words stay enabled. Your own messages are excluded. Each rule can play its own sound."))
            .child(div().h_flex().gap_2().child(Button::new("new-highlight-rule").small().label("New rule").on_click(cx.listener(|this,_,w,cx|this.select(None,w,cx))))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("{}/24 rules",rules.len()))))
            .child(div().id("saved-highlight-rules").v_flex().gap_1().max_h(px(130.)).overflow_y_scroll().children(rules.iter().enumerate().map(|(index,r)|{
                let limit=if f32::from(window.viewport_size().width)<500.{18}else{36};
                let label=r.pattern.chars().take(limit).collect::<String>();let suffix=if r.pattern.chars().count()>limit{"…"}else{""};
                Button::new(("highlight-rule",index)).small().w_full().min_w_0().overflow_hidden().label(format!("{} · {}{}{}{}",index+1,if r.badge{"badge: "}else if r.author{"@"}else{""},label,suffix,if !r.enabled{" · off"}else if self.selected==Some(index){" · editing"}else{""}))
                    .on_click(cx.listener(move|this,_,w,cx|this.select(Some(index),w,cx)))
            })))
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("rule-text").small().label("Message text").selected(!self.draft.author&&!self.draft.badge).when(!self.draft.author&&!self.draft.badge,|b|b.primary()).on_click(cx.listener(|this,_,_,cx|{this.draft.author=false;this.draft.badge=false;this.edited(cx);})))
                .child(Button::new("rule-author").small().label("Sender login").selected(self.draft.author).when(self.draft.author,|b|b.primary()).on_click(cx.listener(|this,_,_,cx|{this.draft.author=true;this.draft.badge=false;this.edited(cx);})))
                .child(Button::new("rule-badge").small().label("Badge").tooltip(format!("Highlight everyone with a badge: {BADGE_EXAMPLES}")).selected(self.draft.badge).when(self.draft.badge,|b|b.primary()).on_click(cx.listener(|this,_,_,cx|{this.draft.badge=true;this.draft.author=false;this.draft.regex=false;this.edited(cx);})))
                .child(Button::new("rule-enabled").small().label(if self.draft.enabled{"Enabled"}else{"Disabled"}).on_click(cx.listener(|this,_,_,cx|{this.draft.enabled = !this.draft.enabled;this.edited(cx);})))
                .when(!self.draft.badge,|el|el.child(Button::new("rule-regex").small().label(if self.draft.regex{"Regex"}else{"Plain text"}).tooltip("Match a regular expression instead of plain text").on_click(cx.listener(|this,_,_,cx|{this.draft.regex = !this.draft.regex;this.edited(cx);})))))
            .when(self.draft.badge,|el|el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("Badge set, e.g. {BADGE_EXAMPLES}. In the preview, type badges in the sender box."))))
            .child(Input::new(&self.pattern).small()).child(Input::new(&self.channel).small())
            .when(!self.draft.author&&!self.draft.badge,|el|el.child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("rule-case").small().label(if self.draft.case_sensitive{"Match case"}else{"Ignore case"}).on_click(cx.listener(|this,_,_,cx|{this.draft.case_sensitive = !this.draft.case_sensitive;this.edited(cx);})))
                .when(!self.draft.regex,|el|el.child(Button::new("rule-word").small().label(if self.draft.whole_word{"Whole words"}else{"Anywhere in text"}).on_click(cx.listener(|this,_,_,cx|{this.draft.whole_word = !this.draft.whole_word;this.edited(cx);}))))))
            .child(div().h_flex().flex_wrap().gap_1().children(COLORS.iter().enumerate().map(|(index,(name,color,_))|
                Button::new(("rule-color",index)).small().label(*name).when(self.draft.color==index,|b|b.icon(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Check))).text_color(rgb(*color)).selected(self.draft.color==index).on_click(cx.listener(move|this,_,_,cx|{this.draft.color=index;this.edited(cx);}))))
                .child(div().h_flex().items_center().gap_1()
                    .child(div().size(px(14.)).rounded(px(3.)).border_1().border_color(rgb(theme::BORDER)).when(custom,|el|el.bg(rgb(accent))))
                    .child(div().w(px(84.)).child(Input::new(&self.hex).small()))
                    .when(custom,|el|el.child(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Check).small().text_color(rgb(accent))))))
            .child(div().h_flex().flex_wrap().items_center().gap_1()
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Sound"))
                .children(SOUNDS.iter().enumerate().map(|(index,label)|{let index=index as u8;
                    let label=if index==CUSTOM_SOUND&&!self.draft.sound_file.is_empty(){std::path::Path::new(&self.draft.sound_file).file_name().map(|n|n.to_string_lossy().into_owned()).unwrap_or_else(||(*label).into())}else{(*label).into()};
                    Button::new(("rule-sound",index as usize)).small().label(label).selected(self.draft.sound==index).when(self.draft.sound==index,|b|b.primary()).on_click(cx.listener(move|this,_,w,cx|{
                        if index==CUSTOM_SOUND{this.choose_sound(w,cx);return;}
                        this.draft.sound=index;if index>=2{crate::sound::play(&if index==2{crate::sound::Tone::Chime}else{crate::sound::Tone::Soft},0.6);}this.edited(cx);}))})))
            .child(div().h_flex().flex_wrap().items_center().gap_3()
                .child(gpui_kit::component::checkbox::Checkbox::new("rule-mentions").label("Show in Mentions").checked(self.draft.mentions).on_click(cx.listener(|this,on:&bool,_,cx|{this.draft.mentions=*on;this.edited(cx);})))
                .child(gpui_kit::component::checkbox::Checkbox::new("rule-flash").label("Flash taskbar").checked(self.draft.flash).on_click(cx.listener(|this,on:&bool,_,cx|{this.draft.flash=*on;this.edited(cx);}))))
            .child(div().border_l_2().border_color(rgb(accent)).bg(rgb(background)).rounded(px(4.)).p_2().v_flex().gap_1()
                .child(div().text_size(px(11.)).child(format!("Preview in #{} · {}",self.preview_channel,if preview{"matches"}else{"does not match"})))
                .child(Input::new(&self.sample).small()).child(Input::new(&self.login).small())
                .when_some(current.err(),|el,error|el.child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(error))))
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("save-highlight-rule").small().label("Save rule · Ctrl+S").on_click(cx.listener(|this,_,w,cx|this.save(w,cx))))
                .when(self.selected.is_some(),|el|el
                    .child(Button::new("rule-up").small().label("Move up").disabled(self.selected==Some(0)).on_click(cx.listener(|this,_,_,cx|this.moved(true,cx))))
                    .child(Button::new("rule-down").small().label("Move down").disabled(self.selected==rules.len().checked_sub(1)).on_click(cx.listener(|this,_,_,cx|this.moved(false,cx))))
                    .child(Button::new("delete-highlight-rule").small().label(if self.delete_armed{"Confirm delete"}else{"Delete…"}).on_click(cx.listener(|this,_,w,cx|{
                        if !this.delete_armed{this.delete_armed=true;this.status=format!("Confirm deletion of {}.",this.draft.pattern);cx.notify();return;}
                        if let Some(index)=this.selected.take()&& index<this.rules.borrow().len(){this.rules.borrow_mut().remove(index);cx.emit(Changed);}
                        this.switch_pending=Some(None);this.select(None,w,cx);this.status="Rule removed.".into();
                    })))))
            .when(!self.status.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(0xE9C99C)).child(self.status.clone())))
    }
}

#[cfg(test)]
mod custom_tests {
    use super::{COLORS, CUSTOM_COLOR, CUSTOM_SOUND, Rule, Rules, colors, json, load, parse_hex};
    use std::{cell::RefCell, rc::Rc};
    #[test]
    fn custom_colours_and_sounds_survive_save_and_load() {
        assert_eq!(parse_hex("#ff8800"), Some(0xFF8800));
        assert_eq!(parse_hex("f80"), Some(0xFF8800));
        assert_eq!(parse_hex("#ff88"), None);
        let rule = Rule {
            pattern: "pizza".into(),
            color: CUSTOM_COLOR | 0xFF8800,
            sound: CUSTOM_SOUND,
            sound_file: "C:/sounds/ding.wav".into(),
            ..Default::default()
        }
        .validate()
        .unwrap();
        let rules: Rules = Rc::new(RefCell::new(vec![rule]));
        let loaded = load(&json(&rules));
        let loaded = loaded.borrow();
        assert_eq!(colors(loaded[0].color).0, 0xFF8800);
        assert_eq!(loaded[0].sound, CUSTOM_SOUND);
        assert_eq!(loaded[0].sound_file, "C:/sounds/ding.wav");
        // Palette colours still read from the palette.
        assert_eq!(colors(1), (COLORS[1].1, COLORS[1].2));
        // "Your file" with no file falls back to the default sound.
        let empty = Rule {
            pattern: "x".into(),
            sound: CUSTOM_SOUND,
            ..Default::default()
        }
        .validate()
        .unwrap();
        assert_eq!(empty.sound, 0);
    }
}

#[cfg(test)]
mod badge_tests {
    use super::{Rule, first_match};
    #[test]
    fn badge_rules_match_badge_sets() {
        let rule = Rule {
            pattern: "VIP".into(),
            badge: true,
            sound: 2,
            ..Default::default()
        }
        .validate()
        .unwrap();
        assert_eq!(rule.pattern, "vip");
        let mut message = chat_core::fixture("c", 0);
        message.badges = vec![chat_core::Badge {
            set_id: "vip".into(),
            id: "1".into(),
        }];
        let rules = [rule];
        assert_eq!(first_match(&rules, &message, "c").map(|r| r.sound), Some(2));
        message.badges.clear();
        assert!(first_match(&rules, &message, "c").is_none());
    }
}
