//! [`Page`]/[`slice_pages`] — the page slicing target (design doc §4.1).
//! Uses [`PageRegionSequence`] under the hood for the page BODY: one
//! region per page, shrinking within a page (a block placed 2/3 down a
//! page only gets the remaining 1/3, then a fresh full region on the next
//! page) — Typst's own model, adopted directly.
//!
//! [`crate::master::PageMaster`] itself (geometry + margins +
//! header/footer + page-number token) moved to `master::page_master` this
//! phase (P2) — see that module's own doc comment for why. This module
//! keeps [`Page`] + [`slice_pages`] + the header/footer/page-number
//! WIRING: header/footer margin-box content is composed ONCE (via a
//! [`FixedRegionSequence`] over its own margin band) and reused UNCHANGED
//! on every page — the ONLY per-page-varying piece is the page-number
//! STRING, precomputed here at slice time via
//! [`crate::master::PageNumberStyle::format_for`] (design doc §4.1:
//! "page_number_token ... resolved per-page during slicing").
//!
//! ## Multi-column pages — `Page` holds N column frames (coordinator
//! decision, report)
//!
//! When [`crate::master::PageMaster::columns`] is `> 1`,
//! [`PageRegionSequence::with_columns`] yields `columns` region PULLS per
//! physical page (column 1, column 2, ..., then the next page's column 1,
//! ...) — [`compose`] therefore returns `columns` [`Frame`]s per page
//! instead of one. Rather than refactor `Page::frame: Frame<'a>` into a
//! `Vec` (which would touch every existing caller of that field —
//! `render.rs`, `export/pdf_adapter.rs`, and this crate's own P0-P4
//! proofs), [`Page`] gains an ADDITIVE [`Page::extra_frames`] field:
//! `frame` stays column 1 (byte-identical to the pre-column single-frame
//! shape when `columns == 1`, since `extra_frames` is simply empty then),
//! `extra_frames` holds columns 2..N. `crate::render::draw_page_layers`/
//! `export::pdf_adapter::pages_to_pdf` both walk `extra_frames` the SAME
//! way they already walk `frame`/`header`/`footer` (design law 1 — same
//! paint/collection recursion, just one more frame list).

use std::collections::{HashMap, HashSet};

use uzor::types::Rect;
use uzor_text::LineShaper;

use crate::compose::{compose, ComposeStyle};
use crate::master::{PageMaster, PageNumberStyle};
use crate::region::{FixedRegionSequence, Frame, PageRegionSequence, Region};
use crate::scene::{resolve_block_ids, BlockId, BlockNode};

