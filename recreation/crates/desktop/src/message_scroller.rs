// Adapted from GPUI Component 0.7.1 message_scroller.rs, Apache-2.0.
// Copyright GPUI Component contributors. See licenses/GPUI-COMPONENT-APACHE-2.0.
// Jawjack adds explicit held pause and resume intent; upstream geometry is retained.
#![allow(dead_code)]
use std::{ops::Range, time::Duration};

use gpui_kit::base::motion::{Transition, transition};
use gpui_kit::{
    AnyElement, App, Axis, Context, ElementId, Entity, FollowMode, Hsla, InteractiveElement as _,
    IntoElement, ListAlignment, ListOffset, ListState, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    linear_color_stop, linear_gradient, list, prelude::FluentBuilder as _, px, rems,
};

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, StyledExt as _, button::Button,
};
use gpui_kit::component::{
    button::ButtonVariants as _,
    scroll::{ScrollableElement as _, ScrollableMask},
};

const LIST_OVERDRAW: gpui_kit::Pixels = px(400.);
/// Settings → Appearance → Scrolling: wheel speed (an index into `SPEEDS`)
/// and animated mouse-wheel steps.
pub const SPEEDS: [(&str, f32); 4] = [
    ("Slow", 0.5),
    ("Normal", 1.),
    ("Fast", 1.6),
    ("Faster", 2.5),
];
pub static SCROLL_SPEED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
pub static SMOOTH_SCROLL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
const JUMP_BUTTON_TRANSITION: Duration = Duration::from_millis(200);
const BOTTOM_FADE_TRANSITION: Duration = Duration::from_millis(200);

/// The entity-owned scrolling state for a [`MessageScroller`].
///
/// The state owns only GPUI's virtual-list bookkeeping. Message data remains
/// with the caller and is read by the row renderer passed to
/// [`MessageScroller::new`].
pub struct MessageScrollerState {
    list_state: ListState,
    manual: bool,
    hover: bool,
    resume_tail: bool,
    suppress_hover: bool,
}

impl MessageScrollerState {
    /// Create a state for `item_count` rows and enable tail following.
    ///
    /// The constructor receives the entity context so the list's scroll
    /// handler can safely defer its entity update until GPUI has released the
    /// list's internal borrow.
    pub fn new(item_count: usize, cx: &mut Context<Self>) -> Self {
        let list_state = ListState::new(item_count, ListAlignment::Top, LIST_OVERDRAW);
        list_state.set_follow_mode(FollowMode::Tail);

        let weak_state = cx.weak_entity();
        list_state.set_scroll_handler(move |_, _, cx| {
            let weak_state = weak_state.clone();

            cx.defer(move |cx| {
                let _ = weak_state.update(cx, |state, cx| {
                    if state.paused() {
                        state.resume_tail = false;
                    }
                    cx.notify();
                });
            });
        });

        Self {
            list_state,
            manual: false,
            hover: false,
            resume_tail: false,
            suppress_hover: false,
        }
    }

