//! Regions — the one shared layout interface (design doc §3.1, lifted from
//! Typst): a layouter never needs to know if it's filling a page body, a
//! slide body, or a table cell, only "here is a region, give me a frame."
//!
//! P0 built [`Region`]/[`RegionSequence`]/[`PageRegionSequence`].
//! [`FixedRegionSequence`] landed P1, ahead of its own P3 slide-mode
//! consumer (design doc §3.1 names it "slide/table-cell mode" — the
//! table-cell half of that pairing was a real P1 consumer:
//! `compose::table_layout`'s per-cell distribute pass needs a single,
//! fixed-size region to lay a cell's content into; its own PROMISED P3
//! consumer, `crate::slice::slice_slide_instance`'s per-placeholder
//! composition, lands this phase). [`CardRegionSequence`] lands THIS
//! phase (P3) — card/fluid slide mode's own driver (design doc §3.1/§4.2).
//!
//! ## Multi-column pages ([`ColumnRegionSequence`]/[`PageRegionSequence::with_columns`])
//!
//! `ColumnRegionSequence` (finite: column 1, column 2, ..., column
//! `columns`, then ends — the single-page/single-row shape the design
//! doc's own §3.1 phrasing describes: "N equal sub-columns per
//! page-row") and [`PageRegionSequence::with_columns`] (the SAME
//! column-splitting geometry, but INFINITE — column 1 -> column 2 -> ...
//! -> next page's column 1 -> ..., [`crate::slice::slice_pages`]'s own
//! multi-page column driver) both land this phase, no longer deferred.
//! Kept as two distinct concrete types rather than one, matching this
//! module's own existing finite ([`FixedRegionSequence`]) vs. infinite
//! ([`PageRegionSequence`], [`CardRegionSequence`]) split — a
//! `RegionSequence` implementation is either bounded or not, never both,
//! everywhere else in this module.

mod frame;

pub use frame::{Frame, ListPlacement, PlacedBlock, PlacedListItem, PlacedTableCell, PlacedTableRow, TablePlacement};

use uzor::types::Rect;

/// Shared column-splitting geometry: column `index` (0-based) of
/// `columns` equal-width sub-columns of `body`, separated by `gap`
/// between adjacent columns (`columns - 1` gaps total — never a gap
/// before the first or after the last column). `columns <= 1` reproduces
/// `body` exactly regardless of `gap` — the "column-less" case every
/// pre-column [`PageRegionSequence::new`] caller relies on staying
/// byte-identical.
pub(crate) fn column_rect(body: Rect, columns: usize, gap: f64, index: usize) -> Rect {
    let columns = columns.max(1);
    let total_gap = gap * (columns - 1) as f64;
    let column_width = ((body.width - total_gap) / columns as f64).max(0.0);
    let x = body.x + index as f64 * (column_width + gap);
    Rect::new(x, body.y, column_width, body.height)
}

/// One region a layouter can flow content into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub rect: Rect,
}

/// Pulls the next available region. Region sequences SHRINK as content is
/// consumed within one page/slide (Typst model) and reset to full size on
/// the next page/slide. `None` ends composition (a finite sequence, e.g. a
/// single table cell); an infinite page sequence never returns `None`.
pub trait RegionSequence {
    fn next(&mut self) -> Option<Region>;
}

/// Infinite region sequence: the same full page-body rect every call —
/// [`crate::slice::slice_pages`]'s own driver. `PageRegionSequence` does
/// not itself know about page numbers/headers/footers (P2 additions); it
/// only ever hands back one fixed body rect, over and over.
pub struct PageRegionSequence {
    body_rect: Rect,
    columns: usize,
    gap: f64,
    column_index: usize,
}

impl PageRegionSequence {
    pub fn new(body_rect: Rect) -> Self {
        Self { body_rect, columns: 1, gap: 0.0, column_index: 0 }
    }

