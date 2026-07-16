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
//! `ColumnRegionSequence` stays deferred — no P3 consumer, per the design
//! doc's own §3.7 "deferred out of this arc" / phase gating.

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
}
