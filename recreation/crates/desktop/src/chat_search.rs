//! Bounded, local search of retained chat. No provider requests or draft mutations.
use crate::{ChannelPane, theme};
use chat_core::Timeline;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use gpui_kit::*;
use std::{collections::BTreeMap, ops::Range, rc::Rc};

/// Named view filters (Chatterino's saved filters): (name, query), applied from
/// any tab's Filter view dialog. Saved with the workspace.
pub static SAVED: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());
pub fn saved_filters() -> Vec<(String, String)> {
    SAVED.lock().map(|s| s.clone()).unwrap_or_default()
}
/// Save (or replace) a named filter; the query must parse.
pub fn save_filter(name: &str, query: &str) -> Result<(), String> {
    let (name, query) = (name.trim(), query.trim());
    if name.is_empty() || name.chars().count() > 32 || name.chars().any(char::is_control) {
        return Err("Give the filter a name of 1–32 characters.".into());
    }
    if query.is_empty() {
        return Err("Type a filter first.".into());
    }
    if let Some(problem) = Query::parse(query).error() {
        return Err(problem.to_owned());
    }
    let mut saved = SAVED.lock().map_err(|_| "Saved filters are unavailable.")?;
    if let Some(slot) = saved.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        slot.1 = query.to_owned();
    } else if saved.len() >= 30 {
        return Err("30 filters are already saved. Remove one first.".into());
    } else {
        saved.push((name.to_owned(), query.to_owned()));
    }
    Ok(())
}
pub fn remove_filter(name: &str) {
    if let Ok(mut saved) = SAVED.lock() {
        saved.retain(|(n, _)| n != name);
    }
}
pub fn load_saved(value: &serde_json::Value) {
    let items = value
        .as_array()
        .into_iter()
        .flatten()
        .take(30)
        .filter_map(|v| {
            Some((
                v["name"].as_str()?.to_owned(),
                v["query"].as_str()?.to_owned(),
            ))
        })
        .collect();
    if let Ok(mut saved) = SAVED.lock() {
        *saved = items;
    }
}
pub fn saved_json() -> serde_json::Value {
    serde_json::Value::Array(
        saved_filters()
            .into_iter()
            .map(|(name, query)| serde_json::json!({"name": name, "query": query}))
            .collect(),
    )
}
pub struct Search {
    pub open: bool,
    pub input: Entity<InputState>,
    pub query: String,
    pub dirty: bool,
    pub hits: Rc<BTreeMap<u64, Vec<Range<usize>>>>,
    pub current: Option<u64>,
}
impl Search {
    pub fn appended(&mut self, message: &chat_core::Message, row: u64, first: u64) {
        if !self.open {
            self.dirty = true;
            return;
        }
        if self.dirty || self.query.trim().is_empty() {
            return;
        }
        let ranges = Query::parse(&self.query).matches(message);
        let hits = Rc::make_mut(&mut self.hits);
        while hits.first_key_value().is_some_and(|(key, _)| *key < first) {
            hits.pop_first();
        }
        if let Some(ranges) = ranges {
            hits.insert(row, ranges);
        }
        if !self.current.is_some_and(|r| hits.contains_key(&r)) {
            self.current = hits.keys().next().copied();
        }
    }

