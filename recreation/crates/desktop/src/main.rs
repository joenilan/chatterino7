use gpui_kit::prelude::FluentBuilder;
mod auth;
mod chat_text;
mod chat_search;
mod message_actions;
mod replies;
mod rich_messages;
mod attention;
mod control;
mod live;
mod media;
mod catalog;
mod community;
mod twitch_assets;
mod emote_picker;
mod inline_chat;
mod storage;
mod theme;
mod workspace;
mod dock;
mod stream_status;
use workspace::Workbench;

use chat_core::{Timeline, selection::Selection};
use chat_text::ChatText;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    menu::ContextMenuExt,
    Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::Button,
    input::{InputEvent, Textarea, TextareaState},
    message_scroller::{MessageScroller, MessageScrollerState},
    notification::{Notification, NotificationDelivery},
};
use gpui_kit::*;
use std::{cell::{Cell, RefCell}, rc::Rc, time::{Duration, Instant}};

gpui_kit::actions!(
    chat_workbench,
    [CopyChatSelection, ClearChatSelection, SelectAllChat, CompleteEmote, FindChat, NextChatMatch, PreviousChatMatch]
);

#[derive(Clone)]
struct DraggedChannel {
    pane: Entity<ChannelPane>,
    name: String,
}
struct DragPreview(String);
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(rgb(theme::PANEL))
            .text_color(rgb(theme::TEXT))
            .border_1()
            .border_color(rgb(0xA99CF4))
            .rounded(px(5.))
            .child(self.0.clone())
    }
}
struct CopyFeedback;
struct ComposerFeedback;
#[derive(Clone)]
struct MessageEntrance {
    row: u64,
    received: Instant,
    started: Rc<Cell<Option<Instant>>>,
}
impl MessageEntrance {
    fn active(&self, now: Instant) -> bool {
        self.started.get().map_or_else(
            || now.duration_since(self.received) < Duration::from_secs(1),
            |start| now.duration_since(start) < Duration::from_millis(260),
        )
    }
}
fn twitch_message_text(text: &str) -> String {
    text.replace("\r\n", " ")
        .replace(['\r', '\n', '\u{2028}', '\u{2029}'], " ")
}
#[derive(Clone)]
enum PaneEvent {
    DragStarted,
    Send { request: u64, text: String, reply_parent: Option<String> },
    Close,
    DraftChanged,
}
impl EventEmitter<PaneEvent> for ChannelPane {}