    pub fn paused(&self) -> bool {
        self.manual || self.hover
    }
    pub fn manual_pause(&self) -> bool {
        self.manual
    }
    pub fn cancel_auto_resume(&mut self) {
        self.resume_tail = false;
    }
    pub fn set_hover_pause(&mut self, active: bool, cx: &mut Context<Self>) {
        if !active {
            self.suppress_hover = false;
        }
        let active = active && !self.suppress_hover;
        if self.hover == active {
            return;
        }
        let was = self.paused();
        self.hover = active;
        self.reconcile_pause(was, cx);
    }
    pub fn hold_for_navigation(&mut self, cx: &mut Context<Self>) {
        let was = self.paused();
        self.manual = true;
        self.reconcile_pause(was, cx);
        self.resume_tail = false;
    }
    pub fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        if self.manual {
            self.scroll_to_end(cx);
            return;
        }
        let was = self.paused();
        self.manual = true;
        self.reconcile_pause(was, cx);
    }
    fn reconcile_pause(&mut self, was: bool, cx: &mut Context<Self>) {
        let paused = self.paused();
        if paused && !was {
            self.resume_tail = self.list_state.is_following_tail();
            self.list_state.set_follow_mode(FollowMode::Normal);
        } else if was && !paused {
            if self.resume_tail {
                self.list_state.set_follow_mode(FollowMode::Tail);
                self.list_state.scroll_to_end();
            } else {
                let anchor = self.list_state.logical_scroll_top();
                self.list_state.set_follow_mode(FollowMode::Tail);
                self.list_state.scroll_to(anchor);
            }
        }
        cx.notify();
    }

    /// Return the current number of rows known by the virtual list.
    /// Whether the item is entirely below the viewport. Visible rows are always
    /// measured, so an unmeasured row past the scroll top (such as a new arrival
    /// while reading history) is below it.
    pub fn item_is_below_viewport(&self, ix: usize) -> bool {
        ix >= self.list_state.logical_scroll_top().item_ix
            && self.list_state.item_is_below_viewport(ix).unwrap_or(true)
    }
    pub fn item_count(&self) -> usize {
        self.list_state.item_count()
    }

    /// Return whether the user has scrolled away from the latest content.
    pub fn is_scrolled_up(&self) -> bool {
        self.list_state.max_offset_for_scrollbar().y > px(0.)
            && !self.list_state.is_following_tail()
            && !self.list_state.is_scrolled_to_end().unwrap_or(false)
    }

    /// Return whether the list is actively following its tail.
    pub fn is_following_tail(&self) -> bool {
        self.list_state.is_following_tail()
    }

    /// Reset the list to `item_count` rows.
    pub fn reset(&mut self, item_count: usize, cx: &mut Context<Self>) {
        self.manual = false;
        self.hover = false;
        self.resume_tail = false;
        self.list_state.reset(item_count);
        self.list_state.set_follow_mode(FollowMode::Tail);
        cx.notify();
    }

    /// Replace `old_range` with `count` new rows.
    ///
    /// Returns `false` when the range is outside the current list and leaves
    /// the state unchanged.
    pub fn splice(
        &mut self,
        old_range: Range<usize>,
        count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.valid_range(&old_range) {
            return false;
        }

        let neighbor = old_range.start.checked_sub(1);
        self.list_state.splice(old_range, count);

        // The default row wrapper pads every row except the last, so a row
        // whose "last" status may have flipped carries a stale measured
        // height. Remeasure the new last row and the survivor next to the
        // splice.
        if let Some(last) = self.list_state.item_count().checked_sub(1) {
            self.list_state.remeasure_items(last..last + 1);
            if let Some(neighbor) = neighbor.filter(|neighbor| *neighbor != last) {
                self.list_state.remeasure_items(neighbor..neighbor + 1);
            }
        }

        cx.notify();
        true
    }

    /// Append `count` rows to the end of the list.
    pub fn append(&mut self, count: usize, cx: &mut Context<Self>) -> bool {
        let item_count = self.list_state.item_count();
        self.splice(item_count..item_count, count, cx)
    }

    /// Prepend `count` rows while preserving the current scroll anchor.
    pub fn prepend(&mut self, count: usize, cx: &mut Context<Self>) -> bool {
        self.splice(0..0, count, cx)
    }

    /// Mark all rows for remeasurement while preserving a proportional anchor.
    pub fn remeasure(&mut self, cx: &mut Context<Self>) {
        self.list_state.remeasure();
        cx.notify();
    }

    /// Mark rows in `range` for remeasurement while preserving an item anchor.
    ///
    /// Returns `false` when the range is outside the current list.
    pub fn remeasure_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> bool {
        if !self.valid_range(&range) {
            return false;
        }

        self.list_state.remeasure_items(range);
        cx.notify();
        true
    }

    /// Scroll to the row at `index`, if it exists.
    pub fn scroll_to_item(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        if index >= self.list_state.item_count() {
            return false;
        }

        self.resume_tail = false;
        self.list_state.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: px(0.),
        });
        cx.notify();
        true
    }

    /// Resume tail following and scroll to the latest row.
    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.manual = false;
        self.hover = false;
        self.resume_tail = false;
        self.suppress_hover = true;
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
        cx.notify();
    }

    fn valid_range(&self, range: &Range<usize>) -> bool {
        range.start <= range.end && range.end <= self.list_state.item_count()
    }
}

