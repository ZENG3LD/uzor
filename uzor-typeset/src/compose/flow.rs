//! [`compose`] — pure function of `(flow, regions, style, shaper)`, no
//! retained document (design law 3). P0 scope: greedy block-stacking only —
//! no keep-break, no tables, no islands, no columns (design doc §7 P0
//! entry).
//!
//! ## Splitting a paragraph across regions (§3.3)
//!
//! `compose` measures each [`crate::scene::Block::Paragraph`] exactly once
//! per region-width via [`uzor_text::layout_paragraph`] (the content_size
//! seam — "one measure path", design law 1) and caches that
//! [`ParagraphLayout`] for as long as the SAME block continues across
//! region boundaries. When a paragraph doesn't fit the remaining space of
//! its current region, [`super::lines_fitting`]/[`super::slice_layout_lines`]
//! split the ALREADY-COMPUTED layout at a line boundary — no glyph is ever
//! duplicated, dropped, or cut mid-cluster, and every line is accounted for
//! in exactly one [`PlacedBlock`].

use uzor::types::Rect;
use uzor_text::{layout_paragraph, FontSpec, LineShaper, Paragraph, ParagraphLayout};

use super::{lines_fitting, slice_layout_lines};
use crate::region::{Frame, PlacedBlock, RegionSequence};
use crate::scene::{resolve_block_ids, Block, BlockNode};

/// Composition-wide styling `compose` itself needs (as opposed to a
/// per-block override, which lives on `BlockNode`/`Block` once P1/P2 land).
#[derive(Debug, Clone, Copy)]
pub struct ComposeStyle {
    /// Auto vertical gap inserted after a FULLY-placed flow block (never
    /// after a block that continues into the next region — see
    /// [`compose`]'s own loop).
    pub paragraph_spacing: f64,
    /// Reserved for later phases that need a fallback font independent of
    /// any one block (e.g. a synthesized page-number token, P2). Not read
    /// by P0's own compose loop — every `Block::Paragraph` already carries
    /// its own `FontSpec` per run.
    pub default_font: FontSpec,
}

impl ComposeStyle {
    pub fn new(paragraph_spacing: f64, default_font: FontSpec) -> Self {
        Self { paragraph_spacing, default_font }
    }
}

