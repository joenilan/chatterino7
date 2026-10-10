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
        if self.dirty || self.query.trim().is_empty() {return;}
        let ranges=Query::parse(&self.query).matches(message);
        let hits=Rc::make_mut(&mut self.hits);
        while hits.first_key_value().is_some_and(|(key,_)|*key<first){hits.pop_first();}
        if let Some(ranges)=ranges{hits.insert(row,ranges);}
        if !self.current.is_some_and(|r|hits.contains_key(&r)){self.current=hits.keys().next().copied();}
    }

    pub fn refresh(&mut self, timeline: &Timeline, first: u64) {
        if !self.open || !self.dirty { return; }
        let query = Query::parse(&self.query);
        let mut hits = BTreeMap::new();
        if !self.query.trim().is_empty() {
            for (index, message) in timeline.messages().iter().enumerate() {
                if let Some(ranges) = query.matches(message) { hits.insert(first + index as u64, ranges); }
            }
        }
        if !self.current.is_some_and(|row| hits.contains_key(&row)) { self.current = hits.keys().next().copied(); }
        self.hits = Rc::new(hits); self.dirty = false;
    }
}
fn token_spans(value:&str)->Vec<Range<usize>> {
    let mut result=Vec::new();let mut start=None;let mut quoted=false;
    for (at,ch) in value.char_indices(){
        if ch=='"'{quoted=!quoted;}
        if ch.is_whitespace()&&!quoted {if let Some(start)=start.take(){result.push(start..at);}}
        else if start.is_none(){start=Some(at);}
    }
    if let Some(start)=start{result.push(start..value.len());}result
}
/// A bounded AND query. Plain unstructured text retains the original phrase behavior.
struct Query { terms: Vec<(bool,String)>, filters: Vec<(bool,String,String)>, error: Option<String> }
impl Query {
    fn parse(value:&str)->Self {
        let value=value.chars().take(256).collect::<String>();
        let tokens=token_spans(&value).into_iter().map(|r|value[r].to_owned()).collect::<Vec<_>>();
        let structured=value.contains('"')||tokens.iter().any(|t|t.starts_with('-')||t.split_once(':').is_some_and(|(k,_)|matches!(k,"from"|"from-id"|"badge"|"has"|"is")));
        if !structured{return Self{terms:vec![(false,value.trim().to_lowercase())],filters:vec![],error:None};}
        let mut result=Self{terms:vec![],filters:vec![],error:None};
        if tokens.len()>32{result.error=Some("Use at most 32 search terms or filters".into());return result;}
        for token in tokens.into_iter().take(32){
            let (negative,token)=token.strip_prefix('-').map_or((false,token.as_str()),|t|(true,t));
            if token.is_empty(){continue;}
            let literal=token.starts_with('"');
            let cleaned=token.replace('"', "");let token=cleaned.as_str();
            if let Some((key,value))=token.split_once(':').filter(|_|!literal) {
                if matches!(key,"from"|"from-id"|"badge"|"has"|"is") {
                    let value=value.to_lowercase();
                    if value.is_empty() || (key=="has"&&!matches!(value.as_str(),"link"|"emote"|"gif"|"bits"|"reply")) || (key=="is"&&!matches!(value.as_str(),"deleted"|"notice")) {
                        result.error=Some(format!("Unknown or empty {key}: filter"));
                    }
                    result.filters.push((negative,key.into(),value));continue;
                }
            }
            result.terms.push((negative,token.to_lowercase()));
        }
        result
    }
    fn matches(&self,message:&chat_core::Message)->Option<Vec<Range<usize>>>{
        use chat_core::Fragment;
        if self.error.is_some()||(self.terms.iter().all(|(_,t)|t.is_empty())&&self.filters.is_empty()){return None;}
        for (negative,key,value) in &self.filters {
            let matched=match key.as_str(){
                "from"=>message.login.as_ref().is_some_and(|s|s.eq_ignore_ascii_case(value.trim_start_matches('@')))||message.display_name.to_lowercase()==value.trim_start_matches('@'),
                "from-id"=>!value.is_empty()&&message.user_id==*value,
                "badge"=>message.badges.iter().any(|badge|badge.set_id.eq_ignore_ascii_case(value)),
                "has"=>match value.as_str(){
                    "link"=>!crate::message_actions::message_links(message).is_empty(),
                    "emote"=>!message.deleted&&message.fragments.iter().any(|f|matches!(f,Fragment::Emote{..})),
                    "gif"=>!message.deleted&&message.fragments.iter().any(|f|matches!(f,Fragment::Gif{..})),
                    "bits"=>!message.deleted&&(message.presentation.bits>0||message.fragments.iter().any(|f|matches!(f,Fragment::Cheer{..}))),
                    "reply"=>!message.deleted&&message.reply.is_some(), _=>false,
                },
                "is"=>match value.as_str(){"deleted"=>message.deleted,"notice"=>!message.deleted&&message.presentation.notice_type.is_some(),_=>false},
                _=>false,
            };
            if matched==*negative{return None;}
        }
        let text=message.copy_line();let mut ranges=Vec::new();
        for (negative,term) in &self.terms {
            if term.is_empty(){continue;}
            let matches=match_ranges(&text,term);
            if matches.is_empty()!=*negative{return None;}
            if !negative{ranges.extend(matches);}
        }
        ranges.sort_by_key(|r|(r.start,r.end));
        let mut merged:Vec<Range<usize>>=Vec::new();
        for range in ranges {if let Some(last)=merged.last_mut().filter(|last|range.start<=last.end){last.end=last.end.max(range.end);}else{merged.push(range);}}
        let mut ranges=merged;ranges.truncate(128);
        // Filter-only results still receive a visible author marker.
        if ranges.is_empty()&&!message.display_name.is_empty(){ranges.push(0..message.display_name.len());}
        Some(ranges)
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
    pub fn search_for_author(&mut self,user_id:&str,window:&mut Window,cx:&mut Context<Self>){
        self.set_search_query(format!("from-id:{user_id}"),window,cx);
        self.open_search(window,cx);
    }
    fn set_search_query(&mut self,value:String,window:&mut Window,cx:&mut Context<Self>){
        self.search.query=value.clone();self.search.current=None;self.search.dirty=true;
        self.search.input.update(cx,|input,cx|input.set_value(value,window,cx));
        self.refresh_search(cx);cx.notify();
    }
    fn toggle_search_filter(&mut self,token:&str,window:&mut Window,cx:&mut Context<Self>){
        let mut value=self.search.query.clone();
        let matches=token_spans(&value).into_iter().filter(|r|&value[r.clone()]==token).collect::<Vec<_>>();
        if matches.is_empty(){if !value.trim().is_empty(){value.push(' ');}value.push_str(token);}
        else{for range in matches.into_iter().rev(){value.replace_range(range, "");}value=value.trim().to_owned();}
        if value.chars().count()<=256{self.set_search_query(value,window,cx);}
        self.search.input.update(cx,|input,cx|input.focus(window,cx));
    }
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
        let error=Query::parse(&self.search.query).error;
        let count=self.search.hits.len();
        let ordinal=self.search.current.and_then(|row|self.search.hits.keys().position(|r|*r==row)).map(|i|i+1).unwrap_or(0);
        let filters=[("Links","has:link"),("Emotes","has:emote"),("Replies","has:reply"),("Bits","has:bits")].into_iter().map(|(label,token)|{
            let active=token_spans(&self.search.query).into_iter().any(|r|&self.search.query[r]==token);
            Button::new(SharedString::from(format!("search-filter-{token}"))).xsmall().label(if active{format!("✓ {label}")}else{label.into()})
                .tooltip(format!("Toggle {token} filter")).on_click(cx.listener(move|this,_,w,cx|this.toggle_search_filter(token,w,cx)))
        }).collect::<Vec<_>>();
        div().v_flex().p_1().gap_1().bg(rgb(theme::PANEL)).border_b_1().border_color(rgb(theme::BORDER))
            .key_context("JawjackSearch")
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Enter,_,cx|{this.next_search(true,cx);cx.stop_propagation();}))
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Escape,w,cx|{this.close_search(w,cx);cx.stop_propagation();}))
            .child(div().h_flex().gap_1().min_w_0()
                .child(div().flex_1().min_w_0().child(Input::new(&self.search.input).small()))
                .child(Button::new("search-close").xsmall().label("×").tooltip("Close search · Escape").on_click(cx.listener(|this,_,w,cx|this.close_search(w,cx)))))
            .child(div().h_flex().flex_wrap().gap_1().children(filters)
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
pub fn create(window:&mut Window,cx:&mut Context<ChannelPane>)->Search {
    let input=cx.new(|cx|InputState::new(window,cx).placeholder("Find text · from:user · has:link…").validate(|text,_|text.chars().count()<=256));
    cx.subscribe_in(&input,window,|pane,_,event,_,cx|{
        if matches!(event,InputEvent::Change){
            pane.search.query=pane.search.input.read(cx).value().to_string();
            pane.search.current=None;pane.search.dirty=true;pane.refresh_search(cx);cx.notify();
        }
    }).detach();
    Search{open:false,input,query:String::new(),dirty:true,hits:Rc::default(),current:None}
}
