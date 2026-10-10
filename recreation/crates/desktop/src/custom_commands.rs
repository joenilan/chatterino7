//! Profile-local text templates. Expansion prepares a draft, never sends or executes.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::{button::Button, input::{Input,InputState,InputEvent}, Sizable, StyledExt};
use crate::theme;
pub type Definitions = Rc<RefCell<BTreeMap<String,String>>>;

pub fn validate(name:&str, template:&str)->Result<String,String>{
    let name=name.trim().trim_start_matches('/').to_ascii_lowercase();
    if name.is_empty() || name.len()>24 || !name.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_') {
        return Err("Use 1–24 letters, numbers or underscores for the command name.".into());
    }
    if crate::commands::COMMANDS.iter().any(|c|c.name.trim_start_matches('/')==name)
        || ["user","cancelreply","me","w","whisper","ban","banid","unban","timeout","untimeout","delete","clear","mod","unmod","vip","unvip","raid","unraid","host","unhost","commercial","announce","announceblue","announcegreen","announceorange","announcepurple","announceprimary","slow","slowoff","followers","followersoff","subscribers","subscribersoff","emoteonly","emoteonlyoff","uniquechat","uniquechatoff","r9kbeta","r9kbetaoff","shield","shieldoff","shoutout","poll","prediction","pin","unpin","monitor","unmonitor","restrict","unrestrict","block","unblock","disconnect","marker"].contains(&name.as_str()) {
        return Err("That name is reserved for a built-in or Twitch command.".into());
    }
    if template.trim().is_empty() || template.chars().count()>500 || template.chars().any(char::is_control) {
        return Err("The message template must contain 1–500 characters on one line.".into());
    }
    if template.trim_start().starts_with('/') {return Err("Templates must produce chat text, not another slash command.".into());}
    // Validate syntax independently of sample arguments.
    expand_inner(template,"channel","one two three four five six seven eight nine",false)?;
    Ok(name)
}
pub fn expand(template:&str,channel:&str,args:&str)->Result<String,String>{
    expand_inner(template,channel,args,true)
}
fn expand_inner(template:&str,channel:&str,args:&str,enforce_output:bool)->Result<String,String>{
    let words=args.split_whitespace().collect::<Vec<_>>();
    let mut result=String::new();let mut chars=template.chars().peekable();
    while let Some(ch)=chars.next(){
        if ch=='{' {
            if chars.peek()==Some(&'{'){chars.next();result.push('{');continue;}
            let mut token=String::new();let mut closed=false;
            for next in chars.by_ref(){if next=='}'{closed=true;break;}token.push(next);}
            if !closed {return Err("Close the template placeholder with }. Use {{ for a literal brace.".into());}
            let value=match token.as_str(){"channel"=>channel,"args"=>args,
                "1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"=>words.get(token.parse::<usize>().unwrap()-1).copied().ok_or_else(||format!("Missing argument {{{token}}}. Your draft is kept."))?,
                _=>return Err(format!("Unknown placeholder {{{token}}}. Use {{channel}}, {{args}}, or {{1}}–{{9}}."))};
            result.push_str(value);
        }else if ch=='}' {if chars.peek()==Some(&'}'){chars.next();result.push('}');}else{return Err("Use }} for a literal closing brace.".into());}}
        else {result.push(ch);}
        if enforce_output && result.chars().count()>500{return Err("Expanded message exceeds Twitch's 500-character limit. Your draft is kept.".into());}
    }
    if enforce_output && result.chars().count()>500 {return Err("Expanded message exceeds Twitch's 500-character limit. Your draft is kept.".into());}
    if enforce_output && (result.trim().is_empty() || result.trim_start().starts_with('/') || result.chars().any(char::is_control)){return Err("Expansion must be nonempty, single-line chat text, not a slash command. Your draft is kept.".into());}
    Ok(result)
}
pub fn load(value:&serde_json::Value)->Definitions{
    let mut entries=BTreeMap::new();
    if let Some(map)=value.as_object(){for (name,value) in map.iter().take(32){if let Some(template)=value.as_str(){if let Ok(name)=validate(name,template){entries.insert(name,template.into());}}}}
    Rc::new(RefCell::new(entries))
}
pub struct Changed;
pub struct Editor {
    definitions:Definitions, name:Entity<InputState>, template:Entity<InputState>, sample:Entity<InputState>,
    channel:String, selected:Option<String>, status:String, delete_armed:bool, switch_pending:Option<String>,
}
impl EventEmitter<Changed> for Editor{}
impl Editor {
    pub fn new(definitions:Definitions,channel:String,window:&mut Window,cx:&mut Context<Self>)->Self{
        let name=cx.new(|cx|InputState::new(window,cx).placeholder("hello"));
        let template=cx.new(|cx|InputState::new(window,cx).placeholder("Welcome {1} to {channel}!"));
        let sample=cx.new(|cx|InputState::new(window,cx).placeholder("Sample arguments for preview"));
        for input in [&name,&template,&sample]{cx.subscribe_in(input,window,|this:&mut Self,_,event,_,cx|{if matches!(event,InputEvent::Change){this.status.clear();this.delete_armed=false;this.switch_pending=None;cx.notify();}}).detach();}
        Self{definitions,name,template,sample,channel,selected:None,status:String::new(),delete_armed:false,switch_pending:None}
    }
    fn select(&mut self,name:Option<String>,window:&mut Window,cx:&mut Context<Self>){
        let current_name=self.name.read(cx).value().to_string();
        let current_template=self.template.read(cx).value().to_string();
        let original=self.selected.as_ref().and_then(|name|self.definitions.borrow().get(name).cloned()).unwrap_or_default();
        let dirty=current_name!=self.selected.clone().unwrap_or_default() || current_template!=original;
        let target=name.clone().unwrap_or_default();
        if dirty && self.switch_pending.as_ref()!=Some(&target){self.switch_pending=Some(target);self.status="Unsaved edits. Save first, or click that choice again to discard them.".into();cx.notify();return;}
        self.switch_pending=None;
        let template=name.as_ref().and_then(|name|self.definitions.borrow().get(name).cloned()).unwrap_or_default();
        self.name.update(cx,|input,cx|input.set_value(name.clone().unwrap_or_default(),window,cx));
        self.template.update(cx,|input,cx|input.set_value(template,window,cx));
        self.selected=name;self.delete_armed=false;self.status.clear();cx.notify();
    }
    fn save(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let template=self.template.read(cx).value().to_string();
        let name=match validate(&self.name.read(cx).value(),&template){Ok(n)=>n,Err(e)=>{self.status=e;cx.notify();return;}};
        let mut definitions=self.definitions.borrow_mut();
        if self.selected.as_ref()!=Some(&name) && definitions.contains_key(&name){self.status="That command already exists. Choose it from the list to edit it.".into();}
        else if self.selected.is_none() && definitions.len()>=32{self.status="This profile already has 32 commands. Edit or remove one first.".into();}
        else {
            if let Some(old)=&self.selected{if old!=&name{definitions.remove(old);}}
            definitions.insert(name.clone(),template);self.selected=Some(name.clone());self.status=format!("Saved /{name}. Available in every workspace.");drop(definitions);self.name.update(cx,|input,cx|input.set_value(name,window,cx));cx.emit(Changed);cx.notify();return;
        }
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
        let entries=self.definitions.borrow().keys().cloned().collect::<Vec<_>>();
        let template=self.template.read(cx).value().to_string();
        let sample=self.sample.read(cx).value().to_string();
        let preview=if template.is_empty(){"Your expanded message appears here. Nothing is sent.".into()}else{expand(&template,&self.channel,&sample).unwrap_or_else(|error|error)};
        let height=(f32::from(window.viewport_size().height)-160.).clamp(100.,580.);
        div().id("custom-command-editor").v_flex().gap_2().max_h(px(height)).overflow_y_scroll()
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Personal text shortcuts for every workspace. Enter expands a command into your draft; review it, then Send. Existing reply targets are kept."))
            .child(div().h_flex().gap_2().child(Button::new("new-custom-command").small().label("New command").on_click(cx.listener(|this,_,w,cx|this.select(None,w,cx))))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("{}/32 saved",entries.len()))))
            .child(div().id("saved-custom-commands").v_flex().gap_1().max_h(px(140.)).overflow_y_scroll().children(entries.into_iter().map(|name|{
                let chosen=self.selected.as_ref()==Some(&name);let label=format!("/{}{}",name,if chosen{" · editing"}else{""});
                Button::new(SharedString::from(format!("edit-custom-{name}"))).small().label(label).on_click(cx.listener(move|this,_,w,cx|this.select(Some(name.clone()),w,cx)))
            })))
            .child(div().text_size(px(11.)).child("Command name"))
            .child(Input::new(&self.name).small())
            .child(div().text_size(px(11.)).child("Message template"))
            .child(Input::new(&self.template).small())
            .child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child("{channel} = channel login · {args} = all arguments · {1}–{9} = individual words · {{ and }} = literal braces"))
            .child(Input::new(&self.sample).small())
            .child(div().p_2().rounded(px(4.)).bg(rgb(theme::CONTROL)).text_size(px(12.)).child(preview))
            .child(div().h_flex().gap_2().child(Button::new("save-custom-command").small().label("Save command").on_click(cx.listener(|this,_,w,cx|this.save(w,cx))))
                .when(self.selected.is_some(),|el|el.child(Button::new("delete-custom-command").small().label(if self.delete_armed{"Confirm delete"}else{"Delete…"}).on_click(cx.listener(|this,_,w,cx|{
                    if !this.delete_armed{this.delete_armed=true;this.status="Click Confirm delete to remove this shortcut.".into();cx.notify();return;}
                    if let Some(name)=this.selected.take(){this.definitions.borrow_mut().remove(&name);cx.emit(Changed);this.switch_pending=Some(String::new());this.select(None,w,cx);this.status="Command removed.".into();}
                })))))
            .when(!self.status.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(0xE9C99C)).child(self.status.clone())))
    }
}