    pub fn refresh(&mut self, timeline: &Timeline, first: u64) {
        if !self.open || !self.dirty {
            return;
        }
        let query = Query::parse(&self.query);
        let mut hits = BTreeMap::new();
        if !self.query.trim().is_empty() {
            for (index, message) in timeline.messages().iter().enumerate() {
                if let Some(ranges) = query.matches(message) {
                    hits.insert(first + index as u64, ranges);
                }
            }
        }
        if !self.current.is_some_and(|row| hits.contains_key(&row)) {
            self.current = hits.keys().next().copied();
        }
        self.hits = Rc::new(hits);
        self.dirty = false;
    }
}
fn token_spans(value: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = None;
    let mut quoted = false;
    for (at, ch) in value.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        }
        if ch.is_whitespace() && !quoted {
            if let Some(start) = start.take() {
                result.push(start..at);
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(start) = start {
        result.push(start..value.len());
    }
    result
}
/// A bounded AND query. Plain unstructured text retains the original phrase behavior.
pub(crate) struct Query {
    terms: Vec<(bool, String)>,
    /// `regex:` terms (case-insensitive, size-bounded).
    patterns: Vec<(bool, regex::Regex)>,
    filters: Vec<(bool, String, String)>,
    error: Option<String>,
}
impl Query {
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub(crate) fn parse(value: &str) -> Self {
        let value = value.chars().take(256).collect::<String>();
        let tokens = token_spans(&value)
            .into_iter()
            .map(|r| value[r].to_owned())
            .collect::<Vec<_>>();
        let structured = value.contains('"')
            || tokens.iter().any(|t| {
                t.starts_with('-')
                    || t.split_once(':').is_some_and(|(k, _)| {
                        matches!(
                            k,
                            "from" | "from-id" | "badge" | "has" | "is" | "subtier" | "regex"
                        )
                    })
            });
        if !structured {
            return Self {
                terms: vec![(false, value.trim().to_lowercase())],
                patterns: vec![],
                filters: vec![],
                error: None,
            };
        }
        let mut result = Self {
            terms: vec![],
            patterns: vec![],
            filters: vec![],
            error: None,
        };
        if tokens.len() > 32 {
            result.error = Some("Use at most 32 search terms or filters".into());
            return result;
        }
        for token in tokens.into_iter().take(32) {
            let (negative, token) = token
                .strip_prefix('-')
                .map_or((false, token.as_str()), |t| (true, t));
            if token.is_empty() {
                continue;
            }
            let literal = token.starts_with('"');
            let cleaned = token.replace('"', "");
            let token = cleaned.as_str();
            if let Some(pattern) = token.strip_prefix("regex:").filter(|_| !literal) {
                match regex::RegexBuilder::new(pattern)
                    .case_insensitive(true)
                    .size_limit(1 << 20)
                    .build()
                {
                    Ok(regex) if !pattern.is_empty() => result.patterns.push((negative, regex)),
                    _ => result.error = Some("That regex: pattern isn't valid".into()),
                }
                continue;
            }
            if let Some((key, value)) = token.split_once(':').filter(|_| !literal)
                && matches!(key, "from" | "from-id" | "badge" | "has" | "is" | "subtier")
            {
                let value = value.to_lowercase();
                if value.is_empty()
                    || (key == "has"
                        && !matches!(value.as_str(), "link" | "emote" | "gif" | "bits" | "reply"))
                    || (key == "is"
                        && !matches!(
                            value.as_str(),
                            "deleted" | "notice" | "mod" | "vip" | "sub" | "broadcaster" | "first"
                        ))
                    || (key == "subtier" && !matches!(value.as_str(), "1" | "2" | "3"))
                {
                    result.error = Some(format!("Unknown or empty {key}: filter"));
                }
                result.filters.push((negative, key.into(), value));
                continue;
            }
            result.terms.push((negative, token.to_lowercase()));
        }
        result
    }
    pub(crate) fn matches(&self, message: &chat_core::Message) -> Option<Vec<Range<usize>>> {
        use chat_core::Fragment;
        if self.error.is_some()
            || (self.terms.iter().all(|(_, t)| t.is_empty())
                && self.filters.is_empty()
                && self.patterns.is_empty())
        {
            return None;
        }
        for (negative, key, value) in &self.filters {
            let matched = match key.as_str() {
                "from" => {
                    message
                        .login
                        .as_ref()
                        .is_some_and(|s| s.eq_ignore_ascii_case(value.trim_start_matches('@')))
                        || message.display_name.to_lowercase() == value.trim_start_matches('@')
                }
                "from-id" => !value.is_empty() && message.user_id == *value,
                "badge" => message
                    .badges
                    .iter()
                    .any(|badge| badge.set_id.eq_ignore_ascii_case(value)),
                "has" => match value.as_str() {
                    "link" => !crate::message_actions::message_links(message).is_empty(),
                    "emote" => {
                        !message.deleted
                            && message
                                .fragments
                                .iter()
                                .any(|f| matches!(f, Fragment::Emote { .. }))
                    }
                    "gif" => {
                        !message.deleted
                            && message
                                .fragments
                                .iter()
                                .any(|f| matches!(f, Fragment::Gif { .. }))
                    }
                    "bits" => {
                        !message.deleted
                            && (message.presentation.bits > 0
                                || message
                                    .fragments
                                    .iter()
                                    .any(|f| matches!(f, Fragment::Cheer { .. })))
                    }
                    "reply" => !message.deleted && message.reply.is_some(),
                    _ => false,
                },
                "is" => {
                    let badge = |sets: &[&str]| {
                        message
                            .badges
                            .iter()
                            .any(|b| sets.contains(&b.set_id.as_str()))
                    };
                    match value.as_str() {
                        "deleted" => message.deleted,
                        "notice" => !message.deleted && message.presentation.notice_type.is_some(),
                        "mod" => badge(&["moderator", "lead_moderator"]),
                        "vip" => badge(&["vip"]),
                        "sub" => badge(&["subscriber", "founder"]),
                        "broadcaster" => badge(&["broadcaster"]),
                        "first" => message.presentation.kind == "user_intro",
                        _ => false,
                    }
                }
                // Subscriber badge IDs carry the tier: 2xxx is tier 2, 3xxx tier 3.
                "subtier" => message.badges.iter().any(|b| {
                    b.set_id == "subscriber"
                        && match b.id.parse::<u32>().map(|n| n / 1000) {
                            Ok(2) => value == "2",
                            Ok(3) => value == "3",
                            _ => value == "1",
                        }
                }),
                _ => false,
            };
            if matched == *negative {
                return None;
            }
        }
        let text = message.copy_line();
        let mut ranges = Vec::new();
        for (negative, term) in &self.terms {
            if term.is_empty() {
                continue;
            }
            let matches = match_ranges(&text, term);
            if matches.is_empty() != *negative {
                return None;
            }
            if !negative {
                ranges.extend(matches);
            }
        }
        for (negative, regex) in &self.patterns {
            let found: Vec<Range<usize>> = regex
                .find_iter(&text)
                .take(64)
                .map(|m| m.range())
                .filter(|r| !r.is_empty())
                .collect();
            if found.is_empty() != *negative {
                return None;
            }
            if !negative {
                ranges.extend(found);
            }
        }
        ranges.sort_by_key(|r| (r.start, r.end));
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut().filter(|last| range.start <= last.end) {
                last.end = last.end.max(range.end);
            } else {
                merged.push(range);
            }
        }
        let mut ranges = merged;
        ranges.truncate(128);
        // Filter-only results still receive a visible author marker.
        if ranges.is_empty() && !message.author_label().is_empty() {
            ranges.push(0..message.author_label().len());
        }
        Some(ranges)
    }
}
fn match_ranges(text: &str, needle: &str) -> Vec<Range<usize>> {
    // Lowercasing can expand one Unicode scalar. Map folded byte offsets back
    // to the original UTF-8 bytes used by both text and inline-emote rendering.
    let mut folded = String::new();
    let mut original = Vec::new();
    for (start, ch) in text.char_indices() {
        for lower in ch.to_lowercase() {
            folded.push(lower);
            original.extend(std::iter::repeat_n(
                start..start + ch.len_utf8(),
                lower.len_utf8(),
            ));
        }
    }
    let mut ranges = Vec::new();
    for (start, _) in folded.match_indices(needle).take(128) {
        let range = original[start].start..original[start + needle.len() - 1].end;
        if ranges.last() != Some(&range) {
            ranges.push(range);
        }
    }
    ranges
}
impl ChannelPane {
    pub fn search_for_author(
        &mut self,
        user_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_search_query(format!("from-id:{user_id}"), window, cx);
        self.open_search(window, cx);
    }
    pub(crate) fn set_search_query(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.query = value.clone();
        self.search.current = None;
        self.search.dirty = true;
        self.search
            .input
            .update(cx, |input, cx| input.set_value(value, window, cx));
        self.refresh_search(cx);
        cx.notify();
    }
    fn toggle_search_filter(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut value = self.search.query.clone();
        let matches = token_spans(&value)
            .into_iter()
            .filter(|r| &value[r.clone()] == token)
            .collect::<Vec<_>>();
        if matches.is_empty() {
            if !value.trim().is_empty() {
                value.push(' ');
            }
            value.push_str(token);
        } else {
            for range in matches.into_iter().rev() {
                value.replace_range(range, "");
            }
            value = value.trim().to_owned();
        }
        if value.chars().count() <= 256 {
            self.set_search_query(value, window, cx);
        }
        self.search
            .input
            .update(cx, |input, cx| input.focus(window, cx));
    }
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = true;
        self.search.dirty = true;
        self.search.input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.refresh_search(cx);
        cx.notify();
    }
    fn refresh_search(&mut self, cx: &mut Context<Self>) {
        let timeline = self.timeline.borrow();
        self.search
            .refresh(&timeline, (self.next_id - timeline.messages().len()) as u64);
        drop(timeline);
        if let Some(row) = self.search.current {
            let first = self.next_id - self.timeline.borrow().messages().len();
            self.scroller.update(cx, |scroller, cx| {
                scroller.scroll_to_item(row as usize - first, cx);
            });
        }
    }
    pub fn next_search(&mut self, forward: bool, cx: &mut Context<Self>) {
        if !self.search.open {
            return;
        }
        let rows = self.search.hits.keys().copied().collect::<Vec<_>>();
        if rows.is_empty() {
            return;
        }
        let at = self
            .search
            .current
            .and_then(|row| rows.iter().position(|r| *r == row))
            .unwrap_or(0);
        let at = if forward {
            (at + 1) % rows.len()
        } else {
            (at + rows.len() - 1) % rows.len()
        };
        self.search.current = Some(rows[at]);
        self.refresh_search(cx);
        cx.notify();
    }
    pub fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = false;
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub fn render_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let error = Query::parse(&self.search.query).error;
        let count = self.search.hits.len();
        let ordinal = self
            .search
            .current
            .and_then(|row| self.search.hits.keys().position(|r| *r == row))
            .map(|i| i + 1)
            .unwrap_or(0);
        let filters = [
            ("Links", "has:link"),
            ("Emotes", "has:emote"),
            ("Replies", "has:reply"),
            ("Bits", "has:bits"),
            ("Mods", "is:mod"),
            ("Subs", "is:sub"),
            ("First", "is:first"),
        ]
        .into_iter()
        .map(|(label, token)| {
            let active = token_spans(&self.search.query)
                .into_iter()
                .any(|r| &self.search.query[r] == token);
            Button::new(SharedString::from(format!("search-filter-{token}")))
                .xsmall()
                .label(if active {
                    format!("✓ {label}")
                } else {
                    label.into()
                })
                .tooltip(format!("Toggle {token} filter"))
                .on_click(
                    cx.listener(move |this, _, w, cx| this.toggle_search_filter(token, w, cx)),
                )
        })
        .collect::<Vec<_>>();
        div().v_flex().p_1().gap_1().bg(rgb(theme::PANEL)).border_b_1().border_color(rgb(theme::BORDER))
            .key_context("JawjackSearch")
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Enter,_,cx|{this.next_search(true,cx);cx.stop_propagation();}))
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Escape,w,cx|{this.close_search(w,cx);cx.stop_propagation();}))
            .child(div().h_flex().gap_1().min_w_0()
                .child(div().flex_1().min_w_0().child(Input::new(&self.search.input).small()))
                .child(Button::new("search-close").ghost().xsmall().icon(IconName::Close).tooltip("Close search · Escape").on_click(cx.listener(|this,_,w,cx|this.close_search(w,cx)))))
            .child(div().h_flex().flex_wrap().gap_1().children(filters)
                .child(Button::new("search-all-channels").xsmall().label("All chats").tooltip("Search all open channels · Ctrl+Shift+F").on_click(cx.listener(|this,_,_,cx|cx.emit(crate::PaneEvent::SearchAll(this.search.query.clone())))))
                .child(Button::new("search-help").xsmall().label("?").tooltip("Combine from:username, badge:moderator (Twitch), has:link/emote/reply/gif/bits, is:notice/deleted, quoted phrases and -excluded words. Search is local to retained chat.")))
            .child(div().h_flex().gap_1().justify_between().text_size(px(11.)).text_color(rgb(theme::MUTED))
                .child(if let Some(error)=error {error}else if self.search.query.trim().is_empty(){"Search retained messages".to_string()}else{format!("{ordinal} / {count} messages")})
                .child(div().h_flex().gap_1()
                    .child(Button::new("search-previous").xsmall().disabled(count==0).label("↑").tooltip("Previous match · Shift+F3").on_click(cx.listener(|this,_,_,cx|this.next_search(false,cx))))
                    .child(Button::new("search-next").xsmall().disabled(count==0).label("↓").tooltip("Next match · Enter or F3").on_click(cx.listener(|this,_,_,cx|this.next_search(true,cx))))
                    .child(Button::new("search-latest").xsmall().label("Latest").tooltip("Return to live chat").on_click(cx.listener(|this,_,w,cx|{this.close_search(w,cx);this.scroller.update(cx,|s,cx|s.scroll_to_end(cx));})))))
            .into_any_element()
    }
}
pub fn create(window: &mut Window, cx: &mut Context<ChannelPane>) -> Search {
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("Find text · from:user · has:link…")
            .validate(|text, _| text.chars().count() <= 256)
    });
    cx.subscribe_in(&input, window, |pane, _, event, _, cx| {
        if matches!(event, InputEvent::Change) {
            pane.search.query = pane.search.input.read(cx).value().to_string();
            pane.search.current = None;
            pane.search.dirty = true;
            pane.refresh_search(cx);
            cx.notify();
        }
    })
    .detach();
    Search {
        open: false,
        input,
        query: String::new(),
        dirty: true,
        hits: Rc::default(),
        current: None,
    }
}

