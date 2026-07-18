//! Pure geometric helpers over an already-computed [`ParagraphLayout`] —
//! the "splitting its already-computed `ParagraphLayout` at a line
//! boundary" half of design doc §3.3. Re-running `layout_paragraph` for a
//! continuation is only needed when the next region's width differs from
//! the one the layout was measured at (P0 never hits that branch — see
//! this crate's `CLAUDE.md` "Divergences from the design doc").
//!
//! ## Widow/orphan control (typography quality wave)
//!
//! [`widow_orphan_count`] is the lua-widow-control-inspired decision
//! ladder adapted to this crate's own single-pass, no-backtracking
//! `compose` loop (research doc §5's "port-algorithm" recommendation,
//! narrowed to a REGION-LOCAL decision rather than a document-wide
//! `\looseness` re-search — see this crate's `CLAUDE.md` typography-wave
//! section for the full ladder and why a local decision is the correct
//! adaptation here). Consulted only at a genuine paragraph split (never
//! when the whole remainder already fits): an **orphan** is this split's
//! own HEAD (what [`lines_fitting`]'s budget would leave behind in the
//! CURRENT region); a **widow** is this split's own TAIL (what the
//! continuation would carry into the next region).

use uzor_text::{DecorationSpan, GlyphLayout, LineBox, ParagraphLayout, PlacedInlineBox};

/// How many of `layout`'s lines, starting at `from_line`, fit within
/// `remaining_height` (summing each kept line's own `.height`).
///
/// When nothing fits and `force_at_least_one` is `true` (the region had no
/// other content placed in it yet), returns `1` anyway — the P0 risk-note
/// degrade: a single line taller than a whole fresh region still gets
/// placed, visibly overflowing, rather than looping
/// `RegionSequence::next()` forever.
pub(crate) fn lines_fitting(layout: &ParagraphLayout, from_line: usize, remaining_height: f64, force_at_least_one: bool) -> usize {
    let mut used = 0.0_f64;
    let mut count = 0usize;

    for line in &layout.lines[from_line..] {
        let next = used + line.height;
        if next > remaining_height {
            break;
        }
        used = next;
        count += 1;
    }

    if count == 0 && force_at_least_one && from_line < layout.lines.len() {
        count = 1;
    }
    count
}

/// Extract lines `[from_line, to_line)` of `layout` into a new, owned
/// [`ParagraphLayout`]: re-indexed so the kept lines' `line_index` starts
/// at `0`, and rebased so the first kept line's `y_top` becomes `0.0`
/// (ready to paint at a fresh region's own origin). `to_line` is clamped to
/// `layout.lines.len()`. An empty range (`from_line >= to_line`) returns an
/// empty layout — never panics.
pub(crate) fn slice_layout_lines(layout: &ParagraphLayout, from_line: usize, to_line: usize) -> ParagraphLayout {
    let to_line = to_line.min(layout.lines.len());
    if from_line >= to_line {
        return ParagraphLayout::default();
    }

    let y_shift = layout.lines[from_line].y_top;

    let lines: Vec<LineBox> = layout.lines[from_line..to_line]
        .iter()
        .map(|l| LineBox {
            line_index: l.line_index - from_line,
            y_top: l.y_top - y_shift,
            baseline_y: l.baseline_y - y_shift,
            height: l.height,
            content_width: l.content_width,
        })
        .collect();

    let glyphs: Vec<GlyphLayout> = layout
        .glyphs
        .iter()
        .filter(|g| g.line_index >= from_line && g.line_index < to_line)
        .map(|g| GlyphLayout {
            cluster: g.cluster.clone(),
            run_index: g.run_index,
            line_index: g.line_index - from_line,
            x: g.x,
            y: g.y - y_shift,
            advance: g.advance,
            width: g.width,
            font: g.font,
            color: g.color,
        })
        .collect();

    let boxes: Vec<PlacedInlineBox> = layout
        .boxes
        .iter()
        .filter(|b| b.line_index >= from_line && b.line_index < to_line)
        .map(|b| PlacedInlineBox {
            id: b.id,
            line_index: b.line_index - from_line,
            x: b.x,
            y_top: b.y_top - y_shift,
            width: b.width,
            height: b.height,
        })
        .collect();

    let decorations: Vec<DecorationSpan> = layout
        .decorations
        .iter()
        .filter(|d| d.line_index >= from_line && d.line_index < to_line)
        .map(|d| DecorationSpan {
            run_index: d.run_index,
            line_index: d.line_index - from_line,
            kind: d.kind,
            x_start: d.x_start,
            x_end: d.x_end,
            y: d.y - y_shift,
            thickness: d.thickness,
            color: d.color,
        })
        .collect();

    let width = lines.iter().map(|l| l.content_width).fold(0.0_f64, f64::max);
    let height = lines.last().map(|l| l.y_top + l.height).unwrap_or(0.0);

    ParagraphLayout { glyphs, lines, boxes, decorations, width, height }
}

