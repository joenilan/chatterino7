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
    author: Option<(String,Option<u32>)>,
    interaction: Option<crate::message_actions::Interaction>,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
}
impl ChatText {
    pub fn with_search(mut self, ranges:Vec<std::ops::Range<usize>>)->Self{self.search=ranges;self}

    pub fn with_interaction(mut self, interaction:crate::message_actions::Interaction)->Self{
        let mut highlights=Vec::new();
        if let Some((name,color))=&self.author{highlights.push((0..name.len(),HighlightStyle{color:Some(rgb(color.unwrap_or(crate::theme::MUTED)).into()),font_weight:Some(FontWeight::SEMIBOLD),..Default::default()}));}
        highlights.extend(interaction.links.iter().map(|link|(link.range.clone(),HighlightStyle{color:Some(rgb(0x91C7E8).into()),underline:Some(UnderlineStyle{thickness:px(1.),color:None,wavy:false}),..Default::default()})));
        self.styled=StyledText::new(self.text.clone()).with_highlights(highlights);self.interaction=Some(interaction);self
    }
    pub fn with_author(mut self, name: &str, color: Option<u32>) -> Self {
        self.author=Some((name.to_owned(),color));
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
            author: None,
            interaction: None,
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
    type PrepaintState = crate::media_hover::HoverState;
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
    ) -> crate::media_hover::HoverState {
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
        let hitbox=window.insert_hitbox(hit_bounds,HitboxBehavior::Normal);
        let mut regions=Vec::new();
        if !self.selection.borrow().dragging {if let Some(interaction)=&self.interaction{
            let mut targets=vec![(0..interaction.author_len,"Ctrl+click to inspect this chatter")];
            targets.extend(interaction.links.iter().map(|link|(link.range.clone(),"Ctrl+click to open link · Right-click for actions")));
            for (target,(range,hint)) in targets.into_iter().enumerate(){
                let mut boxes:Vec<Bounds<Pixels>>=Vec::new();
                let Some(target_text)=self.text.get(range.clone()) else{continue;};
                for (relative,ch) in target_text.char_indices(){
                    let offset=range.start+relative;
                    let Some(start)=layout.position_for_index(offset) else{continue;};
                    let Some(end)=layout.position_for_index(offset+ch.len_utf8()) else{continue;};
                    // At an exact wrap boundary GPUI reports the previous line's
                    // end for start. The character itself belongs to end's line.
                    let origin=if end.y==start.y {start}else{point(bounds.left(),end.y)};
                    let width=end.x-origin.x;
                    let slot=Bounds::new(origin,size(width,layout.line_height())).intersect(&hit_bounds);
                    if slot.size.width<=px(0.)||slot.size.height<=px(0.){continue;}
                    if let Some(last)=boxes.last_mut().filter(|last|last.top()==slot.top()&&(last.right()-slot.left()).abs()<px(1.)){last.size.width=slot.right()-last.left();}else{boxes.push(slot);}
                }
                for (part,slot) in boxes.into_iter().enumerate(){regions.push(crate::media_hover::region(format!("plain-hover-{}-{target}-{part}",self.row),slot,crate::media_hover::Content::Hint(hint.into()),window,cx));}
            }
        }}
        crate::media_hover::HoverState{hitbox,regions}
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut crate::media_hover::HoverState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for region in &mut state.regions {region.paint(window,cx);}
        let hitbox=&mut state.hitbox;
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
        let down_interaction=self.interaction.clone();
        if hitbox.is_hovered(window)&&window.modifiers().control&&layout.index_for_position(window.mouse_position()).ok().is_some_and(|b|self.interaction.as_ref().is_some_and(|i|i.target(b).is_some())){window.set_cursor_style(CursorStyle::PointingHand,hitbox);}
        if let Some(interaction)=self.interaction.clone(){let up_layout=layout.clone();let up_hitbox=hitbox.clone();window.on_mouse_event(move|event:&MouseUpEvent,phase,w,cx|{if phase.bubble(){let byte=up_hitbox.is_hovered(w).then(||up_layout.index_for_position(event.position).ok()).flatten();interaction.up(event,byte,w,cx);}});}

        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble()
                || event.button != MouseButton::Left
                || !down_hitbox.is_hovered(window)
            {
                return;
            }
            if down_interaction.as_ref().is_some_and(|i|i.down(event,down_layout.index_for_position(event.position).ok(),&selection.borrow())){cx.stop_propagation();return;}
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
