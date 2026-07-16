//! [`PageMaster`]/[`slice_pages`] — the page slicing target (design doc
//! §4.1). Uses [`PageRegionSequence`] under the hood: a page body is one
//! region per page, shrinking within a page (a block placed 2/3 down a
//! page only gets the remaining 1/3, then a fresh full region on the next
//! page) — Typst's own model, adopted directly.
//!
//! P0 scope: `width`/`height`/`margins` only — no header/footer margin-box
//! content, no page-number token (both are P2 additions once
//! `master/{page_master,placeholder}.rs`/`style/theme.rs` land, per the
//! design doc's own phase gating).

use uzor::types::Rect;
use uzor_text::LineShaper;

use crate::compose::{compose, ComposeStyle};
use crate::region::{Frame, PageRegionSequence};
use crate::scene::BlockNode;

/// Page margins, one value per edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Margins {
    pub fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self { top, right, bottom, left }
    }

    /// Same margin on every edge.
    pub fn uniform(all: f64) -> Self {
        Self { top: all, right: all, bottom: all, left: all }
    }
}

/// Page geometry: physical size + margins. See this module's own doc
/// comment for what P2 adds on top (header/footer/page-number-token).
#[derive(Debug, Clone, Copy)]
pub struct PageMaster {
    pub width: f64,
    pub height: f64,
    pub margins: Margins,
}

impl PageMaster {
    pub fn new(width: f64, height: f64, margins: Margins) -> Self {
        Self { width, height, margins }
    }

    /// The page body rect (page size, inset by `margins`) — the one region
    /// [`PageRegionSequence`] hands back on every page.
    pub fn body_rect(&self) -> Rect {
        Rect::new(
            self.margins.left,
            self.margins.top,
            (self.width - self.margins.left - self.margins.right).max(0.0),
            (self.height - self.margins.top - self.margins.bottom).max(0.0),
        )
    }
}

/// One sliced page: its 0-based index + the composed [`Frame`] that landed
/// on it.
pub struct Page<'a> {
    pub index: u32,
    pub frame: Frame<'a>,
}

