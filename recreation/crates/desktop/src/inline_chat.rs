//! Pixel-sized inline media with original source byte offsets for selection.
use crate::media::{DecodedMedia, EmoteKey, MediaCache};
use chat_core::{
    Fragment, Message,
    selection::{Point as SelectionPoint, Selection},
};
use gpui_kit::*;
use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};

fn media_width(layers:&[(EmoteKey,Option<Arc<DecodedMedia>>)]) -> f32 {
    let (key,image)=&layers[0];
    image.as_ref().map_or_else(||key.width(),|media|{
        let size=media.image.size(0);
        (28. * u32::from(size.width) as f32 / (u32::from(size.height) as f32).max(1.)).clamp(8.,112.)
    })
}
#[derive(Clone)]
struct Span {
    range: Range<usize>,
    author: bool,
    media: Option<Vec<(EmoteKey, Option<Arc<DecodedMedia>>)>>,
}
#[derive(Clone)]
struct Piece {
    range: Range<usize>,
    bounds: Bounds<Pixels>,
    text: Option<ShapedLine>,
    media: Vec<Option<Arc<DecodedMedia>>>,
}
#[derive(Default)]
struct Layout {
    pieces: Vec<Piece>,
    lines: Vec<Range<usize>>,
    line_height: Pixels,
    origin: Point<Pixels>,
}
impl Layout {
    fn hit(&self, point: Point<Pixels>) -> (usize, Option<Range<usize>>) {
        let point = point - self.origin;
        let row = ((f32::from(point.y.max(px(0.))) / f32::from(self.line_height).max(1.)) as usize)
            .min(self.lines.len().saturating_sub(1));
        let Some(indices) = self.lines.get(row) else {
            return (0, None);
        };
        let Some(first) = self.pieces.get(indices.start) else {
            return (0, None);
        };
        if point.x <= first.bounds.left() {
            return (first.range.start, None);
        }
        for piece in &self.pieces[indices.clone()] {
            if point.x <= piece.bounds.right() {
                if let Some(text) = &piece.text {
                    return (
                        piece.range.start + text.closest_index_for_x(point.x - piece.bounds.left()),
                        None,
                    );
                }
                let byte = if point.x < piece.bounds.left() + piece.bounds.size.width / 2. {
                    piece.range.start
                } else {
                    piece.range.end
                };
                return (byte, Some(piece.range.clone()));
            }
        }
        (self.pieces[indices.end - 1].range.end, None)
    }
}
pub struct InlineChat {
    id: ElementId,
    row: u64,
    text: SharedString,
    spans: Vec<Span>,
    name_color: u32,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    layout: Rc<RefCell<Layout>>,
}
impl InlineChat {
    pub fn new(
        message: &Message,
        row: u64,
        media: &mut MediaCache,
        selection: Rc<RefCell<Selection>>,
        focus: FocusHandle,
        viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
        cx: &mut App,
    ) -> Option<Self> {
        if message.deleted
            || !message
                .fragments
                .iter()
                .any(|f| matches!(f, Fragment::Emote { .. }))
        {
            return None;
        }
        let text: SharedString = message.copy_line().into();
        if text.contains(['\r', '\n']) {
            return None;
        }
        let mut spans = vec![
            Span {
                range: 0..message.display_name.len(),
                author: true,
                media: None,
            },
            Span {
                range: message.display_name.len()..message.display_name.len() + 2,
                author: false,
                media: None,
            },
        ];
        let mut offset = message.display_name.len() + 2;
        for run in chat_core::emotes::group_for_layout(&message.fragments) {
            let end = offset + run.copy_text().len();
            let mut assets = Vec::new();
            if let chat_core::emotes::RenderRun::EmoteStack { layers, .. } = &run {
                for (index, layer) in layers.iter().enumerate() {
                    let animated = layer.animated && !cx.reduce_motion();
                    let key = if layer.provider == "twitch" {
                        EmoteKey::twitch(&layer.id, animated)
                    } else if matches!(layer.provider.as_str(), "7tv" | "bttv" | "ffz") {
                        layer
                            .asset
                            .as_ref()
                            .and_then(|a| EmoteKey::community(&layer.provider, &layer.id, animated, a))
                    } else {
                        None
                    };
                    let Some(mut key) = key else {
                        if index == 0 {
                            break;
                        } else {
                            continue;
                        }
                    };
                    let mut image = media.get(&key, cx);
                    if image.is_none() && media.failed(&key) && key.animated {
                        key.animated = false;
                        image = media.get(&key, cx);
                    }
                    if image.is_none() && media.failed(&key) && index == 0 {
                        break;
                    }
                    assets.push((key, image));
                }
            }
            if end > offset {
                spans.push(Span {
                    range: offset..end,
                    author: false,
                    media: if assets.is_empty() {
                        None
                    } else {
                        Some(assets)
                    },
                });
            }
            offset = end;
        }
        if !spans.iter().any(|s| s.media.is_some()) {
            return None;
        }
        Some(Self {
            id: format!("inline-{}", message.id).into(),
            row,
            text,
            spans,
            name_color: message.name_color.unwrap_or(crate::theme::MUTED),
            selection,
            focus,
            viewport,
            layout: Rc::default(),
        })
    }
}
impl IntoElement for InlineChat {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for InlineChat {
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
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, ()) {
        let text = self.text.clone();
        let spans = self.spans.clone();
        let layout = self.layout.clone();
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height().max(px(30.));
        let name_color = self.name_color;
        let id = window.request_measured_layout(
            Default::default(),
            move |known, available, window, cx| {
                let width = known
                    .width
                    .or_else(|| {
                        if let AvailableSpace::Definite(w) = available.width {
                            Some(w)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(px(800.))
                    .max(px(1.));
                let mut budget = width;
                let mut result = Layout::default();
                for _ in 0..8 {
                    result = Layout {
                        line_height,
                        ..Default::default()
                    };
                    let mut actual = px(0.);
                    let fragments: Vec<_> = spans
                        .iter()
                        .map(|s| {
                            if let Some(media) = &s.media {
                                LineFragment::element(px(media_width(media)), s.range.len())
                            } else {
                                LineFragment::text(&text[s.range.clone()])
                            }
                        })
                        .collect();
                    let mut wrapper = cx.text_system().line_wrapper(style.font(), font_size);
                    let mut boundaries: Vec<_> = wrapper
                        .wrap_line(&fragments, budget, IndentAdjustment::NoIndent)
                        .map(|b| b.ix)
                        .filter(|i| *i > 0 && *i < text.len())
                        .collect();
                    boundaries.push(text.len());
                    boundaries.dedup();
                    let mut start = 0;
                    let mut y = px(0.);
                    for end in boundaries {
                        let begin_piece = result.pieces.len();
                        let mut x = px(0.);
                        for span in &spans {
                            let a = start.max(span.range.start);
                            let b = end.min(span.range.end);
                            if a >= b {
                                continue;
                            }
                            let (shaped, media, w) = if let Some(layers) = &span.media {
                                (
                                    None,
                                    layers.iter().map(|(_, image)| image.clone()).collect(),
                                    px(media_width(layers)),
                                )
                            } else {
                                let mut run_style = style.clone();
                                if span.author {
                                    run_style.color = rgb(name_color).into();
                                    run_style.font_weight = FontWeight::SEMIBOLD;
                                }
                                let shaped = window.text_system().shape_line(
                                    text[a..b].to_owned().into(),
                                    font_size,
                                    &[run_style.to_run(b - a)],
                                    None,
                                );
                                let w = shaped.width();
                                (Some(shaped), Vec::new(), w)
                            };
                            result.pieces.push(Piece {
                                range: a..b,
                                bounds: Bounds::new(point(x, y), size(w, line_height)),
                                text: shaped,
                                media,
                            });
                            x += w;
                        }
                        if result.pieces.len() > begin_piece {
                            result.lines.push(begin_piece..result.pieces.len());
                            y += line_height;
                        }
                        actual = actual.max(x);
                        start = end;
                    }
                    if actual <= width + px(0.5) || budget <= px(28.) {
                        break;
                    }
                    budget = (budget - (actual - width) - px(1.)).max(px(28.));
                }
                let height = line_height * result.lines.len().max(1) as f32;
                *layout.borrow_mut() = result;
                size(width, height)
            },
        );
        (id, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> Hitbox {
        self.layout.borrow_mut().origin = bounds.origin;
        let pointer = window.mouse_position();
        if self.selection.borrow().dragging
            && self.viewport.borrow().is_some_and(|v| v.contains(&pointer))
            && pointer.y >= bounds.top()
            && pointer.y < bounds.bottom()
        {
            let byte = self.layout.borrow().hit(pointer).0;
            self.selection.borrow_mut().extend(SelectionPoint {
                row: self.row,
                byte,
            });
        }
        let hit = self.viewport.borrow().map_or(bounds, |v| {
            Bounds::from_corners(
                point(v.left(), bounds.top().max(v.top())),
                point(
                    (v.right() - px(12.)).max(v.left()),
                    bounds
                        .bottom()
                        .min(v.bottom())
                        .max(bounds.top().max(v.top())),
                ),
            )
        });
        window.insert_hitbox(hit, HitboxBehavior::Normal)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected = self.selection.borrow().range_for(self.row, &self.text);
        for piece in &self.layout.borrow().pieces {
            if let Some(range) = &selected {
                let a = range.start.max(piece.range.start);
                let b = range.end.min(piece.range.end);
                if a < b {
                    let (x1, x2) = piece
                        .text
                        .as_ref()
                        .map(|t| {
                            (
                                t.x_for_index(a - piece.range.start),
                                t.x_for_index(b - piece.range.start),
                            )
                        })
                        .unwrap_or((px(0.), piece.bounds.size.width));
                    let origin = bounds.origin + piece.bounds.origin;
                    window.paint_quad(fill(
                        Bounds::new(
                            origin + point(x1.min(x2), px(0.)),
                            size((x2 - x1).abs(), piece.bounds.size.height),
                        ),
                        rgb(0x315166),
                    ));
                }
            }
            if let Some(text) = &piece.text {
                let _ = text.paint(
                    bounds.origin + piece.bounds.origin,
                    piece.bounds.size.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            } else if piece.media.iter().all(Option::is_none) {
                // Fixed placeholder geometry keeps loading from shifting wrapped rows.
                let label: String = self.text[piece.range.clone()].chars().take(3).collect();
                let mut style = window.text_style();
                style.color = rgb(crate::theme::MUTED).into();
                let line = window.text_system().shape_line(
                    label.clone().into(),
                    px(9.),
                    &[style.to_run(label.len())],
                    None,
                );
                let _ = line.paint(
                    bounds.origin + piece.bounds.origin,
                    piece.bounds.size.height,
                    TextAlign::Left,
                    Some(px(28.)),
                    window,
                    cx,
                );
            }
        }
        for piece in &self.layout.borrow().pieces {
            let slot = Bounds::new(
                bounds.origin
                    + piece.bounds.origin
                    + point(px(0.), (piece.bounds.size.height - px(28.)) / 2.),
                size(piece.bounds.size.width, px(28.)),
            );
            let clip = self.viewport.borrow().map_or(slot, |v| slot.intersect(&v));
            if clip.size.width <= px(0.) || clip.size.height <= px(0.) {
                continue;
            }
            for media in piece.media.iter().flatten() {
                let frame = media.frame(cx.reduce_motion());
                if media.image.frame_count() > 1 && !cx.reduce_motion() {
                    window.request_animation_frame();
                }
                let image_bounds = ObjectFit::Contain.get_bounds(slot, media.image.size(frame));
                let _ = window.paint_image(
                    clip,
                    image_bounds,
                    Corners::all(px(0.)),
                    media.image.clone(),
                    frame,
                    false,
                );
            }
        }
        if let Some(range) = &selected {
            for piece in &self.layout.borrow().pieces {
                if piece.text.is_none()
                    && range.start < piece.range.end
                    && range.end > piece.range.start
                {
                    window.paint_quad(fill(
                        Bounds::new(bounds.origin + piece.bounds.origin, piece.bounds.size),
                        rgba(0x31516666),
                    ));
                }
            }
        }

        let layout = self.layout.clone();
        let selection = self.selection.clone();
        let text = self.text.clone();
        let focus = self.focus.clone();
        let hitbox = hitbox.clone();
        let row = self.row;
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble() || event.button != MouseButton::Left || !hitbox.is_hovered(window) {
                return;
            }
            let (byte, emote) = layout.borrow().hit(event.position);
            focus.focus(window, cx);
            let mut selection = selection.borrow_mut();
            match event.click_count {
                2 => {
                    if let Some(range) = emote {
                        selection.select_span(
                            SelectionPoint {
                                row,
                                byte: range.start,
                            },
                            SelectionPoint {
                                row,
                                byte: range.end,
                            },
                        );
                    } else {
                        selection.select_word(row, &text, byte);
                    }
                }
                3.. => selection.select_line(row, &text),
                _ => selection.begin(SelectionPoint { row, byte }, event.modifiers.shift),
            }
            window.refresh();
            cx.stop_propagation();
        });
        let layout = self.layout.clone();
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
                let byte = layout.borrow().hit(event.position).0;
                selection.borrow_mut().extend(SelectionPoint { row, byte });
                window.refresh();
            }
        });
    }
}