/// Flow `flow`'s blocks through `regions`, producing one [`Frame`] per
/// region actually consumed (composition stops once every block has been
/// placed — an infinite [`crate::region::PageRegionSequence`] is never
/// over-pulled for a fresh, empty trailing region).
///
/// `shaper` is the same [`LineShaper`] every `uzor-text` measure/paint call
/// site takes — not part of the design doc's own abbreviated `compose()`
/// pseudocode (§3.2), added here because `layout_paragraph` requires one;
/// see this crate's `CLAUDE.md` "Divergences from the design doc".
pub fn compose<'a>(flow: &'a [BlockNode<'a>], regions: &mut dyn RegionSequence, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<Frame<'a>> {
    let ids = resolve_block_ids(flow);
    let mut frames = Vec::new();

    let mut block_idx = 0usize;
    let mut next_line = 0usize;
    let mut layout_cache: Option<ParagraphLayout> = None;

    while block_idx < flow.len() {
        let Some(region) = regions.next() else { break };
        let region_bottom = region.rect.y + region.rect.height;
        let mut cursor_y = region.rect.y;
        let mut blocks: Vec<PlacedBlock<'a>> = Vec::new();
        let mut overflow = None;

        while block_idx < flow.len() {
            let node = &flow[block_idx];
            let remaining_height = (region_bottom - cursor_y).max(0.0);
            let region_has_content = !blocks.is_empty();

            match &node.kind {
                Block::Spacer(gap) => {
                    if *gap <= remaining_height || !region_has_content {
                        let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, *gap);
                        blocks.push(PlacedBlock { id: ids[block_idx], rect, kind: &node.kind, paragraph_layout: None });
                        // No extra `style.paragraph_spacing` on top of an
                        // explicit Spacer's own gap — a Spacer already IS
                        // the author's deliberate gap; auto-spacing is only
                        // the DEFAULT applied between two blocks that
                        // didn't get one (see the `Block::Paragraph` arm
                        // below). Doubling the two would make an author's
                        // exact requested gap unrecoverable without first
                        // knowing `paragraph_spacing`'s own value.
                        cursor_y += gap;
                        block_idx += 1;
                        next_line = 0;
                        layout_cache = None;
                    } else {
                        overflow = Some(node);
                        break;
                    }
                }
                Block::Paragraph(paragraph) => {
                    // A flow paragraph inherits its column width from
                    // whatever region it's placed into (Typst's own
                    // "regions own the width" model, §3.1) — never its own
                    // authored `max_width`. `Paragraph` is `Copy`, so this
                    // is a cheap field swap, zero-copy for the actual run
                    // text. P0 only re-measures once per block (cached
                    // across region boundaries): since `PageRegionSequence`
                    // always hands back the same body width every page,
                    // this is always consistent for P0's own regions — see
                    // this crate's `CLAUDE.md` for the (deferred)
                    // different-width-continuation case.
                    let full = layout_cache
                        .get_or_insert_with(|| layout_paragraph(&Paragraph { max_width: region.rect.width, ..*paragraph }, shaper));

                    if full.lines.is_empty() {
                        // Empty paragraph: nothing to place, just advance.
                        block_idx += 1;
                        next_line = 0;
                        layout_cache = None;
                        continue;
                    }

                    let count = lines_fitting(full, next_line, remaining_height, !region_has_content);
                    if count == 0 {
                        overflow = Some(node);
                        break;
                    }

                    let end_line = next_line + count;
                    let placed_layout = slice_layout_lines(full, next_line, end_line);
                    let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, placed_layout.height);
                    cursor_y += placed_layout.height;
                    blocks.push(PlacedBlock { id: ids[block_idx], rect, kind: &node.kind, paragraph_layout: Some(placed_layout) });

                    if end_line >= full.lines.len() {
                        // Whole paragraph placed — the FULL layout is done
                        // with, drop the cache and move on.
                        cursor_y += style.paragraph_spacing;
                        block_idx += 1;
                        next_line = 0;
                        layout_cache = None;
                    } else {
                        // Continues into the next region — keep the cached
                        // full layout so the remainder is a slice, not a
                        // re-measure.
                        next_line = end_line;
                        overflow = Some(node);
                        break;
                    }
                }
            }
        }

        frames.push(Frame { region, blocks, overflow });
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{layout_paragraph, CosmicShaper, Paragraph, StyledRun};

    use crate::region::{PageRegionSequence, Region};
    use crate::scene::BlockNode;

    const TALL_REGION_HEIGHT: f64 = 2000.0;
    const REGION_WIDTH: f64 = 300.0;

    fn tall_region() -> Region {
        Region { rect: Rect::new(0.0, 0.0, REGION_WIDTH, TALL_REGION_HEIGHT) }
    }

    /// A single-region sequence that hands back one fixed rect once, then
    /// ends (`None`) — a finite sequence, matching `RegionSequence`'s own
    /// documented "table cell" case, used here to exercise `compose()`
    /// without pulling in `PageRegionSequence`.
    struct OnceRegionSequence {
        region: Option<Region>,
    }

    impl RegionSequence for OnceRegionSequence {
        fn next(&mut self) -> Option<Region> {
            self.region.take()
        }
    }

    /// Two paragraphs + a spacer, composed into ONE tall region, must stack
    /// top-to-bottom with the spacer's own fixed gap plus
    /// `ComposeStyle::paragraph_spacing` between every consecutive block,
    /// and each paragraph's placed rect height must equal its own
    /// `ParagraphLayout::height` exactly (design law 1: no second position
    /// formula).
    #[test]
    fn two_paragraphs_and_a_spacer_stack_with_correct_gaps_and_exact_heights() {
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let runs_a = [StyledRun::new("First paragraph of the stacking fixture.", font)];
        let runs_b = [StyledRun::new(
            "Second paragraph, long enough to wrap onto more than one line at this narrow region width so the stacking test also exercises multi-line height.",
            font,
        )];
        const SPACER_GAP: f64 = 24.0;
        const PARAGRAPH_SPACING: f64 = 10.0;

        let flow = [
            BlockNode::new(Block::Paragraph(Paragraph::new(&runs_a, REGION_WIDTH))),
            BlockNode::new(Block::Spacer(SPACER_GAP)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&runs_b, REGION_WIDTH))),
        ];

        let expected_a = layout_paragraph(&Paragraph::new(&runs_a, REGION_WIDTH), &shaper);
        let expected_b = layout_paragraph(&Paragraph::new(&runs_b, REGION_WIDTH), &shaper);
        assert!(expected_b.lines.len() > 1, "fixture must wrap to multiple lines");

        let style = ComposeStyle::new(PARAGRAPH_SPACING, font);
        let mut regions = OnceRegionSequence { region: Some(tall_region()) };
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 1, "a single tall region must hold every block in one frame");
        let frame = &frames[0];
        assert_eq!(frame.blocks.len(), 3);
        assert!(frame.overflow.is_none());

        let para_a = &frame.blocks[0];
        assert_eq!(para_a.rect.y, 0.0);
        assert_eq!(para_a.rect.height, expected_a.height, "placed rect height must equal the ParagraphLayout height exactly");

        let spacer = &frame.blocks[1];
        let expected_spacer_y = expected_a.height + PARAGRAPH_SPACING;
        assert!((spacer.rect.y - expected_spacer_y).abs() < 1e-9);
        assert_eq!(spacer.rect.height, SPACER_GAP);
        assert!(spacer.paragraph_layout.is_none());

        let para_b = &frame.blocks[2];
        // No extra `paragraph_spacing` stacks on top of the explicit
        // Spacer's own gap (see `compose::flow`'s `Block::Spacer` arm).
        let expected_b_y = expected_spacer_y + SPACER_GAP;
        assert!((para_b.rect.y - expected_b_y).abs() < 1e-9);
        assert_eq!(para_b.rect.height, expected_b.height, "placed rect height must equal the ParagraphLayout height exactly");
    }

    /// A paragraph taller than a single region must split at a line
    /// boundary: the remainder continues into the next region with no
    /// line orphaned mid-glyph and every line conserved across the two
    /// placements.
    #[test]
    fn a_paragraph_taller_than_one_region_splits_at_a_line_boundary_and_conserves_every_line() {
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";
        let runs = [StyledRun::new(text, font)];
        let narrow_width = 140.0;

        let full = layout_paragraph(&Paragraph::new(&runs, narrow_width), &shaper);
        assert!(full.lines.len() >= 6, "fixture must wrap to several lines");

        // A region tall enough for roughly half the lines, forcing a split.
        let region_height = full.lines[0].height * (full.lines.len() as f64 / 2.0).floor();

        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, narrow_width)))];
        let style = ComposeStyle::new(0.0, font);
        let mut page_regions = PageRegionSequence::new(Rect::new(0.0, 0.0, narrow_width, region_height));
        let frames = compose(&flow, &mut page_regions, &style, &shaper);

        assert_eq!(frames.len(), 2, "the paragraph must span exactly 2 regions");
        assert!(frames[0].overflow.is_some(), "the first frame must report the overflowing block");
        assert!(frames[1].overflow.is_none(), "the second frame must finish the paragraph");

        let head = frames[0].blocks[0].paragraph_layout.as_ref().expect("paragraph placement carries a layout");
        let tail = frames[1].blocks[0].paragraph_layout.as_ref().expect("paragraph placement carries a layout");

        assert_eq!(head.lines.len() + tail.lines.len(), full.lines.len(), "every line must be conserved across the split");
        assert_eq!(head.glyphs.len() + tail.glyphs.len(), full.glyphs.len(), "every glyph must be conserved across the split");
        assert!(head.height <= region_height + 1.0, "the head placement must fit within the region it was split for");
    }
}
