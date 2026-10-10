//! Bounded local highlight rules. Never changes message identity or sends anything.
use std::{cell::RefCell, rc::Rc};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::{button::Button,input::{Input,InputEvent,InputState},Sizable,StyledExt,Disableable};
use crate::theme;
use unicode_segmentation::UnicodeSegmentation;

pub const COLORS: [(&str,u32,u32);5] = [
    ("Lilac",0xA99CF4,0x272237),("Mint",0x91D7BA,0x192D27),
    ("Sky",0x91C7E8,0x1B2934),("Rose",0xE69DCD,0x332333),
    ("Peach",0xE8C68A,0x30291D),
];
pub type Rules = Rc<RefCell<Vec<Rule>>>;
#[derive(Clone,PartialEq,Eq)]
pub struct Rule {
    folded:String,
    pub pattern:String, pub channel:String, pub author:bool, pub case_sensitive:bool,
    pub whole_word:bool, pub enabled:bool, pub color:usize,
}
impl Default for Rule {
    fn default()->Self {Self{folded:String::new(),pattern:String::new(),channel:String::new(),author:false,case_sensitive:false,whole_word:true,enabled:true,color:0}}
}
impl Rule {
    pub fn validate(mut self)->Result<Self,String>{
        self.pattern=self.pattern.trim().to_owned();
        if self.pattern.is_empty()||self.pattern.chars().count()>80||self.pattern.chars().any(char::is_control){return Err("Enter 1–80 characters on one line.".into());}
        if self.author {self.pattern=chat_core::twitch_login(self.pattern.trim_start_matches('@')).ok_or("Enter a Twitch login, not a display name.")?;}
        let channel=self.channel.trim();
        self.channel=if channel.is_empty(){String::new()}else{
            chat_core::twitch_login(channel.strip_prefix('#').unwrap_or(channel)).ok_or("Enter a channel login, or leave it empty for every channel.")?
        };
        self.folded=self.pattern.to_lowercase();
        self.color=self.color.min(COLORS.len()-1); Ok(self)
    }
    pub fn matches(&self,text:&str,login:Option<&str>,channel:&str)->bool{
        if !self.enabled||(!self.channel.is_empty()&&!self.channel.eq_ignore_ascii_case(channel)){return false;}
        if self.author {return login.is_some_and(|login|login.eq_ignore_ascii_case(&self.pattern));}
        text_matches(text,&self.pattern,self.case_sensitive,self.whole_word)
    }

}
fn word(c:char)->bool{c.is_alphanumeric()||c=='_'}
pub fn text_matches(text:&str,needle:&str,case_sensitive:bool,whole:bool)->bool{
    if needle.is_empty(){return false;}
    let (text,needle)=if case_sensitive{(text.to_owned(),needle.to_owned())}else{(text.to_lowercase(),needle.to_lowercase())};
    normalized_match(&text,&needle,whole)
}
pub fn normalized_match(text:&str,needle:&str,whole:bool)->bool{
    if needle.is_empty(){return false;}
    if !whole{return text.contains(needle);}
    let Some(mut at)=text.find(needle)else{return false;};
    let boundaries=text.grapheme_indices(true).map(|(index,_)|index).chain(std::iter::once(text.len())).collect::<Vec<_>>();
    loop {
        if let (Ok(start),Ok(end))=(boundaries.binary_search(&at),boundaries.binary_search(&(at+needle.len()))){
            let before=start==0||!text[boundaries[start-1]..at].chars().any(word);
            let after=end+1>=boundaries.len()||!text[boundaries[end]..boundaries[end+1]].chars().any(word);
            if before&&after{return true;}
        }
        let next=at+text[at..].chars().next().unwrap().len_utf8();
        let Some(relative)=text[next..].find(needle)else{return false;};at=next+relative;
    }
}
pub fn first_color(rules:&[Rule],message:&chat_core::Message,channel:&str)->Option<usize>{
    if message.deleted||rules.is_empty(){return None;}
    let mut segments=Vec::new();let mut text=String::new();
    for fragment in &message.fragments{if let chat_core::Fragment::Text(part)=fragment{text.push_str(part);}else if !text.is_empty(){segments.push(std::mem::take(&mut text));}}
    if !text.is_empty(){segments.push(text);}
    let folded=if rules.iter().any(|r|r.enabled&&!r.author&&!r.case_sensitive){segments.iter().map(|s|s.to_lowercase()).collect::<Vec<_>>()}else{Vec::new()};
    rules.iter().find(|rule|{
        if !rule.enabled||(!rule.channel.is_empty()&&!rule.channel.eq_ignore_ascii_case(channel)){return false;}
        if rule.author{return message.login.as_deref().is_some_and(|login|login.eq_ignore_ascii_case(&rule.pattern));}
        let (segments,needle)=if rule.case_sensitive{(&segments,&rule.pattern)}else{(&folded,&rule.folded)};
        segments.iter().any(|text|normalized_match(text,needle,rule.whole_word))
    }).map(|rule|rule.color)
}

