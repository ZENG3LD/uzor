//! Text selection model — a range over one text run, in chars.
//!
//! Pure data plus pure functions: no clock, no I/O, no widget ids. The same
//! helpers serve a text input (whose selection lives in
//! [`TextFieldStore`](super::TextFieldStore)) and selectable plain text
//! (whose one selection owner per window is the framework's job).
//!
//! Positions are **char indices** (not bytes): `0..=char_count`, a position
//! sits *between* chars. A selection is an `anchor` (where it started) and a
//! `focus` (where the pointer / caret is now); either may be the larger one.
//!
//! Word boundaries are whitespace runs, the same rule as the text store's
//! Ctrl+Arrow movement ([`word_left`] / [`word_right`] live here and the
//! store calls them), so a double click selects exactly what Ctrl+Shift+
//! Arrow would. Scripts without spaces (CJK) are one word per run, the same
//! as the store's word movement.
//!
//! Geometry: a text run is laid out as one or more [`SelectionLine`]s, each
//! carrying the x of every char boundary on that line. Build one for a
//! single-line label from its boundary positions ([`SelectionLine::single`])
//! or for wrapped text from the library's
//! [`WrappedLine`]s ([`SelectionLine::from_wrapped`]); then
//! [`highlight_rects`] gives the rects to fill under the glyphs and
//! [`char_at_point`] maps a pointer to a position.

use crate::render::WrappedLine;
use crate::types::Rect;

// =============================================================================
// TextSelection
// =============================================================================

/// A selection over one text run: `anchor` and `focus` in chars.
///
/// Empty (`anchor == focus`) is a caret. `Default` is a caret at 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextSelection {
    /// Where the selection started (the press, or the fixed end of a
    /// Shift-extend). Char index.
    pub anchor: usize,
    /// The moving end (pointer / caret). Char index.
    pub focus: usize,
}

impl TextSelection {
    /// A selection from `anchor` to `focus`.
    pub const fn new(anchor: usize, focus: usize) -> Self {
        Self { anchor, focus }
    }

    /// An empty selection (a caret) at `at`.
    pub const fn caret(at: usize) -> Self {
        Self { anchor: at, focus: at }
    }

    /// The whole of `text`.
    pub fn all(text: &str) -> Self {
        Self { anchor: 0, focus: text.chars().count() }
    }

    /// The word around the char at `char_idx` (see [`word_range_at`]).
    pub fn word_at(text: &str, char_idx: usize) -> Self {
        let (lo, hi) = word_range_at(text, char_idx);
        Self { anchor: lo, focus: hi }
    }

    /// The source line (split on `'\n'`) around `char_idx` (see
    /// [`line_range_at`]).
    pub fn line_at(text: &str, char_idx: usize) -> Self {
        let (lo, hi) = line_range_at(text, char_idx);
        Self { anchor: lo, focus: hi }
    }

    /// The visual line of a laid-out run around `char_idx` (triple click in
    /// wrapped text selects what the user sees as one line). Empty `lines`
    /// gives a caret at `char_idx`.
    pub fn visual_line_at(lines: &[SelectionLine], char_idx: usize) -> Self {
        match line_index_of(lines, char_idx) {
            Some(i) => Self { anchor: lines[i].start, focus: lines[i].end },
            None => Self::caret(char_idx),
        }
    }

    /// What a press with `click_count` selects at `char_idx`: 1 a caret,
    /// 2 the word, 3 (or more) the source line.
    pub fn for_click_count(text: &str, char_idx: usize, click_count: u8) -> Self {
        match click_count {
            0 | 1 => Self::caret(char_idx.min(text.chars().count())),
            2 => Self::word_at(text, char_idx),
            _ => Self::line_at(text, char_idx),
        }
    }

