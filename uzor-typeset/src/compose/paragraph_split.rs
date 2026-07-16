//! Pure geometric helpers over an already-computed [`ParagraphLayout`] —
//! the "splitting its already-computed `ParagraphLayout` at a line
//! boundary" half of design doc §3.3. Re-running `layout_paragraph` for a
//! continuation is only needed when the next region's width differs from
//! the one the layout was measured at (P0 never hits that branch — see
//! this crate's `CLAUDE.md` "Divergences from the design doc").

use uzor_text::{GlyphLayout, LineBox, ParagraphLayout, PlacedInlineBox};

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

    let width = lines.iter().map(|l| l.content_width).fold(0.0_f64, f64::max);
    let height = lines.last().map(|l| l.y_top + l.height).unwrap_or(0.0);

    ParagraphLayout { glyphs, lines, boxes, width, height }
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
}