pub fn load(value:&serde_json::Value)->Rules {
    let mut rules=Vec::new();
    if let Some(items)=value.as_array(){for item in items.iter().take(24){
        let rule=Rule{folded:String::new(),pattern:item["pattern"].as_str().unwrap_or("").into(),channel:item["channel"].as_str().unwrap_or("").into(),author:item["author"].as_bool().unwrap_or(false),case_sensitive:item["case_sensitive"].as_bool().unwrap_or(false),whole_word:item["whole_word"].as_bool().unwrap_or(true),enabled:item["enabled"].as_bool().unwrap_or(true),color:item["color"].as_u64().unwrap_or(0).min(4)as usize};
        if let Ok(rule)=rule.validate(){rules.push(rule);}
    }} Rc::new(RefCell::new(rules))
}
pub fn json(rules:&Rules)->serde_json::Value {serde_json::Value::Array(rules.borrow().iter().map(|r|serde_json::json!({"pattern":r.pattern,"channel":r.channel,"author":r.author,"case_sensitive":r.case_sensitive,"whole_word":r.whole_word,"enabled":r.enabled,"color":r.color})).collect())}

pub struct Changed;
impl EventEmitter<Changed> for Editor{}
pub struct Editor {
    rules:Rules, pattern:Entity<InputState>,channel:Entity<InputState>,sample:Entity<InputState>,login:Entity<InputState>,
    draft:Rule,selected:Option<usize>,switch_pending:Option<Option<usize>>,delete_armed:bool,status:String,preview_channel:String,
}
impl Editor {
    pub fn new(rules:Rules,channel:String,window:&mut Window,cx:&mut Context<Self>)->Self{
        let pattern=cx.new(|cx|InputState::new(window,cx).placeholder("A phrase or Twitch login").validate(|value,_|value.chars().count()<=80&&!value.chars().any(char::is_control)));
        let scope=cx.new(|cx|InputState::new(window,cx).placeholder("Every channel (optional #channel)").validate(|value,_|value.len()<=26&&!value.chars().any(char::is_control)));
        let sample=cx.new(|cx|InputState::new(window,cx).placeholder("Try a sample message locally").validate(|value,_|value.chars().count()<=500&&!value.chars().any(char::is_control)));
        let login=cx.new(|cx|InputState::new(window,cx).placeholder("Sample sender login").validate(|value,_|value.len()<=25&&!value.chars().any(char::is_control)));
        for input in [&pattern,&scope,&sample,&login]{cx.subscribe_in(input,window,|this:&mut Self,_,event,_,cx|{if matches!(event,InputEvent::Change){this.status.clear();this.delete_armed=false;this.switch_pending=None;cx.notify();}}).detach();}
        Self{rules,pattern,channel:scope,sample,login,draft:Rule::default(),selected:None,switch_pending:None,delete_armed:false,status:String::new(),preview_channel:channel}
    }
    pub fn set_channel(&mut self,channel:String,cx:&mut Context<Self>){self.preview_channel=channel;self.delete_armed=false;self.switch_pending=None;self.status.clear();cx.notify();}
    fn current(&self,cx:&App)->Rule{let mut r=self.draft.clone();r.pattern=self.pattern.read(cx).value().to_string();r.channel=self.channel.read(cx).value().to_string();r}
    fn select(&mut self,index:Option<usize>,window:&mut Window,cx:&mut Context<Self>){
        let original=self.selected.and_then(|i|self.rules.borrow().get(i).cloned()).unwrap_or_default();
        if self.current(cx)!=original&&self.switch_pending!=Some(index){self.switch_pending=Some(index);self.status="Unsaved edits. Save first, or select that choice again to discard them.".into();cx.notify();return;}
        self.draft=index.and_then(|i|self.rules.borrow().get(i).cloned()).unwrap_or_default();
        self.pattern.update(cx,|p,cx|p.set_value(self.draft.pattern.clone(),window,cx));
        self.channel.update(cx,|p,cx|p.set_value(self.draft.channel.clone(),window,cx));
        self.selected=index;self.switch_pending=None;self.delete_armed=false;self.status.clear();cx.notify();
    }
    fn save(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let rule=match self.current(cx).validate(){Ok(r)=>r,Err(e)=>{self.status=e;cx.notify();return;}};
        let mut rules=self.rules.borrow_mut();
        if let Some(index)=self.selected {if let Some(target)=rules.get_mut(index){*target=rule.clone();}else{self.status="Rule no longer exists. Start a new rule.".into();cx.notify();return;}}
        else {if rules.len()>=24{self.status="24 rules are already saved. Edit or remove one first.".into();cx.notify();return;}self.selected=Some(rules.len());rules.push(rule.clone());}
        drop(rules);self.draft=rule;
        self.pattern.update(cx,|p,cx|p.set_value(self.draft.pattern.clone(),window,cx));
        self.channel.update(cx,|p,cx|p.set_value(self.draft.channel.clone(),window,cx));
        self.status="Applied across workspaces; local save queued. Retained messages are re-evaluated.".into();self.delete_armed=false;self.switch_pending=None;cx.emit(Changed);cx.notify();
    }
    fn moved(&mut self,up:bool,cx:&mut Context<Self>){
        let Some(index)=self.selected else{return;};let len=self.rules.borrow().len();
        let next=if up{index.checked_sub(1)}else{index.checked_add(1).filter(|i|*i<len)};
        if let Some(next)=next{self.rules.borrow_mut().swap(index,next);self.selected=Some(next);self.delete_armed=false;self.switch_pending=None;self.status="Priority updated. First matching rule chooses the color.".into();cx.emit(Changed);cx.notify();}
    }
    fn edited(&mut self,cx:&mut Context<Self>){self.delete_armed=false;self.switch_pending=None;self.status.clear();cx.notify();}
}
impl Render for Editor {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let rules=self.rules.borrow().clone();let current=self.current(cx).validate();
        let preview=current.as_ref().map(|r|r.matches(&self.sample.read(cx).value(),Some(&self.login.read(cx).value()),&self.preview_channel)).unwrap_or(false);
        let (_,accent,background)=COLORS[self.draft.color];let max_height=(f32::from(window.viewport_size().height)-150.).clamp(100.,650.);
        div().id("highlight-rule-editor")
            .capture_key_down(cx.listener(|this,event:&KeyDownEvent,w,cx|{if event.keystroke.modifiers.control&&!event.keystroke.modifiers.alt&&event.keystroke.key=="s"{this.save(w,cx);w.prevent_default();cx.stop_propagation();}})).v_flex().gap_2().max_h(px(max_height)).overflow_y_scroll()
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Local highlights across all workspaces. First matching rule chooses the color. Mentions, replies and your simple highlight words stay enabled. Your own messages are excluded; no sounds or external alerts."))
            .child(div().h_flex().gap_2().child(Button::new("new-highlight-rule").small().label("New rule").on_click(cx.listener(|this,_,w,cx|this.select(None,w,cx))))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("{}/24 rules",rules.len()))))
            .child(div().id("saved-highlight-rules").v_flex().gap_1().max_h(px(130.)).overflow_y_scroll().children(rules.iter().enumerate().map(|(index,r)|{
                let limit=if f32::from(window.viewport_size().width)<500.{18}else{36};
                let label=r.pattern.chars().take(limit).collect::<String>();let suffix=if r.pattern.chars().count()>limit{"…"}else{""};
                Button::new(("highlight-rule",index)).small().w_full().min_w_0().overflow_hidden().label(format!("{} · {}{}{}{}",index+1,if r.author{"@"}else{""},label,suffix,if !r.enabled{" · off"}else if self.selected==Some(index){" · editing"}else{""}))
                    .on_click(cx.listener(move|this,_,w,cx|this.select(Some(index),w,cx)))
            })))
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("rule-text").small().label("Message text").disabled(!self.draft.author).on_click(cx.listener(|this,_,_,cx|{this.draft.author=false;this.edited(cx);})))
                .child(Button::new("rule-author").small().label("Sender login").disabled(self.draft.author).on_click(cx.listener(|this,_,_,cx|{this.draft.author=true;this.edited(cx);})))
                .child(Button::new("rule-enabled").small().label(if self.draft.enabled{"Enabled"}else{"Disabled"}).on_click(cx.listener(|this,_,_,cx|{this.draft.enabled=!this.draft.enabled;this.edited(cx);}))))
            .child(Input::new(&self.pattern).small()).child(Input::new(&self.channel).small())
            .when(!self.draft.author,|el|el.child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("rule-case").small().label(if self.draft.case_sensitive{"Match case"}else{"Ignore case"}).on_click(cx.listener(|this,_,_,cx|{this.draft.case_sensitive=!this.draft.case_sensitive;this.edited(cx);})))
                .child(Button::new("rule-word").small().label(if self.draft.whole_word{"Whole words"}else{"Anywhere in text"}).on_click(cx.listener(|this,_,_,cx|{this.draft.whole_word=!this.draft.whole_word;this.edited(cx);})))))
            .child(div().h_flex().flex_wrap().gap_1().children(COLORS.iter().enumerate().map(|(index,(name,color,_))|
                Button::new(("rule-color",index)).small().label(*name).text_color(rgb(*color)).disabled(self.draft.color==index).on_click(cx.listener(move|this,_,_,cx|{this.draft.color=index;this.edited(cx);})))))
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
                        if let Some(index)=this.selected.take(){if index<this.rules.borrow().len(){this.rules.borrow_mut().remove(index);cx.emit(Changed);}}
                        this.switch_pending=Some(None);this.select(None,w,cx);this.status="Rule removed.".into();
                    })))))
            .when(!self.status.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(0xE9C99C)).child(self.status.clone())))
    }
}
