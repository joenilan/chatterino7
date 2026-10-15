//! Pixel-sized inline media with original source byte offsets for selection.
use crate::media::{DecodedMedia, EmoteKey, MediaCache};
use chat_core::{
    Fragment, Message,
    selection::{Point as SelectionPoint, Selection},
};
use gpui_kit::*;
use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};

fn media_width(layers: &[(EmoteKey, Option<Arc<DecodedMedia>>)]) -> f32 {
    let (key, image) = &layers[0];
    image.as_ref().map_or_else(
        || key.width(),
        |media| {
            let size = media.image.size(0);
            (28. * u32::from(size.width) as f32 / (u32::from(size.height) as f32).max(1.))
                .clamp(8., 112.)
        },
    )
}
#[derive(Clone)]
struct Span {
    range: Range<usize>,
    author: bool,
    media: Option<Vec<(EmoteKey, Option<Arc<DecodedMedia>>)>>,
}
#[derive(Clone)]
struct Piece {
    author: bool,
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
    fn exact_text_hit(&self, point: Point<Pixels>) -> Option<usize> {
        let point = point - self.origin;
        self.pieces
            .iter()
            .find(|p| p.text.is_some() && p.bounds.contains(&point))
            .and_then(|p| {
                p.text
                    .as_ref()?
                    .index_for_x(point.x - p.bounds.left())
                    .map(|index| p.range.start + index)
            })
    }

