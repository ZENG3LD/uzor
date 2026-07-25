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
//!
//! ## Anchored islands — text running beside an image (design brief,
//! adapted from §2.3)
//!
//! [`crate::scene::Block::Island`] reserves its own
//! [`crate::scene::AnchoredIsland`] image rect at the CURRENT flow cursor
//! (left/right/centered per its own [`crate::scene::IslandAnchor`]), then
//! hands the flow blocks immediately following it a NARROWED side-strip
//! rect (or two, left-then-right, for `IslandAnchor::Center`) for the
//! island's own vertical band, before resuming full region width below it.
//! Implemented as [`place_into_region`] calling ITSELF once per strip rect
//! — "a compose-level transform, an island-aware wrapper around the
//! CURRENT region" (this feature's own brief), never a new global
//! [`crate::region::RegionSequence`] impl: a strip is still part of the
//! SAME page/column region the island itself sits in (design doc's own
//! "one frame per region consumed" still holds one level up from here —
//! only [`crate::region::ColumnRegionSequence`]/[`crate::region::
//! PageRegionSequence::with_columns`] pull genuinely NEW regions).
//! Islands are ATOMIC exactly like [`crate::scene::Block::Figure`]/
//! [`crate::scene::Block::Image`] — one taller than the remaining region
//! space defers WHOLE to the next region, never split.
//!
//! Because [`place_into_region`]'s own recursive strip calls share the
//! SAME `block_idx`/`progress` state the outer call is threading, a
//! `Paragraph`/`Table` block that starts inside a strip and doesn't fully
//! fit there CONTINUES exactly as it would across any ordinary region
//! boundary (`InProgress`'s own cross-region cache, §3.3) — free for
//! `IslandAnchor::Center`'s own left-then-right strips (equal width, so no
//! re-wrap is ever needed), but ALSO reachable when the LAST available
//! strip runs out and placement resumes at the region's own (wider, or
//! narrower) full width below the island — an accepted, ALREADY-documented
//! divergence (§3.3/this crate's own P0 divergence 7: a block's
//! continuation is never re-wrapped at a later region's different width),
//! now also reachable via an island's strip-to-full-width transition, not
//! a new gap this feature invents.

use uzor::types::Rect;
use uzor_text::{layout_paragraph, FontSpec, LineShaper, Paragraph, ParagraphLayout};

use super::baseline_grid::{start_delta, trailing_extra};
use super::island_layout::{island_placement_rect, island_strip_rects};
use super::keep_break::BreakControl;
use super::list_layout::{items_fitting, list_total_height, measure_list_items, place_list_items, ComposedListItem};
use super::table_layout::{header_group_height, measure_and_layout_table, place_table_rows, rows_fitting, table_total_height, ComposedRow};
use super::{lines_fitting, slice_layout_lines, widow_orphan_count};
use crate::region::{Frame, PlacedBlock, RegionSequence};
use crate::scene::{resolve_block_ids, Block, BlockId, BlockNode};

/// Widow/orphan control's own default minimum line count (both halves) —
/// the task's own chosen v1 default; `0` disables that half entirely (see
/// [`ComposeStyle::with_min_orphan_lines`]/[`ComposeStyle::with_min_widow_lines`]).
const DEFAULT_MIN_WIDOW_ORPHAN_LINES: usize = 2;

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
    /// Minimum lines a paragraph split may leave behind in the CURRENT
    /// region (never fewer, or the whole paragraph defers to the next
    /// region instead) — the lua-widow-control-inspired orphan half of
    /// widow/orphan control (typography quality wave). `0` disables.
    /// Defaults to `2` via [`ComposeStyle::new`]/[`ComposeStyle::from_theme`]
    /// — override via [`ComposeStyle::with_min_orphan_lines`].
    pub min_orphan_lines: usize,
    /// Minimum lines a paragraph split may carry into the CONTINUATION
    /// region (never fewer — a shorter continuation instead pulls a line
    /// back from the head, or defers the whole paragraph if that's not
    /// possible either) — the widow half. `0` disables. Same default/
    /// override shape as [`ComposeStyle::min_orphan_lines`].
    pub min_widow_lines: usize,
    /// Typography track T2 — baseline grid / vertical rhythm: the
    /// document/page-level grid pitch (typically the body leading) every
    /// FRESH flow block's own grid-relevant edge snaps forward to.
    /// `None` (the default via [`ComposeStyle::new`]/[`ComposeStyle::
    /// from_theme`]) is fully OFF — every existing document renders
    /// byte-identically, since every grid code path in `compose::flow` is
    /// gated behind `Some(pitch)`. See `compose::baseline_grid`'s own
    /// module doc comment for the exact snapping model (what snaps, how
    /// `paragraph_spacing` interacts, how a block that genuinely cannot
    /// fit the grid degrades). Set via [`ComposeStyle::with_baseline_grid`].
    pub baseline_grid: Option<f64>,
}

impl ComposeStyle {
    pub fn new(paragraph_spacing: f64, default_font: FontSpec) -> Self {
        Self {
            paragraph_spacing,
            default_font,
            min_orphan_lines: DEFAULT_MIN_WIDOW_ORPHAN_LINES,
            min_widow_lines: DEFAULT_MIN_WIDOW_ORPHAN_LINES,
            baseline_grid: None,
        }
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
        Self {
            paragraph_spacing,
            default_font: theme.font_spec(crate::style::FontRole::Body),
            min_orphan_lines: DEFAULT_MIN_WIDOW_ORPHAN_LINES,
            min_widow_lines: DEFAULT_MIN_WIDOW_ORPHAN_LINES,
            baseline_grid: None,
        }
    }

