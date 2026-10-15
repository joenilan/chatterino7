//! Pinned user cards (Chatterino's pin on a user card): the same card in its
//! own small window, kept on top, that stays open while you keep chatting and
//! updates as the chatter's channel does.
use super::*;

pub(super) struct PinnedCard {
    owner: WeakEntity<ChannelPane>,
    user_id: String,
    name: String,
    login: Option<String>,
    source: Option<String>,
    channel: SharedString,
    on_top: bool,
    _updates: Option<Subscription>,
}
impl Render for PinnedCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_ui_zoom(theme::ui_zoom());
        let frame = crate::message_actions::chatter_card(
            crate::message_actions::CardFrame::default(),
            self.owner.clone(),
            self.user_id.clone(),
            self.name.clone(),
            self.login.clone(),
            self.source.clone(),
            window,
            cx,
        );
        div()
            .v_flex()
            .size_full()
            .bg(rgb(theme::canvas()))
            .text_color(rgb(theme::text()))
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(theme::border()))
                    .child(
                        div()
                            .h_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .text_color(rgb(theme::accent()))
                                    .font_weight(FontWeight::BOLD)
                                    .child("//"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(format!(
                                        "{} · {}",
                                        frame.title,
                                        self.source
                                            .as_ref()
                                            .map(|source| format!("#{source}"))
                                            .unwrap_or_else(|| crate::combined::title(
                                                &self.channel
                                            )),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("card-on-top")
                            .w(px(32.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(if self.on_top {
                                theme::accent()
                            } else {
                                theme::muted()
                            }))
                            .hover(|style| {
                                style.bg(rgb(theme::hover())).text_color(rgb(theme::text()))
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                if crate::workspace::sound_settings::set_on_top(
                                    window,
                                    !this.on_top,
                                ) {
                                    this.on_top = !this.on_top;
                                    cx.notify();
                                }
                            }))
                            .child(Icon::new(gpui_kit::assets::IconName::Pin).small()),
                    )
                    .child(caption_control(
                        "card-close",
                        IconName::WindowClose,
                        WindowControlArea::Close,
                        true,
                    )),
            )
            .child(div().flex_1().min_h_0().p_3().children(frame.body))
    }
}

impl Workbench {
    /// Open a chatter's card in its own window (the card's Pin button).
    pub(super) fn pin_card(
        &mut self,
        pane: Entity<ChannelPane>,
        user_id: String,
        name: String,
        login: Option<String>,
        source: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let channel: SharedString = pane.read(cx).name.to_string().into();
        cx.defer(move |cx| {
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("{name} · Jawjack").into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(440.), px(600.)),
                    cx,
                ))),
                window_min_size: Some(size(px(300.), px(240.))),
                ..Default::default()
            };
            let _ = gpui_kit::open_window(options, cx, |window, cx| {
                let on_top = crate::workspace::sound_settings::set_on_top(window, true);
                cx.new(|cx| PinnedCard {
                    owner: pane.downgrade(),
                    user_id,
                    name,
                    login,
                    source,
                    channel,
                    on_top,
                    // New messages, badges and moderation in that channel refresh the card.
                    _updates: Some(cx.observe(&pane, |_, _, cx| cx.notify())),
                })
            });
        });
    }
}
