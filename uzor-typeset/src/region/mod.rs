//! Regions — the one shared layout interface (design doc §3.1, lifted from
//! Typst): a layouter never needs to know if it's filling a page body, a
//! slide body, or a table cell, only "here is a region, give me a frame."
//!
//! P0 builds [`Region`]/[`RegionSequence`]/[`PageRegionSequence`] only —
//! `CardRegionSequence`/`FixedRegionSequence` (slide/card modes, P3) and
//! `ColumnRegionSequence` (P2+) are later-phase additions, per the design
//! doc's own §3.7 "deferred out of this arc" / phase gating.

mod frame;

pub use frame::{Frame, PlacedBlock};

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
}