#[cfg(test)]
mod tests {
    use super::Query;
    fn message(text: &str, badges: &[(&str, &str)]) -> chat_core::Message {
        chat_core::Message {
            sent_at: Some(0),
            id: "m".into(),
            channel_id: "c".into(),
            user_id: "u".into(),
            display_name: "Bob".into(),
            login: Some("bob".into()),
            reply: None,
            mentions: vec![],
            replyable: true,
            name_color: None,
            badges: badges
                .iter()
                .map(|(set, id)| chat_core::Badge {
                    set_id: (*set).into(),
                    id: (*id).into(),
                })
                .collect(),
            fragments: vec![chat_core::Fragment::Text(text.into())],
            presentation: Default::default(),
            deleted: false,
        }
    }
    #[test]
    fn roles_tiers_and_regex() {
        let sub3 = message("hello 123", &[("subscriber", "3012"), ("moderator", "1")]);
        assert!(Query::parse("is:mod").matches(&sub3).is_some());
        assert!(Query::parse("is:vip").matches(&sub3).is_none());
        assert!(Query::parse("subtier:3").matches(&sub3).is_some());
        assert!(Query::parse("subtier:1").matches(&sub3).is_none());
        assert!(
            Query::parse("subtier:1")
                .matches(&message("x", &[("subscriber", "6")]))
                .is_some()
        );
        let ranges = Query::parse(r"regex:\d+").matches(&sub3).unwrap();
        assert_eq!(&sub3.copy_line()[ranges[0].clone()], "123");
        assert!(Query::parse("-regex:hello").matches(&sub3).is_none());
        assert!(Query::parse("regex:(").error().is_some());
        assert!(Query::parse("is:martian").error().is_some());
    }
}
