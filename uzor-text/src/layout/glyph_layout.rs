//! [`ParagraphLayout`] — the pretext-pattern output: positioned glyphs +
//! line boxes for a single, single-font run of text.

use uzor::render::WrappedLine;

use crate::model::FontSpec;
use crate::shape::LineShaper;

/// One positioned glyph cluster, absolute in the paragraph box.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphLayout {
    /// The cluster's source text (mirrors [`uzor::render::GlyphMetric::cluster`]).
    pub cluster: String,
    /// Which run this glyph came from. Always `0` in Phase 1 (single-run
    /// plain text) — meaningful once Phase 2 adds multi-run `Paragraph`.
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
    /// This line's own content (advance) width — used by [`align_lines`].
    pub content_width: f64,
}

/// Positioned glyphs + line boxes for one laid-out paragraph.
///
/// Produced by [`layout_text`]. Immutable except through [`align_lines`],
/// which is the only supported post-process (design law 1: one measure
/// path — nothing else is allowed to recompute glyph positions).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphLayout {
    pub glyphs: Vec<GlyphLayout>,
    pub lines: Vec<LineBox>,
    /// Widest line's content width.
    pub width: f64,
    /// Total vertical extent (last line's `y_top + height`).
    pub height: f64,
}

/// Horizontal line alignment.
///
/// Phase-1-local: the design doc's Phase 2 introduces a broader
/// `ParagraphAlign::{Left,Center,Right,Justify}` on the rich-span
/// `Paragraph` model (`model/span.rs`) — this narrower `Align` exists only
/// to drive [`align_lines`] for Phase 1's single-run `layout_text`, and is
/// expected to be superseded/merged once Phase 2 lands. Kept deliberately
/// separate so [`layout_text`]'s doc-specified signature
/// (`text, font, max_width, shaper`) stays untouched.
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
/// the text's natural width degenerates to a single line (matches
/// [`crate::shape::LineShaper`]'s underlying `measure_glyphs_wrapped`
/// behavior). Empty text produces an empty [`ParagraphLayout`]; never panics.
pub fn layout_text(text: &str, font: &FontSpec, max_width: f64, shaper: &dyn LineShaper) -> ParagraphLayout {
    let wrapped = shaper.shape_wrapped(text, font, max_width);
    build_layout(&wrapped, font)
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
    }
}

fn build_layout(wrapped: &[WrappedLine], font: &FontSpec) -> ParagraphLayout {
    if wrapped.is_empty() {
        return ParagraphLayout::default();
    }

    let mut glyphs = Vec::new();
    let mut lines = Vec::with_capacity(wrapped.len());

    // Fallback single-line height: matches cosmic-text's own
    // `Metrics::new(font_size, font_size * 1.2)` convention (uzor's
    // shaper.rs) — used only when there is no next line to measure a real
    // delta from.
    let fallback_height = (font.size_px * 1.2).max(1.0);
    let mut prev_delta = fallback_height;

    for (line_index, line) in wrapped.iter().enumerate() {
        for glyph in &line.glyphs {
            glyphs.push(GlyphLayout {
                cluster: glyph.cluster.clone(),
                run_index: 0,
                line_index,
                x: glyph.x_offset,
                y: line.baseline_y + glyph.y_offset,
                advance: glyph.advance,
                width: glyph.width,
            });
        }

        let height = match wrapped.get(line_index + 1) {
            Some(next) => {
                let delta = (next.line_top - line.line_top).max(1.0);
                prev_delta = delta;
                delta
            }
            None => prev_delta,
        };

        lines.push(LineBox {
            line_index,
            y_top: line.line_top,
            baseline_y: line.baseline_y,
            height,
            content_width: line.width,
        });
    }

    let width = lines.iter().map(|l| l.content_width).fold(0.0_f64, f64::max);
    // `lines` is non-empty here (guarded by the `wrapped.is_empty()` early
    // return above), so `last()` always has an element.
    let height = lines.last().map(|l| l.y_top + l.height).unwrap_or(0.0);

    ParagraphLayout { glyphs, lines, width, height }
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
}