/// One sliced page: its 0-based index + the composed [`Frame`] that
/// landed on it, plus (P2) the shared header/footer frames and this
/// page's own resolved page-number placement.
pub struct Page<'a> {
    pub index: u32,
    /// Total pages this `slice_pages` call produced — the SAME value on
    /// every `Page` it returns, needed for `"n of total"`-style page
    /// numbering.
    pub total: u32,
    /// This page's own FIRST column (column 1 when
    /// [`crate::master::PageMaster::columns`] is `> 1`; the WHOLE body
    /// when it's `1`, unchanged from every pre-column phase).
    pub frame: Frame<'a>,
    /// This page's remaining columns (2..N), in order — always empty
    /// when `columns <= 1` (see this module's own "Multi-column pages"
    /// doc comment for why this is additive rather than a `frame: Vec`
    /// refactor).
    pub extra_frames: Vec<Frame<'a>>,
    /// [`crate::master::PageMaster::header`] composed ONCE, shared
    /// verbatim across every page (`None` when the master has no header).
    pub header: Option<Frame<'a>>,
    /// [`crate::master::PageMaster::footer`] composed ONCE, shared
    /// verbatim across every page (`None` when the master has no footer).
    pub footer: Option<Frame<'a>>,
    /// This page's own resolved page-number text + paint rect (`None`
    /// when the master has no [`crate::master::PageMaster::
    /// page_number_token`]).
    pub page_number: Option<PageNumberPlacement>,
    /// Every [`crate::scene::BlockNode::with_outline`]-tagged block that
    /// landed on THIS page (document order), each already resolved to
    /// `page_index == self.index` at [`slice_pages`] time — document
    /// navigation feature pass. Additive shape decision (report): rather
    /// than a second `slice_pages_with_outline(..) -> (Vec<Page>,
    /// Vec<OutlineEntry>)` entry point, entries attach directly to the
    /// `Page` they landed on — matching this module's own established
    /// convention (`header`/`footer`/`page_number` already attach to
    /// `Page` rather than growing `slice_pages`'s own return shape).
    /// [`renumber_pages`] is the "merge outlines across a multi-run
    /// concatenation" helper this attachment shape needs: it re-derives
    /// every contained entry's own `page_index` from its containing
    /// page's (possibly renumbered) `Page::index` — always correct
    /// regardless of how many independent `slice_pages` calls were
    /// concatenated beforehand, since it's recomputed fresh from the
    /// FINAL page positions, never carried forward stale.
    pub outline: Vec<OutlineEntry>,
    /// Every [`crate::scene::BlockNode::with_link_target`]-tagged block's
    /// own placed rect(s) on THIS page — one [`LinkEntry`] per placed
    /// FRAGMENT (a block split across pages gets one entry per fragment,
    /// each covering only that fragment's own on-page rect; unlike
    /// [`Page::outline`], this is intentionally NOT deduplicated to "first
    /// occurrence only" — every visible fragment is a real clickable
    /// area). `target_page` is expected to ALREADY be expressed in
    /// whatever the FINAL concatenated document's page-index space will
    /// be (the same expectation `crate::toc::compose_document_with_toc`'s
    /// own generated rows satisfy via `OutlineEntry::page_index`) —
    /// [`renumber_pages`] deliberately does NOT rewrite `target_page`
    /// (unlike `OutlineEntry::page_index`, which is self-referential to
    /// the page it's attached to and therefore always safe to overwrite;
    /// a link's target is a DIFFERENT page, which `renumber_pages` has no
    /// way to remap without knowing the caller's own pre-concatenation
    /// section boundaries).
    pub links: Vec<LinkEntry>,
}

/// One document-outline/TOC entry, resolved to a concrete page (design
/// doc: counter-introspection multi-pass fixpoint — this is the
/// "introspection index" half: `slice_pages` is the one pass that knows
/// where every tagged block actually landed). `page_index` is 0-based,
/// matching [`Page::index`]'s own convention.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub page_index: u32,
}

/// One internal-link (GoTo) source rect, resolved at [`slice_pages`] time
/// from a [`crate::scene::BlockNode::with_link_target`]-tagged block's own
/// placed [`crate::region::PlacedBlock::rect`] (design law 1 — no second
/// position formula: the SAME rect the block itself painted at).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkEntry {
    pub rect: Rect,
    pub target_page: u32,
}

/// A page's own resolved page-number text, ready to paint at `rect`
/// verbatim (design law 1 — text AND position both resolved once, at
/// slice time, never re-derived at paint time).
#[derive(Debug, Clone, PartialEq)]
pub struct PageNumberPlacement {
    pub text: String,
    pub rect: Rect,
}

