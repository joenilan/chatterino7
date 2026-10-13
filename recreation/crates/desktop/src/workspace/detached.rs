//! Workspaces in their own windows (Chatterino's multiple windows): a
//! workspace opened in a new window shows there with its full layout (tabs,
//! splits, menus) and leaves the main window's list as a shortcut to it.
//!
//! The workbench draws and handles one workspace at a time (`active`), so the
//! window borrows it: `active` points at its workspace while it draws and
//! while it handles input, then goes back to the main window's choice.
use super::*;

pub(super) struct DetachedWorkspace {
    owner: WeakEntity<Workbench>,
    id: u64,
}

/// Point the workbench at window `id`'s workspace for this input event; the
/// main window's workspace comes back once the event is handled.
fn enter(owner: &WeakEntity<Workbench>, id: u64, cx: &mut App) {
    let _ = owner.update(cx, |this, cx| {
        let Some(ix) = this.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        if this.active == ix {
            return;
        }
        let saved = this.active;
        this.active = ix;
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, _| {
                if this.active == ix && saved < this.tabs.len() {
                    this.active = saved;
                }
            });
        });
    });
}

impl Render for DetachedWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_ui_zoom(theme::ui_zoom());
        let id = self.id;
        let drawn = self
            .owner
            .update(cx, |this, cx| this.render_detached(id, window, cx))
            .ok()
            .flatten();
        let Some((name, body)) = drawn else {
            // Its workspace was closed: close the window too.
            cx.defer_in(window, |_, window, _| window.remove_window());
            return div().into_any_element();
        };
        let owner = self.owner.clone();
        div()
            .v_flex()
            .size_full()
            .bg(rgb(theme::canvas()))
            .text_color(rgb(theme::text()))
            .font_family("Segoe UI")
            .text_size(px(14.))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let o = owner.clone();
                        window.on_mouse_event(move |_: &MouseDownEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                enter(&o, id, cx);
                            }
                        });
                        let o = owner.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                enter(&o, id, cx);
                            }
                        });
                        let o = owner.clone();
                        window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                enter(&o, id, cx);
                            }
                        });
                        let o = owner.clone();
                        window.on_mouse_event(move |_: &ScrollWheelEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                enter(&o, id, cx);
                            }
                        });
                        let o = owner.clone();
                        window.on_key_event(move |_: &KeyDownEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                enter(&o, id, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .items_center()
                    .bg(rgb(theme::shell()))
                    .border_b_1()
                    .border_color(rgb(theme::border()))
                    .child(
                        div()
                            .h_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .text_color(rgb(theme::accent()))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(12.))
                                    .child("// JAWJACK"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(name),
                            ),
                    )
                    .child(caption_control(
                        "detached-minimize",
                        IconName::WindowMinimize,
                        WindowControlArea::Min,
                        false,
                    ))
                    .child(caption_control(
                        "detached-maximize",
                        if window.is_maximized() {
                            IconName::WindowRestore
                        } else {
                            IconName::WindowMaximize
                        },
                        WindowControlArea::Max,
                        false,
                    ))
                    .child(caption_control(
                        "detached-close",
                        IconName::WindowClose,
                        WindowControlArea::Close,
                        true,
                    )),
            )
            .child(div().flex_1().min_h_0().p(px(2.)).child(body))
            .into_any_element()
    }
}