struct ChannelPane {
    name: SharedString,
    connection: String,
    connected: bool,
    pending: Option<(u64, String, Option<String>, u64)>,
    compose_revision: u64,
    reply_target: Option<replies::Target>,
    attention: attention::Attention,
    read_eligible: Rc<Cell<bool>>,
    presented_row: Rc<Cell<Option<u64>>>,
    send_status: String,
    timeline: Rc<RefCell<Timeline>>,
    media: Rc<RefCell<media::MediaCache>>,
    catalog: Rc<RefCell<catalog::Catalog>>,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    draft: Entity<TextareaState>,
    font_size: f32,
    picker: emote_picker::Picker,
    search: chat_search::Search,
    link_press: message_actions::PressState,
    emote_search: Entity<TextareaState>,
    last_copy_result: Option<&'static str>,
    scroller: Entity<MessageScrollerState>,
    next_id: usize,
    entrances: Vec<MessageEntrance>,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
}
impl ChannelPane {
    fn new(
        name: &str,
        media: Rc<RefCell<media::MediaCache>>,
        catalog: Rc<RefCell<catalog::Catalog>>,
        saved_draft: &str,
        font_size: f32,
        history_limit: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let timeline = Timeline::new(name, history_limit);
        let draft = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .submit_on_enter(true)
                .placeholder("Write a message…");
            input.set_value(twitch_message_text(saved_draft), window, cx);
            input
        });
        cx.subscribe_in(&draft, window, |this: &mut Self, _, event, _window, cx| {
            match event {
                InputEvent::Change => { this.compose_revision=this.compose_revision.wrapping_add(1); this.complete_query(false,cx); cx.emit(PaneEvent::DraftChanged); },
                _ => {}
            }
            cx.notify();
        })
        .detach();
        let emote_search=cx.new(|cx|TextareaState::new(window,cx).auto_grow(1,1).placeholder("Search emotes…"));
        cx.subscribe_in(&emote_search,window,|this:&mut Self,_,event,_,cx|{if matches!(event,InputEvent::Change){this.picker.page=0;this.refresh_picker(cx);cx.notify();}}).detach();
        let search=chat_search::create(window,cx);
        let scroller = cx.new(|cx| MessageScrollerState::new(timeline.messages().len(), cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();
        Self {
            name: name.to_owned().into(),
            media,
            catalog,
            connection: "Sign in to connect".into(),
            connected: false,
            pending: None,
            reply_target: None,
            attention: Default::default(),
            read_eligible: Rc::new(Cell::new(false)),
            presented_row: Rc::new(Cell::new(None)),
            compose_revision: 0,
            send_status: String::new(),
            timeline: Rc::new(RefCell::new(timeline)),
            selection: Rc::new(RefCell::new(Selection::default())),
            focus: cx.focus_handle(),
            draft,
            font_size,
            picker: emote_picker::Picker::default(),
            emote_search,
            search,
            link_press: Default::default(),
            last_copy_result: None,
            scroller,
            next_id: 0,
            entrances: Vec::new(),
            viewport: Rc::new(RefCell::new(None)),
        }
    }
    fn change_history_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        self.search.dirty=true;
        let removed=self.timeline.borrow_mut().set_capacity(limit);
        if removed>0 {
            let first=(self.next_id-self.timeline.borrow().messages().len()) as u64;
            let mut selection=self.selection.borrow_mut();
            if selection.anchor.is_some_and(|p|p.row<first)||selection.head.is_some_and(|p|p.row<first){selection.clear();}
            drop(selection);
            self.entrances.retain(|e|e.row>=first);
            self.scroller.update(cx,|s,cx|s.splice(0..removed,0,cx));
        }
        self.attention.reconcile(&self.timeline.borrow(),(self.next_id-self.timeline.borrow().messages().len()) as u64);
        cx.notify();
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        let text = twitch_message_text(&self.draft.read(cx).value());
        if !self.connected || text.trim().is_empty() || text.chars().count() > 500 {
            window.push_notification(
                Notification::info(if !self.connected {
                    "Connect this channel before sending. Your draft is saved."
                } else {
                    "Write a message of 1–500 characters."
                })
                .id::<ComposerFeedback>()
                .delivery(NotificationDelivery::InApp),
                cx,
            );
            return;
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let request = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.reply_target.as_ref().is_some_and(|target| target.deleted) {
            window.push_notification(Notification::info("The reply target was deleted. Cancel the reply or choose another message; your draft is kept."),cx);
            return;
        }
        let reply_parent=self.reply_target.as_ref().map(|target|target.id.clone());
        self.pending = Some((request, text.clone(), reply_parent.clone(), self.compose_revision));
        self.send_status = "Sending…".into();
        cx.emit(PaneEvent::Send { request, text, reply_parent });
        cx.notify();
    }
    fn received(&mut self, mut event: chat_core::Event, cx: &mut Context<Self>) {
        if let chat_core::Event::Message(message) = &mut event {
            message.fragments = self.catalog.borrow().expand(&self.name, &message.fragments);
            message.name_color = Some(theme::readable_name_color(&message.user_id, message.name_color));
        }
        self.observe_reply_redaction(&event,cx);
        let change = self.timeline.borrow_mut().apply(event);
        match change {
            chat_core::Change::Appended { evicted } => {
                let now = Instant::now();
                self.entrances.retain(|entry| entry.active(now));
                if self.scroller.read(cx).is_following_tail() && !cx.reduce_motion() {
                    // Keep animating the newest visible arrivals even in busy chat.
                    // Retire the oldest effect rather than disabling all motion.
                    if self.entrances.len() >= 12 { self.entrances.remove(0); }
                    self.entrances.push(MessageEntrance {
                        row: self.next_id as u64,
                        received: now,
                        started: Rc::new(Cell::new(None)),
                    });
                }
                self.next_id += 1;
                { let timeline=self.timeline.borrow();
                  if let Some(message)=timeline.messages().back(){let first=(self.next_id-timeline.messages().len()) as u64;self.search.appended(message,(self.next_id-1) as u64,first);self.attention.appended(message,(self.next_id-1) as u64,first);} }

                self.scroller.update(cx, |scroller, cx| {
                    if evicted {
                        scroller.splice(0..1, 0, cx);
                    }
                    scroller.append(1, cx);
                });
            }
            chat_core::Change::Updated => {
                self.attention.reconcile(&self.timeline.borrow(),(self.next_id-self.timeline.borrow().messages().len()) as u64);
                self.search.dirty=true;
                self.selection.borrow_mut().clear();
                self.scroller.update(cx, |s, cx| s.remeasure(cx));
            }
            chat_core::Change::Ignored => return,
        }
        cx.notify();
    }
    fn sent(
        &mut self,
        request: u64,
        result: &Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((pending, text, parent, revision)) = &self.pending else {
            return;
        };
        if *pending != request {
            return;
        }
        let clear_reply=result.is_ok() && self.compose_revision==*revision && twitch_message_text(&self.draft.read(cx).value()) == *text && self.reply_target.as_ref().map(|r|&r.id)==parent.as_ref();
        if clear_reply {self.reply_target=None;}
        if clear_reply {
            self.draft
                .update(cx, |draft, cx| draft.set_value("", window, cx));
            cx.emit(PaneEvent::DraftChanged);
        }
        if clear_reply {cx.emit(PaneEvent::DraftChanged);}
        self.send_status = match result {
            Ok(()) => "Sent".into(),
            Err(e) => e.clone(),
        };
        self.pending = None;
        cx.notify();
    }
    fn selected_text(&self) -> String {
        let timeline = self.timeline.borrow();
        let first = self.next_id - timeline.messages().len();
        let lines: Vec<_> = timeline
            .messages()
            .iter()
            .map(|message| message.copy_line())
            .collect();
        let text = self.selection.borrow().copy(
            lines
                .iter()
                .enumerate()
                .map(|(index, text)| ((first + index) as u64, text.as_str())),
        );
        text

    }
    fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text=self.selected_text();
        if text.is_empty() {
            self.last_copy_result = Some("empty_selection");
            window.push_notification(
                Notification::info("Select some chat text first")
                    .id::<CopyFeedback>()
                    .delivery(NotificationDelivery::InApp),
                cx,
            );
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        // A write has no Result. Confirm read-back before claiming success or
        // discarding the user's selection; a busy clipboard can be retried.
        if cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref()
            == Some(text.as_str())
        {
            self.last_copy_result = Some("clipboard_verified");
            self.selection.borrow_mut().clear();
            window.push_notification(
                Notification::success(format!("Copied · {} characters", text.chars().count()))
                    .id::<CopyFeedback>()
                    .delivery(NotificationDelivery::InApp),
                cx,
            );
        } else {
            self.last_copy_result = Some("clipboard_unverified");
            window.push_notification(
                Notification::warning(
                    "Clipboard copy could not be verified. Selection kept; try again.",
                )
                .id::<CopyFeedback>()
                .delivery(NotificationDelivery::InApp),
                cx,
            );
        }
        cx.notify();
        window.refresh();
    }
}
impl Render for ChannelPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picker = self.picker.open.then(||self.render_picker(cx));
        let suggestions = (!self.picker.suggestions.is_empty()&&!self.picker.open).then(||self.render_suggestions(cx));
        let catalog=self.catalog.clone();
        let timeline = self.timeline.clone();
        let media = self.media.clone();
        let retained = timeline.borrow().messages().len();
        let first_order = (self.next_id - retained) as u64;
        self.search.refresh(&timeline.borrow(),first_order);
        let search_hits=if self.search.open{self.search.hits.clone()}else{Rc::default()};
        let highlight_rows=self.attention.highlights.clone();
        let presented_row=self.presented_row.clone();let read_eligible=self.read_eligible.clone();
        let search_current=self.search.open.then_some(self.search.current).flatten();
        let search_bar=self.search.open.then(||self.render_search(cx));
        let selection = self.selection.clone();
        let focus = self.focus.clone();
        let viewport = self.viewport.clone();
        let viewport_layout = self.viewport.clone();
        let finish = self.selection.clone();
        let copy_owner = cx.weak_entity();
        let menu_owner = cx.weak_entity();
        let link_press=self.link_press.clone();
        let finish_link=self.link_press.clone();
        let copy_viewport = self.viewport.clone();
        let following = self.scroller.read(cx).is_following_tail();
        let now = Instant::now();
        self.entrances.retain(|entry| entry.active(now));
        let entrances = if following && !cx.reduce_motion()
            && { let selection = self.selection.borrow(); !selection.dragging && selection.anchor == selection.head }
        { self.entrances.clone() } else { Vec::new() };
        div()
            .id("channel-pane")
            .key_context("JawjackChannel")
            .capture_action(cx.listener(|this,_:&gpui_kit::base::input::Search,w,cx|{this.open_search(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&FindChat,w,cx|{this.open_search(w,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&NextChatMatch,_,cx|{this.next_search(true,cx);cx.stop_propagation();}))
            .on_action(cx.listener(|this,_:&PreviousChatMatch,_,cx|{this.next_search(false,cx);cx.stop_propagation();}))
            .v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(rgb(theme::CANVAS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .rounded(px(10.))
            .overflow_hidden()
            .child(
                div()
                    .h_flex()
                    .h(px(26.))
                    .px_2()
                    .gap_2()
                    .bg(rgb(theme::PANEL))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(13.))
                            .id("channel-drag-title")
                            .cursor(CursorStyle::OpenHand)
                            .on_drag(DraggedChannel { pane: cx.entity(), name: self.name.to_string() }, |drag, _, _, cx| {
                                drag.pane.update(cx, |_,cx|cx.emit(PaneEvent::DragStarted));
                                cx.stop_propagation();
                                cx.new(|_| DragPreview(format!("# {}", drag.name)))
                            })
                            .child(format!("# {}", self.name)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::MUTED))
                            .child(if self.connected { format!("{retained}/{} · {}", self.timeline.borrow().capacity(), if following { "Latest" } else { "History" }) } else { self.connection.clone() }),
                    )
                    .child(Button::new("find-chat").xsmall().label("⌕").tooltip("Find in chat · Ctrl+F").on_click(cx.listener(|this,_,w,cx|this.open_search(w,cx))))
                    .child(Button::new("close-pane").xsmall().label("×").tooltip("Close this split; keep draft")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PaneEvent::Close)))),
            )
            .children(search_bar)
            .child(
                div()
                    .id("transcript")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
            .track_focus(&self.focus)
            .key_context("ChatTranscript")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.focus.focus(window, cx);
                        this.selection.borrow_mut().clear();
                        cx.notify();
                    }))
                    .text_size(px(self.font_size))
                    .line_height(px(self.font_size + 8.))
            .on_action(cx.listener(|this, _: &CopyChatSelection, window, cx| {
                this.copy(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &ClearChatSelection, window, cx| {
                if this.search.open {this.close_search(window,cx);}else{this.selection.borrow_mut().clear();}
                cx.notify();
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &SelectAllChat, _, cx| {
                let timeline = this.timeline.borrow();
                let first = this.next_id - timeline.messages().len();
                if let Some(last) = timeline.messages().back() {
                    this.selection.borrow_mut().select_span(
                        chat_core::selection::Point { row: first as u64, byte: 0 },
                        chat_core::selection::Point { row: (this.next_id - 1) as u64, byte: last.copy_line().len() },
                    );
                }
                cx.notify();
                cx.stop_propagation();
            }))
                    .on_prepaint(move |bounds, _, _| {
                        *viewport_layout.borrow_mut() = Some(bounds);
                    })
                    .when(retained == 0, |el| el.child(div().v_flex().p_6().gap_2().text_color(rgb(theme::MUTED))
                        .child(div().text_color(rgb(theme::TEXT)).text_size(px(16.)).child(format!("#{} is ready", self.name)))
                        .child(if self.connected { "Connected. Waiting for messages…".to_string() } else { self.connection.clone() })))
                    .child(
                        MessageScroller::new("chat", self.scroller.clone(), move |index, window, cx| {
                            let messages = timeline.borrow();
                            let Some(message) = messages.messages().get(index) else {
                                return div().into_any_element();
                            };
                            let row = first_order + index as u64;
                            let inline = inline_chat::InlineChat::new(message, row, &mut media.borrow_mut(), selection.clone(), focus.clone(), viewport.clone(), cx);
                            let first_line_height = if inline.is_some() { inline_chat::InlineChat::line_height(window,message) } else { window.line_height() };
                            let row_owner=menu_owner.clone();let row_id=message.id.clone();
                            let rich_heading=rich_messages::heading(message);
                            let rich_attachments=rich_messages::attachments(message,&mut media.borrow_mut(),cx);
                            let reply_line=replies::row_context(message,&messages,menu_owner.clone());
                            let interaction=message_actions::Interaction{owner:menu_owner.clone(),message:message.id.clone(),author_len:message.display_name.len(),links:message_actions::message_links(message),pressed:link_press.clone()};
                            let presented=presented_row.clone();let eligible=read_eligible.clone();
                            let matches=search_hits.get(&row).cloned().unwrap_or_default();
                            let progress = entrances.iter().find(|entry| entry.row == row)
                                .map(|entry| {
                                    let now = Instant::now();
                                    let start = entry.started.get().unwrap_or_else(|| {
                                        entry.started.set(Some(now));
                                        now
                                    });
                                    (now.duration_since(start).as_secs_f32() / 0.26).clamp(0.0, 1.0)
                                }).unwrap_or(1.0);
                            if progress < 1.0 { window.request_animation_frame(); }
                            let eased = 1.0 - (1.0 - progress).powi(3);
                            div()
                                .relative()
                                .border_l_2().border_color(rgba(0x00000000))
                                .when_some(rich_messages::accent(message),|el,color|el.border_color(rgb(color)).bg(rgba(0xA99CF40D)))
                                .when(highlight_rows.contains(&row),|el|el.bg(rgb(0x272237)).border_color(rgb(0xA99CF4)))
                                .on_prepaint(move|bounds,window,cx|{
                                    let clip=window.content_mask().bounds;
                                    if following&&eligible.get()&&window.is_window_active()&&!window.has_active_dialog(cx)
                                        && bounds.origin.x>=clip.origin.x&&bounds.bottom()>clip.origin.y
                                        && bounds.right()<=clip.right()&&bounds.bottom()<=clip.bottom(){
                                        presented.set(Some(presented.get().map_or(row,|old|old.max(row))));
                                    }
                                })
                                .when(search_current==Some(row),|el|el.bg(rgb(0x272331)))
                                .left(px(12.0 * (1.0 - eased)))
                                .top(px(6.0 * (1.0 - eased)))
                                .opacity(0.25 + 0.75 * eased)
                                .id(SharedString::from(message.id.clone()))
                                .min_w_0()
                                .cursor_text()
                                .tooltip(|w,cx|gpui_kit::component::tooltip::Tooltip::new("Ctrl+click a link to open, or a username to inspect · Right-click for actions").build(w,cx))
                                .context_menu(move|menu,_,cx|message_actions::menu(row_owner.clone(),row_id.clone(),menu,cx))
                                .children(rich_heading)
                                .children(reply_line)
                                .child(div().h_flex().items_start().min_w_0().gap_1()
                                .children(message.badges.iter().filter_map(|badge|catalog.borrow().twitch.badge(&message.channel_id,&badge.set_id,&badge.id).cloned()).map(|badge|{
                                    let image=media.borrow_mut().get(&badge.key,cx);
                                    div().id(SharedString::from(badge.key.id.clone())).w(px(18.)).h(first_line_height).flex().items_center().flex_shrink_0().overflow_hidden().tooltip(move|w,cx|gpui_kit::component::tooltip::Tooltip::new(badge.title.clone()).build(w,cx)).child(emote_picker::icon(image,String::new(),18.,18.))
                                }).collect::<Vec<_>>())
                                .child(div().flex_1().min_w_0().child(if let Some(inline) = inline {
                                    inline.with_search(matches).with_interaction(interaction).into_any_element()
                                } else {
                                    ChatText::new(SharedString::from(format!("text-{}", message.id)), row, message.copy_line(), selection.clone(), focus.clone(), viewport.clone())
                                        .with_author(&message.display_name, message.name_color).with_search(matches).with_interaction(interaction).into_any_element()
                                })))
                                .children(rich_attachments)
                                .into_any_element()
                        })
                        .flex_1()
                        .min_h_0()
                        .with_row_style(StyleRefinement::default().px_3().pb_0())
                        .with_list_style(StyleRefinement::default().py_2())
                        .h_full(),
                    ),
            )
            .child(div().key_context("JawjackComposer").v_flex().p_1().gap_1().bg(rgb(theme::PANEL)).border_t_1().border_color(rgb(theme::BORDER))
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if this.picker.open { return; }
                    if event.keystroke.key == "enter" && event.keystroke.modifiers.shift {
                        window.prevent_default();
                        cx.stop_propagation();
                    } else if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform {
                        if let Some(text) = &event.keystroke.key_char {
                            let input = this.draft.read(cx);
                            let current = input.value().to_string();
                            let selected = input.selected_range();
                            let count = current[..selected.start].chars().count() + text.chars().count() + current[selected.end..].chars().count();
                            if count > 500 { window.prevent_default(); cx.stop_propagation(); }
                        }
                    }
                }))
                .capture_action(cx.listener(|this, action: &gpui_kit::base::input::Enter, window, cx| {
                    // Enter has exactly one semantic owner. Never also submit from
                    // InputEvent::PressEnter after inserting a completion.
                    if !action.shift {
                        if this.picker.open {
                            if let Some(choice)=this.picker.choices.get(this.picker.page*40).cloned(){this.insert_emote(&choice,false,window,cx);}
                        } else if !this.completion_action("enter",window,cx) {this.submit(window,cx);}
                    }
                    cx.stop_propagation();
                }))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::MoveDown,window,cx|{this.completion_action("down",window,cx);}))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::MoveUp,window,cx|{this.completion_action("up",window,cx);}))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::IndentInline,window,cx|{this.completion_action("tab",window,cx);}))
                .capture_action(cx.listener(|this,_: &CompleteEmote,window,cx|{
                    if !this.draft.read(cx).focus_handle(cx).is_focused(window)||!this.completion_action("tab",window,cx){window.focus_next(cx);}
                    cx.stop_propagation();
                }))
                .capture_action(cx.listener(|this,_: &gpui_kit::base::input::Escape,window,cx|{
                    if this.picker.open {this.toggle_picker(window,cx);cx.stop_propagation();}else if !this.completion_action("escape",window,cx) && this.reply_target.is_some(){this.cancel_reply(window,cx);cx.stop_propagation();}
                }))
                .capture_action(cx.listener(|this, _: &gpui_kit::base::input::Paste, window, cx| {
                    if this.picker.open {return;}
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = twitch_message_text(&text);
                        let input = this.draft.read(cx);
                        let current = input.value().to_string();
                        let selected = input.selected_range();
                        let count = current[..selected.start].chars().count() + text.chars().count() + current[selected.end..].chars().count();
                        if count <= 500 {
                            this.draft.update(cx, |input, cx| input.replace(text, window, cx));
                            cx.emit(PaneEvent::DraftChanged);
                        } else {
                            window.push_notification(Notification::info("That paste would exceed Twitch’s 500-character limit. Your draft is unchanged.").id::<ComposerFeedback>().delivery(NotificationDelivery::InApp), cx);
                        }
                        cx.stop_propagation();
                    }
                }))
                .children(self.render_reply_target(cx))
                .children(picker)
                .children(suggestions)
                .child(Textarea::new(&self.draft))
                .when(!self.send_status.is_empty(), |el|el.child(div().text_size(px(11.)).text_color(rgb(theme::MUTED)).child(self.send_status.clone())))
                .child(div().h_flex().justify_between().text_size(px(11.)).text_color(rgb(theme::MUTED))
                    .child(div().when(self.draft.read(cx).value().chars().count() > 500, |el|el.text_color(rgb(0xF29D9D))).child(format!("{} / 500", self.draft.read(cx).value().chars().count())))
                    .child(div().h_flex().gap_2().child(Button::new("emotes-toggle").small().label(":)").tooltip("Emotes · type :name or press Tab to complete").on_click(cx.listener(|this,_,w,cx|this.toggle_picker(w,cx))))
                    .child(Button::new("send").small().label(if self.pending.is_some() { "Sending…" } else { "Send" }).disabled(!self.connected || self.pending.is_some()).tooltip("Send to this Twitch channel").on_click(cx.listener(|this,_,window,cx|this.submit(window,cx)))))))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let copy_owner = copy_owner.clone();
                        let copy_viewport = copy_viewport.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase.capture() && event.button == MouseButton::Right && !window.has_active_dialog(cx)
                                && copy_viewport.borrow().is_some_and(|bounds| bounds.contains(&event.position))
                            {
                                let copied=copy_owner.update(cx, |pane, cx| {
                                    if pane.selected_text().is_empty(){return false;}
                                    pane.focus.focus(window,cx);pane.copy(window,cx);true
                                }).unwrap_or(false);
                                if copied{window.prevent_default();cx.stop_propagation();}
                            }
                        });
                        let finish = finish.clone();
                        let finish_link=finish_link.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase.capture() && event.button == MouseButton::Left {
                                finish.borrow_mut().finish();
                                let link=finish_link.clone();window.defer(cx,move|_,_|{link.borrow_mut().take();});
                                window.refresh();
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

fn caption_control(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    close: bool,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(32.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(theme::MUTED))
        .hover(move |style| {
            style
                .bg(rgb(if close { 0xC42B1C } else { theme::HOVER }))
                .text_color(rgb(theme::TEXT))
        })
        .window_control_area(area)
        .child(Icon::new(icon).small())
}
const MIN_WINDOW_WIDTH: f32 = 360.;
const MIN_WINDOW_HEIGHT: f32 = 280.;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--control-call") {
        let result = if args.len() == 3 {
            control::client(&args[1], &args[2])
        } else {
            Err("Usage: --control-call SESSION JSON".into())
        };
        match result {
            Ok(value) => println!("{value}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    let server = if args.is_empty() {
        None
    } else if args.len() == 2 && args[0] == "--control-session" {
        match control::start(&args[1]) {
            Ok(server) => Some(server),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Usage: chat-workbench [--control-session NEW-NAME]");
        std::process::exit(1);
    };

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_app_identity("digital.zombie.jawjack", "Jawjack");
            workspace::bind_keys(cx);
            theme::install(cx);
            cx.bind_keys([
                KeyBinding::new("ctrl-f", FindChat, Some("JawjackChannel")),
                KeyBinding::new("cmd-f", FindChat, Some("JawjackChannel")),
                KeyBinding::new("f3", NextChatMatch, Some("JawjackChannel")),
                KeyBinding::new("shift-f3", PreviousChatMatch, Some("JawjackChannel")),
                KeyBinding::new("shift-enter", PreviousChatMatch, Some("JawjackSearch")),
                KeyBinding::new("tab", CompleteEmote, Some("JawjackComposer")),
                KeyBinding::new("ctrl-c", CopyChatSelection, Some("ChatTranscript")),
                KeyBinding::new("cmd-c", CopyChatSelection, Some("ChatTranscript")),
                KeyBinding::new("ctrl-insert", CopyChatSelection, Some("ChatTranscript")),
                KeyBinding::new("escape", ClearChatSelection, Some("ChatTranscript")),
                KeyBinding::new("ctrl-a", SelectAllChat, Some("ChatTranscript")),
                KeyBinding::new("cmd-a", SelectAllChat, Some("ChatTranscript")),
            ]);
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Jawjack".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(820.)),
                    cx,
                ))),
                window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT))),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let entity = cx.new(|cx| Workbench::new(server.is_some(), window, cx));
                entity.update(cx, |view, cx| view.focus_workspace(window, cx));
                let closing = entity.downgrade();
                window.on_window_should_close(cx, move |window, app| {
                    closing
                        .update(app, |view, cx| view.request_close(window, cx))
                        .unwrap_or(true)
                });
                if let Some(server) = server {
                    control::attach(server, &entity, window, cx);
                }
                entity
            })
            .expect("Could not open the Jawjack window");
        });
}
