//! Search retained timelines in small UI-thread slices without a copied message log.
use super::*;
use crate::chat_search::Query;
use std::time::Instant;

#[derive(Clone)]
struct Hit { pane: WeakEntity<ChannelPane>, id: String, channel: String, workspace: String }
struct Cursor { pane: WeakEntity<ChannelPane>, channel: String, workspace: String, next: usize, first: usize }
struct WorkspaceSearch {
    owner: WeakEntity<Workbench>, input: Entity<InputState>, query: Option<Query>,
    requested: String, hits: BTreeMap<(i64, std::cmp::Reverse<usize>), Hit>, cursors: Vec<Cursor>,
    cursor: usize, scanned: usize, matches: usize, selected: usize, running: bool,
    dirty: bool, error: Option<String>, generation: u64, serial: usize,
    task: Option<Task<()>>, _observers: Vec<Subscription>,
}
impl WorkspaceSearch {
    fn begin(&mut self, cx: &mut Context<Self>) {
        self.task = None; self.generation += 1; self.hits.clear(); self.cursors.clear();
        self.scanned = 0; self.matches = 0; self.selected = 0; self.cursor = 0;
        self.serial = 0; self.error = None; self.dirty = false; self.running = false;
        self.requested = self.input.read(cx).value().trim().to_owned();
        self.query = Some(Query::parse(&self.requested));
        self.error = self.query.as_ref().and_then(|q|q.error().map(str::to_owned));
        if self.requested.is_empty() || self.error.is_some() { cx.notify(); return; }
        let Some(owner) = self.owner.upgrade() else { return; };
        for tab in &owner.read(cx).tabs { for pane in &tab.panes {
            let p = pane.read(cx);
            self.cursors.push(Cursor { pane: pane.downgrade(), channel:p.name.to_string(),
                workspace:tab.name.clone(), next:p.next_id, first:p.next_id-p.timeline.borrow().messages().len() });
        }}
        self.running = !self.cursors.is_empty();
        let generation = self.generation;
        self.task = Some(cx.spawn(async move |view,cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let done = view.update(cx, |this,cx| {
                    if this.generation != generation { return true; }
                    this.scan_slice(cx); !this.running
                }).unwrap_or(true);
                if done { break; }
            }
        }));
        cx.notify();
    }
    fn scan_slice(&mut self, cx:&mut Context<Self>) {
        let start=Instant::now(); let mut visited=0;
        let open=self.owner.upgrade().map(|o|o.read(cx).panes()).unwrap_or_default();
        // Round-robin gives each pane a turn, including hidden channel tabs.
        while !self.cursors.is_empty() && visited<256 && start.elapsed()<Duration::from_millis(6) {
            self.cursor %= self.cursors.len();
            let cursor=&mut self.cursors[self.cursor];
            if cursor.next<=cursor.first { self.cursors.remove(self.cursor); continue; }
            cursor.next-=1; visited+=1;
            if let Some(pane)=cursor.pane.upgrade().filter(|p|open.contains(p)) {
                let p=pane.read(cx); let timeline=p.timeline.borrow();
                let first=p.next_id-timeline.messages().len();
                if let Some(index)=cursor.next.checked_sub(first) {
                    if let Some(message)=timeline.messages().get(index) {
                        self.scanned+=1;
                        if self.query.as_ref().is_some_and(|q|q.matches(message).is_some()) {
                            self.matches+=1; self.serial+=1;
                            self.hits.insert((message.sent_at.unwrap_or(i64::MIN),std::cmp::Reverse(self.serial)),Hit {
                                pane:cursor.pane.clone(), id:message.id.clone(),channel:cursor.channel.clone(),workspace:cursor.workspace.clone()
                            });
                            if self.hits.len()>200 { self.hits.pop_first(); }
                        }
                    } else { cursor.next=cursor.first; }
                } else { cursor.next=cursor.first; }
            } else { cursor.next=cursor.first; }
            self.cursor+=1;
        }
        self.running=!self.cursors.is_empty(); cx.notify();
    }
    fn choose(&mut self, hit:Hit, window:&mut Window, cx:&mut Context<Self>) {
        let Some(pane)=hit.pane.upgrade() else {self.stale(window,cx);return;};
        let valid={let p=pane.read(cx);let timeline=p.timeline.borrow();
            timeline.messages().iter().find(|m|m.id==hit.id).is_some_and(|m|self.query.as_ref().is_some_and(|q|q.matches(m).is_some()))};
        if !valid {self.stale(window,cx);return;}
        let changed=self.owner.update(cx,|owner,cx|{
            let Some(index)=owner.tabs.iter().position(|tab|tab.panes.contains(&pane)) else{return false;};
            let channel=pane.read(cx).name.to_string();
            window.close_dialog(cx); owner.select_tab(index,cx);
            if let Some(dock)=&mut owner.tabs[index].dock {dock.select(&channel);}
            owner.selected_channel=Some(channel);owner.schedule_save(cx);
            pane.update(cx,|p,cx| {
                p.set_search_query(self.requested.clone(),window,cx);
                p.open_search(window,cx);
                let timeline=p.timeline.borrow();
                if let Some(index)=timeline.messages().iter().position(|m|m.id==hit.id) {
                    p.search.current=Some((p.next_id-timeline.messages().len()+index) as u64);
                }
                drop(timeline);
                p.jump_message(&hit.id,window,cx);
            });cx.notify();true
        }).unwrap_or(false);
        if !changed {self.stale(window,cx);}
    }
    fn stale(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        window.push_notification(Notification::info("That result changed or left retained history. Search again to refresh."),cx);cx.notify();
    }
    fn step(&mut self,forward:bool,cx:&mut Context<Self>) {
        if self.running{return;}
        let len=self.hits.len();if len>0 {self.selected=if forward{(self.selected+1)%len}else{(self.selected+len-1)%len};}cx.notify();
    }
}
impl Render for WorkspaceSearch {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let open=self.owner.upgrade().map(|o|o.read(cx).panes()).unwrap_or_default();
        let rows=self.hits.values().rev().cloned().collect::<Vec<_>>();
        self.selected=self.selected.min(rows.len().saturating_sub(1));
        let visible=((f32::from(window.viewport_size().height)-270.)/74.).clamp(1.,7.)as usize;
        let page=self.selected/visible;
        let status=if let Some(error)=&self.error {error.clone()}
            else if self.dirty {"Query changed · Enter to search".into()}
            else if self.requested.is_empty(){"Search text, from:user, has:emote, quoted phrases or -excluded terms".into()}
            else{format!("{} snapshot matches · {} scanned{} · newest 200 shown",self.matches,self.scanned,if self.running{" · searching…"}else{""})};
        div().v_flex().gap_2().min_w_0()
            .capture_action(cx.listener(|s,_:&gpui_kit::base::input::Enter,w,cx|{
                if s.running {w.prevent_default();cx.stop_propagation();return;}
                if s.dirty || s.hits.is_empty() {s.begin(cx);}
                else if !s.running {if let Some(hit)=s.hits.values().rev().nth(s.selected).cloned(){s.choose(hit,w,cx);}}
                w.prevent_default();cx.stop_propagation();
            }))
            .capture_action(cx.listener(|s,_:&gpui_kit::base::input::MoveDown,_,cx|{s.step(true,cx);cx.stop_propagation();}))
            .capture_action(cx.listener(|s,_:&gpui_kit::base::input::MoveUp,_,cx|{s.step(false,cx);cx.stop_propagation();}))
            .child(div().h_flex().gap_1().min_w_0()
                .child(div().flex_1().min_w_0().child(Input::new(&self.input)))
                .child(Button::new("global-search-run").small().label(if self.running{"Restart"}else{"Search"}).on_click(cx.listener(|s,_,_,cx|s.begin(cx)))))
            .child(div().text_size(px(11.)).text_color(rgb(if self.error.is_some(){0xF0AAAA}else{theme::MUTED})).child(status))
            .when(!self.running&&!self.dirty&&self.error.is_none()&&!self.requested.is_empty()&&self.hits.is_empty(),|el|el.child(div().p_2().text_color(rgb(theme::MUTED)).child("No matches in currently retained chat. Try fewer filters or a different phrase.")))
            .children(rows.into_iter().enumerate().skip(page*visible).take(visible).map(|(index,hit)| {
                // Resolve only visible rows from current owners; never retain preview bodies.
                let preview=hit.pane.upgrade().filter(|p|open.contains(p)).and_then(|pane|{
                    let p=pane.read(cx);let timeline=p.timeline.borrow();
                    timeline.messages().iter().find(|m|m.id==hit.id)
                        .filter(|m|self.query.as_ref().is_some_and(|q|q.matches(m).is_some()))
                        .map(|m|(m.author_label().to_owned(),m.body().chars().take(220).collect::<String>()))
                });
                let valid=preview.is_some();let (author,body)=preview.unwrap_or_else(||("Result unavailable".into(),"Message changed, was pruned, or no longer matches.".into()));
                div().id(("global-hit",index)).v_flex().gap_1().p_2().min_w_0().rounded(px(4.))
                    .bg(rgb(if index==self.selected{theme::HOVER}else{theme::CONTROL}))
                    .child(div().h_flex().gap_1().min_w_0()
                        .child(div().flex_1().min_w_0().truncate().text_size(px(11.)).text_color(rgb(0xC9BAF7)).child(format!("#{} · {} · {author}",hit.channel,hit.workspace)))
                        .child(Button::new(("global-jump",index)).xsmall().label("Jump").disabled(!valid||self.dirty||self.running).on_click(cx.listener(move|s,_,w,cx|s.choose(hit.clone(),w,cx)))))
                    .child(div().text_size(px(12.)).max_h(px(36.)).overflow_hidden().child(body))
            }))
            .child(div().h_flex().justify_between().gap_1()
                .child(Button::new("global-results-previous").small().label("← Previous").disabled(page==0||self.running)
                    .on_click(cx.listener(move|s,_,_,cx|{s.selected=s.selected.saturating_sub(visible);cx.notify();})))
                .child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(if self.hits.is_empty(){"No results".into()}else{format!("{}–{} of {}",page*visible+1,((page+1)*visible).min(self.hits.len()),self.hits.len())}))
                .child(Button::new("global-results-next").small().label("Next →").disabled((page+1)*visible>=self.hits.len()||self.running)
                    .on_click(cx.listener(move|s,_,_,cx|{s.selected=(s.selected+visible).min(s.hits.len().saturating_sub(1));cx.notify();}))))
            .child(div().text_size(px(10.)).text_color(rgb(theme::MUTED)).child("Open channels only · ↑ ↓ select · Enter searches / jumps · Esc closes · Search refreshes arrivals"))
    }
}
impl Workbench {
    pub(super) fn open_workspace_search(&mut self,seed:Option<String>,window:&mut Window,cx:&mut Context<Self>) {
        if window.has_active_dialog(cx) {return;}
        let owner=cx.entity().downgrade();let panes=self.panes();
        let view=cx.new(|cx|{
            let input=cx.new(|cx|{
                let mut input=InputState::new(window,cx).placeholder("Search across open chats…").validate(|s,_|s.chars().count()<=256);
                if let Some(value)=&seed{input.set_value(value.clone(),window,cx);}input
            });
            cx.subscribe_in(&input,window,|s:&mut WorkspaceSearch,_,event,_,cx|{
                if matches!(event,InputEvent::Change){s.task=None;s.generation+=1;s.running=false;s.dirty=true;s.hits.clear();s.error=None;cx.notify();}
            }).detach();
            let mut observers=panes.iter().map(|pane|cx.observe(pane,|_,_,cx|cx.notify())).collect::<Vec<_>>();
            if let Some(view)=owner.upgrade(){observers.push(cx.observe(&view,|_,_,cx|cx.notify()));}
            WorkspaceSearch{owner,input,query:None,requested:String::new(),hits:BTreeMap::new(),cursors:vec![],cursor:0,scanned:0,matches:0,selected:0,running:false,dirty:false,error:None,generation:0,serial:0,task:None,_observers:observers}
        });
        let focus=view.read(cx).input.read(cx).focus_handle(cx);let body=view.clone();
        window.open_dialog(cx,move|dialog,w,_|dialog.w(px((f32::from(w.viewport_size().width)-24.).clamp(180.,640.))).title("Search open channels").child(body.clone()));
        focus.focus(window,cx);
        if seed.as_ref().is_some_and(|s|!s.trim().is_empty()){view.update(cx,|s,cx|s.begin(cx));}
    }
}
