//! Split-pane navigation uses the current visible geometry, not hidden channel tabs.
use super::*;
impl Workbench {
    fn pane_navigation_allowed(&self,window:&mut Window,cx:&mut App)->bool {
        !self.adding && !self.settings && !window.has_active_dialog(cx) && !cx.has_active_drag()
    }
    fn focused_pane_name(&self,window:&Window,cx:&App)->Option<String>{
        let panes=self.visible_panes(cx);
        panes.iter().find(|p|{let p=p.read(cx);
            p.focus.contains_focused(window,cx) || p.draft.read(cx).focus_handle(cx).is_focused(window)
                || p.search.input.read(cx).focus_handle(cx).is_focused(window)
                || p.emote_search.read(cx).focus_handle(cx).is_focused(window)
        }).map(|p|p.read(cx).name.to_string())
            .or_else(||self.selected_channel.clone().filter(|n|panes.iter().any(|p|p.read(cx).name.as_ref()==n)))
            .or_else(||panes.first().map(|p|p.read(cx).name.to_string()))
    }
    fn focus_pane_named(&mut self,name:&str,window:&mut Window,cx:&mut Context<Self>){
        let panes=self.visible_panes(cx);
        let Some(target)=panes.iter().find(|p|p.read(cx).name.as_ref()==name).cloned()else{return;};
        // Dismiss transient completion/pickers, preserving drafts, searches and reply targets.
        for pane in &panes {pane.update(cx,|p,cx|{p.picker.open=false;p.picker.suggestions.clear();p.picker.token=None;cx.notify();});}
        self.selected_channel=Some(name.to_owned());
        target.read(cx).draft.clone().update(cx,|input,cx|input.focus(window,cx));
        cx.notify();
    }
    pub(super) fn cycle_pane(&mut self,forward:bool,window:&mut Window,cx:&mut Context<Self>){
        self.cycle_pane_from(forward,None,window,cx);
    }
    pub(super) fn cycle_pane_from(&mut self,forward:bool,origin:Option<String>,window:&mut Window,cx:&mut Context<Self>){
        if !self.pane_navigation_allowed(window,cx){return;}
        let panes=self.visible_panes(cx);if panes.is_empty(){return;}
        let current=origin.or_else(||self.focused_pane_name(window,cx));
        let at=panes.iter().position(|p|Some(p.read(cx).name.as_ref())==current.as_deref()).unwrap_or(0);
        let next=if forward{(at+1)%panes.len()}else{(at+panes.len()-1)%panes.len()};
        let name=panes[next].read(cx).name.to_string();self.focus_pane_named(&name,window,cx);
        cx.stop_propagation();
    }
    pub(super) fn focus_neighbor(&mut self,dx:i32,dy:i32,window:&mut Window,cx:&mut Context<Self>){
        if !self.pane_navigation_allowed(window,cx){return;}
        let Some(current)=self.focused_pane_name(window,cx)else{return;};
        let panes=self.visible_panes(cx);
        let next={
            let geometry=self.dock_bounds.borrow();let Some(origin)=geometry.get(&current)else{return;};
            let ox=f32::from(origin.origin.x);let oy=f32::from(origin.origin.y);
            let ow=f32::from(origin.size.width);let oh=f32::from(origin.size.height);
            let mut candidates=Vec::new();
            for pane in panes {let name=pane.read(cx).name.to_string();if name==current{continue;}
                let Some(b)=geometry.get(&name)else{continue;};
                let x=f32::from(b.origin.x);let y=f32::from(b.origin.y);let w=f32::from(b.size.width);let h=f32::from(b.size.height);
                let horizontal=dx!=0;let delta=if horizontal{(x+w/2.-ox-ow/2.)*dx as f32}else{(y+h/2.-oy-oh/2.)*dy as f32};
                if delta<=0.5{continue;}
                let overlap=if horizontal{(y+h).min(oy+oh)-y.max(oy)}else{(x+w).min(ox+ow)-x.max(ox)};
                let across=if horizontal{(y+h/2.-oy-oh/2.).abs()}else{(x+w/2.-ox-ow/2.).abs()};
                candidates.push((overlap<=0.,delta,across,name));
            }
            candidates.sort_by(|a,b|a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.total_cmp(&b.2)).then(a.3.cmp(&b.3)));
            candidates.into_iter().next().map(|c|c.3)
        };
        if let Some(name)=next{self.focus_pane_named(&name,window,cx);}cx.stop_propagation();
    }
    pub(super) fn equalize_panes(&mut self,target:Option<String>,all:bool,window:&mut Window,cx:&mut Context<Self>){
        if !self.pane_navigation_allowed(window,cx){return;}
        let target=target.or_else(||self.focused_pane_name(window,cx));
        let Some(dock)=self.tabs[self.active].dock.as_mut()else{return;};
        let count=if all{dock.equalize_all()}else{usize::from(target.as_ref().is_some_and(|n|dock.equalize_nearest(n)))};
        if count==0{window.push_notification(Notification::info("This pane is not split"),cx);return;}
        self.tabs[self.active].dock_states.clear();self.schedule_save(cx);cx.notify();
        window.push_notification(Notification::info(if all{"Split proportions equalized"}else{"Current split equalized"}),cx);
    }
}