/// A virtualized message list with optional scrollbar and jump-to-latest UI.
#[derive(IntoElement)]
pub struct MessageScroller {
    id: ElementId,
    state: Entity<MessageScrollerState>,
    renderer: Box<dyn FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static>,
    style: StyleRefinement,
    content_style: StyleRefinement,
    list_style: StyleRefinement,
    row_style: StyleRefinement,
    jump_button_style: StyleRefinement,
    jump_button_renderer: Option<Box<dyn FnOnce(Button) -> Button>>,
    jump_button_transition: Duration,
    bottom_fade: Option<Hsla>,
    scrollbar: bool,
    jump_button: bool,
    jump_button_label: SharedString,
    /// Ticks along the right edge: (position 0..1 down the list, colour).
    markers: Vec<(f32, u32)>,
}

impl MessageScroller {
    /// Create a message scroller with a renderer for each row.
    pub fn new<E>(
        id: impl Into<ElementId>,
        state: Entity<MessageScrollerState>,
        renderer: impl FnMut(usize, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement,
    {
        let mut renderer = renderer;
        Self {
            id: id.into(),
            state,
            renderer: Box::new(move |index, window, cx| {
                renderer(index, window, cx).into_any_element()
            }),
            style: StyleRefinement::default(),
            content_style: StyleRefinement::default(),
            list_style: StyleRefinement::default(),
            row_style: StyleRefinement::default(),
            jump_button_style: StyleRefinement::default(),
            jump_button_renderer: None,
            jump_button_transition: JUMP_BUTTON_TRANSITION,
            bottom_fade: None,
            scrollbar: true,
            jump_button: true,
            jump_button_label: "Jump to latest".into(),
            markers: Vec::new(),
        }
    }

    /// Mark rows on the scrollbar track (mentions, highlights, search hits),
    /// as fractions of the list from top (0) to bottom (1).
    pub fn markers(mut self, markers: Vec<(f32, u32)>) -> Self {
        self.markers = markers;
        self
    }

    /// Enable or disable the virtual-list scrollbar.
    pub fn scrollbar(mut self, scrollbar: bool) -> Self {
        self.scrollbar = scrollbar;
        self
    }

    /// Enable or disable the built-in jump-to-latest button.
    pub fn jump_button(mut self, jump_button: bool) -> Self {
        self.jump_button = jump_button;
        self
    }

    /// Set the label used by the built-in jump-to-latest button.
    pub fn with_jump_button_label(mut self, label: impl Into<SharedString>) -> Self {
        self.jump_button_label = label.into();
        self
    }

    /// Refine the viewport that contains the list and scrollbar.
    pub fn with_content_style(mut self, style: StyleRefinement) -> Self {
        self.content_style = style;
        self
    }

    /// Refine the GPUI list element used to render rows.
    pub fn with_list_style(mut self, style: StyleRefinement) -> Self {
        self.list_style = style;
        self
    }

    /// Refine the full-width wrapper around every rendered row.
    pub fn with_row_style(mut self, style: StyleRefinement) -> Self {
        self.row_style = style;
        self
    }

    /// Refine the built-in jump-to-latest button after its defaults.
    pub fn with_jump_button_style(mut self, style: StyleRefinement) -> Self {
        self.jump_button_style = style;
        self
    }

    /// Customize the built-in jump button without replacing its scroll action.
    ///
    /// The callback receives the fully configured Button, so its variant,
    /// semantic size, icon, tooltip, or instance styling may be adjusted.
    pub fn with_jump_button_renderer(
        mut self,
        renderer: impl FnOnce(Button) -> Button + 'static,
    ) -> Self {
        self.jump_button_renderer = Some(Box::new(renderer));
        self
    }

    /// Set how long the built-in jump button takes to enter or leave.
    ///
    /// A zero duration disables its transition. Reduced-motion preferences
    /// always adopt the final state immediately.
    pub fn with_jump_button_transition(mut self, duration: Duration) -> Self {
        self.jump_button_transition = duration;
        self
    }

    /// Fade the transcript's bottom edge into `color`.
    ///
    /// A partially visible row melts into the surface behind the scroller
    /// instead of clipping mid-line. The fade shows only while the reader is
    /// away from the live edge — at the bottom nothing is clipped. Pass the
    /// color of that surface; the fade is off by default and sits under the
    /// jump button.
    pub fn with_bottom_fade(mut self, color: impl Into<Hsla>) -> Self {
        self.bottom_fade = Some(color.into());
        self
    }
}

impl Styled for MessageScroller {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MessageScroller {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let root_id = self.id.clone();
        let (list_state, scrolled_up) = {
            let state = self.state.read(cx);
            (
                state.list_state.clone(),
                state.is_scrolled_up() || state.paused(),
            )
        };

        let show_jump_button = self.jump_button && scrolled_up;
        let jump_button_visibility = if self.jump_button {
            transition(
                (root_id.clone(), "jump-button-visibility"),
                if show_jump_button { 1. } else { 0. },
                Transition::new(self.jump_button_transition),
                window,
                cx,
            )
        } else {
            0.
        };
        // At the live edge nothing is clipped below, so a visible fade would
        // suggest more content than there is.
        let bottom_fade_visibility = if self.bottom_fade.is_some() {
            transition(
                (root_id.clone(), "bottom-fade-visibility"),
                if scrolled_up { 1. } else { 0. },
                Transition::new(BOTTOM_FADE_TRANSITION),
                window,
                cx,
            )
        } else {
            0.
        };
        let tokens = cx.theme().semantic_tokens();
        let row_style = self.row_style;
        let jump_button_style = self.jump_button_style;
        let jump_button_renderer = self.jump_button_renderer;
        let mut renderer = self.renderer;

        // GPUI's `list` lays rows out at the full list width and offsets them
        // only by vertical padding, so the horizontal component of the list
        // style must be carried by every row wrapper instead.
        let mut list_style = self.list_style;
        let row_inset_left = list_style.padding.left.take();
        let row_inset_right = list_style.padding.right.take();

        // Read the count outside the row closure: the list holds a mutable
        // borrow of its state while rendering rows, so the closure must not
        // borrow it again. The count is stable within one render pass.
        let item_count = list_state.item_count();
        let list = list(list_state.clone(), move |index, window, cx| {
            div()
                .w_full()
                .min_w_0()
                .px_3()
                // Spacing between rows only, like a CSS gap: the list's own
                // bottom padding owns the gap after the last row.
                .when(index + 1 < item_count, |this| this.pb_8())
                .when_some(row_inset_left, |this, left| this.pl(left))
                .when_some(row_inset_right, |this, right| this.pr(right))
                .refine_style(&row_style)
                .child(renderer(index, window, cx))
                .into_any_element()
        })
        .size_full()
        .min_h_0()
        .py_2()
        .refine_style(&list_style);

        let viewport = div()
            .id((root_id.clone(), "viewport"))
            // Announce appended rows as a log region, like shadcn's
            // `role="log"` transcript content.
            .role(Role::Log)
            .size_full()
            .min_h_0()
            .min_w_0()
            .child(list)
            // The fade sits above the rows but below the scrollbar and the
            // jump button, so neither control is washed out by it.
            .when_some(
                self.bottom_fade.filter(|_| bottom_fade_visibility > 0.),
                |this, color| {
                    this.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .h(rems(3.))
                            .opacity(bottom_fade_visibility)
                            .bg(linear_gradient(
                                180.,
                                linear_color_stop(color.opacity(0.), 0.),
                                linear_color_stop(color, 1.),
                            )),
                    )
                },
            )
            .when(self.scrollbar, |this| this.vertical_scrollbar(&list_state))
            .refine_style(&self.content_style);

        div()
            .id(root_id.clone())
            .relative()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            // Mouse-wheel notches at the chosen speed, gliding when smooth is on.
            // Registered before the mask below so it runs first; touchpads
            // (precise deltas) and Ctrl+wheel zoom pass through untouched.
            .child({
                let list_state = list_state.clone();
                gpui_kit::canvas(
                    |bounds, window, _| {
                        window.insert_hitbox(bounds, gpui_kit::HitboxBehavior::Normal)
                    },
                    move |_, hitbox, window, _| {
                        let list_state = list_state.clone();
                        window.on_mouse_event(
                            move |event: &gpui_kit::ScrollWheelEvent, phase, window, cx| {
                                use std::sync::atomic::Ordering::Relaxed;
                                let gpui_kit::ScrollDelta::Lines(lines) = event.delta else {
                                    return;
                                };
                                if !phase.capture()
                                    || event.modifiers.control
                                    || lines.y == 0.
                                    || !hitbox.should_handle_scroll(window)
                                {
                                    return;
                                }
                                let speed =
                                    SPEEDS[SCROLL_SPEED.load(Relaxed).min(SPEEDS.len() - 1)].1;
                                let delta = window.line_height() * (lines.y * speed);
                                list_state.scroll_wheel(delta, SMOOTH_SCROLL.load(Relaxed));
                                window.refresh();
                                cx.stop_propagation();
                            },
                        );
                    },
                )
                .absolute()
                .size_full()
            })
            .child(viewport)
            // Keep vertical wheel scrolling from leaking into an ancestor
            // scroller (like in Table): the mask consumes vertical-dominant
            // wheel events while the list can move and chains to the ancestor
            // only at the edges.
            .child(ScrollableMask::new(Axis::Vertical, &list_state).id(root_id.clone()))
            // Thin ticks on the track; they don't take the pointer, so the scrollbar still works.
            .when(!self.markers.is_empty(), |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(8.))
                        .bottom(px(8.))
                        .right(px(1.))
                        .w(px(4.))
                        .children(self.markers.iter().map(|(at, color)| {
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top(gpui_kit::relative(at.clamp(0., 1.)))
                                .h(px(2.))
                                .rounded(px(1.))
                                .bg(gpui_kit::rgb(*color))
                                .opacity(0.9)
                        })),
                )
            })
            .when(self.jump_button && jump_button_visibility > 0., |this| {
                let state = self.state.clone();

                this.child(
                    // Bottom-right keeps the button clear of centered row
                    // markers such as the last-read divider label.
                    div()
                        .absolute()
                        .right(rems(1.25))
                        .bottom(rems(0.5 + jump_button_visibility * 0.5))
                        .flex()
                        .opacity(jump_button_visibility)
                        .child(
                            // No explicit width or height: Button sizes an
                            // icon-only button as a square on its own, and a
                            // renderer that adds a label or another semantic
                            // size must be able to change the layout.
                            Button::new((root_id, "jump-to-latest"))
                                .secondary()
                                .icon(IconName::ArrowDown)
                                .tooltip(self.jump_button_label)
                                .rounded(cx.theme().radius_full())
                                .border_1()
                                .border_color(tokens.colors.border)
                                .bg(tokens.colors.background)
                                .text_color(tokens.colors.foreground)
                                .refine_style(&jump_button_style)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |state, cx| state.scroll_to_end(cx));
                                })
                                .when_some(jump_button_renderer, |button, renderer| {
                                    renderer(button)
                                })
                                .when(!show_jump_button, |button| button.disabled(true)),
                        ),
                )
            })
            .refine_style(&self.style)
    }
}
