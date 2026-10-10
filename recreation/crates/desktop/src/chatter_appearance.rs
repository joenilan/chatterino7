//! Private display preferences keyed by immutable Twitch user ID.
use std::{cell::RefCell,collections::BTreeMap,rc::Rc};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::{button::Button,input::{Input,InputState,InputEvent},Sizable,StyledExt,Disableable};
use crate::{theme,highlight_rules::COLORS};
#[derive(Clone,Default,PartialEq,Eq)]
pub struct Appearance {pub alias:String,pub color:Option<usize>,pub login:String,pub name:String}
pub type Preferences=Rc<RefCell<BTreeMap<String,Appearance>>>;
#[derive(Clone)]
pub struct Target {pub id:String,pub login:String,pub name:String}
pub fn valid_id(id:&str)->bool{!id.is_empty()&&id.len()<=32&&id.bytes().all(|b|b.is_ascii_digit())}
pub fn validate_alias(alias:&str)->Result<String,String>{
    let alias=alias.trim();
    if alias.chars().count()>40||alias.chars().any(|c|c.is_control()||matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')){return Err("Use up to 40 characters, without line breaks or directional control characters.".into());}
    Ok(alias.into())
}
pub fn load(value:&serde_json::Value)->Preferences{
    let mut result=BTreeMap::new();
    if let Some(map)=value.as_object(){for (id,v) in map.iter().take(256){
        if !valid_id(id){continue;}
        let Ok(alias)=validate_alias(v["alias"].as_str().unwrap_or(""))else{continue;};
        let color=v["color"].as_u64().filter(|c|*c<COLORS.len()as u64).map(|c|c as usize);
        let login=chat_core::twitch_login(v["login"].as_str().unwrap_or("")).unwrap_or_default();
        let name=v["name"].as_str().unwrap_or(&login).chars().filter(|c|!c.is_control()).take(80).collect();
        if !alias.is_empty()||color.is_some(){result.insert(id.clone(),Appearance{alias,color,login,name});}
    }}Rc::new(RefCell::new(result))
}
pub fn json(preferences:&Preferences)->serde_json::Value{serde_json::Value::Object(preferences.borrow().iter().map(|(id,p)|(id.clone(),serde_json::json!({"alias":p.alias,"color":p.color,"login":p.login,"name":p.name}))).collect())}
fn values(preferences:&BTreeMap<String,Appearance>,id:&str,original:Option<u32>)->(Option<String>,Option<u32>){
    let appearance=preferences.get(id);
    (appearance.filter(|a|!a.alias.is_empty()).map(|a|a.alias.clone()),
     Some(appearance.and_then(|a|a.color).map(|color|COLORS.get(color).map(|c|c.1).unwrap_or(theme::TEXT)).unwrap_or_else(||theme::readable_name_color(id,original))))
}
pub fn apply(preferences:&Preferences,message:&mut chat_core::Message){
    let (alias,color)=values(&preferences.borrow(),&message.user_id,message.name_color);
    message.presentation.local_alias=alias;message.presentation.local_color=color;
}
pub fn restyle(preferences:&Preferences,timeline:&mut chat_core::Timeline){
    let preferences=preferences.borrow();timeline.update_author_appearance(|id,color|values(&preferences,id,color));
}

pub struct Changed;
impl EventEmitter<Changed> for Editor{}
pub struct Editor{
    preferences:Preferences,alias:Entity<InputState>,color:Option<usize>,known:BTreeMap<String,Target>,
    selected:Option<String>,switch_pending:Option<Option<String>>,reset_armed:bool,status:String,
}
impl Editor{
    pub fn new(preferences:Preferences,window:&mut Window,cx:&mut Context<Self>)->Self{
        let alias=cx.new(|cx|InputState::new(window,cx).placeholder("Original Twitch display name").validate(|v,_|v.chars().count()<=40&&!v.chars().any(char::is_control)));
        cx.subscribe_in(&alias,window,|this:&mut Self,_,e,_,cx|{if matches!(e,InputEvent::Change){this.edited(cx);}}).detach();
        Self{preferences,alias,color:None,known:BTreeMap::new(),selected:None,switch_pending:None,reset_armed:false,status:String::new()}
    }
    pub fn open(&mut self,target:Option<Target>,window:&mut Window,cx:&mut Context<Self>){
        self.reset_armed=false;self.status.clear();
        if target.is_none(){self.switch_pending=None;}
        if let Some(target)=target.filter(|t|valid_id(&t.id)){let id=target.id.clone();if self.known.len()>=256&&!self.known.contains_key(&id){
            let victim=self.known.keys().find(|key|Some(*key)!=self.selected.as_ref()).cloned();
            if let Some(victim)=victim{self.known.remove(&victim);}
        }self.known.insert(id.clone(),target);if self.selected.as_ref()!=Some(&id){self.select(Some(id),window,cx);}}
        cx.notify();
    }
    fn select(&mut self,id:Option<String>,window:&mut Window,cx:&mut Context<Self>){
        let original=self.selected.as_ref().and_then(|id|self.preferences.borrow().get(id).cloned()).unwrap_or_default();
        let dirty=self.alias.read(cx).value().as_ref()!=original.alias||self.color!=original.color;
        if dirty&&self.switch_pending!=Some(id.clone()){self.switch_pending=Some(id);self.status="Unsaved edits. Save first, or choose that chatter again to discard them.".into();cx.notify();return;}
        let p=id.as_ref().and_then(|id|self.preferences.borrow().get(id).cloned()).unwrap_or_default();
        self.alias.update(cx,|a,cx|a.set_value(p.alias,window,cx));self.color=p.color;self.selected=id;
        self.switch_pending=None;self.reset_armed=false;self.status.clear();cx.notify();
    }
    fn target(&self)->Option<Target>{
        let id=self.selected.as_ref()?;
        self.known.get(id).cloned().or_else(||self.preferences.borrow().get(id).map(|p|Target{id:id.clone(),login:p.login.clone(),name:p.name.clone()}))
    }
    fn edited(&mut self,cx:&mut Context<Self>){self.reset_armed=false;self.switch_pending=None;self.status.clear();cx.notify();}
    fn save(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let Some(target)=self.target()else{self.status="Choose a saved chatter, or right-click a chat message to personalize its author.".into();cx.notify();return;};
        let alias=match validate_alias(&self.alias.read(cx).value()){Ok(alias)=>alias,Err(e)=>{self.status=e;cx.notify();return;}};
        let mut preferences=self.preferences.borrow_mut();
        if alias.is_empty()&&self.color.is_none(){preferences.remove(&target.id);}
        else {if !preferences.contains_key(&target.id)&&preferences.len()>=256{self.status="256 custom appearances are saved. Reset an unused entry first.".into();cx.notify();return;}
            preferences.insert(target.id.clone(),Appearance{alias:alias.clone(),color:self.color,login:target.login.clone(),name:target.name.clone()});}
        drop(preferences);self.known.insert(target.id.clone(),target);self.alias.update(cx,|a,cx|a.set_value(alias,window,cx));self.reset_armed=false;self.switch_pending=None;
        self.status="Applied to retained and new chat. Local save queued; existing text selection was cleared.".into();cx.emit(Changed);cx.notify();
    }
}
impl Render for Editor{
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
        let mut entries=self.preferences.borrow().iter().map(|(id,p)|(id.clone(),Target{id:id.clone(),login:p.login.clone(),name:p.name.clone()})).collect::<BTreeMap<_,_>>();
        entries.extend(self.known.clone());
        let target=self.target();let alias=self.alias.read(cx).value().to_string();let preview=target.as_ref().map(|t|if alias.trim().is_empty(){t.name.clone()}else{alias.trim().into()}).unwrap_or_else(||"Choose a chatter".into());
        let color=self.color.map(|i|COLORS[i].1).unwrap_or(theme::TEXT);
        let height=(f32::from(window.viewport_size().height)-150.).clamp(100.,580.);
        div().id("chatter-appearance-editor").v_flex().gap_2().max_h(px(height)).overflow_y_scroll()
            .capture_key_down(cx.listener(|this,e:&KeyDownEvent,w,cx|{if e.keystroke.modifiers.control&&!e.keystroke.modifiers.alt&&e.keystroke.key=="s"{this.save(w,cx);w.prevent_default();cx.stop_propagation();}}))
            .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Only changes what you see. Real Twitch identity, mentions, replies and sender filters stay unchanged. Copied transcript text uses the displayed nickname; Copy username keeps the real login."))
            .child(div().id("saved-chatter-appearances").v_flex().gap_1().max_h(px(150.)).overflow_y_scroll()
                .when(entries.is_empty(),|el|el.child(div().p_2().text_color(rgb(theme::MUTED)).child("Right-click a message → Personalize chatter… to add someone.")))
                .children(entries.into_iter().map(|(id,t)|{
                    let label=if !t.login.is_empty(){format!("@{}",t.login)}else{format!("{} · {}",t.name.chars().take(20).collect::<String>(),id)};
                    Button::new(SharedString::from(format!("appearance-{id}"))).small().w_full().min_w_0().overflow_hidden().label(format!("{label}{}",if self.selected.as_ref()==Some(&id){" · editing"}else{""})).on_click(cx.listener(move|this,_,w,cx|this.select(Some(id.clone()),w,cx)))
                })))
            .when_some(target.clone(),|el,t|el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("Original: {} · @{} · ID {}",t.name,t.login,t.id))))
            .child(div().text_size(px(12.)).child("Local nickname · leave empty to use Twitch’s display name"))
            .child(Input::new(&self.alias).small())
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("appearance-color-auto").small().label("Twitch color").disabled(self.color.is_none()).on_click(cx.listener(|this,_,_,cx|{this.color=None;this.edited(cx);})))
                .children(COLORS.iter().enumerate().map(|(i,(label,color,_))|Button::new(("appearance-color",i)).small().label(*label).text_color(rgb(*color)).disabled(self.color==Some(i)).on_click(cx.listener(move|this,_,_,cx|{this.color=Some(i);this.edited(cx);})))) )
            .child(div().p_3().rounded(px(4.)).bg(rgb(theme::CANVAS)).v_flex().gap_1()
                .child(div().text_color(rgb(color)).font_weight(FontWeight::SEMIBOLD).child(preview))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Nickname and color preview · not a chat message")))
            .child(div().h_flex().flex_wrap().gap_1()
                .child(Button::new("save-chatter-appearance").small().label("Save · Ctrl+S").disabled(target.is_none()).on_click(cx.listener(|this,_,w,cx|this.save(w,cx))))
                .child(Button::new("reset-chatter-appearance").small().label(if self.reset_armed{"Confirm reset"}else{"Reset to Twitch…"}).disabled(target.is_none()).on_click(cx.listener(|this,_,w,cx|{
                    if !this.reset_armed{this.reset_armed=true;this.status="Confirm to remove this chatter’s saved nickname and color.".into();cx.notify();return;}
                    this.alias.update(cx,|a,cx|a.set_value("",w,cx));this.color=None;this.save(w,cx);
                }))))
            .when(!self.status.is_empty(),|el|el.child(div().text_size(px(11.)).text_color(rgb(0xE9C99C)).child(self.status.clone())))
    }
}