/// Compose `flow` over `master`'s page geometry and slice the result into
/// pages — one [`Page`] per [`Frame`] `compose()` produced. Stops as soon
/// as every block is placed (never emits a trailing empty page).
/// [`crate::master::PageMaster::header`]/`footer` (if present) are
/// composed once against their own margin bands and attached to every
/// returned `Page` unchanged; [`crate::master::PageMaster::
/// page_number_token`] (if present) is formatted per-page from this
/// call's own total page count.
///
/// `style`/`shaper` are not part of the design doc's own abbreviated
/// `slice_pages(scene, master)` pseudocode (§4.1) — added here for the
/// same reason [`compose`] needs them (see this crate's `CLAUDE.md`
/// "Divergences from the design doc"), and because P0 does not build
/// `SceneSpec`/`Theme` (§2.1/§5 wasn't part of P0's own deliverables
/// list), `flow` is taken directly rather than via a `&SceneSpec`. This
/// signature is UNCHANGED from P0/P1 (additive law: `PageMaster`/`Page`
/// grew new fields this phase, `slice_pages` needed no new parameter to
/// wire them — painting a theme onto the result is `crate::render::
/// draw_page`'s own job, not this function's).
pub fn slice_pages<'a>(flow: &'a [BlockNode<'a>], master: &PageMaster<'a>, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<Page<'a>> {
    let mut regions = PageRegionSequence::new(master.body_rect()).with_columns(master.columns, master.column_gap);
    let raw_frames = compose(flow, &mut regions, style, shaper);
    let columns = master.columns.max(1);

    let mut pages: Vec<Page<'a>> = Vec::new();
    let mut raw_frames = raw_frames.into_iter();
    let mut index = 0u32;
    while let Some(frame) = raw_frames.next() {
        let mut extra_frames = Vec::new();
        for _ in 1..columns {
            match raw_frames.next() {
                Some(f) => extra_frames.push(f),
                None => break,
            }
        }
        pages.push(Page { index, total: 0, frame, extra_frames, header: None, footer: None, page_number: None, outline: Vec::new(), links: Vec::new() });
        index += 1;
    }

    let total = pages.len() as u32;
    let header_frame = master.header.map(|content| compose_margin_box(content, master.header_rect(), style, shaper));
    let footer_frame = master.footer.map(|content| compose_margin_box(content, master.footer_content_rect(), style, shaper));

    for page in &mut pages {
        page.total = total;
        page.header = header_frame.clone();
        page.footer = footer_frame.clone();
        page.page_number = master
            .page_number_token
            .as_ref()
            .map(|token| PageNumberPlacement { text: token.format_for(page.index, total), rect: master.page_number_rect() });
    }

    attach_navigation_entries(flow, &mut pages);

    pages
}

/// Walk `flow`'s own `.with_outline()`/`.with_link_target()`-tagged nodes
/// and attach the resulting [`OutlineEntry`]/[`LinkEntry`]s to whichever
/// [`Page`]s their placed content appears on — never header/footer (a
/// SEPARATE flow/id namespace, see [`compose_margin_box`]'s own call
/// sites: cross-referencing `flow`'s ids against header/footer
/// `PlacedBlock`s would be comparing [`BlockId`]s from two UNRELATED
/// [`resolve_block_ids`] calls, which could coincidentally collide). An
/// outline-tagged block is attached to its FIRST page only (`emitted`
/// guards against a duplicate entry for a later fragment sharing the same
/// id) — a link-tagged block is attached on EVERY page its own fragments
/// land on (see [`Page::links`]'s own doc comment for why the two differ).
fn attach_navigation_entries<'a>(flow: &'a [BlockNode<'a>], pages: &mut [Page<'a>]) {
    let ids = resolve_block_ids(flow);
    let mut outline_tags: HashMap<BlockId, &crate::scene::OutlineTag> = HashMap::new();
    let mut link_targets: HashMap<BlockId, u32> = HashMap::new();
    for (node, id) in flow.iter().zip(ids.iter()) {
        if let Some(tag) = &node.outline {
            outline_tags.insert(*id, tag);
        }
        if let Some(target) = node.link_target {
            link_targets.insert(*id, target);
        }
    }
    if outline_tags.is_empty() && link_targets.is_empty() {
        return;
    }

    let mut emitted: HashSet<BlockId> = HashSet::new();
    for page in pages.iter_mut() {
        let mut outline_entries = Vec::new();
        let mut link_entries = Vec::new();
        for placed in page.frame.blocks.iter().chain(page.extra_frames.iter().flat_map(|f| f.blocks.iter())) {
            if let Some(tag) = outline_tags.get(&placed.id) {
                if emitted.insert(placed.id) {
                    outline_entries.push(OutlineEntry { level: tag.level, title: tag.title.clone(), page_index: page.index });
                }
            }
            if let Some(&target_page) = link_targets.get(&placed.id) {
                link_entries.push(LinkEntry { rect: placed.rect, target_page });
            }
        }
        page.outline = outline_entries;
        page.links = link_entries;
    }
}

