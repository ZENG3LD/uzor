//! [`ParagraphLayout`] — the pretext-pattern output: positioned glyphs +
//! line boxes (+ placed inline boxes, Phase 2) for a laid-out paragraph.

use crate::model::{FontSpec, Paragraph, StyledRun};
use crate::shape::LineShaper;

use super::paragraph::layout_paragraph;

/// One positioned glyph cluster, absolute in the paragraph box.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphLayout {
    /// The cluster's source text (mirrors [`uzor::render::GlyphMetric::cluster`]).
    pub cluster: String,
    /// Which run (index into the source [`Paragraph::runs`]) this glyph
    /// came from. Always `0` for [`layout_text`]'s single-run case.
    pub run_index: usize,
    /// Which visual line this glyph belongs to (index into
    /// [`ParagraphLayout::lines`]).
    pub line_index: usize,
    /// x-offset of the cluster's left edge, absolute in the paragraph box.
    pub x: f64,
    /// Baseline y, absolute in the paragraph box.
    pub y: f64,
    /// Pen advance to the next cluster's origin (pixels).
    pub advance: f64,
    /// Tight bbox width of the rendered cluster.
    pub width: f64,
    /// This glyph's own run's font — carried per-glyph (rather than
    /// requiring the caller to hold onto the source [`Paragraph`]) so
    /// [`crate::draw::draw_paragraph`] can set the right font per glyph
    /// from `ParagraphLayout` alone (design law 1: one measure path).
    pub font: FontSpec,
    /// This glyph's own run's color override (packed `0xRRGGBBAA`), or
    /// `None` to defer to the draw call's default color.
    pub color: Option<u32>,
}

/// Geometry of one visual line within a [`ParagraphLayout`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineBox {
    /// Index of this line (matches [`GlyphLayout::line_index`] for its glyphs).
    pub line_index: usize,
    /// y-offset from the paragraph origin to the top of this line.
    pub y_top: f64,
    /// y-offset from the paragraph origin to this line's alphabetic baseline.
    pub baseline_y: f64,
    /// This line's height (vertical space it occupies before the next line starts).
    pub height: f64,
    /// This line's own rendered content width — for [`layout_text`]'s
    /// output this is the natural (pre-alignment) advance width, matching
    /// [`align_lines`]'s expectations; for [`layout_paragraph`]'s output
    /// this already reflects `Justify`'s stretch, since justification
    /// changes the glyphs themselves rather than being a post-process
    /// (unlike `Left`/`Center`/`Right`, which `align_lines` applies as a
    /// pure x-shift after the fact).
    pub content_width: f64,
}

/// One [`crate::model::InlineBox`] placed within a [`ParagraphLayout`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedInlineBox {
    /// Echoes [`crate::model::InlineBox::id`] so the caller can match a
    /// placement back to whatever it should draw there.
    pub id: u64,
    pub line_index: usize,
    /// x-offset of the box's left edge, absolute in the paragraph box.
    pub x: f64,
    /// y-offset of the box's top edge, absolute in the paragraph box.
    pub y_top: f64,
    pub width: f64,
    pub height: f64,
}

/// Which decoration a [`DecorationSpan`] paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Strikethrough,
}

/// One resolved underline/strikethrough segment (typography-gap WAVE 2) —
/// a maximal run of consecutive same-line, same-run, contiguous (no gap —
/// e.g. never spans across a spliced [`crate::model::InlineBox`]) glyphs
/// whose own [`crate::model::StyledRun::decoration`] is non-[`crate::model::TextDecoration::NONE`].
///
/// `y`/`thickness` are already resolved (paragraph-relative, absolute —
/// same convention as [`GlyphLayout::y`]) via
/// [`crate::model::VerticalAlign`]'s sane-fallback ratios (see that type's
/// own doc comment for why: no real font-embedded underline metric is
/// reachable from this crate's production code). A caller paints this as a
/// filled rect: `(x_start, y, x_end - x_start, thickness)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecorationSpan {
    /// Which source run ([`crate::model::Paragraph::runs`]) this span came
    /// from — echoes [`GlyphLayout::run_index`]'s own convention.
    pub run_index: usize,
    pub line_index: usize,
    pub kind: DecorationKind,
    pub x_start: f64,
    pub x_end: f64,
    /// Absolute paragraph-relative y of the painted rule's TOP edge.
    pub y: f64,
    pub thickness: f64,
    /// Echoes the owning run's own color override — `None` defers to
    /// whatever default color the caller paints with (matches
    /// [`GlyphLayout::color`]'s own convention).
    pub color: Option<u32>,
}

/// Positioned glyphs + line boxes (+ placed inline boxes + decoration
/// spans) for one laid-out paragraph.
///
/// Produced by [`layout_text`]/[`layout_paragraph`]. Immutable except
/// through [`align_lines`], which is the only supported post-process
/// (design law 1: one measure path — nothing else is allowed to
/// recompute glyph positions).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphLayout {
    pub glyphs: Vec<GlyphLayout>,
    pub lines: Vec<LineBox>,
    pub boxes: Vec<PlacedInlineBox>,
    /// Underline/strikethrough segments (typography-gap WAVE 2) — empty
    /// for every paragraph whose runs never set
    /// [`crate::model::StyledRun::decoration`].
    pub decorations: Vec<DecorationSpan>,
    /// Widest line's content width.
    pub width: f64,
    /// Total vertical extent (last line's `y_top + height`).
    pub height: f64,
}

