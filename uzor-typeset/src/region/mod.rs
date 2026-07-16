//! Regions — the one shared layout interface (design doc §3.1, lifted from
//! Typst): a layouter never needs to know if it's filling a page body, a
//! slide body, or a table cell, only "here is a region, give me a frame."
//!
//! P0 built [`Region`]/[`RegionSequence`]/[`PageRegionSequence`].
//! [`FixedRegionSequence`] lands THIS phase (P1), ahead of its own P3
//! slide-mode consumer (design doc §3.1 names it "slide/table-cell mode"
//! — the table-cell half of that pairing is a real P1 consumer:
//! `compose::table_layout`'s per-cell distribute pass needs a single,
//! fixed-size region to lay a cell's content into). `CardRegionSequence`
//! (slide/card mode) and `ColumnRegionSequence` stay deferred — neither
//! has a P1 consumer, per the design doc's own §3.7 "deferred out of this
//! arc" / phase gating.

mod frame;

pub use frame::{Frame, ListPlacement, PlacedBlock, PlacedListItem, PlacedTableCell, PlacedTableRow, TablePlacement};

use uzor::types::Rect;

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
}

impl PageRegionSequence {
    pub fn new(body_rect: Rect) -> Self {
        Self { body_rect }
    }
}

impl RegionSequence for PageRegionSequence {
    fn next(&mut self) -> Option<Region> {
        Some(Region { rect: self.body_rect })
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
}
