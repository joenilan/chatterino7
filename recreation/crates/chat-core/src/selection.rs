//! Per-transcript selection anchored to stable row ordinals, not mounted widgets.
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub row: u64,
    pub byte: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub anchor: Option<Point>,
    pub head: Option<Point>,
    pub dragging: bool,
}
impl Selection {
    pub fn begin(&mut self, point: Point, extend: bool) {
        if !extend || self.anchor.is_none() {
            self.anchor = Some(point);
        }
        self.head = Some(point);
        self.dragging = true;
    }
    pub fn extend(&mut self, point: Point) {
        if self.dragging {
            self.head = Some(point);
        }
    }
    pub fn finish(&mut self) {
        self.dragging = false;
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn select_span(&mut self, anchor: Point, head: Point) {
        self.anchor = Some(anchor);
        self.head = Some(head);
        self.dragging = false;
    }
    pub fn select_line(&mut self, row: u64, text: &str) {
        self.anchor = Some(Point { row, byte: 0 });
        self.head = Some(Point {
            row,
            byte: text.len(),
        });
        self.dragging = false;
    }
    pub fn select_word(&mut self, row: u64, text: &str, byte: usize) {
        let byte = boundary(text, byte);
        let Some((_, ch)) = text
            .char_indices()
            .find(|(index, _)| *index >= byte)
            .or_else(|| text.char_indices().last())
        else {
            return;
        };
        let class = |c: char| {
            if c.is_alphanumeric() || c == '_' {
                0
            } else if c.is_whitespace() {
                1
            } else {
                2
            }
        };
        let target = class(ch);
        let at = if byte == text.len() {
            text.char_indices().last().map_or(0, |(i, _)| i)
        } else {
            byte
        };
        let mut start = at;
        for (i, c) in text[..at].char_indices().rev() {
            if class(c) != target || target == 2 {
                break;
            }
            start = i;
        }
        let mut end = at;
        for (i, c) in text[at..].char_indices() {
            if i != 0 && (class(c) != target || target == 2) {
                break;
            }
            end = at + i + c.len_utf8();
        }
        self.anchor = Some(Point { row, byte: start });
        self.head = Some(Point { row, byte: end });
        self.dragging = false;
    }
    pub fn range_for(&self, row: u64, text: &str) -> Option<Range<usize>> {
        let (anchor, head) = (self.anchor?, self.head?);
        if anchor == head {
            return None;
        }
        let (start, end) = if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        };
        if row < start.row || row > end.row {
            return None;
        }
        let a = if row == start.row {
            boundary(text, start.byte)
        } else {
            0
        };
        let b = if row == end.row {
            boundary(text, end.byte)
        } else {
            text.len()
        };
        Some(a.min(b)..b.max(a))
    }
    /// Caller supplies the retained document in ascending row order, including
    /// rows outside the current viewport. Missing/pruned rows are not fabricated.
    pub fn copy<'a>(&self, rows: impl IntoIterator<Item = (u64, &'a str)>) -> String {
        rows.into_iter()
            .filter_map(|(row, text)| self.range_for(row, text).map(|range| &text[range]))
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn prune_before(&mut self, first_row: u64) {
        if self.anchor.is_some_and(|p| p.row < first_row)
            || self.head.is_some_and(|p| p.row < first_row)
        {
            self.clear();
        }
    }
}
fn boundary(text: &str, byte: usize) -> usize {
    let mut result = byte.min(text.len());
    while !text.is_char_boundary(result) {
        result -= 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn select_all_span_includes_offscreen_rows_and_unicode() {
        let mut s = Selection::default();
        s.select_span(
            Point { row: 4, byte: 0 },
            Point {
                row: 6,
                byte: "世界".len(),
            },
        );
        assert_eq!(
            s.copy([(4, "hello"), (5, "offscreen"), (6, "世界")]),
            "hello\noffscreen\n世界"
        );
        assert!(!s.dragging);
    }
    #[test]
    fn successful_copy_clear_prevents_stale_repeat_copy() {
        let mut s = Selection::default();
        s.select_line(4, "copy me");
        assert_eq!(s.copy([(4, "copy me")]), "copy me");
        s.clear();
        assert_eq!(s.copy([(4, "copy me")]), "");
        assert_eq!(s.anchor, None);
        assert_eq!(s.head, None);
        assert!(!s.dragging);
    }
    #[test]
    fn empty_click_does_not_produce_clipboard_payload() {
        let mut s = Selection::default();
        s.begin(Point { row: 4, byte: 2 }, false);
        s.finish();
        assert_eq!(s.copy([(4, "existing clipboard must stay")]), "");
    }
    #[test]
    fn escape_cancels_drag_without_changing_content() {
        let mut s = Selection::default();
        s.begin(Point { row: 4, byte: 0 }, false);
        s.extend(Point { row: 4, byte: 4 });
        s.clear();
        s.extend(Point { row: 4, byte: 8 });
        assert_eq!(s.copy([(4, "unchanged")]), "");
    }
    #[test]
    fn forward_and_reverse_copy_all_model_rows() {
        let rows = [(1, "alpha"), (2, "offscreen"), (3, "omega")];
        let mut s = Selection::default();
        s.begin(Point { row: 1, byte: 2 }, false);
        s.extend(Point { row: 3, byte: 3 });
        s.finish();
        assert_eq!(s.copy(rows), "pha\noffscreen\nome");
        s.begin(Point { row: 3, byte: 3 }, false);
        s.extend(Point { row: 1, byte: 2 });
        s.finish();
        assert_eq!(s.copy(rows), "pha\noffscreen\nome");
    }
    #[test]
    fn separate_panes_do_not_share_selection() {
        let mut a = Selection::default();
        let b = Selection::default();
        a.select_line(0, "left");
        assert_eq!(a.copy([(0, "left")]), "left");
        assert_eq!(b.copy([(0, "right")]), "");
    }
    #[test]
    fn utf8_invalid_offsets_are_clamped_safely() {
        let mut s = Selection::default();
        s.begin(Point { row: 0, byte: 1 }, false);
        s.extend(Point { row: 0, byte: 99 });
        assert_eq!(s.copy([(0, "日本語")]), "日本語");
    }
    #[test]
    fn selection_survives_new_arrivals_and_finishes_drag() {
        let mut s = Selection::default();
        s.select_line(5, "keep");
        s.extend(Point { row: 6, byte: 3 });
        assert_eq!(s.copy([(5, "keep"), (6, "new")]), "keep");
    }
    #[test]
    fn eviction_clears_only_affected_endpoints() {
        let mut s = Selection::default();
        s.select_line(5, "keep");
        s.prune_before(5);
        assert!(s.anchor.is_some());
        s.prune_before(6);
        assert!(s.anchor.is_none());
    }
    #[test]
    fn redacted_content_is_copied_from_current_model() {
        let mut s = Selection::default();
        s.select_line(0, "some original message");
        assert_eq!(s.copy([(0, "[message deleted]")]), "[message deleted]");
    }
    #[test]
    fn double_click_words_and_triple_click_line() {
        let mut s = Selection::default();
        s.select_word(0, "hello world", 8);
        assert_eq!(s.copy([(0, "hello world")]), "world");
        s.select_line(0, "hello world");
        assert_eq!(s.copy([(0, "hello world")]), "hello world");
    }
    #[test]
    fn shift_extends_existing_anchor() {
        let mut s = Selection::default();
        s.begin(Point { row: 0, byte: 1 }, false);
        s.finish();
        s.begin(Point { row: 0, byte: 4 }, true);
        s.finish();
        assert_eq!(s.copy([(0, "hello")]), "ell");
    }
}