    /// Whether nothing is selected (a caret).
    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }

    /// `(lo, hi)` with `lo <= hi`.
    pub fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.focus), self.anchor.max(self.focus))
    }

    /// Lower end, in chars.
    pub fn start(&self) -> usize {
        self.anchor.min(self.focus)
    }

    /// Upper end, in chars.
    pub fn end(&self) -> usize {
        self.anchor.max(self.focus)
    }

    /// Selected char count.
    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    /// Whether the focus is before the anchor (a right-to-left drag).
    pub fn is_backward(&self) -> bool {
        self.focus < self.anchor
    }

    /// Move the focus, keeping the anchor (drag / Shift-extend).
    pub fn extend_to(&mut self, focus: usize) {
        self.focus = focus;
    }

    /// Collapse to a caret at the focus.
    pub fn collapse(&mut self) {
        self.anchor = self.focus;
    }

    /// Both ends clamped to `0..=char_count` (the text shrank).
    pub fn clamped(self, char_count: usize) -> Self {
        Self { anchor: self.anchor.min(char_count), focus: self.focus.min(char_count) }
    }

    /// Whether this covers all of `text` (non-empty text only) — the check
    /// a Ctrl+A escalation uses before widening to the next scope.
    pub fn is_all(&self, text: &str) -> bool {
        let n = text.chars().count();
        n > 0 && self.range() == (0, n)
    }

    /// The selected slice of `text`. Out-of-range ends are clamped.
    pub fn selected_text<'a>(&self, text: &'a str) -> &'a str {
        let (lo, hi) = self.range();
        &text[char_to_byte(text, lo)..char_to_byte(text, hi)]
    }
}

// =============================================================================
// Char / word / line helpers
// =============================================================================

/// Byte offset of char `char_idx` in `text`; `text.len()` past the end.
pub fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(text.len())
}

/// Char index of the start of the word before `cursor`: skip whitespace
/// backwards, then non-whitespace (Ctrl+Left).
pub fn word_left(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut pos = cursor.min(chars.len());
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }
    while pos > 0 && !chars[pos - 1].is_whitespace() {
        pos -= 1;
    }
    pos
}

/// Char index after the word at or after `cursor`: skip non-whitespace,
/// then whitespace (Ctrl+Right).
pub fn word_right(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut pos = cursor.min(n);
    while pos < n && !chars[pos].is_whitespace() {
        pos += 1;
    }
    while pos < n && chars[pos].is_whitespace() {
        pos += 1;
    }
    pos
}

/// `(lo, hi)` of the run around the char AT `char_idx` (the char under the
/// pointer — see [`char_index_at_x`]): a run of non-whitespace (a word), or
/// the whitespace run itself when the char is whitespace. `char_idx` past
/// the end means the last char. Empty text gives `(0, 0)`.
///
/// A word's ends are exactly [`word_left`] / the word's end before
/// [`word_right`] skips the trailing whitespace, so double-click and
/// Ctrl+Arrow agree.
pub fn word_range_at(text: &str, char_idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 {
        return (0, 0);
    }
    let mid = char_idx.min(n - 1);
    let ws = chars[mid].is_whitespace();
    let mut lo = mid;
    while lo > 0 && chars[lo - 1].is_whitespace() == ws {
        lo -= 1;
    }
    let mut hi = mid;
    while hi < n && chars[hi].is_whitespace() == ws {
        hi += 1;
    }
    (lo, hi)
}

/// `(lo, hi)` of the source line (split on `'\n'`, the newline excluded)
/// containing position `char_idx`. A position on a `'\n'` belongs to the
/// line that newline ends. Past the end means the last line.
pub fn line_range_at(text: &str, char_idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let at = char_idx.min(n);
    let mut lo = at;
    while lo > 0 && chars[lo - 1] != '\n' {
        lo -= 1;
    }
    let mut hi = at;
    while hi < n && chars[hi] != '\n' {
        hi += 1;
    }
    (lo, hi)
}