impl Workbench {
    pub(super) fn detached_window(&self, id: u64) -> Option<AnyWindowHandle> {
        self.detached
            .iter()
            .find(|(d, _)| *d == id)
            .map(|(_, handle)| *handle)
    }
    pub(super) fn is_detached(&self, id: u64) -> bool {
        self.detached.iter().any(|(d, _)| *d == id) || self.opening_detached.contains(&id)
    }
    /// Bring a workspace's own window forward.
    pub(super) fn show_detached(&self, id: u64, cx: &mut App) -> bool {
        let Some(handle) = self.detached_window(id) else {
            return false;
        };
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        true
    }
    /// The main window shows a workspace that isn't in its own window.
    pub(super) fn keep_main_attached(&mut self) {
        if self
            .tabs
            .get(self.active)
            .is_some_and(|t| self.is_detached(t.id))
            && let Some(ix) = self.tabs.iter().position(|t| !self.is_detached(t.id))
        {
            self.active = ix;
        }
    }
    /// Open workspace `id` in its own window.
    pub(super) fn detach_workspace(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.show_detached(id, cx) {
            return;
        }
        if !self
            .tabs
            .iter()
            .any(|t| t.id != id && !self.is_detached(t.id))
        {
            window.push_notification(
                Notification::info("Keep at least one workspace in the main window."),
                cx,
            );
            return;
        }
        self.open_detached(id, cx);
        if self.tabs.get(self.active).is_some_and(|t| t.id == id)
            && let Some(ix) = self.tabs.iter().position(|t| !self.is_detached(t.id))
        {
            self.select_tab(ix, cx);
        }
        self.schedule_save(cx);
        cx.notify();
    }
    pub(super) fn open_detached(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(name) = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.clone())
        else {
            return;
        };
        self.opening_detached.push(id);
        let owner = cx.entity();
        cx.defer(move |cx| {
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("{name} · Jawjack").into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(900.), px(680.)),
                    cx,
                ))),
                window_min_size: Some(size(px(360.), px(260.))),
                ..Default::default()
            };
            let weak = owner.downgrade();
            let built = gpui_kit::open_window(options, cx, |window, cx| {
                let closing = weak.clone();
                window.on_window_should_close(cx, move |_, cx| {
                    let _ = closing.update(cx, |this, cx| {
                        this.detached.retain(|(d, _)| *d != id);
                        this.schedule_save(cx);
                        cx.notify();
                    });
                    true
                });
                cx.new(|_| DetachedWorkspace {
                    owner: weak.clone(),
                    id,
                })
            });
            owner.update(cx, |this, cx| {
                this.opening_detached.retain(|d| *d != id);
                if let Ok((handle, _)) = built {
                    this.detached.push((id, handle));
                }
                cx.notify();
            });
        });
    }
    /// Close a workspace's own window; it shows in the main window again.
    pub(super) fn attach_workspace(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(handle) = self.detached_window(id) {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        self.detached.retain(|(d, _)| *d != id);
        self.select_tab_id(id, cx);
        self.schedule_save(cx);
        cx.notify();
    }
    /// Draw workspace `id` for its own window: (name, body).
    fn render_detached(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(SharedString, AnyElement)> {
        let ix = self.tabs.iter().position(|t| t.id == id)?;
        let saved = self.active;
        self.active = ix;
        let reading_available = window.is_window_active() && !window.has_active_dialog(cx);
        let body =
            match self.tabs[ix].dock.clone() {
                Some(dock) => {
                    let min = dock.minimum();
                    let available = window.viewport_size();
                    let width = (f32::from(available.width) - 4.).max(min.0);
                    let height = (f32::from(available.height) - 36.).max(min.1);
                    let body =
                        self.render_dock(&dock, Vec::new(), width, height, reading_available, cx);
                    div()
                        .id("detached-dock")
                        .size_full()
                        .overflow_scroll()
                        .child(
                            div()
                                .size_full()
                                .min_w(px(min.0))
                                .min_h(px(min.1))
                                .child(body),
                        )
                        .into_any_element()
                }
                None => div()
                    .v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_color(rgb(theme::muted()))
                    .child("No channels in this workspace.")
                    .child(div().text_size(px(12.)).child(
                        "Drag a channel's tab onto this workspace in the main window's list.",
                    ))
                    .into_any_element(),
            };
        self.active = saved;
        Some((self.tabs[ix].name.clone().into(), body))
    }
}
