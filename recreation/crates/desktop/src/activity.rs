//! Workbench attention UI; uses existing timeline references rather than another log.
use super::*;
use std::collections::BTreeMap;
struct FontFeedback;
impl Workbench {
    pub(super) fn sync_attention(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let identity=self.account.read(cx).identity().and_then(|i|chat_core::twitch_login(&self.account.read(cx).label()).map(|login|(i.user_id,login)));
        let visible=self.visible_panes(cx);
        let available=window.is_window_active()&&!window.has_active_dialog(cx)&&!self.settings&&!self.adding;
        let mut changed=false;
        for pane in self.panes().iter().chain(self.closed_tabs.iter().flat_map(|t|t.panes.iter())) {
            let eligible=available&&visible.contains(pane);
            pane.update(cx,|p,cx|{
                let timeline=p.timeline.borrow();let first=(p.next_id-timeline.messages().len()) as u64;
                let configured=p.attention.configure(identity.clone(),&self.highlight_terms,&timeline,first,p.next_id as u64);
                drop(timeline);
                let eligible=eligible&&p.scroller.read(cx).is_following_tail()&&!p.search.open;
                let was=p.read_eligible.replace(eligible);
                let read=if eligible&&was {p.presented_row.take().is_some_and(|row|p.attention.mark_through(row))}
                    else{p.presented_row.set(None);false};
                if configured||read||was!=eligible{cx.notify();changed=true;}
            });
        }
        if changed{cx.notify();}
    }
    pub(super) fn displayed_attention(&self,pane:&Entity<ChannelPane>,reading_available:bool,cx:&App)->(usize,usize){
        let p=pane.read(cx);
        // Presentation only: actual unread is acknowledged after rows are painted.
        // Hide the one-tick arrival/ack gap for a chat the owner is already reading.
        let reading=reading_available && !self.settings && !self.adding && !p.search.open
            && p.scroller.read(cx).is_following_tail() && self.visible_panes(cx).contains(pane);
        if reading {(0,0)} else {p.attention.counts()}
    }
    pub(super) fn attention_counts(&self,reading_available:bool,cx:&App)->(usize,usize){
        self.panes().iter().map(|p|self.displayed_attention(p,reading_available,cx)).fold((0,0),|(u,h),(a,b)|(u+a,h+b))
    }
    pub(super) fn mark_all_read(&mut self,cx:&mut Context<Self>){
        for pane in self.panes(){pane.update(cx,|p,cx|{if p.attention.mark_all(){cx.notify();}});}
        cx.notify();
    }
    pub(super) fn font_feedback(&mut self,delta:f32,reset:bool,window:&mut Window,cx:&mut Context<Self>){
        let delta=if reset{14.-self.font_size}else{delta};
        self.change_font(delta,cx);
        window.push_notification(Notification::info(format!("Chat font · {} px · Ctrl+0 to reset",self.font_size as u32)).id::<FontFeedback>(),cx);
    }
    fn jump_activity(&mut self,channel:&str,id:&str,window:&mut Window,cx:&mut Context<Self>){
        let order=std::iter::once(self.active).chain((0..self.tabs.len()).filter(|ix|*ix!=self.active));
        let target=order.filter_map(|ix|self.tabs[ix].panes.iter().find(|p|p.read(cx).name.as_ref()==channel&&p.read(cx).timeline.borrow().messages().iter().any(|m|m.id==id&&!m.deleted)).cloned().map(|p|(ix,p))).next();
        let Some((ix,pane))=target else{window.push_notification(Notification::info("That message is no longer in an open channel's retained history"),cx);return;};
        self.select_tab(ix,cx);if let Some(dock)=&mut self.tabs[ix].dock{dock.select(channel);}
        self.selected_channel=Some(channel.to_owned());self.schedule_save(cx);
        pane.update(cx,|p,cx|p.jump_message(id,window,cx));cx.notify();
    }
    pub(super) fn open_activity(&mut self,window:&mut Window,cx:&mut Context<Self>){
        let owner=cx.entity().downgrade();
        window.open_dialog(cx,move|dialog,window,cx|{
            let Some(view)=owner.upgrade()else{return dialog.title("Workspace closed");};
            let this=view.read(cx);let all=this.activity_all;
            let mut rows=BTreeMap::new();
            for pane in this.panes(){
                let p=pane.read(cx);let timeline=p.timeline.borrow();let first=(p.next_id-timeline.messages().len()) as u64;
                for (index,m) in timeline.messages().iter().enumerate(){
                    let row=first+index as u64;let highlight=p.attention.highlights.contains(&row);let unread=p.attention.unread.contains(&row);
                    if m.deleted||if all{!unread}else{!highlight}{continue;}
                    let Some(at)=p.attention.arrivals.get(&row).copied()else{continue;};
                    let key=(p.name.to_string(),m.id.clone());
                    let value=(at,p.name.to_string(),m.id.clone(),pane.clone(),index,highlight,unread);
                    rows.entry(key).and_modify(|v:&mut (std::time::Instant,String,String,Entity<ChannelPane>,usize,bool,bool)|{v.6|=unread;v.5|=highlight;}).or_insert(value);
                }
            }
            let count=rows.len();let mut rows=rows.into_values().collect::<Vec<_>>();rows.sort_by(|a,b|b.0.cmp(&a.0));rows.truncate(200);
            let width=(f32::from(window.viewport_size().width)-24.).clamp(180.,600.);
            let height=(f32::from(window.viewport_size().height)-210.).clamp(50.,520.);
            let highlights=owner.clone();let unread=owner.clone();let mark=owner.clone();
            dialog.w(px(width)).title("Highlights & unread")
                .child(div().v_flex().gap_2().min_w_0()
                    .child(div().h_flex().flex_wrap().gap_1()
                        .child(Button::new("activity-highlights").small().disabled(!all).label("Highlights").on_click(move|_,_,cx|{let _=highlights.update(cx,|p,cx|{p.activity_all=false;cx.notify();});}))
                        .child(Button::new("activity-unread").small().disabled(all).label("All unread").on_click(move|_,_,cx|{let _=unread.update(cx,|p,cx|{p.activity_all=true;cx.notify();});}))
                        .child(Button::new("activity-mark-read").small().label("Mark all read").on_click(move|_,_,cx|{let _=mark.update(cx,|p,cx|p.mark_all_read(cx));})))
                    .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(format!("{count} retained messages · newest 200 shown · this session")))
                    .child(div().id("activity-history").v_flex().gap_2().max_h(px(height)).overflow_y_scroll().min_w_0()
                        .when(rows.is_empty(),|el|el.child(div().p_3().text_color(rgb(theme::MUTED)).child(if all{"You're caught up."}else{"Mentions, replies to you and your highlight words appear here."})))
                        .children(rows.into_iter().filter_map(|(_,channel,id,pane,index,highlight,unread)|{
                            let p=pane.read(cx);let timeline=p.timeline.borrow();let message=timeline.messages().get(index).filter(|m|m.id==id&&!m.deleted)?;
                            let name=message.display_name.clone();let body=message.body();
                            let jump=owner.clone();let text=body.chars().take(240).collect::<String>();let suffix=if body.chars().count()>240{"…"}else{""};
                            Some(div().p_2().v_flex().gap_1().rounded(px(4.)).bg(rgb(if highlight{0x272237}else{theme::CONTROL})).min_w_0()
                                .child(div().h_flex().min_w_0().gap_1()
                                    .child(div().flex_1().min_w_0().text_size(px(11.)).text_color(rgb(0xC9BAF7)).overflow_hidden().text_ellipsis().child(format!("#{channel} · {name}{}",if unread{" · unread"}else{""})))
                                    .child(Button::new(SharedString::from(format!("activity-jump-{channel}-{id}"))).xsmall().label("Jump").on_click(move|_,w,cx|{w.close_dialog(cx);let _=jump.update(cx,|p,cx|p.jump_activity(&channel,&id,w,cx));})))
                                .child(div().text_size(px(12.)).child(format!("{text}{suffix}"))))
                        }))))
        });
    }
}
