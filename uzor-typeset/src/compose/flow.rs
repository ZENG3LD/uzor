//! [`compose`] — pure function of `(flow, regions, style, shaper)`, no
//! retained document (design law 3).
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
//!
//! ## Row-atomic table splitting (§3.5)
//!
//! [`crate::scene::Block::Table`] follows the SAME "measure once, cache
//! across regions" shape as a paragraph, just at ROW granularity instead
//! of line granularity: [`super::table_layout::measure_and_layout_table`]
//! runs once (at the width of the region the table first entered),
//! [`super::table_layout::rows_fitting`]/[`super::table_layout::place_table_rows`]
//! do the per-region row slicing a paragraph's `lines_fitting`/
//! `slice_layout_lines` do for lines.
//!
//! ## Atomic blocks — figures, images, lists (§2.2/§3.6)
//!
//! [`crate::scene::Block::Figure`]/[`Block::Image`]/[`Block::List`] never
//! split: each resolves a single content height (via
//! [`crate::scene::BlockSizing::resolve_height`] for figures/images, via
//! [`super::list_layout::place_list`]'s own total for lists) and is placed
//! whole-or-deferred, exactly like [`Block::Spacer`]'s own gap.
//!
//! ## Keep/break control (§3.4)
//!
//! [`crate::compose::keep_break::BreakControl`] is consulted at three
//! points: `ForceBefore` (checked before this node is considered at all —
//! if the region already has other content, defer to a fresh one),
//! `ForceAfter` (checked right after a FULL placement — ends the current
//! frame so the NEXT node starts fresh), and `AvoidInside`/`AvoidAfter`
//! (both need to peek ahead — see [`full_height_if_whole`]/
//! [`has_room_for_next`]'s own docs).

use uzor::types::Rect;
use uzor_text::{layout_paragraph, FontSpec, LineShaper, Paragraph, ParagraphLayout};

use super::keep_break::BreakControl;
use super::list_layout::place_list;
use super::table_layout::{measure_and_layout_table, place_table_rows, rows_fitting, table_total_height, ComposedRow};
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
    /// Fallback font independent of any one block. P0 reserved this field
    /// unread; P1's [`crate::scene::ListBlock`] is its first real
    /// consumer — every list marker (`•`, `"3."`, ...) paints using this
    /// font (see `compose::list_layout`).
    pub default_font: FontSpec,
}

impl ComposeStyle {
    pub fn new(paragraph_spacing: f64, default_font: FontSpec) -> Self {
        Self { paragraph_spacing, default_font }
    }

    /// Build a `ComposeStyle` whose `default_font` resolves through
    /// `theme`'s own semantic tier (`FontRole::Body`) — design doc §5:
    /// "paragraph default fonts/colors resolve through [the theme]." The
    /// per-run/per-paragraph color a `draw_paragraph` fallback paints
    /// with is a separate, `crate::render::draw_page`-side resolution
    /// (`Theme::color_hex(ColorRole::Ink)`) — `ComposeStyle` itself only
    /// ever carries layout-affecting fields (design law 3), never a paint
    /// color.
    pub fn from_theme(theme: &crate::style::Theme, paragraph_spacing: f64) -> Self {
        Self { paragraph_spacing, default_font: theme.font_spec(crate::style::FontRole::Body) }
    }
}

/// In-progress split state for whichever flow block `compose`'s loop is
/// CURRENTLY placing (persists only across the SAME block's own
/// continuation into a later region — reset to `None` the moment that
/// block is fully placed or a different block starts). Atomic block kinds
/// (`Figure`/`Image`/`List`) never need an entry here — an atomic block is
/// either placed whole or deferred whole, nothing to resume.
enum InProgress<'a> {
    None,
    Paragraph { layout: ParagraphLayout, next_line: usize },
    Table { column_widths: Vec<f64>, rows: Vec<ComposedRow<'a>>, next_row: usize },
}

/// The height this node WOULD occupy if placed FULLY (no split) at
/// `region_width` — used only by the `AvoidAfter` keep-with-next
/// lookahead (never by real placement, which always re-derives its own
/// split-aware height per block kind through its own normal path).
fn full_height_if_whole(kind: &Block<'_>, region_width: f64, remaining_height: f64, style: &ComposeStyle, shaper: &dyn LineShaper) -> f64 {
    match kind {
        Block::Spacer(gap) => *gap,
        Block::Paragraph(p) => layout_paragraph(&Paragraph { max_width: region_width, ..*p }, shaper).height,
        Block::Figure(fb) => fb.sizing.resolve_height(region_width, remaining_height),
        Block::Image(ib) => ib.sizing.resolve_height(region_width, remaining_height),
        Block::Table(table) => {
            let (_, rows) = measure_and_layout_table(table, region_width, style, shaper);
            table_total_height(&rows)
        }
        Block::List(list) => {
            let (_, total_height) = place_list(list, Rect::new(0.0, 0.0, region_width, remaining_height), style, shaper);
            total_height
        }
    }
}

