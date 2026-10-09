mod chat_text;
mod control;
mod theme;

use chat_core::{Event, Timeline, fixture, selection::Selection};
use chat_text::ChatText;
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    Icon, IconName, Sizable, StyledExt, WindowExt,
    button::Button,
    input::{Input, InputState},
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

struct ChannelPane {
    name: SharedString,
    timeline: Rc<RefCell<Timeline>>,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    draft: Entity<InputState>,
    last_copy_result: Option<&'static str>,
    scroller: Entity<MessageScrollerState>,
    next_id: usize,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
}
impl ChannelPane {
    fn new(name: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut timeline = Timeline::new(name, 10_000);
        for i in 0..250 {
            timeline.apply(Event::Message(fixture(name, i)));
        }
        let scroller = cx.new(|cx| MessageScrollerState::new(timeline.messages().len(), cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();
        Self {
            name: name.into(),
            timeline: Rc::new(RefCell::new(timeline)),
            selection: Rc::new(RefCell::new(Selection::default())),
            focus: cx.focus_handle(),
            draft: cx.new(|cx| {
                InputState::new(window, cx).placeholder("Local draft · Twitch is offline")
            }),
            last_copy_result: None,
            scroller,
            next_id: 250,
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
                            .child(format!("{retained} messages · {}", if following { "Following latest" } else { "Reading history" })),
                    ),
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
                    .line_height(px(22.))
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
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .p_2()
                    .gap_1()
                    .bg(rgb(theme::PANEL))
                    .border_t_1()
                    .border_color(rgb(theme::BORDER))
                    .child(Button::new("pane-latest").small().label("Latest")
                        .tooltip("Return this pane to the newest messages")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.scroller.update(cx, |state, cx| state.scroll_to_end(cx));
                        })))
                    .child(
                        Button::new("copy")
                            .small()
                            .label("Copy selection")
                            .tooltip("Copy selected chat text (right-click or Ctrl+C)")
                            .on_click(cx.listener(|this, _, window, cx| this.copy(window, cx))),
                    ),
            )
            .child(div().px_2().pb_2().bg(rgb(theme::PANEL))
                .child(Input::new(&self.draft).aria_label(format!("Local draft for {}", self.name)))
                .child(div().pt_1().text_size(px(11.)).text_color(rgb(theme::MUTED))
                    .child("Local draft only · Ctrl+X/C/V and undo/redo · Not saved after closing")))
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
struct Workbench {
    control_enabled: bool,
    panes: Vec<Entity<ChannelPane>>,
}
impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().v_flex().size_full().font_family("Segoe UI").text_size(px(14.)).bg(rgb(theme::SHELL)).text_color(rgb(theme::TEXT))
            .child(div().h_flex().h(px(40.)).flex_shrink_0().border_b_1().border_color(rgb(theme::BORDER))
                .child(div().h_flex().px_3().h_full().gap_3()
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("CHAT WORKBENCH"))
                    .child(Button::new("jump-latest").small().label("All latest").tooltip("Return both panes to the newest messages").on_click(cx.listener(|this, _, _, cx| {
                        for pane in &this.panes { pane.update(cx, |pane, cx| pane.scroller.update(cx, |state, cx| state.scroll_to_end(cx))); }
                    }))))
                .child(div().flex_1().mt(px(6.)).h(px(34.)).window_control_area(WindowControlArea::Drag))
                .child(caption_control("minimize", IconName::WindowMinimize, WindowControlArea::Min, false))
                .child(caption_control("maximize", if window.is_maximized() { IconName::WindowRestore } else { IconName::WindowMaximize }, WindowControlArea::Max, false))
                .child(caption_control("close", IconName::WindowClose, WindowControlArea::Close, true)))
            .child(div().h_flex().h(px(36.)).px_3().gap_2().text_size(px(12.)).text_color(rgb(theme::MUTED))
                .child(if self.control_enabled { "LOCAL APP CONTROL · OFFLINE" } else { "OFFLINE · Sample messages" }).child("/ synthetic data / Twitch not connected"))
            .child(div().h_flex().flex_1().min_h_0().gap_2().px_2().children(self.panes.iter().cloned()))
            .child(div().h(px(28.)).px_3().h_flex().text_size(px(12.)).text_color(rgb(theme::MUTED))
                .child("Select → right-click to copy · Ctrl+C / Ctrl+Insert · Ctrl+A selects chat · Esc clears"))
    }
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
                    title: Some("Chat workbench".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(820.)),
                    cx,
                ))),
                window_min_size: Some(size(px(760.), px(480.))),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let entity = cx.new(|cx| Workbench {
                    control_enabled: server.is_some(),
                    panes: vec![
                        cx.new(|cx| ChannelPane::new("workbench", window, cx)),
                        cx.new(|cx| ChannelPane::new("second-channel", window, cx)),
                    ],
                });
                if let Some(server) = server {
                    control::attach(server, &entity, window, cx);
                }
                entity
            })
            .expect("Could not open the chat workbench window");
        });
}