    /// Builder: split each page's own body into `columns` equal
    /// sub-columns separated by `gap`, flowing column 1 -> column 2 ->
    /// ... -> the NEXT page's column 1 -> ... (design doc §3.1's "N equal
    /// sub-columns per page-row," extended across page breaks — the
    /// genuinely multi-page column-flow case [`ColumnRegionSequence`]
    /// deliberately does not cover, since that type is finite/
    /// single-row by design; [`crate::slice::PageMaster::with_columns`]
    /// is this constructor's own real caller). `columns <= 1` (the
    /// default set by [`PageRegionSequence::new`]) reproduces the
    /// pre-column single-body-rect-per-page behavior byte-for-byte
    /// ([`column_rect`] with one column returns `body_rect` verbatim
    /// regardless of `gap`).
    pub fn with_columns(mut self, columns: usize, gap: f64) -> Self {
        self.columns = columns.max(1);
        self.gap = gap;
        self
    }
}

impl RegionSequence for PageRegionSequence {
    fn next(&mut self) -> Option<Region> {
        let index = self.column_index % self.columns;
        self.column_index += 1;
        Some(Region { rect: column_rect(self.body_rect, self.columns, self.gap, index) })
    }
}

/// Finite region sequence: hands back exactly ONE fixed rect, then `None`
/// forever after — the table-cell/slide-mode driver (design doc §3.1).
pub struct FixedRegionSequence {
    rect: Option<Rect>,
}

impl FixedRegionSequence {
    pub fn new(rect: Rect) -> Self {
        Self { rect: Some(rect) }
    }
}

impl RegionSequence for FixedRegionSequence {
    fn next(&mut self) -> Option<Region> {
        self.rect.take().map(|rect| Region { rect })
    }
}

/// Height large enough that no real card/slide content ever fills it —
/// "unbounded" in practice, while staying comfortably far from
/// `f64::MAX` so every subtraction `compose::flow`'s own arithmetic does
/// against a region's height (`region_bottom - cursor_y`, etc.) stays
/// finite and precise.
const UNBOUNDED_HEIGHT: f64 = 1.0e9;

/// Infinite region sequence: a fresh, full-width, effectively-unbounded-
/// height region on EVERY call (design doc §3.1: "ONE region, unbounded
/// height — card/fluid mode"; "ONE region" describes a single card's own
/// shape — it never shrinks the way [`PageRegionSequence`] shrinks WITHIN
/// a page, because nothing this crate composes is ever tall enough to
/// fill it) — card/fluid slide mode's own driver (§4.2).
///
/// Composing a group of blocks against this sequence therefore ALWAYS
/// lands everything in exactly one [`crate::region::Frame`] — no content
/// this crate produces overflows a region this tall, so a card's own
/// natural height is simply that one frame's tallest placed block. When a
/// scene needs MORE than one card, that split is driven entirely by
/// `crate::compose::BreakControl::ForceBefore`/`ForceAfter` card-break
/// markers BEFORE composition even reaches a `RegionSequence`
/// (`crate::slice::slides`'s own `split_groups` — design law 4: no new
/// compose logic for grouping, the existing keep/break vocabulary already
/// says everything a card break needs to say).
pub struct CardRegionSequence {
    width: f64,
}

impl CardRegionSequence {
    pub fn new(width: f64) -> Self {
        Self { width }
    }
}

impl RegionSequence for CardRegionSequence {
    fn next(&mut self) -> Option<Region> {
        Some(Region { rect: Rect::new(0.0, 0.0, self.width, UNBOUNDED_HEIGHT) })
    }
}

/// N equal sub-columns of ONE page-row/body rect (design doc §3.1) — a
/// FINITE sequence: column 1 rect, column 2, ..., column `columns`, then
/// ends (`None`) — the single-page/single-row shape (this feature's own
/// task brief: "for single-page use it ends after the last column").
/// Multi-page column flow (columns repeating across page breaks) is
/// [`PageRegionSequence::with_columns`]'s own job instead — see that
/// constructor's own doc comment for why the two stay distinct concrete
/// types.
pub struct ColumnRegionSequence {
    body: Rect,
    columns: usize,
    gap: f64,
    next_index: usize,
}

impl ColumnRegionSequence {
    pub fn new(body: Rect, columns: usize, gap: f64) -> Self {
        Self { body, columns: columns.max(1), gap, next_index: 0 }
    }
}

