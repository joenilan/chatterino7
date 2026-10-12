//! Popout windows: one channel in its own OS window. The pane is an ordinary
//! ChannelPane created in the new window and fed by the same live connection,
//! so sending, highlights, sounds and moderation all work there too.
use super::*;

/// Drafts and replies of popouts are keyed apart from workspace tabs.
pub(super) const POPOUT_TAB: u64 = u64::MAX;

pub(super) struct Popout {
    pub pane: Entity<ChannelPane>,
    pub window: AnyWindowHandle,
}

/// The popout window's content: a slim draggable title bar above the pane.
pub(super) struct PopoutView {
    pane: Entity<ChannelPane>,
    channel: SharedString,
    /// Kept above other windows (title bar pin).
    on_top: bool,
}
impl Render for PopoutView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .bg(rgb(theme::CANVAS))
            .text_color(rgb(theme::TEXT))
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .h_flex()
                            .flex_1()
                            .h_full()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .text_color(rgb(theme::ACCENT))
                                    .font_weight(FontWeight::BOLD)
                                    .child("//"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("#{}", self.channel)),
                            ),
                    )
                    .child(
                        div()
                            .id("popout-on-top")
                            .w(px(32.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(if self.on_top {
                                theme::ACCENT
                            } else {
                                theme::MUTED
                            }))
                            .hover(|style| style.bg(rgb(theme::HOVER)).text_color(rgb(theme::TEXT)))
                            .tooltip({
                                let on_top = self.on_top;
                                move |w, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(if on_top {
                                        "Always on top: on (click to turn off)"
                                    } else {
                                        "Keep this window on top of others"
                                    })
                                    .build(w, cx)
                                }
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
                        "popout-minimize",
                        IconName::WindowMinimize,
                        WindowControlArea::Min,
                        false,
                    ))
                    .child(caption_control(
                        "popout-maximize",
                        if window.is_maximized() {
                            IconName::WindowRestore
                        } else {
                            IconName::WindowMaximize
                        },
                        WindowControlArea::Max,
                        false,
                    ))
                    .child(caption_control(
                        "popout-close",
                        IconName::WindowClose,
                        WindowControlArea::Close,
                        true,
                    )),
            )
            .child(div().flex_1().min_h_0().p(px(2.)).child(self.pane.clone()))
    }
}

impl Workbench {
    pub(super) fn is_popout(&self, pane: &Entity<ChannelPane>) -> bool {
        self.popouts.iter().any(|p| p.pane == *pane)
    }
    pub(super) fn popout_channels(&self, cx: &App) -> Vec<String> {
        self.popouts
            .iter()
            .map(|p| p.pane.read(cx).name.to_string())
            .collect()
    }
    /// Open `channel` in its own window (or bring its existing popout forward).
    pub(super) fn open_popout(&mut self, channel: String, cx: &mut Context<Self>) {
        if let Some(existing) = self
            .popouts
            .iter()
            .find(|p| p.pane.read(cx).name.as_ref() == channel)
        {
            let _ = existing
                .window
                .update(cx, |_, window, _| window.activate_window());
            return;
        }
        let owner = cx.entity();
        // Opened after this update finishes: building the pane needs the workbench again.
        cx.defer(move |cx| {
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("#{channel} · Jawjack").into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(420.), px(720.)),
                    cx,
                ))),
                window_min_size: Some(size(px(300.), px(240.))),
                ..Default::default()
            };
            let built = gpui_kit::open_window(options, cx, |window, cx| {
                let pane = owner.update(cx, |this, cx| {
                    let pane = this.make_pane(POPOUT_TAB, &channel, window, cx);
                    this.seed_popout(&pane, cx);
                    pane
                });
                let closing = owner.downgrade();
                let closing_pane = pane.clone();
                window.on_window_should_close(cx, move |_, cx| {
                    let _ = closing.update(cx, |this, cx| {
                        this.popouts.retain(|p| p.pane != closing_pane);
                        this.schedule_save(cx);
                        cx.notify();
                    });
                    true
                });
                cx.new(|_| PopoutView {
                    pane,
                    channel: channel.clone().into(),
                    on_top: false,
                })
            });
            if let Ok((window, view)) = built {
                let pane = view.read(cx).pane.clone();
                owner.update(cx, |this, cx| {
                    this.popouts.push(Popout { pane, window });
                    this.schedule_save(cx);
                    cx.notify();
                });
            }
        });
    }
    /// Start a popout with the chat already loaded in a tab for the same channel.
    fn seed_popout(&mut self, pane: &Entity<ChannelPane>, cx: &mut Context<Self>) {
        let channel = pane.read(cx).name.clone();
        let Some(source) = self
            .tab_panes()
            .into_iter()
            .find(|p| p.read(cx).name == channel)
        else {
            return;
        };
        let messages: Vec<chat_core::Message> = source
            .read(cx)
            .timeline
            .borrow()
            .messages()
            .iter()
            .cloned()
            .collect();
        let (connected, connection) = {
            let s = source.read(cx);
            (s.connected, s.connection.clone())
        };
        pane.update(cx, |p, cx| {
            p.quiet_pings = true;
            for message in messages {
                p.received(chat_core::Event::Message(message), cx);
            }
            p.quiet_pings = false;
            // Copied rows were already seen in the tab.
            p.attention.mark_all();
            p.connected = connected;
            p.connection = connection;
            p.scroller.update(cx, |s, cx| s.scroll_to_end(cx));
        });
    }
    /// Close a popout window (its pane's close button, or the workspace closing).
    pub(super) fn close_popout(&mut self, pane: &Entity<ChannelPane>, cx: &mut Context<Self>) {
        let Some(index) = self.popouts.iter().position(|p| p.pane == *pane) else {
            return;
        };
        let popout = self.popouts.remove(index);
        let _ = popout
            .window
            .update(cx, |_, window, _| window.remove_window());
        self.schedule_save(cx);
        cx.notify();
    }
}
