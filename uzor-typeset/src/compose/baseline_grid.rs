//! Typography track T2 — baseline grid / vertical rhythm. Opt-in
//! (`ComposeStyle::baseline_grid: Option<f64>`, default `None`): a
//! document/page-level grid pitch that pulls every FRESH flow block's own
//! grid-relevant edge forward to the nearest multiple of `pitch`, so
//! adjacent columns (and adjacent pages sharing the same
//! [`crate::master::PageMaster`]) end up sharing one shared, absolute
//! line grid — the exact "adjacent columns don't share a line grid" gap
//! this track closes.
//!
//! ## The snapping model (report, the design decisions this file makes)
//!
//! **What snaps.** A `Block::Paragraph`'s own FIRST LINE baseline snaps
//! to the grid ([`start_delta`]'s `Paragraph` arm — `pitch` is
//! "typically the body leading" per this track's own brief, so when a
//! paragraph's own `line_height` genuinely equals `pitch`, every
//! SUBSEQUENT line in that same paragraph lands on the grid too, for
//! free, without this module doing anything further — snapping only the
//! first line is sufficient in the common case this track targets).
//! **Headings get no special case at all**: this crate has no distinct
//! `Block::Heading` kind (a heading is authored as an ordinary
//! `Block::Paragraph`, just at a larger size/weight, optionally
//! `BreakControl::AvoidAfter`) — the SAME `Paragraph` arm below already
//! snaps a heading's own first (usually only) line, with zero extra code.
//! A heading's own taller natural leading before it still steps forward
//! to whatever grid line is next, exactly like any other paragraph.
//! **Non-text blocks** (`Figure`/`Image`/`Island`/`Table`/`List`) have no
//! baseline to speak of — they snap their own TOP edge to the grid
//! instead (`start_delta`'s non-`Paragraph` arm, offset `0.0`), and
//! separately round their own consumed HEIGHT up to a whole number of
//! grid steps ([`trailing_extra`], called from `compose::flow` right
//! where each such block's cursor is finalized) — so the block's own
//! BOTTOM edge, where the next block starts, lands back on the grid too.
//! This is this track's own literal, chosen answer to "what about
//! figures/tables/list items" (a table/list is treated as ONE opaque
//! block for grid purposes — its own internal cell/item content is never
//! individually re-snapped; only its outer box's top/bottom edges are).
//! **`Block::Spacer` is deliberately EXEMPT from both** (never
//! start-snapped, never height-rounded) — an explicit `Spacer` already IS
//! the author's own deliberate gap (this crate's own pre-existing P0
//! divergence #8), and grid mode must never silently inflate it. This
//! doesn't break the grid: whatever REAL block follows the spacer (a
//! `Paragraph`) still self-snaps regardless of what came before it, so
//! the grid is never actually broken by an unsnapped spacer — only
//! locally, harmlessly loosened by however much of `pitch` remains.
//!
//! **How auto spacing interacts.** [`crate::compose::ComposeStyle::
//! paragraph_spacing`] is completely UNCHANGED by this track — grid
//! rounding is applied to a block's own NATURAL bottom edge, strictly
//! BEFORE `paragraph_spacing` is added on top (see every non-`Spacer`,
//! non-`Paragraph` arm in `compose::flow::place_into_region`). This
//! keeps `paragraph_spacing`'s own meaning identical whether the grid is
//! on or off; whatever the grid ROUNDING plus the ORDINARY spacing
//! together leave slightly off-grid is simply absorbed by the next fresh
//! block's own start-snap (paragraphs) — never left permanently
//! accumulating drift, since every fresh paragraph re-derives its own
//! snap from wherever the cursor genuinely is, not from where it "should"
//! be.
//!
//! **Degrading sanely when a block genuinely cannot fit the grid.**
//! [`trailing_extra`] NEVER pushes a block's own rounded bottom edge past
//! `region_bottom` — a region's own fixed physical size is never
//! stretched to satisfy the grid. When rounding up would overflow (an
//! oversized figure/table that already reaches close to the column's own
//! bottom, or a paragraph whose own `line_height` is simply taller than
//! `pitch`), the block's own NATURAL (unrounded) bottom edge is kept
//! instead — the very next fresh block's own start-snap absorbs whatever
//! misalignment remains, and a table/list/paragraph split across a page
//! boundary always resumes at a FRESH region's own top (`region_rect.y`,
//! itself already grid-aligned by construction, local offset `0`), so a
//! degrade on one page can never propagate misalignment onto the next.
//! This is the SAME "visible, bounded degrade rather than a silently
//! stretched region" convention this crate's own P0 risk note already
//! established for an ordinary block taller than a whole region.