/// Nearest char boundary to `x` given the boundary x positions
/// (`boundaries.len() == char_count + 1`): the caret position for a click.
/// Empty `boundaries` gives 0.
pub fn char_from_x(boundaries: &[f64], x: f64) -> usize {
    if boundaries.is_empty() {
        return 0;
    }
    let char_count = boundaries.len() - 1;
    for i in 0..char_count {
        let mid = (boundaries[i] + boundaries[i + 1]) * 0.5;
        if x < mid {
            return i;
        }
    }
    char_count
}

/// The char whose box `[boundaries[i], boundaries[i + 1])` holds `x`
/// (clamped to the first / last char): the char under the pointer, for word
/// expansion. Empty text (`boundaries.len() <= 1`) gives 0.
pub fn char_index_at_x(boundaries: &[f64], x: f64) -> usize {
    let char_count = boundaries.len().saturating_sub(1);
    if char_count == 0 {
        return 0;
    }
    for i in 0..char_count {
        if x < boundaries[i + 1] {
            return i;
        }
    }
    char_count - 1
}

// =============================================================================
// Geometry
// =============================================================================

/// One laid-out line of a text run, for hit testing and highlight rects.
///
/// Holds chars `start..end` of the run; `boundaries[i]` is the x of the
/// boundary before char `start + i`, relative to `x`, so
/// `boundaries.len() == end - start + 1`. Chars between two lines (the
/// whitespace or `'\n'` a wrap consumed) belong to no line.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionLine {
    /// First char of the line.
    pub start: usize,
    /// One past the last char of the line.
    pub end: usize,
    /// Line origin x; `boundaries` are relative to it.
    pub x: f64,
    /// Line top y.
    pub top: f64,
    /// Line height (the highlight's height).
    pub height: f64,
    /// Char boundary x positions relative to `x`.
    pub boundaries: Vec<f64>,
}

impl SelectionLine {
    /// A single-line run: `boundaries` are absolute x positions of every
    /// char boundary (`char_count + 1` of them — the same vector a text
    /// input stamps via `TextFieldStore::update_field`).
    pub fn single(boundaries: &[f64], top: f64, height: f64) -> Self {
        Self {
            start: 0,
            end: boundaries.len().saturating_sub(1),
            x: 0.0,
            top,
            height,
            boundaries: if boundaries.is_empty() { vec![0.0] } else { boundaries.to_vec() },
        }
    }

    /// Lines of a wrapped run from `measure_text_wrapped`'s output for
    /// `text`, each `line_height` tall, in paragraph coordinates (origin =
    /// the paragraph's top-left; `WrappedLine::line_top` is the line's y).
    ///
    /// Glyph clusters are matched back onto `text` in order; the whitespace
    /// a wrap consumed (and any the layout collapsed) is skipped, so a
    /// line's `start..end` are real char indices of `text`. A char inside a
    /// multi-char cluster gets an x interpolated across the cluster's
    /// advance.
    pub fn from_wrapped(text: &str, lines: &[WrappedLine], line_height: f64) -> Vec<Self> {
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let mut pos = 0usize;
        let mut out = Vec::with_capacity(lines.len());

        for line in lines {
            let mut start: Option<usize> = None;
            let mut cur = pos;
            let mut boundaries: Vec<f64> = Vec::new();
            let mut right_edge = 0.0f64;

            for g in &line.glyphs {
                let cluster: Vec<char> = g.cluster.chars().collect();
                let k = cluster.len().max(1);
                let matches_at = |p: usize| p + cluster.len() <= n && chars[p..p + cluster.len()] == cluster[..];
                // Skip whitespace the layout did not keep (a break, a
                // collapsed run) until the cluster matches; fall back to
                // positional if it never does.
                let mut p = cur;
                while p < n && !matches_at(p) && chars[p].is_whitespace() {
                    p += 1;
                }
                if !matches_at(p) {
                    p = cur;
                }
                if start.is_none() {
                    start = Some(p);
                    cur = p;
                }
                // Zero-width boundaries for chars skipped inside the line.
                while cur < p {
                    boundaries.push(g.x_offset);
                    cur += 1;
                }
                for j in 0..k {
                    if cur + j >= n {
                        break;
                    }
                    boundaries.push(g.x_offset + g.advance * j as f64 / k as f64);
                }
                cur = (p + k).min(n);
                right_edge = g.x_offset + g.advance;
            }

            let start = start.unwrap_or(pos);
            // Chars left of the first glyph (none when start was set) and the
            // closing boundary.
            if boundaries.is_empty() {
                cur = start;
            }
            boundaries.push(right_edge);
            // A line never ends before it starts.
            let end = cur.max(start);
            boundaries.truncate(end - start + 1);
            while boundaries.len() < end - start + 1 {
                boundaries.push(right_edge);
            }
            out.push(Self { start, end, x: 0.0, top: line.line_top, height: line_height, boundaries });
            pos = end;
        }
        out
    }

