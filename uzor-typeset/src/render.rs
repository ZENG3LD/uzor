//! Paint a composed [`Page`] onto a [`RenderContext`] — design law 6
//! (native measure/paint seams, no bespoke ones): paragraphs paint through
//! `uzor_text::draw_paragraph` unchanged, this crate invents no new text
//! drawing primitive. Not itself named as a separate file in the design
//! doc's P0 deliverables list, but required to fulfill that same phase's
//! own "headless via `uzor-export::render_to_png`" demo/proof requirement.

use uzor::render::RenderContext;
use uzor_text::draw_paragraph;

use crate::scene::Block;
use crate::slice::Page;

/// Paint every placed block of `page` onto `ctx`, each at its own
/// frame-relative rect origin (design law 1: one transform — every draw
/// derives its position from the SAME `PlacedBlock::rect`/
/// `paragraph_layout` [`crate::compose::compose`] already computed, never
/// a second ad hoc position formula).
pub fn draw_page(ctx: &mut dyn RenderContext, page: &Page<'_>, default_color: &str) {
    for placed in &page.frame.blocks {
        match (placed.kind, &placed.paragraph_layout) {
            (Block::Paragraph(_), Some(layout)) => {
                draw_paragraph(ctx, (placed.rect.x, placed.rect.y), layout, default_color, false);
            }
            // A paragraph placement always carries a layout (compose()'s
            // own invariant) and a spacer paints nothing — both fall
            // through here without drawing.
            (Block::Paragraph(_), None) | (Block::Spacer(_), _) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    //! Headless proof: a fixed seeded 2-page report-shaped document —
    //! mixed alignments (Left/Center/Right) plus one Justify +
    //! `KnuthPlass` + `Hyphenation::English` paragraph (proves `uzor-text`
    //! is fully live inside `uzor-typeset`, not just re-exported), sliced
    //! at A4-ish 595x842 via `slice_pages` and painted through
    //! [`draw_page`] — written to `uzor/out/` for a human to eyeball
    //! (design law 8: one screenshot is never proof, hence the 2-page
    //! pair).

    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_export::{render_to_png, ExportSpec};
    use uzor_text::{BreakStrategy, CosmicShaper, FontSpec, Hyphenation, Paragraph, ParagraphAlign, StyledRun};

    use super::draw_page;
    use crate::compose::ComposeStyle;
    use crate::scene::{Block, BlockNode};
    use crate::slice::{slice_pages, Margins, PageMaster};

    const PAGE_WIDTH: u32 = 595;
    const PAGE_HEIGHT: u32 = 842;

    fn out_dir() -> PathBuf {
        // Fixed path — `uzor/out/` is the shared human-eyeball drop point
        // for every headless proof render in this workspace (matches
        // `uzor-text`/`uzor-figures`'s own `out_dir` convention).
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    /// Fixed seeded fixture text (design law 8: no lorem-ipsum RNG). Long
    /// enough, repeated across several paragraphs, to overflow one A4-ish
    /// page body and split a paragraph across the page boundary.
    const FILLER_A: &str = "The confidence-matrix narrative for this section walks through every \
        traced hop in order, noting timestamps, counterparties, and the amount observed at each \
        step of the flow, so a reader can follow the chain of custody from the initial deposit all \
        the way through to the resting balance without needing to cross-reference a separate table.";
    const FILLER_B: &str = "Business documents full of long compound words like implementation, \
        infrastructure, counterparty, and accountability often expose uneven interword spacing in a \
        narrow justified column, especially once hyphenation is enabled and a break lands mid-word \
        near the right margin, which is exactly the case this fixed fixture paragraph is built to show.";

    fn seeded_flow() -> Vec<BlockNode<'static>> {
        // `'static` runs/text so the fixture can be built once and shared
        // by both the layout-count assertions and the PNG render closure
        // below without lifetime friction.
        static TITLE_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "Case Report — uzor-typeset P0 Proof", font: TITLE_FONT, color: None }];
        static LEFT_A_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None }];
        static LEFT_B_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];
        static CENTER_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "Section II — Centered Summary Heading", font: BODY_FONT, color: None }];
        static RIGHT_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "— Exhibit reference, right-aligned —", font: BODY_FONT, color: None }];
        static JUSTIFY_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];
        static TAIL_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None }];
        static EXTRA_A_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];
        static EXTRA_B_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None }];
        static EXTRA_C_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 16.0, bold: false, italic: false };

        let body_width = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0)).body_rect().width;

        vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&TITLE_RUN, body_width))),
            BlockNode::new(Block::Spacer(20.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&LEFT_A_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(
                Paragraph::new(&CENTER_RUN, body_width).with_align(ParagraphAlign::Center),
            )),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&RIGHT_RUN, body_width).with_align(ParagraphAlign::Right))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(
                Paragraph::new(&JUSTIFY_RUN, body_width)
                    .with_align(ParagraphAlign::Justify)
                    .with_break_strategy(BreakStrategy::KnuthPlass)
                    .with_hyphenation(Hyphenation::English),
            )),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&LEFT_B_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&TAIL_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&EXTRA_A_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&EXTRA_B_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&EXTRA_C_RUN, body_width))),
        ]
    }

    #[test]
    fn seeded_two_page_document_renders_to_two_valid_pngs_with_margins_and_a_split_paragraph() {
        let flow = seeded_flow();
        let master = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0));
        let style = ComposeStyle::new(14.0, FontSpec::new(FontFamily::Roboto, 16.0));
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2, "fixture is tuned to land on exactly 2 pages, got {}", pages.len());

        // A REAL mid-paragraph split (not just a whole-block deferral) —
        // page 1's last placed block and page 2's first placed block share
        // the same effective `BlockId` (same source paragraph) and each
        // carries only part of its full line count — the "split paragraph
        // continues cleanly on page 2" proof condition.
        assert!(pages[0].frame.overflow.is_some(), "fixture must force a mid-document split across the 2 pages");
        let page1_last = pages[0].frame.blocks.last().expect("page 1 must have content");
        let page2_first = pages[1].frame.blocks.first().expect("page 2 must have content");
        assert_eq!(page1_last.id, page2_first.id, "the split must continue the SAME source block onto page 2, not start a different one");
        let head_lines = page1_last.paragraph_layout.as_ref().expect("split block carries a layout").lines.len();
        let tail_lines = page2_first.paragraph_layout.as_ref().expect("split block carries a layout").lines.len();
        assert!(head_lines > 0 && tail_lines > 0, "both halves of the split must carry at least one whole line, never an empty placement");

        let spec = ExportSpec { width_px: PAGE_WIDTH, height_px: PAGE_HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };

        let page1_bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[0], "#111111")).expect("page 1 proof render should succeed");
        assert_eq!(decoded_png_dims(&page1_bytes), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p0_page1.png", &page1_bytes);

        let page2_bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[1], "#111111")).expect("page 2 proof render should succeed");
        assert_eq!(decoded_png_dims(&page2_bytes), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p0_page2.png", &page2_bytes);

        // Every placed block on both pages must respect the page margins
        // (never paint outside the 40px-inset body rect).
        let body = master.body_rect();
        for page in &pages {
            for placed in &page.frame.blocks {
                assert!(placed.rect.x >= body.x - 0.01, "block must not start left of the margin");
                assert!(placed.rect.x + placed.rect.width <= body.x + body.width + 0.01, "block must not extend past the right margin");
            }
        }
    }
}