/// Tolerance for "already exactly on the grid" — guards against a tiny
/// floating-point overshoot nudging an already-aligned position one WHOLE
/// `pitch` further forward than it should (`ceil` of e.g. `1.9999999999`
/// must still read as the SAME grid line, not the next one).
const GRID_EPSILON: f64 = 1e-6;

/// The smallest multiple of `pitch` that is `>= local` (a position
/// already measured relative to the grid's own origin), tolerant of a
/// tiny float overshoot around an already-aligned value (see
/// [`GRID_EPSILON`]).
fn round_up_to_grid_local(local: f64, pitch: f64) -> f64 {
    ((local - GRID_EPSILON) / pitch).ceil() * pitch
}

/// The delta to ADD to `cursor_y` so this FRESH block's own grid-relevant
/// edge lands on a multiple of `pitch`, measured from `grid_origin` (the
/// CURRENT region's own top — every region a [`crate::region::
/// PageRegionSequence`]/[`crate::region::ColumnRegionSequence`] hands
/// back for the SAME [`crate::master::PageMaster`] shares an identical
/// `y`, which is exactly why two page columns — and every later page —
/// end up sharing one line grid for free, with no per-column/per-page
/// special-casing anywhere in this crate). Never negative (a snap only
/// ever pushes a block FORWARD, never back into content already placed
/// above it).
///
/// `offset` is the block's own grid-relevant edge, relative to its OWN
/// top (`0.0` for every non-text block kind — see this module's own top
/// doc comment; a `Paragraph`'s own first line's `baseline_y`, i.e. its
/// ascent, for text). Only ever called for a FRESH block (never a
/// continuation of an already-in-flight split) — see `compose::flow`'s
/// own call site for exactly why continuations are excluded.
pub(crate) fn start_delta(cursor_y: f64, grid_origin: f64, pitch: f64, offset: f64) -> f64 {
    let local_before = cursor_y - grid_origin;
    let target_local = round_up_to_grid_local(local_before + offset, pitch) - offset;
    (target_local - local_before).max(0.0)
}

