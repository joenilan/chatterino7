//! Compact keyboard navigation across existing channels and workspaces.
use super::*;
struct Switcher {owner:WeakEntity<Workbench>,input:Entity<InputState>,selected:usize}
#[derive(Clone)]
struct Target {workspace:u64,workspace_name:String,channel:String,live:Option<bool>,unread:usize}
impl Switcher {
    fn targets(&self,cx:&App)->Vec<Target>{
        let Some(owner)=self.owner.upgrade()else{return vec![];};let owner=owner.read(cx);
        let query=self.input.read(cx).value().trim().trim_start_matches('#').to_lowercase();
        let mut rows=vec![];
        for tab in &owner.tabs{for pane in &tab.panes{let pane=pane.read(cx);let channel=pane.name.to_string();
            if query.split_whitespace().all(|word|channel.to_lowercase().contains(word)||tab.name.to_lowercase().contains(word)){
                rows.push(Target{workspace:tab.id,workspace_name:tab.name.clone(),live:owner.streams.get(&channel),unread:pane.attention.counts().0,channel});
            }
        }}
        rows.sort_by_key(|r|(!r.channel.to_lowercase().starts_with(&query),r.workspace!=owner.tabs[owner.active].id,r.channel.clone(),r.workspace));rows
    }
    fn choose(&mut self,target:Option<Target>,window:&mut Window,cx:&mut Context<Self>){
        let target=target.or_else(||self.targets(cx).get(self.selected).cloned());let Some(target)=target else{return;};
        let _=self.owner.update(cx,|owner,cx|{
            let Some(index)=owner.tabs.iter().position(|tab|tab.id==target.workspace)else{return;};
            let Some(pane)=owner.tabs[index].panes.iter().find(|p|p.read(cx).name.as_ref()==target.channel).cloned()else{return;};
            window.close_dialog(cx);owner.select_tab(index,cx);if let Some(dock)=&mut owner.tabs[index].dock{dock.select(&target.channel);}
            owner.selected_channel=Some(target.channel);owner.schedule_save(cx);
            pane.update(cx,|p,cx|p.draft.update(cx,|draft,cx|draft.focus(window,cx)));cx.notify();
        });
    }
    fn step(&mut self,forward:bool,window:&mut Window,cx:&mut Context<Self>){let len=self.targets(cx).len();if len>0{self.selected=if forward{(self.selected+1)%len}else{(self.selected+len-1)%len};}window.prevent_default();cx.stop_propagation();cx.notify();}
}
impl Render for Switcher {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
        let targets=self.targets(cx);self.selected=self.selected.min(targets.len().saturating_sub(1));
        let visible=((f32::from(window.viewport_size().height)-205.)/38.).clamp(1.,8.)as usize;
        let page=self.selected/visible;let total=targets.len();
        div().v_flex().gap_2().min_w_0()
            .capture_action(cx.listener(|s,_:&gpui_kit::base::input::Enter,w,cx|{s.choose(None,w,cx);w.prevent_default();cx.stop_propagation();}))
            .capture_action(cx.listener(|s,_:&gpui_kit::base::input::MoveDown,w,cx|s.step(true,w,cx)))
            .capture_action(cx.listener(|s,_:&gpui_kit::base::input::MoveUp,w,cx|s.step(false,w,cx)))
            .child(Input::new(&self.input))
            .when(total==0,|el|el.child(div().p_2().text_color(rgb(theme::MUTED)).child("No matching open channels")))
            .children(targets.into_iter().enumerate().skip(page*visible).take(visible).map(|(index,target)|{
                div().id(("switch-channel",index)).h_flex().h(px(36.)).px_2().gap_2().min_w_0().rounded(px(4.)).cursor_pointer()
                    .bg(rgb(if index==self.selected{theme::HOVER}else{theme::PANEL})).hover(|s|s.bg(rgb(theme::HOVER)))
                    .child(stream_marker(&target.channel,target.live))
                    .child(div().v_flex().flex_1().min_w_0().child(div().truncate().child(format!("#{}",target.channel))).child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).truncate().child(target.workspace_name.clone())))
                    .when(target.unread>0,|el|el.child(div().text_size(px(11.)).text_color(rgb(0xC5B8E6)).child(crate::attention::count(target.unread))))
                    .on_click(cx.listener(move|s,_,w,cx|s.choose(Some(target.clone()),w,cx)))
            }))
            .child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child(format!("{total} channels · ↑ ↓ select · Enter opens · Esc closes")))
    }
}
impl Workbench {
    pub(super) fn open_switcher(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if window.has_active_dialog(cx){return;}
        let owner=cx.entity().downgrade();let view=cx.new(|cx|{
            let input=cx.new(|cx|InputState::new(window,cx).placeholder("Find channel or workspace…"));
            cx.subscribe_in(&input,window,|s:&mut Switcher,_,event,_,cx|{match event{InputEvent::Change=>{s.selected=0;cx.notify();},_=>{}}}).detach();
            Switcher{owner,input,selected:0}
        });
        let focus=view.read(cx).input.read(cx).focus_handle(cx);let body=view.clone();
        window.open_dialog(cx,move|dialog,w,_|dialog.w(px((f32::from(w.viewport_size().width)-24.).clamp(180.,440.))).title("Switch channel").child(body.clone()));
        focus.focus(window,cx);
    }
}