/// Widow/orphan-adjusted line count for a paragraph split at a region
/// boundary. `full_len`/`next_line` describe the paragraph's total line
/// count and where this split starts; `budget_count` is
/// [`lines_fitting`]'s own answer (how many lines fit the remaining
/// space). `0`/either threshold disables that half of the control
/// entirely (§ task's own "0 disables").
///
/// Returns `None` when neither `min_orphan_lines` nor `min_widow_lines`
/// can be satisfied by narrowing the count — the caller then defers the
/// WHOLE remaining paragraph, exactly like a `budget_count == 0` overflow.
///
/// `allow_defer` is `false` only when the region is otherwise empty
/// (`lines_fitting`'s own force-at-least-one convention already
/// guarantees `budget_count >= 1` there, matching every other break
/// control in this crate's own "existing empty-region force-place
/// convention" — `compose::flow`'s `AvoidInside`/`AvoidAfter` checks use
/// the identical `region_has_content` gate). Deferring in that case would
/// either violate the forced-progress guarantee or loop forever across
/// same-height regions, so this function only ever NARROWS the count when
/// `allow_defer` is `false`, never defers.
pub(crate) fn widow_orphan_count(
    full_len: usize,
    next_line: usize,
    budget_count: usize,
    min_orphan_lines: usize,
    min_widow_lines: usize,
    allow_defer: bool,
) -> Option<usize> {
    if budget_count == 0 {
        return Some(0);
    }
    let remaining = full_len - next_line;
    if budget_count >= remaining {
        // The whole rest of the paragraph fits in this region — no split
        // actually happens, nothing to protect.
        return Some(budget_count);
    }

    // Orphan: this split's own HEAD (what stays behind in this region).
    if allow_defer && min_orphan_lines > 0 && budget_count < min_orphan_lines {
        return None;
    }

    // Widow: this split's own TAIL (what the continuation would carry).
    let tail = remaining - budget_count;
    if min_widow_lines == 0 || tail >= min_widow_lines {
        return Some(budget_count);
    }

    // Pull `deficit` lines from the head over to the tail — never below 1
    // line placed (the forced-progress floor), and never below
    // `min_orphan_lines` when deferring instead is actually an option.
    let deficit = min_widow_lines - tail;
    let floor = if allow_defer { min_orphan_lines.max(1) } else { 1 };
    if budget_count > deficit && budget_count - deficit >= floor {
        Some(budget_count - deficit)
    } else if allow_defer {
        None
    } else {
        Some(budget_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{layout_paragraph, CosmicShaper, FontSpec, Paragraph, StyledRun};

    const WRAPPING_TEXT: &str =
        "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen";

    fn wrapped_layout() -> ParagraphLayout {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(WRAPPING_TEXT, font)];
        let paragraph = Paragraph::new(&runs, 150.0);
        let shaper = CosmicShaper::headless();
        layout_paragraph(&paragraph, &shaper)
    }

    #[test]
    fn lines_fitting_returns_the_max_whole_lines_within_budget() {
        let layout = wrapped_layout();
        assert!(layout.lines.len() > 3, "fixture must wrap to several lines");

        let one_line_height = layout.lines[0].height;
        let count = lines_fitting(&layout, 0, one_line_height * 2.5, false);
        assert_eq!(count, 2, "2.5 line-heights of budget should fit exactly 2 whole lines");
    }

    #[test]
    fn lines_fitting_forces_one_line_when_nothing_fits_and_the_region_is_fresh() {
        let layout = wrapped_layout();
        let count = lines_fitting(&layout, 0, 1.0, true);
        assert_eq!(count, 1, "a fresh region must make progress even if the line overflows it");
    }

    #[test]
    fn lines_fitting_returns_zero_when_nothing_fits_and_the_region_already_has_content() {
        let layout = wrapped_layout();
        let count = lines_fitting(&layout, 0, 1.0, false);
        assert_eq!(count, 0, "a non-fresh region must defer the whole block rather than force an overflow");
    }

    #[test]
    fn widow_orphan_count_defers_whole_when_the_head_would_be_an_orphan() {
        // 10-line paragraph, budget fits only 1 line — min_orphan_lines=2
        // forbids leaving a 1-line orphan behind.
        assert_eq!(widow_orphan_count(10, 0, 1, 2, 2, true), None);
    }

    #[test]
    fn widow_orphan_count_pulls_a_line_over_when_the_tail_would_be_a_widow() {
        // 10-line paragraph, budget fits 9 lines (tail would be 1 line) —
        // min_widow_lines=2 pulls 1 line back, leaving head=8/tail=2.
        assert_eq!(widow_orphan_count(10, 0, 9, 2, 2, true), Some(8));
    }

    #[test]
    fn widow_orphan_count_defers_whole_when_pulling_a_line_over_would_violate_the_orphan_floor() {
        // 3-line paragraph, budget fits 2 (tail=1, a widow). Pulling 1 line
        // over would leave head=1, which itself violates min_orphan_lines=2
        // — neither side can be satisfied, so defer whole.
        assert_eq!(widow_orphan_count(3, 0, 2, 2, 2, true), None);
    }

    #[test]
    fn widow_orphan_count_zero_disables_both_controls_reproducing_the_raw_budget() {
        assert_eq!(widow_orphan_count(10, 0, 1, 0, 0, true), Some(1), "min_orphan_lines=0 never defers an orphan");
        assert_eq!(widow_orphan_count(10, 0, 9, 0, 0, true), Some(9), "min_widow_lines=0 never pulls a line over");
    }

    #[test]
    fn widow_orphan_count_never_defers_when_the_region_is_otherwise_empty() {
        // Same orphan-triggering shape as the first test above, but
        // `allow_defer=false` (region has no other content) — must keep
        // the forced budget rather than defer (would loop forever across
        // same-height regions otherwise).
        assert_eq!(widow_orphan_count(10, 0, 1, 2, 2, false), Some(1));
    }

    #[test]
    fn widow_orphan_count_still_pulls_a_widow_line_over_even_in_an_otherwise_empty_region() {
        // Pulling a line over never reduces progress to zero, so it's safe
        // even when `allow_defer=false`.
        assert_eq!(widow_orphan_count(10, 0, 9, 2, 2, false), Some(8));
    }

    #[test]
    fn widow_orphan_count_places_the_full_budget_when_no_split_actually_happens() {
        // budget_count >= remaining: the whole rest of the paragraph fits,
        // nothing to protect.
        assert_eq!(widow_orphan_count(5, 0, 5, 2, 2, true), Some(5));
    }

    #[test]
    fn slice_conserves_every_line_with_no_overlap_and_no_gap() {
        let layout = wrapped_layout();
        let total = layout.lines.len();
        let split_at = total / 2;

        let head = slice_layout_lines(&layout, 0, split_at);
        let tail = slice_layout_lines(&layout, split_at, total);

        assert_eq!(head.lines.len() + tail.lines.len(), total, "total lines must be conserved across the split");
        assert_eq!(head.lines.len(), split_at);
        assert_eq!(tail.lines.len(), total - split_at);

        // Every glyph on the original layout must land in exactly one side.
        let original_glyph_count = layout.glyphs.len();
        assert_eq!(head.glyphs.len() + tail.glyphs.len(), original_glyph_count);

        // Head's own y_top must start exactly at 0 (rebased for painting at
        // a fresh region's own origin) — never a stray offset carried over
        // from the source layout.
        assert_eq!(head.lines[0].y_top, 0.0);
        assert_eq!(tail.lines[0].y_top, 0.0);
    }

    #[test]
    fn slice_of_an_empty_range_is_an_empty_layout_not_a_panic() {
        let layout = wrapped_layout();
        let empty = slice_layout_lines(&layout, 5, 5);
        assert!(empty.lines.is_empty());
        assert!(empty.glyphs.is_empty());
    }

    /// Typography-gap WAVE 2: a decoration span belonging to a kept line
    /// survives the split, rebased the SAME way `lines`/`glyphs` are (line
    /// index shifted, `y` rebased by the same `y_shift`); a span belonging
    /// to a dropped line never leaks into the wrong half.
    #[test]
    fn slice_rebases_decoration_spans_the_same_way_as_lines_and_glyphs() {
        use uzor::fonts::FontFamily;
        use uzor_text::{layout_paragraph, CosmicShaper, FontSpec, Paragraph, StyledRun, TextDecoration};

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(WRAPPING_TEXT, font).with_decoration(TextDecoration::underline())];
        let paragraph = Paragraph::new(&runs, 150.0);
        let shaper = CosmicShaper::headless();
        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 3, "fixture must wrap to several lines");
        assert!(!layout.decorations.is_empty(), "the fully-underlined fixture must produce at least one span per line");

        let split_at = layout.lines.len() / 2;
        let head = slice_layout_lines(&layout, 0, split_at);
        let tail = slice_layout_lines(&layout, split_at, layout.lines.len());

        assert_eq!(
            head.decorations.len() + tail.decorations.len(),
            layout.decorations.len(),
            "every decoration span must land in exactly one side"
        );
        for d in &head.decorations {
            assert!(d.line_index < split_at, "head span line_index must be re-indexed within the head's own range");
        }
        for d in &tail.decorations {
            assert!(d.line_index < layout.lines.len() - split_at, "tail span line_index must be re-indexed starting at 0");
        }

        // The tail's first-line decoration `y` must be rebased by the SAME
        // `y_shift` the tail's own first `LineBox::y_top` (already 0.0,
        // per the existing regression test above) was rebased by — i.e.
        // it must sit at a SMALLER absolute `y` than the source layout's
        // own corresponding span.
        let source_tail_span = layout.decorations.iter().find(|d| d.line_index == split_at).expect("source must have a span on the split line");
        let rebased_span = tail.decorations.iter().find(|d| d.line_index == 0).expect("tail must have a span on its own first line");
        assert!(rebased_span.y < source_tail_span.y, "tail span y must be rebased upward (smaller) after the split");
    }
}