    /// x (absolute) of the boundary before char `char_idx`, clamped to the
    /// line.
    pub fn x_of(&self, char_idx: usize) -> f64 {
        let i = char_idx.clamp(self.start, self.end) - self.start;
        self.x + self.boundaries.get(i).copied().unwrap_or(0.0)
    }

    /// Whether position `char_idx` is on this line (ends inclusive).
    pub fn contains(&self, char_idx: usize) -> bool {
        char_idx >= self.start && char_idx <= self.end
    }
}

/// Index of the line holding position `char_idx`: the first whose
/// `start..=end` contains it, else the last line that starts at or before
/// it (a position in the whitespace between lines), else the first.
fn line_index_of(lines: &[SelectionLine], char_idx: usize) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    if let Some(i) = lines.iter().position(|l| l.contains(char_idx)) {
        return Some(i);
    }
    Some(lines.iter().rposition(|l| l.start <= char_idx).unwrap_or(0))
}

/// Rects to fill under the glyphs for `sel` over the laid-out `lines`, one
/// per line the selection touches, in the lines' coordinates. An empty
/// selection gives none.
pub fn highlight_rects(sel: &TextSelection, lines: &[SelectionLine]) -> Vec<Rect> {
    let (lo, hi) = sel.range();
    if lo == hi {
        return Vec::new();
    }
    let mut rects = Vec::new();
    for line in lines {
        let a = lo.max(line.start);
        let b = hi.min(line.end);
        if a >= b {
            continue;
        }
        let x0 = line.x_of(a);
        let x1 = line.x_of(b);
        let (left, right) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        rects.push(Rect::new(left, line.top, right - left, line.height));
    }
    rects
}

/// [`highlight_rects`] for a single-line run: `boundaries` are absolute
/// char boundary x positions, the rect spans `top..top + height`.
pub fn highlight_rects_single(sel: &TextSelection, boundaries: &[f64], top: f64, height: f64) -> Vec<Rect> {
    highlight_rects(sel, &[SelectionLine::single(boundaries, top, height)])
}

