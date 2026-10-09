use gpui_kit::prelude::FluentBuilder;
mod auth;
mod chat_text;
mod control;
mod storage;
mod theme;
mod workspace;
use workspace::Workbench;

use chat_core::{Timeline, selection::Selection};
use chat_text::ChatText;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::Button,
    input::{InputEvent, Textarea, TextareaState},
    message_scroller::{MessageScroller, MessageScrollerState},
    notification::{Notification, NotificationDelivery},
};
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc};

gpui_kit::actions!(
    chat_workbench,
    [CopyChatSelection, ClearChatSelection, SelectAllChat]
);

struct CopyFeedback;
#[derive(Clone)]
enum PaneEvent {
    Close,
    DraftChanged,
}
impl EventEmitter<PaneEvent> for ChannelPane {}

struct ChannelPane {
    name: SharedString,
    timeline: Rc<RefCell<Timeline>>,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    draft: Entity<TextareaState>,
    font_size: f32,
    last_copy_result: Option<&'static str>,
    scroller: Entity<MessageScrollerState>,
    next_id: usize,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
}
impl ChannelPane {
    fn new(
        name: &str,
        saved_draft: &str,
        font_size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let timeline = Timeline::new(name, 10_000);
        let draft = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .submit_on_enter(true)
                .placeholder("Write a message…");
            input.set_value(saved_draft.to_owned(), window, cx);
            input
        });
        cx.subscribe_in(&draft, window, |_: &mut Self, _, event, window, cx| {
            match event {
                InputEvent::Change => cx.emit(PaneEvent::DraftChanged),
                InputEvent::PressEnter { shift: false, .. } => {
                    window.push_notification(
                        Notification::info(
                            "Twitch is disconnected. Your draft is saved; nothing was sent.",
                        )
                        .id::<PaneEvent>()
                        .delivery(NotificationDelivery::InApp),
                        cx,
                    );
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();
        let scroller = cx.new(|cx| MessageScrollerState::new(timeline.messages().len(), cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();
        Self {
            name: name.to_owned().into(),
            timeline: Rc::new(RefCell::new(timeline)),
            selection: Rc::new(RefCell::new(Selection::default())),
            focus: cx.focus_handle(),
            draft,
            font_size,
            last_copy_result: None,
            scroller,
            next_id: 0,
            viewport: Rc::new(RefCell::new(None)),
        }
    }
    fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        drop(timeline);
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
        let timeline = self.timeline.clone();
        let retained = timeline.borrow().messages().len();
        let first_order = (self.next_id - retained) as u64;
        let selection = self.selection.clone();
        let focus = self.focus.clone();
        let viewport = self.viewport.clone();
        let viewport_layout = self.viewport.clone();
        let finish = self.selection.clone();
        let copy_owner = cx.weak_entity();
        let copy_viewport = self.viewport.clone();
        let following = self.scroller.read(cx).is_following_tail();
        div()
            .id("channel-pane")
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
                    .h(px(36.))
                    .px_3()
                    .gap_2()
                    .bg(rgb(theme::PANEL))
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(13.))
                            .child(format!("# {}", self.name)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::MUTED))
                            .child(if retained == 0 { "Offline".to_string() } else { format!("{retained} · {}", if following { "Latest" } else { "History" }) }),
                    )
                    .child(Button::new("close-pane").xsmall().label("×").tooltip("Close this split; keep draft")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PaneEvent::Close)))),
            )
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
            .on_action(cx.listener(|this, _: &ClearChatSelection, _, cx| {
                this.selection.borrow_mut().clear();
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
                        .child("Your channel is saved. Chat will appear here once Twitch is connected.")))
                    .child(
                        MessageScroller::new("chat", self.scroller.clone(), move |index, _, _| {
                            let messages = timeline.borrow();
                            let Some(message) = messages.messages().get(index) else {
                                return div().into_any_element();
                            };
                            div()
                                .id(SharedString::from(message.id.clone()))
                                .min_w_0()
                                .cursor_text()
                                .child(ChatText::new(
                                    SharedString::from(format!("text-{}", message.id)),
                                    first_order + index as u64,
                                    message.copy_line(),
                                    selection.clone(),
                                    focus.clone(),
                                    viewport.clone(),
                                ))
                                .into_any_element()
                        })
                        .flex_1()
                        .min_h_0()
                        .with_row_style(StyleRefinement::default().px_3().pb_0())
                        .with_list_style(StyleRefinement::default().py_2())
                        .h_full(),
                    ),
            )
            .child(div().v_flex().p_2().gap_2().bg(rgb(theme::PANEL)).border_t_1().border_color(rgb(theme::BORDER))
                .child(Textarea::new(&self.draft))
                .child(div().h_flex().justify_between().text_size(px(11.)).text_color(rgb(theme::MUTED))
                    .child(format!("{} / 500 · local draft · Shift+Enter newline", self.draft.read(cx).value().chars().count()))
                    .child(Button::new("send").small().label("Send").disabled(true).tooltip("Connect Twitch to send messages"))))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let copy_owner = copy_owner.clone();
                        let copy_viewport = copy_viewport.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase.capture() && event.button == MouseButton::Right
                                && copy_viewport.borrow().is_some_and(|bounds| bounds.contains(&event.position))
                            {
                                window.prevent_default();
                                let _ = copy_owner.update(cx, |pane, cx| {
                                    pane.focus.focus(window, cx);
                                    pane.copy(window, cx);
                                });
                                cx.stop_propagation();
                            }
                        });
                        let finish = finish.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _| {
                            if phase.capture() && event.button == MouseButton::Left {
                                finish.borrow_mut().finish();
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
        .w(px(46.))
        .h(px(34.))
        .mt(px(6.))
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
                window_min_size: Some(size(px(1050.), px(640.))),
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