/// Renumber a concatenation of several independently-sliced [`Page`]
/// sections into one continuous document (the showcase's own 3-run
/// concat pattern, and [`crate::toc::compose_document_with_toc`]'s own
/// TOC-pages-then-body-pages concatenation) — fixes up `Page::index`/
/// `Page::total`, every contained [`OutlineEntry::page_index`] (so a
/// generated TOC/PDF-outline built from a LATER `pages[i].outline` read
/// always matches this call's own FINAL page positions, never a stale
/// per-section index), and — when `page_number_style` is given —
/// re-formats every already-attached [`PageNumberPlacement::text`]. Only
/// TEXT/index bookkeeping is rewritten; every already-baked geometry
/// field (frame/extra_frames/header/footer content, the page-number's own
/// paint rect) is untouched.
pub fn renumber_pages<'a>(mut pages: Vec<Page<'a>>, page_number_style: Option<&PageNumberStyle>) -> Vec<Page<'a>> {
    let total = pages.len() as u32;
    for (i, page) in pages.iter_mut().enumerate() {
        page.index = i as u32;
        page.total = total;
        for entry in &mut page.outline {
            entry.page_index = page.index;
        }
        if let Some(style) = page_number_style {
            if let Some(number) = &mut page.page_number {
                number.text = style.format_for(page.index, total);
            }
        }
    }
    pages
}

