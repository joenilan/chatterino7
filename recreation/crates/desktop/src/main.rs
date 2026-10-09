use std::cell::RefCell;
use std::rc::Rc;

use chat_core::{Event, Timeline, fixture};
use gpui_kit::base::SelectableText;
use gpui_kit::component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
    message_scroller::{MessageScroller, MessageScrollerState},
};
use gpui_kit::*;

struct ChannelPane {
    name: SharedString,
    timeline: Rc<RefCell<Timeline>>,
    scroller: Entity<MessageScrollerState>,
    next_id: usize,
}
impl ChannelPane {
    fn new(name: &'static str, cx: &mut Context<Self>) -> Self {
        let mut timeline = Timeline::new(name, 10_000);
        for i in 0..250 {
            timeline.apply(Event::Message(fixture(name, i)));
        }
        let scroller = cx.new(|cx| MessageScrollerState::new(timeline.messages().len(), cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();
        Self {
            name: name.into(),
            timeline: Rc::new(RefCell::new(timeline)),
            scroller,
            next_id: 250,
        }
    }
    fn burst(&mut self, cx: &mut Context<Self>) {
        for _ in 0..100 {
            let change = self
                .timeline
                .borrow_mut()
                .apply(Event::Message(fixture(&self.name, self.next_id)));
            self.next_id += 1;
            if let chat_core::Change::Appended { evicted } = change {
                self.scroller.update(cx, |state, cx| {
                    if evicted {
                        state.splice(0..1, 0, cx);
                    }
                    state.append(1, cx);
                });
            }
        }
        cx.notify();
    }
    fn redact(&mut self, cx: &mut Context<Self>) {
        self.timeline.borrow_mut().apply(Event::ClearUser {
            channel_id: self.name.to_string(),
            user_id: "fixture-1".into(),
        });
        self.scroller.update(cx, |state, cx| {
            state.remeasure(cx);
        });
        cx.notify();
    }
}
impl Render for ChannelPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let timeline = self.timeline.clone();
        let retained = timeline.borrow().messages().len();
        div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .h_flex()
                    .p_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("# {}", self.name)),
                    )
                    .child(format!("{retained} retained")),
            )
            .child(
                MessageScroller::new("chat", self.scroller.clone(), move |index, _, _| {
                    let messages = timeline.borrow();
                    let Some(message) = messages.messages().get(index) else {
                        return div().into_any_element();
                    };
                    div()
                        .id(SharedString::from(message.id.clone()))
                        .min_w_0()
                        .child(
                            SelectableText::new(
                                SharedString::from(format!("text-{}", message.id)),
                                message.copy_line(),
                            )
                            .document_order(index as u64),
                        )
                        .into_any_element()
                })
                .flex_1()
                .min_h_0()
                .with_row_style(StyleRefinement::default().px_2().pb_1())
                .with_list_style(StyleRefinement::default().py_1()),
            )
            .child(
                div()
                    .h_flex()
                    .p_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("burst")
                            .label("Replay +100")
                            .on_click(cx.listener(|this, _, _, cx| this.burst(cx))),
                    )
                    .child(
                        Button::new("redact")
                            .label("Test timeout")
                            .on_click(cx.listener(|this, _, _, cx| this.redact(cx))),
                    ),
            )
    }
}
struct Workbench {
    panes: Vec<Entity<ChannelPane>>,
}
impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().v_flex().size_full().bg(cx.theme().background).text_color(cx.theme().foreground)
            .child(div().v_flex().p_3().gap_1()
                .child(div().text_lg().font_weight(FontWeight::BOLD).child("Chat workbench"))
                .child("OFFLINE REPLAY · No Twitch account connected · Synthetic messages only"))
            .child(div().h_flex().flex_1().min_h_0().gap_2().px_2().children(self.panes.iter().cloned()))
            .child(div().p_2().child("Foundation preview · Scroll, select text, replay bursts and test redaction. Live chat and emote rendering are next."))
    }
}
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|cx| Workbench {
                    panes: vec![
                        cx.new(|cx| ChannelPane::new("workbench", cx)),
                        cx.new(|cx| ChannelPane::new("second-channel", cx)),
                    ],
                })
            })
            .expect("Could not open the chat workbench window");
        });
}