/// Horizontal line alignment.
///
/// Phase-1-local: [`crate::model::ParagraphAlign`] is the richer,
/// `Paragraph`-facing Phase 2 alignment type (adds `Justify`). This
/// narrower `Align` is kept, unchanged, as the type [`align_lines`] and
/// [`layout_text`]'s single-run callers use — deliberately NOT removed or
/// renamed (Phase 1 regression guard), even though `ParagraphAlign` is now
/// the type new (multi-run) code should reach for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Lay out `text` in `font`, word-wrapped to `max_width`, via `shaper`.
///
/// Pure function (design law 3): `(text, font, max_width, shaper)` in,
/// [`ParagraphLayout`] out — no retained state. A `max_width` at or beyond
/// the text's natural width degenerates to a single line. Empty text
/// produces an empty [`ParagraphLayout`]; never panics.
///
/// A thin wrapper over [`layout_paragraph`] with a single-run [`Paragraph`]
/// (Phase 2 regression guard: this signature is unchanged from Phase 1).
pub fn layout_text(text: &str, font: &FontSpec, max_width: f64, shaper: &dyn LineShaper) -> ParagraphLayout {
    let runs = [StyledRun::new(text, *font)];
    let paragraph = Paragraph::new(&runs, max_width);
    layout_paragraph(&paragraph, shaper)
}

