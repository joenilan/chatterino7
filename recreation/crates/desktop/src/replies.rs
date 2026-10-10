//! Reply composition and retained conversations share the bounded channel timeline.
use gpui_kit::{*, component::{button::Button, notification::Notification, Sizable, StyledExt, WindowExt}};
use gpui_kit::prelude::FluentBuilder;
use serde_json::{Value,json};
use crate::{ChannelPane,PaneEvent,theme};

#[derive(Clone)]
pub struct Target {pub id:String,pub user_id:String,pub name:String,pub deleted:bool}
impl Target {
    pub fn json(&self)->Value {json!({"id":self.id,"user_id":self.user_id,"name":self.name,"deleted":self.deleted})}
    pub fn from_json(v:&Value)->Option<Self>{
        let id=v["id"].as_str()?;
        if id.is_empty()||id.len()>128||!id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'){return None;}
        Some(Self{id:id.into(),user_id:v["user_id"].as_str().unwrap_or("").chars().take(128).collect(),name:v["name"].as_str().unwrap_or("Chatter").chars().take(100).collect(),deleted:v["deleted"].as_bool().unwrap_or(false)})
    }
}
fn short(text:&str,limit:usize)->String {
    let mut result=text.chars().take(limit).collect::<String>().replace(['\n','\r']," ");
    if text.chars().count()>limit {result.push('…');} result
}
fn root(message:&chat_core::Message)->String{message.reply.as_ref().map(|r|r.thread_id.clone()).unwrap_or_else(||message.id.clone())}