    fn hit(&self, point: Point<Pixels>) -> (usize, Option<Range<usize>>) {
        let point = point - self.origin;
        let row = ((f32::from(point.y.max(px(0.))) / f32::from(self.line_height).max(1.)) as usize)
            .min(self.lines.len().saturating_sub(1));
        let Some(indices) = self.lines.get(row) else {
            return (0, None);
        };
        if indices.is_empty() {
            return (0, None);
        }
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
/// Draw deletion markup only across body glyphs/media. The source is still
/// one unchanged byte string: selection, emote targets and copying never see
/// the visual strike. Each piece already has its post-wrap x/y coordinates.
fn deleted_stroke(piece: &Piece, body_start: usize) -> Option<Bounds<Pixels>> {
    if piece.range.end <= body_start {
        return None;
    }
    let first = piece.range.start.max(body_start);
    let from_x = piece
        .text
        .as_ref()
        .map_or(px(0.), |text| text.x_for_index(first - piece.range.start));
    let width = piece.bounds.size.width - from_x;
    if width <= px(0.) {
        return None;
    }
    Some(Bounds::new(
        point(
            piece.bounds.left() + from_x,
            piece.bounds.top() + piece.bounds.size.height * 0.52,
        ),
        size(width, px(1.)),
    ))
}

/// Plan byte-preserving visual wraps: line one has less room because it also
/// contains timestamp/badges/actions, but every continuation gets full width.
/// `wrap_from` returns sorted *absolute* byte offsets, never inserted newlines.
fn plan_line_boundaries<F>(
    text_len: usize,
    first_width: Pixels,
    full_width: Pixels,
    mut wrap_from: F,
) -> (usize, Vec<usize>)
where
    F: FnMut(usize, Pixels) -> Vec<usize>,
{
    let first_end = if first_width > px(14.) {
        wrap_from(0, first_width)
            .into_iter()
            .next()
            .unwrap_or(text_len)
    } else {
        0
    };
    let mut boundaries = Vec::new();
    if first_end > 0 {
        boundaries.push(first_end);
    }
    if first_end < text_len {
        boundaries.extend(wrap_from(first_end, full_width));
        boundaries.push(text_len);
    }
    boundaries.dedup();
    (first_end, boundaries)
}

pub struct InlineChat {
    id: ElementId,
    row: u64,
    search: Vec<std::ops::Range<usize>>,
    text: SharedString,
    spans: Vec<Span>,
    name_color: u32,
    paint: Option<crate::name_paint::Request>,
    emote_size: f32,
    first_line_inset: Pixels,
    selectable: bool,
    strike_from: Option<usize>,
    interaction: Option<crate::message_actions::Interaction>,
    selection: Rc<RefCell<Selection>>,
    focus: FocusHandle,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    layout: Rc<RefCell<Layout>>,
}
impl InlineChat {
    pub fn with_interaction(mut self, interaction: crate::message_actions::Interaction) -> Self {
        self.interaction = Some(interaction);
        self
    }

    pub fn with_search(mut self, ranges: Vec<std::ops::Range<usize>>) -> Self {
        self.search = ranges;
        self
    }

    /// Metadata is painted at the row origin. Only the first visual line reserves
    /// its width; later lines wrap to the ordinary left padding.
    pub fn with_first_line_inset(mut self, inset: Pixels) -> Self {
        self.first_line_inset = inset;
        self
    }

    pub fn visual_line_height(&self, window: &Window) -> Pixels {
        window.line_height().max(px(self.emote_size + 2.))
    }

    /// Plain and rich messages use the same soft-wrap, hit-test and selection
    /// coordinates. This retains the original copy_line bytes and author offsets.
    pub fn plain(
        message: &Message,
        paint: Option<crate::name_paint::Request>,
        row: u64,
        selection: Rc<RefCell<Selection>>,
        focus: FocusHandle,
        viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        let text: SharedString = message.copy_line().into();
        let author_end = message.author_label().len().min(text.len());
        let strike_from = message.deleted.then_some((author_end + 2).min(text.len()));
        let mut spans = vec![Span {
            range: 0..author_end,
            author: true,
            media: None,
        }];
        if author_end < text.len() {
            spans.push(Span {
                range: author_end..text.len(),
                author: false,
                media: None,
            });
        }
        Self {
            id: format!("inline-{}", message.id).into(),
            row,
            search: Vec::new(),
            text,
            spans,
            name_color: message.author_color().unwrap_or(crate::theme::muted()),
            paint,
            emote_size: 0.,
            first_line_inset: px(0.),
            selectable: true,
            strike_from,
            interaction: None,
            selection,
            focus,
            viewport,
            layout: Rc::default(),
        }
    }

    /// Typed notices retain text-only presentation, with the same full-width
    /// soft wraps. These were not selectable before: avoid mapping their
    /// notice-local offsets to Message::copy_line's different author prefix.
    pub fn notice(
        id: &str,
        notice: impl Into<SharedString>,
        row: u64,
        selection: Rc<RefCell<Selection>>,
        focus: FocusHandle,
        viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        let text = notice.into();
        let spans = if text.is_empty() {
            Vec::new()
        } else {
            vec![Span {
                range: 0..text.len(),
                author: false,
                media: None,
            }]
        };
        Self {
            id: format!("inline-notice-{id}").into(),
            row,
            search: Vec::new(),
            text,
            spans,
            name_color: crate::theme::muted(),
            paint: None,
            emote_size: 0.,
            first_line_inset: px(0.),
            selectable: false,
            strike_from: None,
            interaction: None,
            selection,
            focus,
            viewport,
            layout: Rc::default(),
        }
    }

    pub fn new(
        message: &Message,
        paint: Option<crate::name_paint::Request>,
        row: u64,
        media: &mut MediaCache,
        selection: Rc<RefCell<Selection>>,
        focus: FocusHandle,
        viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
        cx: &mut App,
    ) -> Option<Self> {
        // Emote images off: the plain text row shows their names.
        if !emote_option(0) {
            return None;
        }
        let adjusted;
        let message = if emote_option(1) && !emote_option(2) && !emote_option(3) {
            message
        } else {
            let mut copy = message.clone();
            copy.fragments = displayed_fragments(&message.fragments);
            adjusted = copy;
            &adjusted
        };
        if !message.fragments.iter().any(|f| {
            matches!(
                f,
                Fragment::Emote { .. } | Fragment::Cheer { asset: Some(_), .. }
            )
        }) {
            return None;
        }
        let text: SharedString = message.copy_line().into();
        if text.contains(['\r', '\n']) {
            return None;
        }
        let mut spans = vec![
            Span {
                range: 0..message.author_label().len(),
                author: true,
                media: None,
            },
            Span {
                range: message.author_label().len()..message.author_label().len() + 2,
                author: false,
                media: None,
            },
        ];
        let mut offset = message.author_label().len() + 2;
        for run in chat_core::emotes::group_for_layout(&message.fragments) {
            let end = offset + run.copy_text().len();
            let mut assets = Vec::new();
            if let chat_core::emotes::RenderRun::EmoteStack { layers, .. } = &run {
                for (index, layer) in layers.iter().enumerate() {
                    let animated = layer.animated && !cx.reduce_motion();
                    let key = if layer.provider == "twitch" {
                        EmoteKey::twitch(&layer.id, animated)
                    } else if layer.provider == "twitch-cheer" {
                        layer
                            .asset
                            .as_ref()
                            .and_then(|a| EmoteKey::cheer(&layer.id, animated, a))
                    } else if layer.provider == "emoji" {
                        layer
                            .asset
                            .as_ref()
                            .and_then(|a| EmoteKey::emoji(&layer.id, a))
                    } else if matches!(layer.provider.as_str(), "7tv" | "bttv" | "ffz") {
                        layer.asset.as_ref().and_then(|a| {
                            EmoteKey::community(&layer.provider, &layer.id, animated, a)
                        })
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
                    let mut image = if index == 0 {
                        media.get_layout(&key, cx)
                    } else {
                        media.get(&key, cx)
                    };
                    if image.is_none() && media.failed(&key) && key.animated {
                        key.animated = false;
                        image = if index == 0 {
                            media.get_layout(&key, cx)
                        } else {
                            media.get(&key, cx)
                        };
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
        let strike_from = message
            .deleted
            .then_some((message.author_label().len() + 2).min(text.len()));
        Some(Self {
            id: format!("inline-{}", message.id).into(),
            row,
            search: Vec::new(),
            text,
            spans,
            name_color: message.author_color().unwrap_or(crate::theme::muted()),
            paint,
            emote_size: base_emote_size(message) * emote_scale(),
            first_line_inset: px(0.),
            selectable: true,
            strike_from,
            interaction: None,
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
    type PrepaintState = crate::media_hover::HoverState;
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
        let emote_size = self.emote_size;
        let line_height = window.line_height().max(px(emote_size + 2.));
        let name_color = self.name_color;
        let first_line_inset = self.first_line_inset;
        let id = window.request_measured_layout(
            Default::default(),
            move |known, available, window, cx| {
                let width = known
                    .width
                    .or({
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
                    // Wrap the first visual line using only the space following the
                    // timestamp/actions/source/badges. Reflow the remaining text
                    // against the ENTIRE row width, not a permanent flex gutter.
                    // The same source spans and byte offsets drive hit testing.
                    let wrap_from = |start: usize, line_budget: Pixels| {
                        let fragments: Vec<_> = spans
                            .iter()
                            .filter_map(|span| {
                                let a = start.max(span.range.start);
                                let b = span.range.end;
                                if a >= b {
                                    return None;
                                }
                                Some(if let Some(media) = &span.media {
                                    LineFragment::element(
                                        px(media_width(media) * emote_size / 28.).min(width),
                                        b - a,
                                    )
                                } else {
                                    LineFragment::text(&text[a..b])
                                })
                            })
                            .collect();
                        let mut wrapper = cx.text_system().line_wrapper(style.font(), font_size);
                        wrapper
                            .wrap_line(&fragments, line_budget, IndentAdjustment::NoIndent)
                            .map(|boundary| start + boundary.ix)
                            .filter(|end| *end > start && *end < text.len())
                            .collect::<Vec<_>>()
                    };
                    let (first_end, boundaries) = plan_line_boundaries(
                        text.len(),
                        budget - first_line_inset,
                        budget,
                        wrap_from,
                    );

                    let mut actual = px(0.);
                    let mut start = 0;
                    // A metadata-heavy row may have no space for the message on
                    // line 1. Leave that first visual line for its metadata.
                    let mut y = if first_end == 0 {
                        result.lines.push(0..0);
                        line_height
                    } else {
                        px(0.)
                    };
                    for end in boundaries {
                        let begin_piece = result.pieces.len();
                        let mut x = if start == 0 && first_end > 0 {
                            first_line_inset
                        } else {
                            px(0.)
                        };
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
                                    px(media_width(layers) * emote_size / 28.).min(width),
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
                                author: span.author,
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
        cx: &mut App,
    ) -> crate::media_hover::HoverState {
        self.layout.borrow_mut().origin = bounds.origin;
        let pointer = window.mouse_position();
        if self.selectable
            && self.selection.borrow().dragging
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
        let hitbox = window.insert_hitbox(hit, HitboxBehavior::Normal);
        let mut regions = Vec::new();
        if !self.selection.borrow().dragging {
            for (index, piece) in self.layout.borrow().pieces.iter().enumerate() {
                if piece.text.is_none() {
                    let slot = Bounds::new(
                        bounds.origin
                            + piece.bounds.origin
                            + point(
                                px(0.),
                                (piece.bounds.size.height - px(self.emote_size)) / 2.,
                            ),
                        size(piece.bounds.size.width, px(self.emote_size)),
                    );
                    let slot = slot.intersect(&hit);
                    if slot.size.width <= px(0.) || slot.size.height <= px(0.) {
                        continue;
                    }
                    let source = self
                        .spans
                        .iter()
                        .find(|s| {
                            s.range.start <= piece.range.start && s.range.end >= piece.range.end
                        })
                        .and_then(|s| s.media.as_ref())
                        .and_then(|m| m.first())
                        .map(|(key, _)| crate::media_hover::source(key.provider()))
                        .unwrap_or("Emote");
                    regions.push(crate::media_hover::region(
                        format!("emote-hover-{}-{index}", self.row),
                        slot,
                        crate::media_hover::Content::Emote {
                            label: self.text[piece.range.clone()].into(),
                            source: source.into(),
                            images: piece.media.iter().flatten().cloned().collect(),
                        },
                        window,
                        cx,
                    ));
                } else if let (Some(text), Some(interaction)) = (&piece.text, &self.interaction) {
                    use crate::media_hover::Content;
                    let mut targets = vec![(
                        0..interaction.author_len,
                        Content::Hint(crate::message_actions::name_hint("this chatter")),
                    )];
                    targets.extend(
                        interaction
                            .links
                            .iter()
                            .map(|link| (link.range.clone(), Content::Link(link.url.clone()))),
                    );
                    targets.extend(interaction.mentions.iter().map(|mention| {
                        (
                            mention.range.clone(),
                            Content::Hint(crate::message_actions::name_hint("mentioned chatter")),
                        )
                    }));
                    for (target, (range, content)) in targets.into_iter().enumerate() {
                        let a = range.start.max(piece.range.start);
                        let b = range.end.min(piece.range.end);
                        if a >= b {
                            continue;
                        }
                        let left = text.x_for_index(a - piece.range.start);
                        let right = text.x_for_index(b - piece.range.start);
                        let slot = Bounds::new(
                            bounds.origin + piece.bounds.origin + point(left, px(0.)),
                            size(right - left, piece.bounds.size.height),
                        )
                        .intersect(&hit);
                        if slot.size.width > px(0.) && slot.size.height > px(0.) {
                            regions.push(crate::media_hover::region(
                                format!("text-hover-{}-{index}-{target}", self.row),
                                slot,
                                content.clone(),
                                window,
                                cx,
                            ));
                        }
                    }
                }
            }
        }
        crate::media_hover::HoverState { hitbox, regions }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut crate::media_hover::HoverState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for region in &mut state.regions {
            region.paint(window, cx);
        }
        let hitbox = &mut state.hitbox;
        let selected = if self.selectable {
            self.selection.borrow().range_for(self.row, &self.text)
        } else {
            None
        };
        if let Some(interaction) = &self.interaction {
            for piece in &self.layout.borrow().pieces {
                if let Some(text) = &piece.text {
                    for (range, color) in interaction
                        .links
                        .iter()
                        .map(|link| (&link.range, 0x91C7E8))
                        .chain(
                            interaction
                                .mentions
                                .iter()
                                .map(|mention| (&mention.range, mention.color.unwrap_or(0xB6A9E6))),
                        )
                    {
                        let a = range.start.max(piece.range.start);
                        let b = range.end.min(piece.range.end);
                        if a < b {
                            let left = text.x_for_index(a - piece.range.start);
                            let right = text.x_for_index(b - piece.range.start);
                            window.paint_quad(fill(
                                Bounds::new(
                                    bounds.origin
                                        + piece.bounds.origin
                                        + point(left, piece.bounds.size.height - px(3.)),
                                    size((right - left).max(px(1.)), px(1.)),
                                ),
                                rgb(color),
                            ));
                        }
                    }
                }
            }
            if hitbox.is_hovered(window)
                && self
                    .layout
                    .borrow()
                    .exact_text_hit(window.mouse_position())
                    .is_some_and(|b| {
                        interaction.target(b).is_some_and(|action| {
                            window.modifiers().control
                                || crate::message_actions::plain_click(&action)
                        })
                    })
            {
                window.set_cursor_style(CursorStyle::PointingHand, hitbox);
            }
        }

        let mut highlights = self
            .search
            .iter()
            .cloned()
            .map(|r| (r, 0x66502D))
            .collect::<Vec<_>>();
        if let Some(range) = selected.clone() {
            highlights.push((range, 0x315166));
        }
        for piece in &self.layout.borrow().pieces {
            for (range, color) in &highlights {
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
                        rgb(*color),
                    ));
                }
            }
            if let Some(text) = &piece.text {
                if piece.author
                    && piece.range.start == 0
                    && self.spans.first().is_some_and(|s| s.range == piece.range)
                    && self.paint.as_ref().is_some_and(|paint| {
                        paint.draw(
                            text,
                            bounds.origin + piece.bounds.origin,
                            piece.bounds.size.height,
                            self.name_color,
                            window,
                            cx,
                        )
                    })
                {
                    continue;
                }
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
                style.color = rgb(crate::theme::muted()).into();
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
                    + point(
                        px(0.),
                        (piece.bounds.size.height - px(self.emote_size)) / 2.,
                    ),
                size(piece.bounds.size.width, px(self.emote_size)),
            );
            let clip = self.viewport.borrow().map_or(slot, |v| slot.intersect(&v));
            if clip.size.width <= px(0.) || clip.size.height <= px(0.) {
                continue;
            }
            for media in piece.media.iter().flatten() {
                let frame = media.frame(crate::media::still(cx));
                if media.image.frame_count() > 1 && !crate::media::still(cx) {
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
        for (range, color) in &highlights {
            for piece in &self.layout.borrow().pieces {
                if piece.text.is_none()
                    && range.start < piece.range.end
                    && range.end > piece.range.start
                {
                    window.paint_quad(fill(
                        Bounds::new(bounds.origin + piece.bounds.origin, piece.bounds.size),
                        rgba((*color << 8) | 0x66),
                    ));
                }
            }
        }

        // Paint the strike AFTER media and search/selection overlays so it
        // stays visible on both static and animated emotes. Only the body gets
        // crossed out: author, badges, timestamp and metadata stay intact.
        if let Some(body_start) = self.strike_from {
            for piece in &self.layout.borrow().pieces {
                if let Some(stroke) = deleted_stroke(piece, body_start) {
                    window.paint_quad(fill(
                        Bounds::new(bounds.origin + stroke.origin, stroke.size),
                        rgb(crate::theme::text()),
                    ));
                }
            }
        }

        if !self.selectable {
            return;
        }
        let layout = self.layout.clone();
        let selection = self.selection.clone();
        let text = self.text.clone();
        let focus = self.focus.clone();
        let hitbox = hitbox.clone();
        let row = self.row;
        let down_interaction = self.interaction.clone();
        if let Some(interaction) = self.interaction.clone() {
            let up_layout = layout.clone();
            let up_hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, w, cx| {
                if phase.bubble() {
                    let byte = up_hitbox
                        .is_hovered(w)
                        .then(|| up_layout.borrow().exact_text_hit(event.position))
                        .flatten();
                    interaction.up(event, byte, w, cx);
                }
            });
        }
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            // Capture phase: the message menu opens in bubble before this text sees it.
            if phase.capture()
                && event.button == MouseButton::Right
                && hitbox.is_hovered(window)
                && down_interaction.as_ref().is_some_and(|i| {
                    i.right_click(
                        event,
                        layout.borrow().exact_text_hit(event.position),
                        window,
                        cx,
                    )
                })
            {
                cx.stop_propagation();
                return;
            }
            if !phase.bubble() || event.button != MouseButton::Left || !hitbox.is_hovered(window) {
                return;
            }
            if down_interaction.as_ref().is_some_and(|i| {
                i.down(
                    event,
                    layout.borrow().exact_text_hit(event.position),
                    &selection.borrow(),
                )
            }) {
                cx.stop_propagation();
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

/// Settings → Appearance → Emote options (Chatterino's): [images, zero-width
/// stacking, remove spaces between emotes, stack bits].
pub const EMOTE_OPTION_NAMES: [&str; 4] = ["images", "zero_width", "remove_spaces", "stack_bits"];
pub static EMOTE_OPTIONS: [std::sync::atomic::AtomicBool; 4] = [
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(true),
    std::sync::atomic::AtomicBool::new(false),
    std::sync::atomic::AtomicBool::new(false),
];
pub fn emote_option(index: usize) -> bool {
    EMOTE_OPTIONS[index].load(std::sync::atomic::Ordering::Relaxed)
}
/// The fragments as drawn under the emote options: zero-width emotes unstacked
/// when that's off, spaces between emotes dropped, runs of the same cheer
/// combined ("Cheer100 Cheer100" → "Cheer200").
pub fn displayed_fragments(fragments: &[Fragment]) -> Vec<Fragment> {
    arrange(fragments, emote_option(1), emote_option(2), emote_option(3))
}
fn arrange(
    fragments: &[Fragment],
    zero_width: bool,
    no_spaces: bool,
    stack_bits: bool,
) -> Vec<Fragment> {
    let mut out: Vec<Fragment> = Vec::with_capacity(fragments.len());
    let is_emote = |f: &Fragment| {
        matches!(
            f,
            Fragment::Emote { .. } | Fragment::Cheer { asset: Some(_), .. }
        )
    };
    for (index, fragment) in fragments.iter().enumerate() {
        let mut fragment = fragment.clone();
        if !zero_width && let Fragment::Emote { overlay, .. } = &mut fragment {
            *overlay = false;
        }
        // Whitespace between two emotes.
        if no_spaces
            && let Fragment::Text(text) = &fragment
            && text.trim().is_empty()
            && out.last().is_some_and(is_emote)
            && fragments.get(index + 1).is_some_and(is_emote)
        {
            continue;
        }
        if stack_bits && let Fragment::Cheer { prefix, bits, .. } = &fragment {
            // Skip the space we kept after the previous cheer, then merge.
            let space = matches!(out.last(), Some(Fragment::Text(t)) if t.trim().is_empty());
            let at = out.len() - usize::from(space);
            if let Some(Fragment::Cheer {
                prefix: p,
                bits: b,
                label,
                ..
            }) = at.checked_sub(1).and_then(|i| out.get_mut(i))
                && p.eq_ignore_ascii_case(prefix)
            {
                *b += bits;
                *label = format!("{}{}", &label[..p.len().min(label.len())], b);
                if space {
                    out.pop();
                }
                continue;
            }
        }
        out.push(fragment);
    }
    out
}
/// Settings → Appearance → Emote size, in percent of the normal 28 px.
pub static EMOTE_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);
pub fn emote_scale() -> f32 {
    EMOTE_SCALE
        .load(std::sync::atomic::Ordering::Relaxed)
        .clamp(50, 200) as f32
        / 100.
}
fn base_emote_size(message: &Message) -> f32 {
    if message.presentation.kind == "power_ups_gigantified_emote" {
        56.
    } else {
        28.
    }
}

#[cfg(test)]
mod wrapping_tests {
    use super::{Layout, Piece, plan_line_boundaries};
    use gpui_kit::{Bounds, Pixels, point, px, size};

    fn ten_pixel_fragments(start: usize, width: Pixels, len: usize) -> Vec<usize> {
        let per_line = (f32::from(width) / 10.).floor() as usize;
        ((start + per_line)..len).step_by(per_line).collect()
    }

    #[test]
    fn continuation_lines_reclaim_header_gutter() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        let (first_end, breaks) =
            plan_line_boundaries(text.len(), px(36.), px(100.), |start, width| {
                ten_pixel_fragments(start, width, text.len())
            });
        assert_eq!(first_end, 3);
        assert_eq!(breaks, [3, 13, 23, 26]);
        // Copy/selection still sees one logical message, without newlines or
        // metadata strings injected at the visual boundaries.
        let mut reconstructed = String::new();
        let mut start = 0;
        for end in breaks {
            reconstructed.push_str(&text[start..end]);
            start = end;
        }
        assert_eq!(reconstructed, text);
    }

    #[test]
    fn metadata_only_first_line_and_no_metadata_both_wrap_safely() {
        let len = 26;
        let wrap = |start, width| ten_pixel_fragments(start, width, len);
        let (first, breaks) = plan_line_boundaries(len, px(9.), px(100.), wrap);
        assert_eq!(first, 0);
        assert_eq!(breaks, [10, 20, 26]);
        let (first, breaks) = plan_line_boundaries(len, px(100.), px(100.), wrap);
        assert_eq!(first, 10);
        assert_eq!(breaks, [10, 20, 26]);
    }

    #[test]
    fn hit_testing_uses_each_visual_lines_actual_left_edge() {
        let layout = Layout {
            line_height: px(24.),
            origin: point(px(100.), px(200.)),
            lines: vec![0..1, 1..2],
            pieces: vec![
                Piece {
                    author: true,
                    range: 0..5,
                    bounds: Bounds::new(point(px(68.), px(0.)), size(px(32.), px(24.))),
                    text: None,
                    media: Vec::new(),
                },
                Piece {
                    author: false,
                    range: 5..18,
                    bounds: Bounds::new(point(px(0.), px(24.)), size(px(100.), px(24.))),
                    text: None,
                    media: Vec::new(),
                },
            ],
        };
        assert_eq!(layout.hit(point(px(132.), px(206.))).0, 0);
        assert_eq!(layout.hit(point(px(102.), px(230.))).0, 5);
    }

    #[test]
    fn deletion_stroke_skips_name_and_prefix_then_uses_wrapped_x_coordinates() {
        let part = |range: std::ops::Range<usize>, x: f32, y: f32, width: f32| Piece {
            author: false,
            range,
            bounds: Bounds::new(point(px(x), px(y)), size(px(width), px(24.))),
            text: None,
            media: Vec::new(),
        };
        // "Sery: deleted text here" -- strike after "Sery: ", not over
        // the author label or the colon, and at x=0 on line two.
        assert!(super::deleted_stroke(&part(0..4, 40., 0., 24.), 6).is_none());
        assert!(super::deleted_stroke(&part(4..6, 64., 0., 12.), 6).is_none());
        let line1 = super::deleted_stroke(&part(6..13, 76., 0., 60.), 6).unwrap();
        let line2 = super::deleted_stroke(&part(13..24, 0., 24., 110.), 6).unwrap();
        assert_eq!(line1.left(), px(76.));
        assert_eq!(line1.size.width, px(60.));
        assert_eq!(line2.left(), px(0.));
        assert_eq!(line2.size.width, px(110.));
        assert!(line2.top() > line1.top());
    }

    #[test]
    fn metadata_only_visual_line_has_no_fake_text_hit() {
        let layout = Layout {
            line_height: px(24.),
            origin: point(px(10.), px(20.)),
            lines: vec![0..0, 0..1],
            pieces: vec![Piece {
                author: false,
                range: 0..6,
                bounds: Bounds::new(point(px(0.), px(24.)), size(px(70.), px(24.))),
                text: None,
                media: Vec::new(),
            }],
        };
        assert_eq!(layout.hit(point(px(18.), px(25.))).0, 0);
        assert_eq!(layout.hit(point(px(11.), px(51.))).0, 0);
    }
}

#[cfg(test)]
mod option_tests {
    use super::{Fragment, arrange};
    fn emote(label: &str) -> Fragment {
        Fragment::Emote {
            provider: "7tv".into(),
            id: label.into(),
            label: label.into(),
            overlay: false,
            animated: false,
            asset: None,
        }
    }
    fn cheer(bits: u64) -> Fragment {
        Fragment::Cheer {
            prefix: "Cheer".into(),
            bits,
            tier: 1,
            label: format!("Cheer{bits}"),
            asset: None,
        }
    }
    #[test]
    fn spaces_between_emotes_and_stacked_bits() {
        let text = |t: &str| Fragment::Text(t.into());
        let fragments = vec![emote("A"), text(" "), emote("B"), text(" hi "), emote("C")];
        let kept: Vec<_> = arrange(&fragments, true, true, false)
            .iter()
            .map(|f| f.copy_text().to_owned())
            .collect();
        assert_eq!(kept, ["A", "B", " hi ", "C"]);
        let cheers = vec![cheer(100), text(" "), cheer(50), text(" gg")];
        let stacked: Vec<_> = arrange(&cheers, true, false, true)
            .iter()
            .map(|f| f.copy_text().to_owned())
            .collect();
        assert_eq!(stacked, ["Cheer150", " gg"]);
    }
}