/// Would `next` get to place AT LEAST SOME content within
/// `remaining_height` — the `AvoidAfter`/keep-with-next lookahead's own
/// "is there a point in keeping these two together" test. An atomic block
/// (`Figure`/`Image`/`List`) only ever gets "some" room by fitting
/// ENTIRELY (there's no partial placement for one); a `Paragraph` gets
/// "some" room the moment even one line fits; a `Table` gets "some" room
/// the moment its first row fits.
fn has_room_for_next(next: &Block<'_>, region_width: f64, remaining_height: f64, style: &ComposeStyle, shaper: &dyn LineShaper) -> bool {
    if remaining_height <= 0.0 {
        return false;
    }
    match next {
        Block::Spacer(gap) => *gap <= remaining_height,
        Block::Paragraph(p) => {
            let full = layout_paragraph(&Paragraph { max_width: region_width, ..*p }, shaper);
            full.lines.is_empty() || lines_fitting(&full, 0, remaining_height, false) > 0
        }
        Block::Figure(fb) => fb.sizing.resolve_height(region_width, remaining_height) <= remaining_height,
        Block::Image(ib) => ib.sizing.resolve_height(region_width, remaining_height) <= remaining_height,
        Block::Table(table) => {
            let (_, rows) = measure_and_layout_table(table, region_width, style, shaper);
            rows.first().map_or(true, |r| r.height <= remaining_height)
        }
        Block::List(list) => {
            let (_, total_height) = place_list(list, Rect::new(0.0, 0.0, region_width, remaining_height), style, shaper);
            total_height <= remaining_height
        }
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
    let mut progress: InProgress<'a> = InProgress::None;

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

            // `ForceBefore`: never re-triggers on a node's own
            // continuation into a later region — a continuing block is
            // always the very first thing considered in its fresh frame,
            // where `region_has_content` is already `false`.
            if node.break_control == BreakControl::ForceBefore && region_has_content {
                overflow = Some(node);
                break;
            }

            // `AvoidAfter` (keep-with-next): only applies the FIRST time
            // this node is considered in a region that already holds
            // other content — deferring a node that's already first in
            // its own fresh region would risk looping forever if this
            // node can never coexist with what follows within one
            // region's fixed size (see this module's own risk note,
            // mirrored from P0's `lines_fitting` force-degrade).
            if node.break_control == BreakControl::AvoidAfter && region_has_content && block_idx + 1 < flow.len() {
                let trial_height = full_height_if_whole(&node.kind, region.rect.width, remaining_height, style, shaper);
                if trial_height <= remaining_height {
                    let remaining_after = (remaining_height - trial_height - style.paragraph_spacing).max(0.0);
                    let next_fits = has_room_for_next(&flow[block_idx + 1].kind, region.rect.width, remaining_after, style, shaper);
                    if !next_fits {
                        overflow = Some(node);
                        break;
                    }
                }
            }

            // `true` when `node` was FULLY placed this iteration (never a
            // pre-declared default — every arm below either diverges via
            // `break`/`continue` or ends in a real `bool`, so there is no
            // dead initial value to satisfy the unused-assignments lint).
            let node_fully_placed = match &node.kind {
                Block::Spacer(gap) => {
                    if *gap <= remaining_height || !region_has_content {
                        let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, *gap);
                        blocks.push(PlacedBlock {
                            id: ids[block_idx],
                            rect,
                            kind: &node.kind,
                            paragraph_layout: None,
                            table_placement: None,
                            list_placement: None,
                        });
                        // No extra `style.paragraph_spacing` on top of an
                        // explicit Spacer's own gap — a Spacer already IS
                        // the author's deliberate gap.
                        cursor_y += gap;
                        block_idx += 1;
                        progress = InProgress::None;
                        true
                    } else {
                        overflow = Some(node);
                        break;
                    }
                }
                Block::Paragraph(paragraph) => {
                    if !matches!(progress, InProgress::Paragraph { .. }) {
                        let measured = layout_paragraph(&Paragraph { max_width: region.rect.width, ..*paragraph }, shaper);
                        progress = InProgress::Paragraph { layout: measured, next_line: 0 };
                    }
                    let InProgress::Paragraph { layout: full, next_line } = &progress else {
                        unreachable!("just ensured Paragraph progress")
                    };
                    let next_line = *next_line;

                    if full.lines.is_empty() {
                        block_idx += 1;
                        progress = InProgress::None;
                        continue;
                    }

                    // `AvoidInside` ("keep-together"): only gates the
                    // FIRST placement attempt — once a prior region's
                    // fresh-but-too-small degrade already split this
                    // paragraph, there's no benefit to blocking further
                    // splits of what remains.
                    if node.break_control == BreakControl::AvoidInside && next_line == 0 && region_has_content {
                        let whole_height: f64 = full.lines.iter().map(|l| l.height).sum();
                        if whole_height > remaining_height {
                            overflow = Some(node);
                            break;
                        }
                    }

                    let count = lines_fitting(full, next_line, remaining_height, !region_has_content);
                    if count == 0 {
                        overflow = Some(node);
                        break;
                    }

                    let end_line = next_line + count;
                    let placed_layout = slice_layout_lines(full, next_line, end_line);
                    let full_len = full.lines.len();
                    let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, placed_layout.height);
                    cursor_y += placed_layout.height;
                    blocks.push(PlacedBlock {
                        id: ids[block_idx],
                        rect,
                        kind: &node.kind,
                        paragraph_layout: Some(placed_layout),
                        table_placement: None,
                        list_placement: None,
                    });

                    if end_line >= full_len {
                        cursor_y += style.paragraph_spacing;
                        block_idx += 1;
                        progress = InProgress::None;
                        true
                    } else {
                        if let InProgress::Paragraph { next_line, .. } = &mut progress {
                            *next_line = end_line;
                        }
                        overflow = Some(node);
                        break;
                    }
                }
                Block::Figure(fb) => {
                    let content_height = fb.sizing.resolve_height(region.rect.width, remaining_height);
                    if content_height <= remaining_height || !region_has_content {
                        let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, content_height);
                        cursor_y += content_height;
                        blocks.push(PlacedBlock {
                            id: ids[block_idx],
                            rect,
                            kind: &node.kind,
                            paragraph_layout: None,
                            table_placement: None,
                            list_placement: None,
                        });
                        cursor_y += style.paragraph_spacing;
                        block_idx += 1;
                        progress = InProgress::None;
                        true
                    } else {
                        overflow = Some(node);
                        break;
                    }
                }
                Block::Image(ib) => {
                    let content_height = ib.sizing.resolve_height(region.rect.width, remaining_height);
                    if content_height <= remaining_height || !region_has_content {
                        let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, content_height);
                        cursor_y += content_height;
                        blocks.push(PlacedBlock {
                            id: ids[block_idx],
                            rect,
                            kind: &node.kind,
                            paragraph_layout: None,
                            table_placement: None,
                            list_placement: None,
                        });
                        cursor_y += style.paragraph_spacing;
                        block_idx += 1;
                        progress = InProgress::None;
                        true
                    } else {
                        overflow = Some(node);
                        break;
                    }
                }
                Block::Table(table) => {
                    if !matches!(progress, InProgress::Table { .. }) {
                        let (column_widths, rows) = measure_and_layout_table(table, region.rect.width, style, shaper);
                        progress = InProgress::Table { column_widths, rows, next_row: 0 };
                    }
                    let InProgress::Table { column_widths, rows, next_row } = &progress else {
                        unreachable!("just ensured Table progress")
                    };
                    let next_row = *next_row;

                    if rows.is_empty() {
                        block_idx += 1;
                        progress = InProgress::None;
                        continue;
                    }

                    if node.break_control == BreakControl::AvoidInside && next_row == 0 && region_has_content {
                        let whole_height = table_total_height(rows);
                        if whole_height > remaining_height {
                            overflow = Some(node);
                            break;
                        }
                    }

                    let count = rows_fitting(rows, next_row, remaining_height, !region_has_content);
                    if count == 0 {
                        overflow = Some(node);
                        break;
                    }

                    let (placed_rows, placed_height) =
                        place_table_rows(rows, next_row, count, column_widths, table.cell_padding, (region.rect.x, cursor_y));
                    let total_rows = rows.len();
                    let column_widths_snapshot = column_widths.clone();
                    let rect = Rect::new(region.rect.x, cursor_y, column_widths_snapshot.iter().sum(), placed_height);
                    cursor_y += placed_height;
                    blocks.push(PlacedBlock {
                        id: ids[block_idx],
                        rect,
                        kind: &node.kind,
                        paragraph_layout: None,
                        table_placement: Some(crate::region::TablePlacement { column_widths: column_widths_snapshot, rows: placed_rows }),
                        list_placement: None,
                    });

                    if next_row + count >= total_rows {
                        cursor_y += style.paragraph_spacing;
                        block_idx += 1;
                        progress = InProgress::None;
                        true
                    } else {
                        if let InProgress::Table { next_row, .. } = &mut progress {
                            *next_row += count;
                        }
                        overflow = Some(node);
                        break;
                    }
                }
                Block::List(list) => {
                    let origin = Rect::new(region.rect.x, cursor_y, region.rect.width, remaining_height);
                    let (items, total_height) = place_list(list, origin, style, shaper);

                    if total_height <= remaining_height || !region_has_content {
                        let rect = Rect::new(region.rect.x, cursor_y, region.rect.width, total_height);
                        cursor_y += total_height;
                        blocks.push(PlacedBlock {
                            id: ids[block_idx],
                            rect,
                            kind: &node.kind,
                            paragraph_layout: None,
                            table_placement: None,
                            list_placement: Some(crate::region::ListPlacement { items }),
                        });
                        cursor_y += style.paragraph_spacing;
                        block_idx += 1;
                        progress = InProgress::None;
                        true
                    } else {
                        overflow = Some(node);
                        break;
                    }
                }
            };

            if node_fully_placed && node.break_control == BreakControl::ForceAfter {
                break;
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

    /// A figure ([`crate::scene::BlockSizing::FixedHeight`]) taller than
    /// the remaining space in its current region must defer WHOLE to the
    /// next region — never split, never squashed to fit.
    #[test]
    fn a_figure_taller_than_the_remaining_region_lands_whole_on_the_next_region() {
        use crate::scene::{Block, BlockSizing, FigureBlock, TypesetFigure};
        use uzor::render::RenderContext;
        use uzor_figures::FigureTheme;

        struct StubFigure;
        impl TypesetFigure for StubFigure {
            fn render(&self, _ctx: &mut dyn RenderContext, _rect: Rect, _theme: &FigureTheme) {}
        }

        let shaper = CosmicShaper::headless();
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let style = ComposeStyle::new(0.0, font);

        const PAGE_HEIGHT: f64 = 300.0;
        const FILLER_GAP: f64 = 250.0; // leaves only 50px remaining
        const FIGURE_HEIGHT: f64 = 120.0; // taller than the 50px remaining

        let stub = StubFigure;
        let flow = [
            BlockNode::new(Block::Spacer(FILLER_GAP)),
            BlockNode::new(Block::Figure(FigureBlock::new(&stub, BlockSizing::FixedHeight(FIGURE_HEIGHT)))),
        ];

        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, 400.0, PAGE_HEIGHT));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 2, "the figure must land on a fresh second region, never split");
        assert_eq!(frames[0].blocks.len(), 1, "page 1 holds only the filler spacer");
        assert!(matches!(frames[0].blocks[0].kind, Block::Spacer(_)));

        assert_eq!(frames[1].blocks.len(), 1, "the figure is placed WHOLE on page 2");
        let figure_placement = &frames[1].blocks[0];
        assert!(matches!(figure_placement.kind, Block::Figure(_)));
        assert_eq!(figure_placement.rect.height, FIGURE_HEIGHT, "a deferred figure is never squashed to fit — full height preserved");
        assert_eq!(figure_placement.rect.y, 0.0, "the figure starts at the top of the fresh region");
    }

    /// `AvoidAfter` ("keep-with-next"): a heading paragraph immediately
    /// followed by a body paragraph must never land separated across a
    /// region boundary — either both fit in the current region, or BOTH
    /// defer to the next one.
    #[test]
    fn keep_with_next_never_separates_a_heading_from_its_following_block_across_a_region() {
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let heading_run = [StyledRun::new("Section Heading", font)];
        let body_text = "This body paragraph is long enough that it will not fit in a sliver of \
            remaining space left after a heading placed near the bottom of a short region, forcing \
            the keep-with-next rule to move the heading down onto the next region together with it.";
        let body_run = [StyledRun::new(body_text, font)];

        const REGION_HEIGHT: f64 = 220.0;
        const FILLER_GAP: f64 = 180.0; // leaves ~40px remaining for the heading alone

        let flow = [
            BlockNode::new(Block::Spacer(FILLER_GAP)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&heading_run, 300.0))).with_break_control(BreakControl::AvoidAfter),
            BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, 300.0))),
        ];

        let style = ComposeStyle::new(6.0, font);
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, 300.0, REGION_HEIGHT));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        // The heading must NOT be the last thing placed on page 1 while
        // the body starts page 2 — either neither is on page 1, or both
        // are (fully or the body's own natural first split, but never
        // the heading alone stranded at the bottom).
        let page1_has_heading = frames[0].blocks.iter().any(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == "Section Heading"));
        assert!(!page1_has_heading, "the heading must be pushed onto the next region together with its following block, not stranded alone");

        // The second region must carry BOTH the heading and (at least the
        // start of) the body paragraph.
        let page2 = frames.iter().find(|f| f.blocks.iter().any(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == "Section Heading")));
        let page2 = page2.expect("the heading must land on some later region");
        assert!(
            page2.blocks.len() >= 2 || page2.overflow.is_some(),
            "the heading's own region must also carry (or be about to carry) the following block"
        );
    }

    /// `ComposeStyle::from_theme` must resolve `default_font` through the
    /// theme's own `FontRole::Body`, not some independent hardcoded value
    /// (design doc §5: "paragraph default fonts ... resolve through the
    /// theme").
    #[test]
    fn compose_style_from_theme_resolves_default_font_through_the_theme() {
        let theme = crate::style::Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 8.0);
        assert_eq!(style.default_font, theme.font_spec(crate::style::FontRole::Body));
        assert_eq!(style.paragraph_spacing, 8.0);
    }

    /// A table taller than one region must split BETWEEN rows only — every
    /// row placed on either side of the split stays whole (never a
    /// partially-painted row), and every source row is conserved exactly
    /// once across the two regions.
    #[test]
    fn a_table_taller_than_one_region_splits_between_rows_never_mid_row() {
        use crate::scene::{ColumnSpec, TableBlock, TableCell, TableRow};

        let shaper = CosmicShaper::headless();
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let style = ComposeStyle::new(0.0, font);

        // 6 rows of short single-line cells — every row's height is the
        // SAME (one line), so the split point is exactly predictable.
        let cell_run = [StyledRun::new("cell", font)];
        let cell_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&cell_run, f64::MAX)))];
        let cells = [TableCell::new(&cell_nodes)];
        let rows = [
            TableRow::new(&cells),
            TableRow::new(&cells),
            TableRow::new(&cells),
            TableRow::new(&cells),
            TableRow::new(&cells),
            TableRow::new(&cells),
        ];
        let columns = [ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows);

        // Measure one row's real height up front so the fixture region can
        // be tuned to fit exactly 4 of the 6 rows.
        let (_, measured_rows) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        let row_height = measured_rows[0].height;
        assert!(measured_rows.iter().all(|r| (r.height - row_height).abs() < 1e-6), "fixture rows must be equal height");

        let region_height = row_height * 4.0 + 0.5; // room for exactly 4 whole rows

        let flow = [BlockNode::new(Block::Table(TableBlock::new(&columns, &rows)))];
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, 200.0, region_height));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 2, "the table must span exactly 2 regions");
        assert!(frames[0].overflow.is_some(), "the first frame must report the table as still overflowing");
        assert!(frames[1].overflow.is_none(), "the second frame must finish the table");

        let page1_table = frames[0].blocks[0].table_placement.as_ref().expect("table placement present");
        let page2_table = frames[1].blocks[0].table_placement.as_ref().expect("table placement present");

        assert_eq!(page1_table.rows.len(), 4, "page 1 must hold exactly the 4 rows that fit whole");
        assert_eq!(page2_table.rows.len(), 2, "page 2 must hold the remaining 2 rows");
        assert_eq!(page1_table.rows.len() + page2_table.rows.len(), rows.len(), "every source row must be conserved exactly once");

        // Every placed row on both pages stays fully within its own
        // region's height budget — never a partially-painted row.
        for placed_row in page1_table.rows.iter().chain(page2_table.rows.iter()) {
            assert!((placed_row.rect.height - row_height).abs() < 1e-6, "every placed row keeps its own full, whole height — never split mid-row");
        }
    }
}