    /// Builder: override the default orphan-control minimum (`0` disables).
    pub fn with_min_orphan_lines(mut self, min_orphan_lines: usize) -> Self {
        self.min_orphan_lines = min_orphan_lines;
        self
    }

    /// Builder: override the default widow-control minimum (`0` disables).
    pub fn with_min_widow_lines(mut self, min_widow_lines: usize) -> Self {
        self.min_widow_lines = min_widow_lines;
        self
    }

    /// Builder: opt into the baseline grid (typography track T2) at
    /// `pitch` — typically the body leading. `pitch <= 0.0` is treated as
    /// disabling the grid (`None`), the same defensive floor every other
    /// degenerate-input guard in this crate already uses, rather than a
    /// fallible surface (a zero/negative pitch would otherwise divide by
    /// zero or loop forever inside `compose::baseline_grid`'s own
    /// rounding math).
    pub fn with_baseline_grid(mut self, pitch: f64) -> Self {
        self.baseline_grid = if pitch > 0.0 { Some(pitch) } else { None };
        self
    }
}

/// In-progress split state for whichever flow block `compose`'s loop is
/// CURRENTLY placing (persists only across the SAME block's own
/// continuation into a later region — reset to `None` the moment that
/// block is fully placed or a different block starts). Genuinely atomic
/// block kinds (`Figure`/`Image`/`Island`) never need an entry here — an
/// atomic block is either placed whole or deferred whole, nothing to
/// resume. `List` gained its own splittable variant this pass (typography
/// quality wave) — item-atomic, but the list AS A WHOLE now splits BETWEEN
/// items across regions exactly like `Table` already splits BETWEEN rows.
enum InProgress<'a> {
    None,
    Paragraph { layout: ParagraphLayout, next_line: usize },
    Table { column_widths: Vec<f64>, rows: Vec<ComposedRow<'a>>, next_row: usize },
    List { items: Vec<ComposedListItem<'a>>, next_item: usize },
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
        // The lookahead only cares about the island's OWN reserved band
        // height (its width is a caller choice, independent of
        // `region_width`) — the side-strip content beside it is a
        // separate concern real placement handles, never this lookahead.
        Block::Island(island) => island.image.sizing.resolve_height(island.width, remaining_height),
        Block::Table(table) => {
            let (_, rows) = measure_and_layout_table(table, region_width, style, shaper);
            table_total_height(&rows)
        }
        Block::List(list) => {
            let content_width = (region_width - list.indent_px).max(0.0);
            let items = measure_list_items(list, content_width, style, shaper);
            list_total_height(&items, style.paragraph_spacing)
        }
    }
}

/// Would `next` get to place AT LEAST SOME content within
/// `remaining_height` — the `AvoidAfter`/keep-with-next lookahead's own
/// "is there a point in keeping these two together" test. A genuinely
/// atomic block (`Figure`/`Image`/`Island`) only ever gets "some" room by
/// fitting ENTIRELY (there's no partial placement for one); a `Paragraph`
/// gets "some" room the moment even one line fits; a `Table`/`List` (both
/// splittable at row/item granularity) get "some" room the moment their
/// own first row/item fits.
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
        Block::Island(island) => island.image.sizing.resolve_height(island.width, remaining_height) <= remaining_height,
        Block::Table(table) => {
            let (_, rows) = measure_and_layout_table(table, region_width, style, shaper);
            rows.first().map_or(true, |r| r.height <= remaining_height)
        }
        Block::List(list) => {
            let content_width = (region_width - list.indent_px).max(0.0);
            let items = measure_list_items(list, content_width, style, shaper);
            items.first().map_or(true, |it| it.content_height <= remaining_height)
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
        let (blocks, overflow) = place_into_region(flow, &ids, &mut block_idx, &mut progress, region.rect, style, shaper);
        frames.push(Frame { region, blocks, overflow });
    }

    frames
}