pub fn row_context(message:&chat_core::Message,timeline:&chat_core::Timeline,owner:WeakEntity<ChannelPane>)->Option<AnyElement>{
    if message.deleted{return None;}
    let reply=message.reply.as_ref()?;
    let parent=timeline.messages().iter().find(|m|m.id==reply.parent_id);
    let preview=if reply.parent_deleted||parent.is_some_and(|m|m.deleted){"[message deleted]".into()}
        else{parent.map(|m|short(&m.body(),100)).unwrap_or_else(||"Original outside retained history".into())};
    let label=format!("↱ {} · {preview}",short(&reply.parent_name,30));let id=message.id.clone();
    Some(div().id(SharedString::from(format!("reply-{}",message.id))).px_1().text_size(px(11.)).line_height(px(16.)).text_color(rgb(0xB6A9E6)).min_w_0().overflow_hidden().text_ellipsis().cursor_pointer()
        .hover(|s|s.bg(rgb(theme::CONTROL)))
        .on_click(move|_,w,cx|{let _=owner.update(cx,|p,cx|p.message_action(&id,crate::message_actions::Action::Thread,w,cx));cx.stop_propagation();})
        .child(label).into_any_element())
}
impl ChannelPane {
    pub fn begin_reply(&mut self,message:&chat_core::Message,window:&mut Window,cx:&mut Context<Self>){
        if message.deleted||!message.replyable {window.push_notification(Notification::info("This message cannot be replied to in this channel"),cx);return;}
        self.compose_revision=self.compose_revision.wrapping_add(1);
        self.reply_target=Some(Target{id:message.id.clone(),user_id:message.user_id.clone(),name:message.display_name.clone(),deleted:false});
        self.picker.open=false;self.picker.suggestions.clear();self.picker.token=None;
        self.draft.update(cx,|input,cx|input.focus(window,cx));
        cx.emit(PaneEvent::DraftChanged);cx.notify();
    }
    pub fn cancel_reply(&mut self,window:&mut Window,cx:&mut Context<Self>){
        self.compose_revision=self.compose_revision.wrapping_add(1);
        self.reply_target=None;self.draft.update(cx,|i,cx|i.focus(window,cx));cx.emit(PaneEvent::DraftChanged);cx.notify();
    }
    pub fn observe_reply_redaction(&mut self,event:&chat_core::Event,cx:&mut Context<Self>){
        let Some(target)=&mut self.reply_target else{return;};
        let deleted=match event {
            chat_core::Event::DeleteMessage{channel_id,message_id}=>channel_id==self.name.as_ref()&&message_id==&target.id,
            chat_core::Event::ClearUser{channel_id,user_id}=>channel_id==self.name.as_ref()&&user_id==&target.user_id,
            chat_core::Event::ClearChannel{channel_id}=>channel_id==self.name.as_ref(),
            _=>false,
        };
        if deleted&&!target.deleted {target.deleted=true;cx.emit(PaneEvent::DraftChanged);cx.notify();}
    }
    pub fn render_reply_target(&self,cx:&mut Context<Self>)->Option<AnyElement>{
        let target=self.reply_target.as_ref()?;
        let timeline=self.timeline.borrow();let message=timeline.messages().iter().find(|m|m.id==target.id);
        let label=if target.deleted||message.is_some_and(|m|m.deleted){"Reply target deleted · cancel or choose another".to_owned()}
            else{format!("Replying to {}",short(&target.name,32))};
        let preview=message.map(|m|short(&m.body(),100)).unwrap_or_else(||"Original outside retained history; reply target saved".into());
        let id=target.id.clone();
        Some(div().h_flex().gap_1().min_w_0().px_2().py_1().bg(rgb(0x282333)).border_l_2().border_color(rgb(0xB6A9E6))
            .child(div().flex_1().min_w_0().v_flex()
                .child(div().text_size(px(11.)).text_color(rgb(0xC9BAF7)).overflow_hidden().text_ellipsis().child(label))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).overflow_hidden().text_ellipsis().child(preview)))
            .when(message.is_some(),|el|el.child(Button::new("reply-context").xsmall().label("↱").tooltip("View retained conversation").on_click(cx.listener(move|p,_,w,cx|p.message_action(&id,crate::message_actions::Action::Thread,w,cx)))))
            .child(Button::new("reply-cancel").xsmall().label("×").tooltip("Cancel reply; keep draft · Escape").on_click(cx.listener(|p,_,w,cx|p.cancel_reply(w,cx))))
            .into_any_element())
    }
    pub fn jump_message(&mut self,id:&str,window:&mut Window,cx:&mut Context<Self>){
        let index=self.timeline.borrow().messages().iter().position(|m|m.id==id);
        if let Some(index)=index{self.scroller.update(cx,|s,cx|s.scroll_to_item(index,cx));self.focus.focus(window,cx);cx.notify();}
        else{window.push_notification(Notification::info("That message is no longer retained"),cx);}
    }
    pub fn open_conversation(&mut self,message:&chat_core::Message,window:&mut Window,cx:&mut Context<Self>){
        let root=root(message);let owner=cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,window,cx|{
            let Some(pane)=owner.upgrade()else{return dialog.title("Channel closed");};
            let p=pane.read(cx);let timeline=p.timeline.borrow();
            let all=timeline.messages().iter().filter(|m|m.id==root||m.reply.as_ref().is_some_and(|r|r.thread_id==root)).collect::<Vec<_>>();
            let count=all.len();let has_root=all.iter().any(|m|m.id==root);
            let rows=all.into_iter().rev().take(100).collect::<Vec<_>>().into_iter().rev().map(|m|(m.id.clone(),m.display_name.clone(),m.body(),m.deleted,m.replyable)).collect::<Vec<_>>();
            let width=(f32::from(window.viewport_size().width)-24.).clamp(180.,560.);
            let height=(f32::from(window.viewport_size().height)-180.).clamp(70.,520.);
            dialog.w(px(width)).title("Retained conversation")
                .child(div().v_flex().gap_2().min_w_0()
                    .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("#{channel} · {count} retained messages{suffix}",channel=p.name,suffix=if count>100{" · newest 100 shown"}else{""})))
                    .when(!has_root,|el|el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child("Original message is outside retained history.")))
                    .child(div().id("conversation-history").v_flex().gap_2().max_h(px(height)).overflow_y_scroll()
                        .children(rows.into_iter().map(|(id,name,body,deleted,replyable)|{
                            let jump=owner.clone();let jump_id=id.clone();let reply=owner.clone();
                            div().p_2().v_flex().gap_1().bg(rgb(theme::CONTROL)).rounded(px(4.)).min_w_0()
                                .child(div().h_flex().gap_1().min_w_0().child(div().flex_1().min_w_0().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).overflow_hidden().text_ellipsis().child(name))
                                    .child(Button::new(SharedString::from(format!("thread-jump-{id}"))).xsmall().label("Jump").on_click(move|_,w,cx|{w.close_dialog(cx);let _=jump.update(cx,|p,cx|p.jump_message(&jump_id,w,cx));}))
                                    .when(!deleted&&replyable,|el|el.child(Button::new(SharedString::from(format!("thread-reply-{id}"))).xsmall().label("Reply").on_click(move|_,w,cx|{w.close_dialog(cx);let _=reply.update(cx,|p,cx|p.message_action(&id,crate::message_actions::Action::Reply,w,cx));}))))
                                .child(div().text_size(px(12.)).child(body))
                        }))))
        });
    }
}