/// The rounded-up absolute bottom edge for a NON-TEXT block whose
/// natural bottom edge is `natural_bottom` (`cursor_y` after its own
/// content height was added) — the smallest multiple of `pitch` (from
/// `grid_origin`) that is `>= natural_bottom`, UNLESS that would push
/// past `region_bottom`, in which case `natural_bottom` is returned
/// unchanged (this module's own documented "never stretch a fixed
/// region" degrade — see this module's own top doc comment).
pub(crate) fn trailing_extra(grid_origin: f64, region_bottom: f64, natural_bottom: f64, pitch: f64) -> f64 {
    let local = natural_bottom - grid_origin;
    let rounded_local = round_up_to_grid_local(local, pitch);
    let rounded_bottom = grid_origin + rounded_local;
    if rounded_bottom <= region_bottom + GRID_EPSILON {
        (rounded_bottom - natural_bottom).max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_up_to_grid_local_finds_the_next_multiple_of_pitch() {
        assert_eq!(round_up_to_grid_local(0.0, 20.0), 0.0, "an already-aligned value stays put");
        assert_eq!(round_up_to_grid_local(1.0, 20.0), 20.0);
        assert_eq!(round_up_to_grid_local(19.999, 20.0), 20.0);
        assert_eq!(round_up_to_grid_local(20.0, 20.0), 20.0, "an EXACT multiple must never bump to the next one");
        assert_eq!(round_up_to_grid_local(20.0001, 20.0), 40.0);
    }

    #[test]
    fn round_up_to_grid_local_tolerates_a_tiny_float_overshoot_at_an_aligned_value() {
        // A value that is "20.0" up to float noise must still read as
        // already-aligned, never bumped a whole pitch further forward.
        let almost_exact = 20.0 - 1e-9;
        assert_eq!(round_up_to_grid_local(almost_exact, 20.0), 20.0);
        let just_over = 20.0 + 1e-9;
        assert_eq!(round_up_to_grid_local(just_over, 20.0), 20.0);
    }

    #[test]
    fn start_delta_pushes_a_fresh_block_forward_to_the_nearest_grid_line_never_backward() {
        // Zero offset (a non-text block's own top edge) at an
        // already-aligned position needs zero delta.
        assert_eq!(start_delta(100.0, 0.0, 20.0, 0.0), 0.0);
        // Just past a grid line — must push forward to the NEXT one, not
        // snap back to the one just passed.
        let delta = start_delta(101.0, 0.0, 20.0, 0.0);
        assert!(delta > 0.0);
        assert_eq!(101.0 + delta, 120.0);
    }

    #[test]
    fn start_delta_accounts_for_a_nonzero_baseline_offset() {
        // A text block's own baseline sits `offset` below its own top —
        // the SNAPPED quantity is `cursor_y + offset`, not `cursor_y`
        // itself, so the returned delta positions the block's own TOP
        // such that `(new_cursor_y + offset)` lands exactly on grid.
        let grid_origin = 0.0;
        let pitch = 20.0;
        let offset = 14.0; // e.g. a 16px font's own typical ascent
        let cursor_y = 10.0;
        let delta = start_delta(cursor_y, grid_origin, pitch, offset);
        let new_cursor_y = cursor_y + delta;
        let baseline_local = (new_cursor_y - grid_origin) + offset;
        let remainder = baseline_local % pitch;
        assert!(remainder.abs() < 1e-6 || (pitch - remainder).abs() < 1e-6, "the baseline itself (top + offset) must land on a grid multiple, got remainder {remainder}");
    }

    #[test]
    fn start_delta_is_zero_when_already_exactly_on_grid() {
        assert_eq!(start_delta(40.0, 0.0, 20.0, 0.0), 0.0);
        // cursor_y=54 + offset=6 = baseline 60, already an exact multiple
        // of pitch=20 — zero delta needed.
        assert_eq!(start_delta(54.0, 0.0, 20.0, 6.0), 0.0);
    }

    #[test]
    fn trailing_extra_rounds_a_blocks_bottom_edge_up_to_the_next_grid_line() {
        let extra = trailing_extra(0.0, 1000.0, 105.0, 20.0);
        assert!((extra - 15.0).abs() < 1e-6, "105 must round up to 120 (a 15.0 extra), got extra={extra}");
    }

    #[test]
    fn trailing_extra_is_zero_when_the_natural_bottom_is_already_on_grid() {
        let extra = trailing_extra(0.0, 1000.0, 100.0, 20.0);
        assert_eq!(extra, 0.0);
    }

    #[test]
    fn trailing_extra_never_pushes_past_the_regions_own_bottom_bound() {
        // Rounding 105 up to 120 would overflow a region ending at 110 —
        // must degrade to a ZERO extra (natural bottom kept) rather than
        // stretching the region.
        let extra = trailing_extra(0.0, 110.0, 105.0, 20.0);
        assert_eq!(extra, 0.0, "rounding must never push a block's own bottom edge past the region's own fixed bound");
    }

    #[test]
    fn trailing_extra_is_relative_to_a_nonzero_grid_origin() {
        // Grid origin at 50.0 (e.g. a page body starting 50px down) — a
        // natural bottom of 155.0 is local offset 105.0, rounds up to
        // local 120.0, i.e. absolute 170.0 (a 15.0 extra).
        let extra = trailing_extra(50.0, 1000.0, 155.0, 20.0);
        assert!((extra - 15.0).abs() < 1e-6);
    }
}