/// Compose `flow` over `master`'s page geometry and slice the result into
/// pages — one [`Page`] per [`Frame`] `compose()` produced. Stops as soon
/// as every block is placed (never emits a trailing empty page).
///
/// `style`/`shaper` are not part of the design doc's own abbreviated
/// `slice_pages(scene, master)` pseudocode (§4.1) — added here for the same
/// reason [`compose`] needs them (see this crate's `CLAUDE.md`
/// "Divergences from the design doc"), and because P0 does not build
/// `SceneSpec`/`Theme` (§2.1/§5, not part of P0's own deliverables list),
/// `flow` is taken directly rather than via a `&SceneSpec`.
pub fn slice_pages<'a>(flow: &'a [BlockNode<'a>], master: &PageMaster, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<Page<'a>> {
    let mut regions = PageRegionSequence::new(master.body_rect());
    compose(flow, &mut regions, style, shaper)
        .into_iter()
        .enumerate()
        .map(|(i, frame)| Page { index: i as u32, frame })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{layout_paragraph, CosmicShaper, FontSpec, Paragraph, StyledRun};

    use crate::scene::{Block, BlockNode};

    /// A repeated sentence long enough that ~2.5 pages of it wrap to 3
    /// pages at the fixture's chosen width/font/page size, with the last
    /// page only partially filled.
    const SENTENCE: &str =
        "This fixed seeded sentence exists only to consume vertical space \
        for the page-slicing test, repeated enough times to overflow more \
        than two pages but noticeably less than three whole pages of body \
        text at this fixture's chosen page geometry.";

    /// Wraps to several lines at a ~400px width — tall enough to overflow
    /// the remaining 1/3 of the 300px-tall fixture page used by the
    /// region-shrink test below.
    const WRAPPING_PARAGRAPH: &str =
        "A paragraph placed near the bottom of the page, deliberately long \
        enough that it must wrap across several lines and therefore cannot \
        possibly fit whole into the small remaining sliver of vertical \
        space left below a filler spacer that already consumed two thirds \
        of this fixture's short page body.";

    fn body_font() -> FontSpec {
        FontSpec::new(FontFamily::Roboto, 16.0)
    }

    #[test]
    fn content_worth_about_two_and_a_half_pages_slices_into_exactly_three() {
        let font = body_font();
        // 20 short paragraphs at a 16px line height comfortably exceed 2
        // pages of a 595x842 (A4-ish) body at 40px margins, and land short
        // of a full 3rd page.
        let paragraph_texts: Vec<String> = (0..20).map(|i| format!("Paragraph {i}: {SENTENCE}")).collect();
        let runs: Vec<[uzor_text::StyledRun<'_>; 1]> =
            paragraph_texts.iter().map(|t| [StyledRun::new(t.as_str(), font)]).collect();
        let flow: Vec<BlockNode<'_>> = runs
            .iter()
            .map(|r| BlockNode::new(Block::Paragraph(Paragraph::new(r, 515.0))))
            .collect();

        let master = PageMaster::new(595.0, 842.0, Margins::uniform(40.0));
        let style = ComposeStyle::new(12.0, font);
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 3, "fixture is tuned to land on exactly 3 pages, got {}", pages.len());

        for page in &pages[..2] {
            assert!(!page.frame.blocks.is_empty(), "every non-final page must carry content");
        }
        let last = pages.last().expect("at least one page");
        assert!(!last.frame.blocks.is_empty(), "the final page must still carry its partial content");

        // Determinism: composing the exact same flow twice must produce
        // the exact same page count and per-page block count.
        let pages_again = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), pages_again.len());
        for (a, b) in pages.iter().zip(pages_again.iter()) {
            assert_eq!(a.frame.blocks.len(), b.frame.blocks.len());
        }
    }

    #[test]
    fn a_block_placed_two_thirds_down_a_page_only_gets_the_remaining_third() {
        let font = body_font();
        let master = PageMaster::new(400.0, 300.0, Margins::uniform(0.0));
        let body = master.body_rect();

        let filler_gap = body.height * (2.0 / 3.0);
        // Long enough to wrap to several lines at this width — taller than
        // the remaining 1/3 of the page, so it MUST split across pages
        // rather than fit whole (the meaningful version of this test: a
        // short one-liner would fit either way and prove nothing about
        // region-shrink).
        let runs = [StyledRun::new(WRAPPING_PARAGRAPH, font)];
        let flow = [BlockNode::new(Block::Spacer(filler_gap)), BlockNode::new(Block::Paragraph(Paragraph::new(&runs, body.width)))];

        let style = ComposeStyle::new(0.0, font);
        let shaper = CosmicShaper::headless();

        // Independently confirm the paragraph's OWN full layout (measured
        // at the page body's width) is taller than the remaining 1/3 of
        // the page — otherwise this test's split assumption is untuned.
        let full_layout = layout_paragraph(&Paragraph::new(&runs, body.width), &shaper);
        let remaining_before_paragraph = body.height - filler_gap;
        assert!(
            full_layout.height > remaining_before_paragraph,
            "fixture must be tall enough to overflow the remaining 1/3 of the page"
        );

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2, "fixture must split across exactly 2 pages");

        let first_page = &pages[0];
        let paragraph_placement = first_page
            .frame
            .blocks
            .iter()
            .find(|b| matches!(b.kind, Block::Paragraph(_)))
            .expect("paragraph must be placed on the first page (even if only partially)");

        assert!(
            paragraph_placement.rect.height <= remaining_before_paragraph + 1.0,
            "paragraph placed 2/3 down the page must only get the remaining 1/3 of region height, got {} vs budget {}",
            paragraph_placement.rect.height,
            remaining_before_paragraph
        );

        let second_page = &pages[1];
        let remainder_placement = second_page
            .frame
            .blocks
            .iter()
            .find(|b| matches!(b.kind, Block::Paragraph(_)))
            .expect("the paragraph's remainder must continue onto the second page");
        let remainder_layout = remainder_placement.paragraph_layout.as_ref().expect("paragraph placements always carry a layout");
        let head_layout = paragraph_placement.paragraph_layout.as_ref().expect("paragraph placements always carry a layout");
        assert_eq!(
            head_layout.lines.len() + remainder_layout.lines.len(),
            full_layout.lines.len(),
            "every line of the paragraph must be conserved across the split, none dropped or duplicated"
        );
    }
}
