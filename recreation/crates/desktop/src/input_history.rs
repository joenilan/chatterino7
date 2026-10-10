//! Session-local, successful sends only. Navigation never submits chat.
use crate::{ChannelPane,PaneEvent};
use gpui_kit::*;
#[derive(Default)]
pub struct History {account:Option<String>,items:Vec<String>,cursor:Option<usize>,draft:String,shown:String}
impl History {
    pub fn account(&mut self,id:Option<&str>){if self.account.as_deref()!=id {*self=Self{account:id.map(str::to_owned),..Default::default()};}}
    pub fn record(&mut self,text:&str){
        if !text.is_empty() && self.items.last().is_none_or(|last|last!=text) {self.items.push(text.into());if self.items.len()>100{
                // Keep the item currently being recalled stable while an async send finishes.
                let remove=if self.cursor==Some(0){1}else{0};self.items.remove(remove);
                if let Some(index)=self.cursor {if index>remove{self.cursor=Some(index-1);}}
            }}
        if self.cursor.is_none(){self.draft.clear();self.shown.clear();}
    }
    fn navigate(&mut self,current:&str,older:bool)->Option<String>{
        if self.items.is_empty(){return None;}
        if self.cursor.is_some() && current!=self.shown {self.cursor=None;}
        let next=match self.cursor {
            None if older=>{self.draft=current.into();self.items.len()-1},
            None=>return None,
            Some(index) if older=>index.saturating_sub(1),
            Some(index) if index+1<self.items.len()=>index+1,
            Some(_)=>{self.cursor=None;self.shown=self.draft.clone();return Some(self.draft.clone());}
        };
        self.cursor=Some(next);self.shown=self.items[next].clone();Some(self.shown.clone())
    }
}
impl ChannelPane {
    pub fn recall_input(&mut self,older:bool,window:&mut Window,cx:&mut Context<Self>){
        if !self.draft.read(cx).focus_handle(cx).is_focused(window){return;}
        let current=self.draft.read(cx).value().to_string();
        if let Some(value)=self.input_history.navigate(&current,older){
            self.compose_revision=self.compose_revision.wrapping_add(1);
            self.draft.update(cx,|input,cx|{input.set_value(value.clone(),window,cx);input.set_selected_range(value.len()..value.len(),cx);});
            self.picker.open=false;self.picker.suggestions.clear();self.picker.token=None;
            cx.emit(PaneEvent::DraftChanged);cx.notify();
        }
        window.prevent_default();cx.stop_propagation();
    }
}
