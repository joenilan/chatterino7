//! Bounded, local search of retained chat. No provider requests or draft mutations.
use std::{collections::BTreeMap, ops::Range, rc::Rc};
use chat_core::Timeline;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{button::Button, Disableable, Sizable, StyledExt};
use gpui_kit::*;
use crate::{ChannelPane, theme};

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
        if !self.open {self.dirty=true;return;}
        if self.dirty || self.query.is_empty() {return;}
        let ranges=match_ranges(&message.copy_line(),&self.query.to_lowercase());
        let hits=Rc::make_mut(&mut self.hits);
        while hits.first_key_value().is_some_and(|(key,_)|*key<first){hits.pop_first();}
        if !ranges.is_empty(){hits.insert(row,ranges);}
        if !self.current.is_some_and(|r|hits.contains_key(&r)){self.current=hits.keys().next().copied();}
    }

    pub fn refresh(&mut self, timeline: &Timeline, first: u64) {
        if !self.open || !self.dirty { return; }
        let needle: String = self.query.chars().take(256).collect::<String>().to_lowercase();
        let mut hits = BTreeMap::new();
        if !needle.is_empty() {
            for (index, message) in timeline.messages().iter().enumerate() {
                let ranges = match_ranges(&message.copy_line(), &needle);
                if !ranges.is_empty() { hits.insert(first + index as u64, ranges); }
            }
        }
        if !self.current.is_some_and(|row| hits.contains_key(&row)) { self.current = hits.keys().next().copied(); }
        self.hits = Rc::new(hits); self.dirty = false;
    }
}
fn match_ranges(text: &str, needle: &str) -> Vec<Range<usize>> {
    // Lowercasing can expand one Unicode scalar. Map folded byte offsets back
    // to the original UTF-8 bytes used by both text and inline-emote rendering.
    let mut folded = String::new(); let mut original = Vec::new();
    for (start, ch) in text.char_indices() {
        for lower in ch.to_lowercase() {
            folded.push(lower);
            original.extend(std::iter::repeat_n(start..start + ch.len_utf8(), lower.len_utf8()));
        }
    }
    let mut ranges = Vec::new();
    for (start, _) in folded.match_indices(needle).take(128) {
        let range = original[start].start..original[start + needle.len() - 1].end;
        if ranges.last() != Some(&range) { ranges.push(range); }
    }
    ranges
}
impl ChannelPane {
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = true; self.search.dirty = true;
        self.search.input.update(cx, |input,cx| {input.focus(window,cx);input.select_all(window,cx);} );
        self.refresh_search(cx); cx.notify();
    }
    fn refresh_search(&mut self, cx: &mut Context<Self>) {
        let timeline = self.timeline.borrow();
        self.search.refresh(&timeline, (self.next_id - timeline.messages().len()) as u64);
        drop(timeline);
        if let Some(row) = self.search.current {
            let first = self.next_id - self.timeline.borrow().messages().len();
            self.scroller.update(cx, |scroller,cx| { scroller.scroll_to_item(row as usize - first,cx); });
        }
    }
    pub fn next_search(&mut self, forward: bool, cx: &mut Context<Self>) {
        if !self.search.open { return; }
        let rows = self.search.hits.keys().copied().collect::<Vec<_>>();
        if rows.is_empty() { return; }
        let at = self.search.current.and_then(|row|rows.iter().position(|r|*r==row)).unwrap_or(0);
        let at = if forward {(at+1)%rows.len()}else{(at+rows.len()-1)%rows.len()};
        self.search.current=Some(rows[at]); self.refresh_search(cx); cx.notify();
    }
    pub fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open=false; self.focus.focus(window,cx); cx.notify();
    }
    pub fn render_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let count=self.search.hits.len();
        let ordinal=self.search.current.and_then(|row|self.search.hits.keys().position(|r|*r==row)).map(|i|i+1).unwrap_or(0);
        div().v_flex().p_1().gap_1().bg(rgb(theme::PANEL)).border_b_1().border_color(rgb(theme::BORDER))
            .key_context("JawjackSearch")
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Enter,_,cx|{this.next_search(true,cx);cx.stop_propagation();}))
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Escape,w,cx|{this.close_search(w,cx);cx.stop_propagation();}))
            .child(div().h_flex().gap_1().min_w_0()
                .child(div().flex_1().min_w_0().child(Input::new(&self.search.input).small()))
                .child(Button::new("search-close").xsmall().label("×").tooltip("Close search · Escape").on_click(cx.listener(|this,_,w,cx|this.close_search(w,cx)))))
            .child(div().h_flex().gap_1().justify_between().text_size(px(11.)).text_color(rgb(theme::MUTED))
                .child(if self.search.query.is_empty(){"Search retained messages".to_string()}else{format!("{ordinal} / {count} messages")})
                .child(div().h_flex().gap_1()
                    .child(Button::new("search-previous").xsmall().disabled(count==0).label("↑").tooltip("Previous match · Shift+F3").on_click(cx.listener(|this,_,_,cx|this.next_search(false,cx))))
                    .child(Button::new("search-next").xsmall().disabled(count==0).label("↓").tooltip("Next match · Enter or F3").on_click(cx.listener(|this,_,_,cx|this.next_search(true,cx))))
                    .child(Button::new("search-latest").xsmall().label("Latest").tooltip("Return to live chat").on_click(cx.listener(|this,_,w,cx|{this.close_search(w,cx);this.scroller.update(cx,|s,cx|s.scroll_to_end(cx));})))))
            .into_any_element()
    }
}
pub fn create(window:&mut Window,cx:&mut Context<ChannelPane>)->Search {
    let input=cx.new(|cx|InputState::new(window,cx).placeholder("Find text, user, or emote…").validate(|text,_|text.chars().count()<=256));
    cx.subscribe_in(&input,window,|pane,_,event,_,cx|{
        if matches!(event,InputEvent::Change){
            pane.search.query=pane.search.input.read(cx).value().to_string();
            pane.search.current=None;pane.search.dirty=true;pane.refresh_search(cx);cx.notify();
        }
    }).detach();
    Search{open:false,input,query:String::new(),dirty:true,hits:Rc::default(),current:None}
}