/// Compose fixed margin-box content (header/footer) ONCE into a single
/// [`Frame`] at `rect` — the SAME content painted unchanged on every page
/// (design doc §4.1). Content that overflows `rect` degrades the same
/// documented way any atomic overflow elsewhere in this crate does (P0's
/// own risk note): whatever fits is placed, the remainder is simply not
/// represented (a margin box has no second region to spill into — a
/// header/footer is expected to be a couple of short lines).
fn compose_margin_box<'a>(content: &'a [BlockNode<'a>], rect: Rect, style: &ComposeStyle, shaper: &dyn LineShaper) -> Frame<'a> {
    let mut regions = FixedRegionSequence::new(rect);
    compose(content, &mut regions, style, shaper)
        .into_iter()
        .next()
        .unwrap_or(Frame { region: Region { rect }, blocks: Vec::new(), overflow: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{layout_paragraph, CosmicShaper, FontSpec, Paragraph, StyledRun};

    use crate::master::{Margins, PageNumberFormat, PageNumberStyle};
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

    /// P2: a master's header/footer content + page-number token must be
    /// attached to EVERY sliced page, with the correct `"n of total"`
    /// text per page (design doc §4.1: "resolved per-page during
    /// slicing").
    #[test]
    fn header_footer_and_page_number_are_attached_to_every_sliced_page() {
        let font = body_font();
        let shaper = CosmicShaper::headless();

        let header_run = [StyledRun::new("Header", font)];
        let header_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&header_run, 200.0)))];
        let footer_run = [StyledRun::new("Footer", font)];
        let footer_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&footer_run, 200.0)))];

        let master = PageMaster::new(400.0, 300.0, Margins::uniform(30.0))
            .with_header(&header_flow)
            .with_footer(&footer_flow)
            .with_page_number(PageNumberStyle::new(PageNumberFormat::OfTotal, 1));

        // Enough repeated paragraphs to force at least 2 pages of this
        // small (400x300, 30px margins) fixture page.
        let paragraph_texts: Vec<String> = (0..8).map(|i| format!("Paragraph {i}: {SENTENCE}")).collect();
        let runs: Vec<[StyledRun<'_>; 1]> = paragraph_texts.iter().map(|t| [StyledRun::new(t.as_str(), font)]).collect();
        let flow: Vec<BlockNode<'_>> =
            runs.iter().map(|r| BlockNode::new(Block::Paragraph(Paragraph::new(r, master.body_rect().width)))).collect();

        let style = ComposeStyle::new(0.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert!(pages.len() >= 2, "fixture must span at least 2 pages to prove the number/total wiring");

        for (i, page) in pages.iter().enumerate() {
            assert!(page.header.as_ref().is_some_and(|h| !h.blocks.is_empty()), "every page must carry the header");
            assert!(page.footer.as_ref().is_some_and(|f| !f.blocks.is_empty()), "every page must carry the footer");
            let number = page.page_number.as_ref().expect("page-number token was configured on the master");
            assert_eq!(number.text, format!("{} of {}", i + 1, pages.len()));
        }
    }

    #[test]
    fn a_master_without_header_footer_or_page_number_produces_none_for_all_three() {
        let font = body_font();
        let shaper = CosmicShaper::headless();
        let runs = [StyledRun::new("A single short paragraph.", font)];
        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, 400.0)))];
        let master = PageMaster::new(500.0, 700.0, Margins::uniform(40.0));
        let style = ComposeStyle::new(0.0, font);

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].header.is_none());
        assert!(pages[0].footer.is_none());
        assert!(pages[0].page_number.is_none());
    }

    /// `PageMaster::with_columns`: content that overflows a single column
    /// lands in `Page::frame` (column 1) THEN `Page::extra_frames[0]`
    /// (column 2), both on the SAME physical page (`Page::index == 0`) —
    /// never spilling to a second `Page` while column 2 still has room.
    /// Column 1 must fill BEFORE column 2 (every column-1 block's own `y`
    /// stays within column 1's own vertical span before column 2 gets any
    /// content), and every placed block's rect must stay within its own
    /// column's width — never the full body width.
    #[test]
    fn two_column_master_fills_column_one_before_column_two_on_the_same_page() {
        let font = body_font();
        let shaper = CosmicShaper::headless();
        let master = PageMaster::new(600.0, 500.0, Margins::uniform(20.0)).with_columns(2, 18.0);
        let column_width = master.column_width();
        assert!(column_width < master.body_rect().width, "a 2-column body must be narrower per column than the full body");

        // Enough repeated short paragraphs that column 1 fills up and
        // content spills into column 2, but not so much that it spills
        // onto a second physical page.
        let paragraph_texts: Vec<String> = (0..14).map(|i| format!("Column paragraph number {i} of the fixture.")).collect();
        let runs: Vec<[StyledRun<'_>; 1]> = paragraph_texts.iter().map(|t| [StyledRun::new(t.as_str(), font)]).collect();
        let flow: Vec<BlockNode<'_>> = runs.iter().map(|r| BlockNode::new(Block::Paragraph(Paragraph::new(r, column_width)))).collect();

        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "fixture must be tuned to fit within column 1 + column 2 of ONE page");
        assert!(!pages[0].frame.blocks.is_empty(), "column 1 must carry content");
        assert!(!pages[0].extra_frames.is_empty(), "column 2 must carry the overflow from column 1");

        let col1_width = pages[0].frame.region.rect.width;
        let col2_width = pages[0].extra_frames[0].region.rect.width;
        assert!((col1_width - column_width).abs() < 1e-6);
        assert!((col2_width - column_width).abs() < 1e-6);

        // Every block placed in column 1 must never exceed column 1's own
        // width, and likewise for column 2 — text never crosses into the
        // other column or the gap between them.
        for placed in &pages[0].frame.blocks {
            assert!(placed.rect.width <= col1_width + 0.01, "column 1 content must never exceed column 1's own width");
        }
        for placed in &pages[0].extra_frames[0].blocks {
            assert!(placed.rect.width <= col2_width + 0.01, "column 2 content must never exceed column 2's own width");
        }

        // Column 1 must be FULLY consumed (its own frame reports
        // overflow — content continued into column 2) before column 2
        // received anything at all — proving "fills column 1 before
        // column 2," not an arbitrary split.
        assert!(pages[0].frame.overflow.is_some(), "column 1 must report overflow into column 2 for this fixture to be meaningful");
    }

    /// A table placed inside a 2-column master sizes its own columns to
    /// the CONTAINING column's width, never the full (wider) page body.
    #[test]
    fn a_table_inside_a_column_is_sized_to_the_column_width_not_the_full_body() {
        use crate::scene::{Block, ColumnSpec, TableBlock, TableCell, TableRow};

        let font = body_font();
        let shaper = CosmicShaper::headless();
        let master = PageMaster::new(600.0, 500.0, Margins::uniform(20.0)).with_columns(2, 18.0);
        let column_width = master.column_width();

        let cell_run = [StyledRun::new("cell", font)];
        let cell_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&cell_run, f64::MAX)))];
        let cells = [TableCell::new(&cell_nodes)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Fraction(1.0)];
        let table = TableBlock::new(&columns, &rows);
        let flow = [BlockNode::new(Block::Table(table))];

        let style = ComposeStyle::new(6.0, font);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        let placed = pages[0].frame.blocks.first().expect("table must be placed in column 1");
        let table_width: f64 = placed.table_placement.as_ref().expect("table placement present").column_widths.iter().sum();

        assert!(
            (table_width - column_width).abs() < 1e-6,
            "a Fraction(1.0) column inside a table must fill the CONTAINING column's width ({column_width}), got {table_width}"
        );
        assert!(table_width < master.body_rect().width - 1.0, "the table must be narrower than the full (un-columned) body width");
    }
}