/// Shift every line's glyphs horizontally so the paragraph reads as
/// left/center/right-aligned within a `box_width`-wide container.
///
/// A per-line x-shift only (design doc §3.2: alignment never touches
/// wrap/measure results, only the caller-supplied box it paints into).
/// `box_width` is typically the same `max_width` passed to [`layout_text`]
/// but may be any container width the caller wants to align against.
/// No-op for [`Align::Left`] or a non-finite `box_width`.
pub fn align_lines(layout: &mut ParagraphLayout, box_width: f64, align: Align) {
    if align == Align::Left || !box_width.is_finite() {
        return;
    }

    for line in &layout.lines {
        let shift = match align {
            Align::Left => 0.0,
            Align::Center => (box_width - line.content_width) / 2.0,
            Align::Right => box_width - line.content_width,
        };
        if shift == 0.0 {
            continue;
        }
        for glyph in layout.glyphs.iter_mut().filter(|g| g.line_index == line.line_index) {
            glyph.x += shift;
        }
        for b in layout.boxes.iter_mut().filter(|b| b.line_index == line.line_index) {
            b.x += shift;
        }
        for d in layout.decorations.iter_mut().filter(|d| d.line_index == line.line_index) {
            d.x_start += shift;
            d.x_end += shift;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::CosmicShaper;
    use uzor::fonts::FontFamily;

    const LONG_SENTENCE: &str = "The quick brown fox jumps over the lazy dog \
        and then keeps running further down the road without stopping for a \
        very long time indeed";

    /// Parity: `layout_text` at an effectively-unwrapped width produces the
    /// same total advance / per-glyph geometry as `uzor::shaper::measure_glyphs`
    /// for the same (text, font) — within the same epsilon tolerance the
    /// Phase 0 wrap-unlock test itself uses (`shaper.rs`'s
    /// `wrapped_at_huge_width_matches_unwrapped_glyph_for_glyph`), since both
    /// paths bottom out in the same cosmic-text call with different
    /// buffer wrap settings.
    #[test]
    fn layout_text_unwrapped_matches_measure_glyphs() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "Hello, uzor-text!";

        let layout = layout_text(text, &font, f64::MAX, &shaper);
        assert_eq!(layout.lines.len(), 1, "expected exactly one line at f64::MAX width");

        let glyphs = uzor::shaper::measure_glyphs(text, &font.to_css_font());
        let expected_extent = glyphs.iter().map(|g| g.x_offset + g.advance).fold(0.0_f64, f64::max);

        assert!((layout.width - expected_extent).abs() < 0.01);
        assert_eq!(layout.glyphs.len(), glyphs.len());
        for (a, b) in layout.glyphs.iter().zip(glyphs.iter()) {
            assert_eq!(a.cluster, b.cluster);
            assert!((a.x - b.x_offset).abs() < 0.01);
            assert!((a.advance - b.advance).abs() < 0.01);
            assert!((a.width - b.width).abs() < 0.01);
        }
    }

    /// Narrow `max_width` forces wrapping: >1 lines, every line's content
    /// width within `max_width` (+ tolerance), baselines strictly
    /// increasing, spacing consistent line-to-line.
    #[test]
    fn wrap_produces_multiple_lines_within_width_with_consistent_spacing() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = 150.0;

        let layout = layout_text(LONG_SENTENCE, &font, max_width, &shaper);
        assert!(layout.lines.len() > 1, "expected wrap into multiple lines, got {}", layout.lines.len());

        let mut prev_baseline = f64::MIN;
        for line in &layout.lines {
            assert!(
                line.content_width <= max_width + 1.0,
                "line {} width {} exceeds max_width {max_width}",
                line.line_index,
                line.content_width
            );
            assert!(line.baseline_y > prev_baseline, "baselines must strictly increase");
            prev_baseline = line.baseline_y;
        }

        // Consistent spacing: consecutive baseline deltas agree within 0.5px.
        let deltas: Vec<f64> = layout
            .lines
            .windows(2)
            .map(|w| w[1].baseline_y - w[0].baseline_y)
            .collect();
        for d in &deltas {
            assert!((d - deltas[0]).abs() < 0.5, "line spacing must be consistent, got deltas {deltas:?}");
        }
    }

    /// Empty text produces a sane, empty layout — no panic.
    #[test]
    fn empty_text_produces_empty_layout() {
        let font = FontSpec::default();
        let shaper = CosmicShaper::headless();

        let layout = layout_text("", &font, 100.0, &shaper);
        assert!(layout.glyphs.is_empty());
        assert!(layout.lines.is_empty());
        assert_eq!(layout.width, 0.0);
        assert_eq!(layout.height, 0.0);
    }

    /// Whitespace-only text must not panic and stays a single line.
    #[test]
    fn whitespace_only_text_does_not_panic() {
        let font = FontSpec::default();
        let shaper = CosmicShaper::headless();

        let layout = layout_text("    ", &font, 100.0, &shaper);
        assert!(layout.lines.len() <= 1);
    }

    /// `align_lines(Center)` shifts every line by half its remaining width.
    #[test]
    fn align_center_shifts_each_line_by_half_the_remaining_width() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = 220.0;
        let text = "one two three four five six seven eight nine ten eleven twelve";

        let base = layout_text(text, &font, max_width, &shaper);
        assert!(base.lines.len() > 1, "fixture must wrap to multiple lines");

        let mut centered = base.clone();
        align_lines(&mut centered, max_width, Align::Center);

        for line in &base.lines {
            let expected_shift = (max_width - line.content_width) / 2.0;
            let base_xs: Vec<f64> =
                base.glyphs.iter().filter(|g| g.line_index == line.line_index).map(|g| g.x).collect();
            let centered_xs: Vec<f64> =
                centered.glyphs.iter().filter(|g| g.line_index == line.line_index).map(|g| g.x).collect();
            assert_eq!(base_xs.len(), centered_xs.len());
            for (b, c) in base_xs.iter().zip(centered_xs.iter()) {
                assert!((c - (b + expected_shift)).abs() < 1e-6);
            }
        }
    }

    /// `align_lines(Right)` shifts every line so its right edge reaches
    /// `box_width` exactly.
    #[test]
    fn align_right_shifts_each_line_to_the_far_edge() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = 220.0;
        let text = "one two three four five six seven eight nine ten eleven twelve";

        let base = layout_text(text, &font, max_width, &shaper);
        assert!(base.lines.len() > 1, "fixture must wrap to multiple lines");

        let mut right = base.clone();
        align_lines(&mut right, max_width, Align::Right);

        for line in &right.lines {
            let expected_shift = max_width - line.content_width;
            let last_glyph = right
                .glyphs
                .iter()
                .filter(|g| g.line_index == line.line_index)
                .last();
            if let Some(g) = last_glyph {
                assert!(
                    (g.x + g.advance - max_width).abs() < 0.5,
                    "right-aligned line {} should reach max_width {max_width}, got {}",
                    line.line_index,
                    g.x + g.advance
                );
            }
            assert!(expected_shift.is_finite());
        }
    }

    /// Typography-gap WAVE 2: `align_lines(Center)` shifts a decoration
    /// span's `x_start`/`x_end` by the SAME per-line amount it shifts that
    /// line's glyphs — a decoration must never desync from the text it
    /// underlines/strikes through after alignment.
    #[test]
    fn align_lines_shifts_decoration_spans_by_the_same_amount_as_glyphs() {
        use crate::model::TextDecoration;

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let max_width = 220.0;
        let runs = [StyledRun::new("short", font).with_decoration(TextDecoration::underline())];
        let paragraph = Paragraph::new(&runs, max_width);
        let shaper = CosmicShaper::headless();

        let base = layout_paragraph(&paragraph, &shaper);
        assert_eq!(base.decorations.len(), 1);

        let mut centered = base.clone();
        align_lines(&mut centered, max_width, Align::Center);

        let expected_shift = (max_width - base.lines[0].content_width) / 2.0;
        assert!((centered.decorations[0].x_start - (base.decorations[0].x_start + expected_shift)).abs() < 1e-6);
        assert!((centered.decorations[0].x_end - (base.decorations[0].x_end + expected_shift)).abs() < 1e-6);
        // y/thickness/kind/color are untouched by a horizontal-only shift.
        assert_eq!(centered.decorations[0].y, base.decorations[0].y);
        assert_eq!(centered.decorations[0].thickness, base.decorations[0].thickness);
    }
}
