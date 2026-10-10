// Element lifecycle and highlight geometry adapted from GPUI Kit v0.7.1
// crates/base/src/selectable_text.rs, licensed Apache-2.0.
// Changed: selection is owned by the chat model, rather than mounted row widgets.
use std::{cell::RefCell, rc::Rc};

use chat_core::selection::{Point as SelectionPoint, Selection};
use gpui_kit::*;

pub struct ChatText {
    id: ElementId,
    row: u64,
    search: Vec<std::ops::Range<usize>>,
    text: SharedString,
    styled: StyledText,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
}
impl ChatText {
    pub fn with_search(mut self, ranges:Vec<std::ops::Range<usize>>)->Self{self.search=ranges;self}

    pub fn with_author(mut self, name: &str, color: Option<u32>) -> Self {
        if !self.text.starts_with(name) { return self; }
        self.styled = StyledText::new(self.text.clone()).with_highlights([
            (0..name.len(), HighlightStyle { color: Some(rgb(color.unwrap_or(crate::theme::MUTED)).into()), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })
        ]);
        self
    }
    pub fn new(
        id: impl Into<ElementId>,
        row: u64,
        text: impl Into<SharedString>,
        selection: Rc<RefCell<Selection>>,
        focus: FocusHandle,
        viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        let text = text.into();
        Self {
            id: id.into(),
            row,
            search: Vec::new(),
            styled: StyledText::new(text.clone()),
            text,
            selection,
            focus,
            viewport,
        }
    }
}
impl IntoElement for ChatText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ChatText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.styled.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        self.styled
            .prepaint(id, inspector, bounds, state, window, cx);
        let layout = self.styled.layout().clone();
        let pointer = window.mouse_position();
        let inside = self.viewport.borrow().is_some_and(|v| v.contains(&pointer))
            && pointer.y >= bounds.top()
            && pointer.y < bounds.bottom();
        if inside && self.selection.borrow().dragging {
            let byte = layout
                .index_for_position(pointer)
                .unwrap_or_else(|index| index);
            let point = SelectionPoint {
                row: self.row,
                byte,
            };
            let changed = self.selection.borrow().head != Some(point);
            if changed {
                self.selection.borrow_mut().extend(point);
            }
        }
        // Keep row breathing room inside the line box, and let trailing padding
        // place the caret. Leave the scrollbar gutter to the scroll control.
        let hit_bounds = self.viewport.borrow().map_or(bounds, |viewport| {
            Bounds::from_corners(
                point(viewport.left(), bounds.top().max(viewport.top())),
                point(
                    (viewport.right() - px(12.)).max(viewport.left()),
                    bounds
                        .bottom()
                        .min(viewport.bottom())
                        .max(bounds.top().max(viewport.top())),
                ),
            )
        });
        window.insert_hitbox(hit_bounds, HitboxBehavior::Normal)
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.styled.layout().clone();
        let mut highlights=self.search.iter().cloned().map(|r|(r,0x66502D)).collect::<Vec<_>>();
        if let Some(range)=self.selection.borrow().range_for(self.row,&self.text){highlights.push((range,0x315166));}
        for (range,color) in highlights {
        if !range.is_empty()
            && let (Some(start), Some(end)) = (
                layout.position_for_index(range.start),
                layout.position_for_index(range.end),
            )
        {
            let height = layout.line_height();
            let mut quads = Vec::new();
            if start.y == end.y {
                quads.push(Bounds::from_corners(start, point(end.x, end.y + height)));
            } else {
                quads.push(Bounds::from_corners(
                    start,
                    point(bounds.right(), start.y + height),
                ));
                if end.y > start.y + height {
                    quads.push(Bounds::from_corners(
                        point(bounds.left(), start.y + height),
                        point(bounds.right(), end.y),
                    ));
                }
                quads.push(Bounds::from_corners(
                    point(bounds.left(), end.y),
                    point(end.x, end.y + height),
                ));
            }
            for quad in quads {
                window.paint_quad(fill(quad, rgb(color)));
            }
        }
        }
        self.styled
            .paint(id, inspector, bounds, &mut (), &mut (), window, cx);
        let row = self.row;
        let selection = self.selection.clone();
        let text = self.text.clone();
        let focus = self.focus.clone();
        let down_layout = layout.clone();
        let down_hitbox = hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble()
                || event.button != MouseButton::Left
                || !down_hitbox.is_hovered(window)
            {
                return;
            }
            let byte = down_layout
                .index_for_position(event.position)
                .unwrap_or_else(|index| index);
            focus.focus(window, cx);
            let mut selection = selection.borrow_mut();
            match event.click_count {
                2 => selection.select_word(row, &text, byte),
                3.. => selection.select_line(row, &text),
                _ => selection.begin(SelectionPoint { row, byte }, event.modifiers.shift),
            }
            window.refresh();
            cx.stop_propagation();
        });
        let selection = self.selection.clone();
        let viewport = self.viewport.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase.bubble()
                && event.pressed_button == Some(MouseButton::Left)
                && viewport
                    .borrow()
                    .is_some_and(|v| v.contains(&event.position))
                && event.position.y >= bounds.top()
                && event.position.y < bounds.bottom()
                && selection.borrow().dragging
            {
                let byte = layout
                    .index_for_position(event.position)
                    .unwrap_or_else(|index| index);
                selection.borrow_mut().extend(SelectionPoint { row, byte });
                window.refresh();
            }
        });
    }
}