impl RegionSequence for ColumnRegionSequence {
    fn next(&mut self) -> Option<Region> {
        if self.next_index >= self.columns {
            return None;
        }
        let rect = column_rect(self.body, self.columns, self.gap, self.next_index);
        self.next_index += 1;
        Some(Region { rect })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_region_sequence_never_ends_and_repeats_the_same_body_rect() {
        let body = Rect::new(10.0, 20.0, 400.0, 600.0);
        let mut regions = PageRegionSequence::new(body);

        for _ in 0..5 {
            let region = regions.next().expect("page region sequence is infinite");
            assert_eq!(region.rect, body);
        }
    }

    #[test]
    fn fixed_region_sequence_hands_back_its_rect_exactly_once_then_ends() {
        let rect = Rect::new(0.0, 0.0, 200.0, f64::MAX);
        let mut regions = FixedRegionSequence::new(rect);

        let first = regions.next().expect("first pull must return the fixed rect");
        assert_eq!(first.rect, rect);
        assert!(regions.next().is_none(), "a fixed region sequence is finite — exactly one region");
    }

    #[test]
    fn card_region_sequence_never_ends_and_hands_back_the_same_full_width_unbounded_height_rect() {
        let mut regions = CardRegionSequence::new(320.0);

        for _ in 0..5 {
            let region = regions.next().expect("card region sequence is infinite");
            assert_eq!(region.rect.x, 0.0);
            assert_eq!(region.rect.y, 0.0);
            assert_eq!(region.rect.width, 320.0);
            assert!(region.rect.height > 1.0e8, "card region height must be effectively unbounded");
        }
    }

    #[test]
    fn column_region_sequence_yields_exactly_n_equal_width_columns_then_ends() {
        let body = Rect::new(10.0, 20.0, 424.0, 600.0); // 424 = 2*200 + 24 gap
        let mut regions = ColumnRegionSequence::new(body, 2, 24.0);

        let col1 = regions.next().expect("first column").rect;
        assert_eq!(col1, Rect::new(10.0, 20.0, 200.0, 600.0));

        let col2 = regions.next().expect("second column").rect;
        assert_eq!(col2, Rect::new(234.0, 20.0, 200.0, 600.0));

        assert!(regions.next().is_none(), "a column region sequence is finite — exactly `columns` regions");
    }

    #[test]
    fn column_region_sequence_with_one_column_reproduces_the_body_rect_exactly() {
        let body = Rect::new(0.0, 0.0, 300.0, 400.0);
        let mut regions = ColumnRegionSequence::new(body, 1, 18.0);
        assert_eq!(regions.next().expect("one column").rect, body);
        assert!(regions.next().is_none());
    }

    #[test]
    fn page_region_sequence_with_columns_cycles_column1_then_column2_then_repeats_on_the_next_page() {
        let body = Rect::new(0.0, 0.0, 424.0, 600.0);
        let mut regions = PageRegionSequence::new(body).with_columns(2, 24.0);

        let page1_col1 = regions.next().expect("page 1 column 1").rect;
        let page1_col2 = regions.next().expect("page 1 column 2").rect;
        let page2_col1 = regions.next().expect("page 2 column 1").rect;
        let page2_col2 = regions.next().expect("page 2 column 2").rect;

        assert_eq!(page1_col1, Rect::new(0.0, 0.0, 200.0, 600.0));
        assert_eq!(page1_col2, Rect::new(224.0, 0.0, 200.0, 600.0));
        assert_eq!(page2_col1, page1_col1, "page 2 must repeat the SAME column 1 rect as page 1");
        assert_eq!(page2_col2, page1_col2, "page 2 must repeat the SAME column 2 rect as page 1");
    }

    #[test]
    fn page_region_sequence_default_columns_is_one_and_matches_pre_column_behavior() {
        let body = Rect::new(5.0, 5.0, 500.0, 700.0);
        let mut regions = PageRegionSequence::new(body);
        for _ in 0..3 {
            assert_eq!(regions.next().expect("infinite").rect, body, "no `with_columns` call must behave exactly like before this feature");
        }
    }
}