/// Fills `region_rect` with as much of `flow` (starting at `*block_idx`,
/// carrying `*progress`'s own in-flight continuation state) as fits — the
/// exact body of [`compose`]'s own per-region loop, factored out so an
/// encountered [`Block::Island`] can recurse into THIS SAME function for
/// its own side-strip band (this module's own "Anchored islands" doc
/// comment above) without duplicating this match statement. Returns this
/// region's own placed blocks + the block that overflowed it (if any) —
/// [`compose`] wraps this into one [`Frame`] per REAL region pulled from
/// its own [`RegionSequence`]; a recursive strip call instead merges its
/// own returned blocks directly into the CALLER's own blocks list (a strip
/// is not a separate [`Frame`] — it's still the SAME page/region, just
/// narrower for a while).
#[allow(clippy::too_many_arguments)]
fn place_into_region<'a>(
    flow: &'a [BlockNode<'a>],
    ids: &[BlockId],
    block_idx: &mut usize,
    progress: &mut InProgress<'a>,
    region_rect: Rect,
    style: &ComposeStyle,
    shaper: &dyn LineShaper,
) -> (Vec<PlacedBlock<'a>>, Option<&'a BlockNode<'a>>) {
    let region_bottom = region_rect.y + region_rect.height;
    let mut cursor_y = region_rect.y;
    let mut blocks: Vec<PlacedBlock<'a>> = Vec::new();
    let mut overflow = None;

    while *block_idx < flow.len() {
        let node = &flow[*block_idx];

        // Baseline grid (typography track T2): a FRESH (non-continuation)
        // block's own grid-relevant edge is snapped forward BEFORE any
        // fit/lookahead decision below ever reads `cursor_y` — every
        // downstream check (`ForceBefore`/`AvoidAfter`/`Island`/the
        // per-kind match) must see the POST-snap cursor, never a stale
        // pre-snap one.
        //
        // "Fresh" means "no content of THIS block has been placed in an
        // earlier region yet" — NOT simply `*progress == InProgress::None`:
        // the Paragraph/Table/List arms below all populate `*progress`
        // BEFORE their own "does anything fit at all" check runs, so a
        // block that gets deferred WHOLE (zero lines/rows/items fit,
        // `overflow = Some(node)`) already carries a `next_line`/
        // `next_row`/`next_item == 0` progress value by the time this
        // SAME node is reconsidered fresh on the NEXT region — genuinely
        // atomic kinds (`Spacer`/`Figure`/`Image`/`Island`) never touch
        // `*progress` ahead of their own fit check, so `InProgress::None`
        // alone is exactly right for them. `Block::Spacer` is
        // deliberately excluded either way (an explicit gap is never
        // grid-snapped). See `compose::baseline_grid`'s own module doc
        // for the full snapping model.
        if let Some(pitch) = style.baseline_grid {
            let is_fresh = match &*progress {
                InProgress::None => true,
                InProgress::Paragraph { next_line, .. } => *next_line == 0,
                InProgress::Table { next_row, .. } => *next_row == 0,
                InProgress::List { next_item, .. } => *next_item == 0,
            };
            if is_fresh && !matches!(node.kind, Block::Spacer(_)) {
                let offset = match (&node.kind, &*progress) {
                    // Already measured by an earlier (deferred-whole)
                    // attempt at a previous region — reuse it verbatim
                    // (design law 1: one measure path) rather than
                    // re-running `layout_paragraph` a second time.
                    (Block::Paragraph(_), InProgress::Paragraph { layout, .. }) => layout.lines.first().map_or(0.0, |l| l.baseline_y),
                    (Block::Paragraph(p), _) => {
                        let full = layout_paragraph(&Paragraph { max_width: region_rect.width, ..*p }, shaper);
                        full.lines.first().map_or(0.0, |l| l.baseline_y)
                    }
                    _ => 0.0,
                };
                cursor_y += start_delta(cursor_y, region_rect.y, pitch, offset);
            }
        }

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
        if node.break_control == BreakControl::AvoidAfter && region_has_content && *block_idx + 1 < flow.len() {
            let trial_height = full_height_if_whole(&node.kind, region_rect.width, remaining_height, style, shaper);
            if trial_height <= remaining_height {
                let remaining_after = (remaining_height - trial_height - style.paragraph_spacing).max(0.0);
                let next_fits = has_room_for_next(&flow[*block_idx + 1].kind, region_rect.width, remaining_after, style, shaper);
                if !next_fits {
                    overflow = Some(node);
                    break;
                }
            }
        }

        // Islands need their OWN control flow — they recurse into THIS
        // SAME function for their own side strip(s) before this loop's
        // next iteration, which doesn't fit the "was this node fully
        // placed" bool the match below produces for every other block
        // kind. Handled BEFORE that match (see this module's own
        // "Anchored islands" doc comment for the full mechanics).
        if let Block::Island(island) = &node.kind {
            let island_height = island.image.sizing.resolve_height(island.width, remaining_height);
            if island_height <= remaining_height || !region_has_content {
                let island_rect = island_placement_rect(island, region_rect, cursor_y, island_height);
                blocks.push(PlacedBlock {
                    id: ids[*block_idx],
                    rect: island_rect,
                    kind: &node.kind,
                    paragraph_layout: None,
                    table_placement: None,
                    list_placement: None,
                });
                let band_bottom = cursor_y + island_height;
                let force_after = node.break_control == BreakControl::ForceAfter;
                *block_idx += 1;
                *progress = InProgress::None;

                for strip in island_strip_rects(island, region_rect, island_rect, band_bottom) {
                    if *block_idx >= flow.len() {
                        break;
                    }
                    let (strip_blocks, _strip_overflow) = place_into_region(flow, ids, block_idx, progress, strip, style, shaper);
                    blocks.extend(strip_blocks);
                    // Whether or not THIS strip's own content overflowed,
                    // move on to the NEXT strip (if any) — an overflowing
                    // strip only means "this strip is full," never "stop
                    // trying further strips" (Center anchor's own
                    // left-then-right fill order).
                }

                // Baseline grid (T2): the island's own placed rect keeps
                // its NATURAL `island_height` (never stretched) — only
                // the RESUME position (where full-width content picks up
                // below the island's own band) rounds up to the next
                // grid line, exactly like every other non-text block's
                // own trailing-height rounding below. Strip geometry
                // above (already computed from the UNROUNDED
                // `band_bottom`) is unaffected — a strip only ever hugs
                // the island's own real band, never a decorative padding
                // zone.
                let resume_y = match style.baseline_grid {
                    Some(pitch) => band_bottom + trailing_extra(region_rect.y, region_bottom, band_bottom, pitch),
                    None => band_bottom,
                };
                cursor_y = resume_y + style.paragraph_spacing;
                if force_after {
                    break;
                }
                continue;
            } else {
                overflow = Some(node);
                break;
            }
        }

        // `true` when `node` was FULLY placed this iteration (never a
        // pre-declared default — every arm below either diverges via
        // `break`/`continue` or ends in a real `bool`, so there is no
        // dead initial value to satisfy the unused-assignments lint).
        let node_fully_placed = match &node.kind {
            Block::Spacer(gap) => {
                if *gap <= remaining_height || !region_has_content {
                    let rect = Rect::new(region_rect.x, cursor_y, region_rect.width, *gap);
                    blocks.push(PlacedBlock {
                        id: ids[*block_idx],
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
                    *block_idx += 1;
                    *progress = InProgress::None;
                    true
                } else {
                    overflow = Some(node);
                    break;
                }
            }
            Block::Paragraph(paragraph) => {
                if !matches!(*progress, InProgress::Paragraph { .. }) {
                    let measured = layout_paragraph(&Paragraph { max_width: region_rect.width, ..*paragraph }, shaper);
                    *progress = InProgress::Paragraph { layout: measured, next_line: 0 };
                }
                let InProgress::Paragraph { layout: full, next_line } = &*progress else {
                    unreachable!("just ensured Paragraph progress")
                };
                let next_line = *next_line;

                if full.lines.is_empty() {
                    *block_idx += 1;
                    *progress = InProgress::None;
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

                // Widow/orphan control (typography quality wave): may
                // narrow `count` (pull a line back from the head to avoid
                // stranding a too-short continuation) or defer the WHOLE
                // remaining paragraph (an orphan-violating head, or a
                // widow that can't be fixed without violating the orphan
                // floor) — see `compose::paragraph_split::widow_orphan_count`'s
                // own doc for the full decision ladder.
                let count = match widow_orphan_count(full.lines.len(), next_line, count, style.min_orphan_lines, style.min_widow_lines, region_has_content) {
                    Some(n) => n,
                    None => {
                        overflow = Some(node);
                        break;
                    }
                };

                let end_line = next_line + count;
                let placed_layout = slice_layout_lines(full, next_line, end_line);
                let full_len = full.lines.len();
                let rect = Rect::new(region_rect.x, cursor_y, region_rect.width, placed_layout.height);
                cursor_y += placed_layout.height;
                blocks.push(PlacedBlock {
                    id: ids[*block_idx],
                    rect,
                    kind: &node.kind,
                    paragraph_layout: Some(placed_layout),
                    table_placement: None,
                    list_placement: None,
                });

                if end_line >= full_len {
                    cursor_y += style.paragraph_spacing;
                    *block_idx += 1;
                    *progress = InProgress::None;
                    true
                } else {
                    if let InProgress::Paragraph { next_line, .. } = &mut *progress {
                        *next_line = end_line;
                    }
                    overflow = Some(node);
                    break;
                }
            }
            Block::Figure(fb) => {
                let content_height = fb.sizing.resolve_height(region_rect.width, remaining_height);
                if content_height <= remaining_height || !region_has_content {
                    let rect = Rect::new(region_rect.x, cursor_y, region_rect.width, content_height);
                    cursor_y += content_height;
                    blocks.push(PlacedBlock {
                        id: ids[*block_idx],
                        rect,
                        kind: &node.kind,
                        paragraph_layout: None,
                        table_placement: None,
                        list_placement: None,
                    });
                    // Baseline grid (T2): a non-text block never stretches
                    // its own placed `rect` — only the CURSOR's own
                    // trailing gap grows, rounding this block's own
                    // bottom edge up to the next grid line so whatever
                    // follows in this SAME frame starts back on grid.
                    if let Some(pitch) = style.baseline_grid {
                        cursor_y += trailing_extra(region_rect.y, region_bottom, cursor_y, pitch);
                    }
                    cursor_y += style.paragraph_spacing;
                    *block_idx += 1;
                    *progress = InProgress::None;
                    true
                } else {
                    overflow = Some(node);
                    break;
                }
            }
            Block::Image(ib) => {
                let content_height = ib.sizing.resolve_height(region_rect.width, remaining_height);
                if content_height <= remaining_height || !region_has_content {
                    let rect = Rect::new(region_rect.x, cursor_y, region_rect.width, content_height);
                    cursor_y += content_height;
                    blocks.push(PlacedBlock {
                        id: ids[*block_idx],
                        rect,
                        kind: &node.kind,
                        paragraph_layout: None,
                        table_placement: None,
                        list_placement: None,
                    });
                    // Baseline grid (T2): a non-text block never stretches
                    // its own placed `rect` — only the CURSOR's own
                    // trailing gap grows, rounding this block's own
                    // bottom edge up to the next grid line so whatever
                    // follows in this SAME frame starts back on grid.
                    if let Some(pitch) = style.baseline_grid {
                        cursor_y += trailing_extra(region_rect.y, region_bottom, cursor_y, pitch);
                    }
                    cursor_y += style.paragraph_spacing;
                    *block_idx += 1;
                    *progress = InProgress::None;
                    true
                } else {
                    overflow = Some(node);
                    break;
                }
            }
            // Handled above, before this match — never reached.
            Block::Island(_) => unreachable!("Block::Island is handled before this match, via its own recursive strip-fill branch"),
            Block::Table(table) => {
                if !matches!(*progress, InProgress::Table { .. }) {
                    let (column_widths, rows) = measure_and_layout_table(table, region_rect.width, style, shaper);
                    *progress = InProgress::Table { column_widths, rows, next_row: 0 };
                }
                let InProgress::Table { column_widths, rows, next_row } = &*progress else {
                    unreachable!("just ensured Table progress")
                };
                let next_row = *next_row;

                if rows.is_empty() {
                    *block_idx += 1;
                    *progress = InProgress::None;
                    continue;
                }

                if node.break_control == BreakControl::AvoidInside && next_row == 0 && region_has_content {
                    let whole_height = table_total_height(rows);
                    if whole_height > remaining_height {
                        overflow = Some(node);
                        break;
                    }
                }

                // Header-row repeat (typography quality wave): a
                // continuation fragment (`next_row > 0`) reserves room for
                // a re-placed `rows[0]` at its own top when
                // `table.header_repeat` is on — the table's own FIRST
                // fragment never reserves (it already starts with the
                // real header row).
                let header_reserved = if table.header_repeat && next_row > 0 { header_group_height(rows) } else { 0.0 };
                let count = rows_fitting(rows, next_row, remaining_height, !region_has_content, header_reserved);
                if count == 0 {
                    overflow = Some(node);
                    break;
                }

                let (placed_rows, placed_height) = place_table_rows(
                    rows,
                    next_row,
                    count,
                    column_widths,
                    table.cell_padding,
                    (region_rect.x, cursor_y),
                    table.header_repeat,
                );
                let total_rows = rows.len();
                let column_widths_snapshot = column_widths.clone();
                let rect = Rect::new(region_rect.x, cursor_y, column_widths_snapshot.iter().sum(), placed_height);
                cursor_y += placed_height;
                blocks.push(PlacedBlock {
                    id: ids[*block_idx],
                    rect,
                    kind: &node.kind,
                    paragraph_layout: None,
                    table_placement: Some(crate::region::TablePlacement { column_widths: column_widths_snapshot, rows: placed_rows }),
                    list_placement: None,
                });

                if next_row + count >= total_rows {
                    // Baseline grid (T2): only when the table finishes
                    // IN THIS fragment — a continuation's own trailing
                    // gap is irrelevant (nothing else places in this
                    // region afterward; the region simply ends).
                    if let Some(pitch) = style.baseline_grid {
                        cursor_y += trailing_extra(region_rect.y, region_bottom, cursor_y, pitch);
                    }
                    cursor_y += style.paragraph_spacing;
                    *block_idx += 1;
                    *progress = InProgress::None;
                    true
                } else {
                    if let InProgress::Table { next_row, .. } = &mut *progress {
                        *next_row += count;
                    }
                    overflow = Some(node);
                    break;
                }
            }
            Block::List(list) => {
                if !matches!(*progress, InProgress::List { .. }) {
                    let content_width = (region_rect.width - list.indent_px).max(0.0);
                    let items = measure_list_items(list, content_width, style, shaper);
                    *progress = InProgress::List { items, next_item: 0 };
                }
                let InProgress::List { items, next_item } = &*progress else {
                    unreachable!("just ensured List progress")
                };
                let next_item = *next_item;

                if items.is_empty() {
                    *block_idx += 1;
                    *progress = InProgress::None;
                    continue;
                }

                if node.break_control == BreakControl::AvoidInside && next_item == 0 && region_has_content {
                    let whole_height = list_total_height(items, style.paragraph_spacing);
                    if whole_height > remaining_height {
                        overflow = Some(node);
                        break;
                    }
                }

                let count = items_fitting(items, next_item, remaining_height, style.paragraph_spacing, !region_has_content);
                if count == 0 {
                    overflow = Some(node);
                    break;
                }

                let (placed_items, placed_height) =
                    place_list_items(items, next_item, count, list.indent_px, style.paragraph_spacing, (region_rect.x, cursor_y));
                let total_items = items.len();
                let rect = Rect::new(region_rect.x, cursor_y, region_rect.width, placed_height);
                cursor_y += placed_height;
                blocks.push(PlacedBlock {
                    id: ids[*block_idx],
                    rect,
                    kind: &node.kind,
                    paragraph_layout: None,
                    table_placement: None,
                    list_placement: Some(crate::region::ListPlacement { items: placed_items }),
                });

                if next_item + count >= total_items {
                    // Baseline grid (T2): same "only round when this
                    // fragment finishes the block" convention as Table's
                    // own arm above.
                    if let Some(pitch) = style.baseline_grid {
                        cursor_y += trailing_extra(region_rect.y, region_bottom, cursor_y, pitch);
                    }
                    cursor_y += style.paragraph_spacing;
                    *block_idx += 1;
                    *progress = InProgress::None;
                    true
                } else {
                    if let InProgress::List { next_item, .. } = &mut *progress {
                        *next_item += count;
                    }
                    overflow = Some(node);
                    break;
                }
            }
        };

        if node_fully_placed && node.break_control == BreakControl::ForceAfter {
            break;
        }
    }

    (blocks, overflow)
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

    /// Widow/orphan control (typography quality wave): a filler spacer
    /// leaves only ~1 line of room for a paragraph that starts a fresh
    /// region's remaining space — under the DEFAULT `min_orphan_lines = 2`,
    /// that 1-line head would be an orphan, so the WHOLE paragraph must
    /// defer to the next (fresh) region instead of splitting.
    #[test]
    fn widow_orphan_control_defers_the_whole_paragraph_when_the_head_would_be_a_1_line_orphan() {
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";
        let runs = [StyledRun::new(text, font)];
        let narrow_width = 140.0;

        let full = layout_paragraph(&Paragraph::new(&runs, narrow_width), &shaper);
        assert!(full.lines.len() >= 6, "fixture must wrap to several lines");
        let one_line = full.lines[0].height;

        // A page tall enough to hold the WHOLE paragraph fresh, but a
        // filler spacer leaves just over one line's worth of room on page 1.
        let page_height = full.height + one_line * 3.0;
        let filler_gap = page_height - (one_line + 0.5);

        let flow = [
            BlockNode::new(Block::Spacer(filler_gap)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&runs, narrow_width))),
        ];
        let style = ComposeStyle::new(0.0, font);
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, narrow_width, page_height));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames[0].blocks.len(), 1, "page 1 holds only the filler spacer — the paragraph never starts here");
        assert!(
            frames[0].blocks.iter().all(|b| !matches!(b.kind, Block::Paragraph(_))),
            "an orphan-violating 1-line head must never be placed"
        );

        let page2 = &frames[1];
        let placed = page2.blocks.iter().find_map(|b| b.paragraph_layout.as_ref()).expect("the paragraph lands on the fresh next region");
        assert!(placed.lines.len() >= 2, "the deferred paragraph must start with at least min_orphan_lines on its fresh region");
    }

    /// Widow/orphan control: a region sized to naturally leave exactly ONE
    /// line for the continuation (a widow) must instead pull one more line
    /// back from the head under the DEFAULT `min_widow_lines = 2`, so the
    /// continuation carries 2 lines instead of 1.
    #[test]
    fn widow_orphan_control_pulls_a_line_over_when_the_continuation_would_be_a_1_line_widow() {
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";
        let runs = [StyledRun::new(text, font)];
        let narrow_width = 140.0;

        let full = layout_paragraph(&Paragraph::new(&runs, narrow_width), &shaper);
        assert!(full.lines.len() >= 6, "fixture must wrap to several lines");
        let one_line = full.lines[0].height;
        assert!(full.lines.iter().all(|l| (l.height - one_line).abs() < 1e-6), "fixture lines must be equal height for a predictable split point");

        // Just enough room for (len - 1) lines, not the whole paragraph —
        // the RAW budget (before widow/orphan control) would leave exactly
        // 1 line for the continuation.
        let region_height = one_line * (full.lines.len() - 1) as f64 + 0.5;

        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, narrow_width)))];
        let style = ComposeStyle::new(0.0, font); // default min_orphan/min_widow = 2
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, narrow_width, region_height));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 2, "the paragraph must still span exactly 2 regions");
        let head = frames[0].blocks[0].paragraph_layout.as_ref().expect("head placement carries a layout");
        let tail = frames[1].blocks[0].paragraph_layout.as_ref().expect("tail placement carries a layout");

        assert_eq!(tail.lines.len(), 2, "the widow-prone 1-line continuation must be widened to 2 lines by pulling a line back from the head");
        assert_eq!(head.lines.len(), full.lines.len() - 2, "the head shrinks by exactly the 1 line pulled over");
        assert_eq!(head.lines.len() + tail.lines.len(), full.lines.len(), "every line must still be conserved across the split");
    }

    /// `min_orphan_lines = 0` / `min_widow_lines = 0` must reproduce the
    /// OLD (pre-widow/orphan-control) split point byte-identically — the
    /// exact same fixture/region shape as the widow test above, but with
    /// both controls disabled: the continuation stays at its RAW 1-line
    /// budget, never pulled over.
    #[test]
    fn widow_orphan_control_disabled_reproduces_the_old_split_point_byte_identically() {
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";
        let runs = [StyledRun::new(text, font)];
        let narrow_width = 140.0;

        let full = layout_paragraph(&Paragraph::new(&runs, narrow_width), &shaper);
        let one_line = full.lines[0].height;
        let region_height = one_line * (full.lines.len() - 1) as f64 + 0.5;

        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, narrow_width)))];
        let style = ComposeStyle::new(0.0, font).with_min_orphan_lines(0).with_min_widow_lines(0);
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, narrow_width, region_height));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 2);
        let head = frames[0].blocks[0].paragraph_layout.as_ref().expect("head placement carries a layout");
        let tail = frames[1].blocks[0].paragraph_layout.as_ref().expect("tail placement carries a layout");

        assert_eq!(head.lines.len(), full.lines.len() - 1, "disabled controls must reproduce the raw lines_fitting budget exactly");
        assert_eq!(tail.lines.len(), 1, "disabled controls must leave the raw 1-line widow untouched");
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

    /// Typography track T2 (baseline grid): a `Block::Figure` with a
    /// deliberately ODD (non-grid-multiple) height rounds its own
    /// CONSUMED height up to the next whole grid step — so the paragraph
    /// immediately following it still lands on the shared grid — isolated
    /// to ONE region/column, independent of `slice::pages`'s own larger
    /// 2-column integration proof.
    #[test]
    fn baseline_grid_rounds_a_figures_trailing_height_so_the_following_paragraph_lands_on_grid() {
        use crate::scene::{BlockSizing, FigureBlock, TypesetFigure};
        use uzor::render::RenderContext;
        use uzor_figures::FigureTheme;

        struct StubFigure;
        impl TypesetFigure for StubFigure {
            fn render(&self, _ctx: &mut dyn RenderContext, _rect: Rect, _theme: &FigureTheme) {}
        }

        const PITCH: f64 = 20.0;
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 14.0);
        let body_run = [StyledRun::new("After the figure.", font)];
        let stub = StubFigure;

        let flow = [
            BlockNode::new(Block::Figure(FigureBlock::new(&stub, BlockSizing::FixedHeight(97.0)))), // deliberately not a multiple of PITCH
            BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, 300.0))),
        ];
        let shaper = CosmicShaper::headless();

        let style_off = ComposeStyle::new(6.0, font);
        let mut regions_off = PageRegionSequence::new(Rect::new(0.0, 0.0, 300.0, 500.0));
        let frames_off = compose(&flow, &mut regions_off, &style_off, &shaper);
        let body_off = frames_off[0].blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(_))).expect("body paragraph placed");
        let baseline_off = body_off.rect.y + body_off.paragraph_layout.as_ref().expect("paragraph layout present").lines[0].baseline_y;
        let remainder_off = baseline_off.rem_euclid(PITCH);
        assert!(remainder_off.min(PITCH - remainder_off) > 1.0, "the UN-gridded baseline must not already coincidentally land on the grid, or the ON case below would prove nothing");

        let style_on = ComposeStyle::new(6.0, font).with_baseline_grid(PITCH);
        let mut regions_on = PageRegionSequence::new(Rect::new(0.0, 0.0, 300.0, 500.0));
        let frames_on = compose(&flow, &mut regions_on, &style_on, &shaper);
        let body_on = frames_on[0].blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(_))).expect("body paragraph placed");
        let baseline_on = body_on.rect.y + body_on.paragraph_layout.as_ref().expect("paragraph layout present").lines[0].baseline_y;
        let remainder_on = baseline_on.rem_euclid(PITCH);
        assert!(remainder_on.min(PITCH - remainder_on) < 1e-3, "the paragraph after the odd-height figure must land on grid, got baseline {baseline_on} (remainder {remainder_on})");

        // The figure's own placed rect keeps its NATURAL height — grid
        // rounding never stretches the figure's own box, only the
        // trailing cursor gap after it.
        let figure_on = frames_on[0].blocks.iter().find(|b| matches!(b.kind, Block::Figure(_))).expect("figure placed");
        assert_eq!(figure_on.rect.height, 97.0, "grid rounding must never stretch a figure's own placed rect — only the cursor advances further");
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

    fn stub_island<'a>(rgba: &'a [u8], anchor: crate::scene::IslandAnchor, width: f64, margin: f64, height: f64) -> crate::scene::AnchoredIsland<'a> {
        use crate::scene::{BlockSizing, ImageBlock, ImageFit};
        crate::scene::AnchoredIsland::new(ImageBlock::new(rgba, 100, 100, BlockSizing::FixedHeight(height), ImageFit::Cover), anchor, width, margin)
    }

    /// An island ([`crate::scene::BlockSizing::FixedHeight`]) taller than
    /// the remaining space in its current region must defer WHOLE to the
    /// next region — same atomic degrade `Block::Figure`/`Block::Image`
    /// already prove.
    #[test]
    fn an_island_taller_than_the_remaining_region_lands_whole_on_the_next_region() {
        use crate::scene::IslandAnchor;

        let shaper = CosmicShaper::headless();
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 16.0);
        let style = ComposeStyle::new(0.0, font);

        const PAGE_HEIGHT: f64 = 300.0;
        const FILLER_GAP: f64 = 250.0; // leaves only 50px remaining
        const ISLAND_HEIGHT: f64 = 120.0; // taller than the 50px remaining

        let rgba = [0u8; 4];
        let island = stub_island(&rgba, IslandAnchor::Left, 150.0, 10.0, ISLAND_HEIGHT);
        let flow = [BlockNode::new(Block::Spacer(FILLER_GAP)), BlockNode::new(Block::Island(island))];

        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, 400.0, PAGE_HEIGHT));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 2, "the island must land on a fresh second region, never split");
        assert_eq!(frames[0].blocks.len(), 1, "page 1 holds only the filler spacer");
        assert_eq!(frames[1].blocks.len(), 1, "the island is placed WHOLE on page 2 (no strip content follows it here)");
        let island_placement = &frames[1].blocks[0];
        assert!(matches!(island_placement.kind, Block::Island(_)));
        assert_eq!(island_placement.rect.height, ISLAND_HEIGHT, "a deferred island is never squashed to fit — full height preserved");
        assert_eq!(island_placement.rect.y, 0.0, "the island starts at the top of the fresh region");
    }

    /// `IslandAnchor::Left` yields exactly ONE strip, to the RIGHT of the
    /// image; `IslandAnchor::Right` yields exactly ONE strip, to the LEFT.
    #[test]
    fn island_left_anchor_fills_a_single_right_strip_and_right_anchor_fills_a_single_left_strip() {
        use crate::scene::IslandAnchor;

        let shaper = CosmicShaper::headless();
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 14.0);
        let style = ComposeStyle::new(4.0, font);
        const REGION_W: f64 = 400.0;
        const ISLAND_W: f64 = 150.0;
        const MARGIN: f64 = 10.0;
        const ISLAND_H: f64 = 100.0;

        let rgba = [0u8; 4];

        // Left anchor: strip content must sit strictly to the RIGHT of
        // the island (never overlapping it), within the island's own
        // vertical band.
        let left_island = stub_island(&rgba, IslandAnchor::Left, ISLAND_W, MARGIN, ISLAND_H);
        let strip_run = [StyledRun::new("beside the left-anchored image", font)];
        let flow = [
            BlockNode::new(Block::Island(left_island)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&strip_run, REGION_W))),
        ];
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, REGION_W, 500.0));
        let frames = compose(&flow, &mut regions, &style, &shaper);
        assert_eq!(frames.len(), 1);
        let island_placement = frames[0].blocks.iter().find(|b| matches!(b.kind, Block::Island(_))).expect("island placed");
        let text_placement = frames[0].blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(_))).expect("strip text placed");
        assert!(text_placement.rect.x >= island_placement.rect.x + island_placement.rect.width, "Left anchor: text must sit at/right of the island's own right edge (plus margin)");
        assert!(
            (text_placement.rect.y - island_placement.rect.y).abs() < 1e-6,
            "strip content starts at the SAME y as the island's own top — beside it, not below it"
        );

        // Right anchor: strip content must sit strictly to the LEFT of
        // the island.
        let right_island = stub_island(&rgba, IslandAnchor::Right, ISLAND_W, MARGIN, ISLAND_H);
        let flow2 = [
            BlockNode::new(Block::Island(right_island)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&strip_run, REGION_W))),
        ];
        let mut regions2 = PageRegionSequence::new(Rect::new(0.0, 0.0, REGION_W, 500.0));
        let frames2 = compose(&flow2, &mut regions2, &style, &shaper);
        let island_placement2 = frames2[0].blocks.iter().find(|b| matches!(b.kind, Block::Island(_))).expect("island placed");
        let text_placement2 = frames2[0].blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(_))).expect("strip text placed");
        assert!(
            text_placement2.rect.x + text_placement2.rect.width <= island_placement2.rect.x + 0.01,
            "Right anchor: text must sit at/left of the island's own left edge (minus margin)"
        );
    }

    /// `IslandAnchor::Center`: side-strip widths must match this feature's
    /// own formula `(region_width - island.width - 2*margin) / 2`; text
    /// fills the LEFT strip first, then the RIGHT strip, never overlapping
    /// the island's own rect; once both strips are exhausted (an island
    /// band sized to hold exactly ONE short line per strip — measured,
    /// never guessed), the third paragraph resumes at FULL region width,
    /// starting at/below the island's own bottom edge.
    #[test]
    fn island_center_anchor_flows_text_down_left_strip_then_right_strip_then_resumes_full_width_below() {
        use crate::scene::IslandAnchor;

        let shaper = CosmicShaper::headless();
        let font = uzor_text::FontSpec::new(FontFamily::Roboto, 14.0);
        let style = ComposeStyle::new(4.0, font);
        const REGION_W: f64 = 500.0;
        const ISLAND_W: f64 = 200.0;
        const MARGIN: f64 = 10.0;
        let expected_side_width = (REGION_W - ISLAND_W - 2.0 * MARGIN) / 2.0;

        let left_run = [StyledRun::new("Left.", font)];
        let right_run = [StyledRun::new("Right.", font)];
        let below_run = [StyledRun::new("Below.", font)];

        // Measure a single short line's REAL height at the strip's own
        // width (never guessed), and size the island's own band to hold
        // exactly ONE such line per strip — a second flow paragraph is
        // therefore forced to move on (left -> right -> full width
        // below), proving the fill order as data.
        let left_layout = layout_paragraph(&Paragraph::new(&left_run, expected_side_width), &shaper);
        assert_eq!(left_layout.lines.len(), 1, "fixture text must stay on one line at the strip's own width");
        let island_height = left_layout.height + 2.0; // room for exactly one line, not two

        let rgba = [0u8; 4];
        let island = stub_island(&rgba, IslandAnchor::Center, ISLAND_W, MARGIN, island_height);

        let flow = [
            BlockNode::new(Block::Island(island)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&left_run, REGION_W))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&right_run, REGION_W))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&below_run, REGION_W))),
        ];

        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, REGION_W, 600.0));
        let frames = compose(&flow, &mut regions, &style, &shaper);
        assert_eq!(frames.len(), 1);
        let blocks = &frames[0].blocks;
        assert_eq!(blocks.len(), 4, "island + 3 paragraphs, all on ONE page");

        let island_placement = blocks.iter().find(|b| matches!(b.kind, Block::Island(_))).expect("island placed");
        let left_placed = blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == "Left.")).expect("left text placed");
        let right_placed = blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == "Right.")).expect("right text placed");
        let below_placed = blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == "Below.")).expect("below text placed");

        assert!((left_placed.rect.width - expected_side_width).abs() < 1e-6, "left-strip text must be measured at the left strip's own width");
        assert!(left_placed.rect.x + left_placed.rect.width <= island_placement.rect.x + 0.01, "left-strip text must never overlap the island rect");

        assert!((right_placed.rect.width - expected_side_width).abs() < 1e-6, "right-strip text must be measured at the right strip's own width");
        assert!(right_placed.rect.x >= island_placement.rect.x + island_placement.rect.width - 0.01, "right-strip text must never overlap the island rect");
        assert!(right_placed.rect.x > left_placed.rect.x, "the right strip must sit to the right of the left strip");

        assert!((below_placed.rect.width - REGION_W).abs() < 1e-6, "content after the island's own band must resume at FULL region width");
        assert!(
            below_placed.rect.y >= island_placement.rect.y + island_placement.rect.height - 0.01,
            "full-width content must resume at/below the island's own bottom edge"
        );
    }
}