/// Caret position for a pointer at `(x, y)` over the laid-out `lines`: the
/// line under `y` (clamped to the first / last), then the nearest boundary
/// to `x`. Empty `lines` gives 0.
pub fn char_at_point(lines: &[SelectionLine], x: f64, y: f64) -> usize {
    let Some(first) = lines.first() else { return 0 };
    let line = if y < first.top {
        first
    } else {
        lines
            .iter()
            .find(|l| y >= l.top && y < l.top + l.height)
            .unwrap_or_else(|| {
                // Below the last line, or in a gap: the last line starting above y.
                lines.iter().rev().find(|l| l.top <= y).unwrap_or(first)
            })
    };
    line.start + char_from_x(&line.boundaries, x - line.x).min(line.end - line.start)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::GlyphMetric;

    fn uniform(n: usize, w: f64) -> Vec<f64> {
        (0..=n).map(|i| i as f64 * w).collect()
    }

    #[test]
    fn anchor_and_focus_in_both_directions() {
        let fwd = TextSelection::new(2, 5);
        let back = TextSelection::new(5, 2);
        assert_eq!(fwd.range(), (2, 5));
        assert_eq!(back.range(), (2, 5));
        assert!(!fwd.is_backward());
        assert!(back.is_backward());
        assert_eq!(fwd.len(), 3);
        assert_eq!(back.len(), 3);
        assert_eq!(fwd.selected_text("hello world"), "llo");
        assert_eq!(back.selected_text("hello world"), "llo");

        let mut s = TextSelection::caret(4);
        assert!(s.is_empty());
        s.extend_to(1);
        assert_eq!((s.anchor, s.focus), (4, 1));
        assert_eq!(s.selected_text("abcdef"), "bcd");
        s.extend_to(6);
        assert_eq!(s.selected_text("abcdef"), "ef");
        s.collapse();
        assert!(s.is_empty());
        assert_eq!(s.anchor, 6);
    }

    #[test]
    fn selected_text_counts_chars_and_clamps() {
        let text = "привет мир";
        assert_eq!(TextSelection::new(0, 6).selected_text(text), "привет");
        assert_eq!(TextSelection::new(7, 99).selected_text(text), "мир");
        assert_eq!(TextSelection::new(99, 99).selected_text(text), "");
        assert_eq!(TextSelection::new(8, 20).clamped(10), TextSelection::new(8, 10));
    }

    #[test]
    fn select_all() {
        let s = TextSelection::all("你好 世界");
        assert_eq!(s.range(), (0, 5));
        assert!(s.is_all("你好 世界"));
        assert_eq!(s.selected_text("你好 世界"), "你好 世界");
        assert!(!TextSelection::new(0, 4).is_all("你好 世界"));
        assert!(TextSelection::all("").is_empty());
        assert!(!TextSelection::all("").is_all(""));
        // backward covers all too
        assert!(TextSelection::new(5, 0).is_all("你好 世界"));
    }

    #[test]
    fn word_expansion_middle_edges_and_whitespace() {
        let t = "hello  big world";
        assert_eq!(word_range_at(t, 0), (0, 5));
        assert_eq!(word_range_at(t, 4), (0, 5));
        // the double space is its own run
        assert_eq!(word_range_at(t, 5), (5, 7));
        assert_eq!(word_range_at(t, 6), (5, 7));
        assert_eq!(word_range_at(t, 7), (7, 10));
        // last char and past the end → last word
        assert_eq!(word_range_at(t, 15), (11, 16));
        assert_eq!(word_range_at(t, 99), (11, 16));
        assert_eq!(word_range_at("", 3), (0, 0));
        assert_eq!(word_range_at("x", 0), (0, 1));
    }

    #[test]
    fn word_expansion_agrees_with_ctrl_arrow_movement() {
        let t = "one two  three";
        for i in 0..t.chars().count() {
            let (lo, hi) = word_range_at(t, i);
            if !t.chars().nth(i).unwrap().is_whitespace() {
                assert_eq!(word_left(t, hi), lo, "at {i}");
                // word_right from lo lands after the trailing whitespace
                let wr = word_right(t, lo);
                assert!(wr >= hi, "at {i}");
                assert!(t.chars().skip(hi).take(wr - hi).all(char::is_whitespace));
            }
        }
    }

    #[test]
    fn word_expansion_is_in_chars_for_cyrillic_and_cjk() {
        let t = "привет мир";
        assert_eq!(word_range_at(t, 2), (0, 6));
        assert_eq!(word_range_at(t, 8), (7, 10));
        assert_eq!(TextSelection::word_at(t, 9).selected_text(t), "мир");

        let c = "你好 世界";
        assert_eq!(word_range_at(c, 1), (0, 2));
        assert_eq!(word_range_at(c, 3), (3, 5));
        assert_eq!(TextSelection::word_at(c, 4).selected_text(c), "世界");
        // a CJK run with no spaces is one word (same as Ctrl+Arrow)
        assert_eq!(word_range_at("日本語テキスト", 3), (0, 7));
    }

    #[test]
    fn line_expansion_edges_and_unicode() {
        let t = "first\nвторая строка\n\n末行";
        assert_eq!(line_range_at(t, 0), (0, 5));
        assert_eq!(line_range_at(t, 5), (0, 5), "the newline belongs to the line it ends");
        assert_eq!(line_range_at(t, 6), (6, 19));
        assert_eq!(TextSelection::line_at(t, 10).selected_text(t), "вторая строка");
        assert_eq!(line_range_at(t, 20), (20, 20), "an empty line");
        assert_eq!(TextSelection::line_at(t, 22).selected_text(t), "末行");
        assert_eq!(line_range_at(t, 99), (21, 23));
        assert_eq!(line_range_at("", 0), (0, 0));
        // single-line text: the line is everything
        assert_eq!(line_range_at("one line", 3), (0, 8));
    }

    #[test]
    fn click_count_picks_caret_word_or_line() {
        let t = "ab cd\nef";
        assert_eq!(TextSelection::for_click_count(t, 4, 1), TextSelection::caret(4));
        assert_eq!(TextSelection::for_click_count(t, 4, 2), TextSelection::new(3, 5));
        assert_eq!(TextSelection::for_click_count(t, 4, 3), TextSelection::new(0, 5));
        assert_eq!(TextSelection::for_click_count(t, 7, 3), TextSelection::new(6, 8));
        assert_eq!(TextSelection::for_click_count(t, 99, 1), TextSelection::caret(8));
    }

    #[test]
    fn char_from_x_and_char_index_at_x() {
        let b = uniform(4, 10.0);
        assert_eq!(char_from_x(&b, -5.0), 0);
        assert_eq!(char_from_x(&b, 4.0), 0);
        assert_eq!(char_from_x(&b, 6.0), 1);
        assert_eq!(char_from_x(&b, 99.0), 4);
        assert_eq!(char_from_x(&[], 3.0), 0);

        assert_eq!(char_index_at_x(&b, 9.0), 0);
        assert_eq!(char_index_at_x(&b, 10.0), 1);
        assert_eq!(char_index_at_x(&b, 39.0), 3);
        assert_eq!(char_index_at_x(&b, 99.0), 3);
        assert_eq!(char_index_at_x(&[0.0], 5.0), 0);
    }

    #[test]
    fn highlight_rects_single_line_both_directions() {
        let b: Vec<f64> = uniform(5, 10.0).into_iter().map(|x| x + 100.0).collect();
        let r = highlight_rects_single(&TextSelection::new(1, 3), &b, 20.0, 16.0);
        assert_eq!(r, vec![Rect::new(110.0, 20.0, 20.0, 16.0)]);
        let r = highlight_rects_single(&TextSelection::new(3, 1), &b, 20.0, 16.0);
        assert_eq!(r, vec![Rect::new(110.0, 20.0, 20.0, 16.0)]);
        // all
        let r = highlight_rects_single(&TextSelection::new(0, 5), &b, 0.0, 10.0);
        assert_eq!(r, vec![Rect::new(100.0, 0.0, 50.0, 10.0)]);
        // clamped past the end
        let r = highlight_rects_single(&TextSelection::new(4, 50), &b, 0.0, 10.0);
        assert_eq!(r, vec![Rect::new(140.0, 0.0, 10.0, 10.0)]);
        // empty → nothing
        assert!(highlight_rects_single(&TextSelection::caret(2), &b, 0.0, 10.0).is_empty());
        assert!(highlight_rects_single(&TextSelection::new(0, 3), &[], 0.0, 10.0).is_empty());
    }

    /// A per-char monospace "layout" of `lines` (each a line's kept text),
    /// 10 px per char, line tops 20 px apart.
    fn wrapped(lines: &[&str]) -> Vec<WrappedLine> {
        lines
            .iter()
            .enumerate()
            .map(|(i, l)| WrappedLine {
                glyphs: l
                    .chars()
                    .enumerate()
                    .map(|(j, c)| GlyphMetric {
                        cluster: c.to_string(),
                        x_offset: j as f64 * 10.0,
                        y_offset: 0.0,
                        advance: 10.0,
                        width: 10.0,
                    })
                    .collect(),
                line_top: i as f64 * 20.0,
                baseline_y: i as f64 * 20.0 + 14.0,
                width: l.chars().count() as f64 * 10.0,
            })
            .collect()
    }

    #[test]
    fn wrapped_lines_map_back_onto_source_chars() {
        let text = "hello big world";
        // the wrap consumed the spaces after "hello" and "big"
        let lines = SelectionLine::from_wrapped(text, &wrapped(&["hello", "big", "world"]), 18.0);
        assert_eq!(lines.len(), 3);
        assert_eq!((lines[0].start, lines[0].end), (0, 5));
        assert_eq!((lines[1].start, lines[1].end), (6, 9));
        assert_eq!((lines[2].start, lines[2].end), (10, 15));
        for l in &lines {
            assert_eq!(l.boundaries.len(), l.end - l.start + 1);
        }
        assert_eq!(lines[1].top, 20.0);
        assert_eq!(lines[1].height, 18.0);
        assert_eq!(lines[2].x_of(12), 20.0);
    }

    #[test]
    fn wrapped_lines_keep_trailing_space_and_unicode() {
        // a layout that keeps the break space at the end of the line
        let text = "привет мир 你好";
        let lines = SelectionLine::from_wrapped(text, &wrapped(&["привет ", "мир ", "你好"]), 20.0);
        assert_eq!((lines[0].start, lines[0].end), (0, 7));
        assert_eq!((lines[1].start, lines[1].end), (7, 11));
        assert_eq!((lines[2].start, lines[2].end), (11, 13));
        assert_eq!(TextSelection::visual_line_at(&lines, 8).selected_text(text), "мир ");
    }

    #[test]
    fn wrapped_highlight_rects_span_lines() {
        let text = "hello big world";
        let lines = SelectionLine::from_wrapped(text, &wrapped(&["hello", "big", "world"]), 18.0);
        // "llo big wo"
        let r = highlight_rects(&TextSelection::new(2, 12), &lines);
        assert_eq!(
            r,
            vec![
                Rect::new(20.0, 0.0, 30.0, 18.0),
                Rect::new(0.0, 20.0, 30.0, 18.0),
                Rect::new(0.0, 40.0, 20.0, 18.0),
            ]
        );
        // backward over one line only
        let r = highlight_rects(&TextSelection::new(9, 7), &lines);
        assert_eq!(r, vec![Rect::new(10.0, 20.0, 20.0, 18.0)]);
        // select-all covers every line
        assert_eq!(highlight_rects(&TextSelection::all(text), &lines).len(), 3);
        // a selection of only the consumed break space draws nothing
        assert!(highlight_rects(&TextSelection::new(5, 6), &lines).is_empty());
    }

    #[test]
    fn wrapped_lines_with_hard_breaks_and_empty_lines() {
        let text = "ab\n\ncd";
        let lines = SelectionLine::from_wrapped(text, &wrapped(&["ab", "", "cd"]), 20.0);
        assert_eq!((lines[0].start, lines[0].end), (0, 2));
        assert_eq!(lines[1].start, lines[1].end);
        assert_eq!((lines[2].start, lines[2].end), (4, 6));
        assert_eq!(lines[2].x_of(5), 10.0);
    }

    #[test]
    fn char_at_point_picks_line_then_boundary() {
        let text = "hello big world";
        let lines = SelectionLine::from_wrapped(text, &wrapped(&["hello", "big", "world"]), 18.0);
        assert_eq!(char_at_point(&lines, 12.0, 5.0), 1);
        assert_eq!(char_at_point(&lines, 16.0, 25.0), 8);
        assert_eq!(char_at_point(&lines, 999.0, 25.0), 9);
        assert_eq!(char_at_point(&lines, 0.0, -10.0), 0);
        assert_eq!(char_at_point(&lines, 999.0, 999.0), 15);
        assert_eq!(char_at_point(&[], 1.0, 1.0), 0);
    }
}
