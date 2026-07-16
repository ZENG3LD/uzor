//! [`TextMetrics`] — stateless text measurement queries.
//!
//! Paint `expand()` calls and layout passes take `&dyn TextMetrics` directly
//! rather than the full `&dyn RenderContext`, keeping dependency surfaces small.

/// Geometric bounding box of a text run relative to the text-origin point
/// (the point passed to `fill_text(x, y)`).
///
/// For a default alphabetic baseline:
/// - `x` ≈ 0 (text starts at origin; may be slightly negative if the first
///   glyph has a negative left side-bearing)
/// - `y` = `-ascent` (the top of the text is above the origin)
/// - `w` = advance width of the string
/// - `h` = `ascent + descent`
#[derive(Debug, Clone, Copy)]
pub struct TextBounds {
    /// x of bbox left edge, relative to origin.
    pub x: f64,
    /// y of bbox top edge, relative to origin (usually negative).
    pub y: f64,
    /// Bbox width — equals `measure_text(text)` modulo side-bearing.
    pub w: f64,
    /// Bbox height — `ascent + descent`.
    pub h: f64,
    /// Distance from origin to top of tallest glyph (positive number).
    pub ascent: f64,
    /// Distance from origin to bottom of deepest descender (positive number).
    pub descent: f64,
}

/// Per-cluster shaping metrics returned by [`TextMetrics::measure_text_glyphs`].
///
/// "Cluster" typically matches one Rust `char`, but for the real Phase 4
/// shaper implementation it will map to a Unicode grapheme cluster (e.g. `é`
/// as a single cluster, emoji ZWJ sequences as one cluster).
#[derive(Debug, Clone)]
pub struct GlyphMetric {
    /// The cluster's source text (1+ chars). The same string you would pass to
    /// `fill_text` to draw just this cluster.
    pub cluster: String,
    /// x-offset of the cluster's left edge, relative to text origin (pixels).
    pub x_offset: f64,
    /// y-offset of the cluster relative to text origin (0.0 for horizontal text).
    pub y_offset: f64,
    /// Pen advance to the next cluster's origin (pixels).
    pub advance: f64,
    /// Tight bbox width of the rendered cluster (without side bearings).
    pub width: f64,
}

/// One visual line of a word-wrapped paragraph, returned by
/// [`TextMetrics::measure_text_wrapped`].
///
/// `glyphs` mirrors [`measure_text_glyphs`](TextMetrics::measure_text_glyphs)'s
/// per-cluster metrics, but positioned relative to **this line's** origin
/// (not the whole paragraph) — `glyphs[i].x_offset` is the cluster's left
/// edge from the line's own left edge. `line_top`/`baseline_y` place that
/// line within the paragraph's vertical flow.
///
/// The position of a `WrappedLine` within the `Vec` returned by
/// `measure_text_wrapped` **is** its visual line index. Do not look for a
/// separate index field — cosmic-text's own `LayoutRun::line_i` is the
/// *source*-line index (increments only on `\n`), not the visual-line
/// index, so callers must never substitute one for the other.
#[derive(Debug, Clone)]
pub struct WrappedLine {
    /// Per-cluster glyph metrics for this line, relative to the line's own origin.
    pub glyphs: Vec<GlyphMetric>,
    /// Y offset from the paragraph origin to the top of this line (pixels).
    pub line_top: f64,
    /// Y offset from the paragraph origin to this line's alphabetic baseline (pixels).
    pub baseline_y: f64,
    /// Total advance width of this line (pixels).
    pub width: f64,
}

/// Stateless text measurement — can be taken as `&dyn TextMetrics` without
/// requiring a mutable context.
///
/// Backends implement this alongside [`TextRenderer`](super::TextRenderer).
pub trait TextMetrics {
    /// Width of `text` in the current font (pixels).
    ///
    /// Uses the currently active font state (set via
    /// [`set_font`](super::TextRenderer::set_font)).
    fn measure_text(&self, text: &str) -> f64;

    /// Geometric bounds of `text` rendered in `font`. **Stateless** — does not
    /// depend on or modify the renderer's current font state.
    ///
    /// `font` is a CSS-style font shorthand, e.g. `"bold 16px Inter"`.
    ///
    /// The returned box is relative to the **text-origin point** (the `(x, y)`
    /// you would pass to `fill_text`):
    /// - `x` ≈ 0 (first glyph left edge)
    /// - `y` = `-ascent` (top of tallest glyph, above baseline)
    /// - `w` = total advance width
    /// - `h` = `ascent + descent`
    fn text_bounds(&self, text: &str, font: &str) -> TextBounds;

    /// Rasterize `text` in `font` to an SVG path `d` string covering the
    /// glyph outlines. **Stateless**.
    ///
    /// The returned `d` string is the union of all glyph outlines, positioned
    /// at the implicit origin `(0, 0)`. Caller translates via `save()` +
    /// `translate(x, y)` before calling
    /// [`push_clip_svg_path`](super::Masking::push_clip_svg_path) on the result:
    ///
    /// ```text
    /// let d = ctx.text_to_path("HELLO", "bold 48px Inter");
    /// ctx.save();
    /// ctx.translate(x, y);
    /// ctx.push_clip_svg_path(&d);
    /// // draw inside-glyphs content
    /// ctx.pop_mask();
    /// ctx.restore();
    /// ```
    ///
    /// Coordinate system: SVG y-down.  Coordinates are rounded to integer pixels.
    ///
    /// **Default impl**: returns empty string.  Backends with the `shaper`
    /// feature (tiny-skia, vello-gpu/cpu/hybrid) override with a real
    /// cosmic-text + swash outline implementation.
    fn text_to_path(&self, text: &str, font: &str) -> String {
        let _ = (text, font);
        String::new()
    }

    /// Per-cluster shaping metrics for `text` in `font`. **Stateless**.
    ///
    /// Returns one [`GlyphMetric`] per Unicode cluster in visual left-to-right
    /// order. Backends with the `shaper` feature enabled (tiny-skia,
    /// vello-gpu/cpu/hybrid) override this with a real cosmic-text shaper that
    /// handles grapheme clusters, emoji ZWJ sequences, kerning, and ligatures.
    ///
    /// **Default impl** (canvas2d, wgpu-instanced): per-`char` approximation
    /// using `text_bounds` for advance width. Does not handle multi-codepoint
    /// graphemes, ligatures, kerning, or RTL scripts.
    ///
    /// # TODO(phase-?): canvas2d / wgpu-instanced — replace with cosmic-text
    fn measure_text_glyphs(&self, text: &str, font: &str) -> Vec<GlyphMetric> {
        let mut cumulative = 0.0f64;
        text.chars()
            .map(|c| {
                let cluster = c.to_string();
                let bounds = self.text_bounds(&cluster, font);
                let advance = bounds.w;
                let x_off = cumulative;
                cumulative += advance;
                GlyphMetric {
                    cluster,
                    x_offset: x_off,
                    y_offset: 0.0,
                    advance,
                    width: advance,
                }
            })
            .collect()
    }

    /// Word-wrap `text` in `font` to `max_width` and return one
    /// [`WrappedLine`] per visual line. **Stateless**.
    ///
    /// `font` is a CSS-style font shorthand, e.g. `"bold 16px Inter"`.
    /// `max_width` is in pixels; a width at or beyond the text's natural
    /// width degenerates to a single line.
    ///
    /// Backends with the `shaper` feature enabled (tiny-skia, vello-gpu/cpu/
    /// hybrid) override this with a real cosmic-text `Wrap::Word` layout via
    /// `uzor::shaper::measure_glyphs_wrapped` (grapheme-cluster-correct wrap,
    /// matches `measure_text_glyphs`'s quality for the unwrapped path).
    ///
    /// **Default impl** (canvas2d, wgpu-instanced): naive greedy word-wrap
    /// using only [`measure_text`](Self::measure_text) per word to decide
    /// line breaks (splits `text` on whitespace, accumulates words onto the
    /// current line while it fits, starts a new line otherwise — a single
    /// word wider than `max_width` is still placed alone on its own line
    /// rather than dropped or panicking). Each finished line's glyphs come
    /// from [`measure_text_glyphs`](Self::measure_text_glyphs) (whatever
    /// approximation this backend already uses), and line geometry
    /// (`line_top`/`baseline_y`/`width`) comes from
    /// [`text_bounds`](Self::text_bounds) on the assembled line text. Does
    /// not handle grapheme clusters/RTL/kerning any better than
    /// `measure_text_glyphs` already does — lower quality than the real
    /// shaper path, never panics.
    fn measure_text_wrapped(&self, text: &str, font: &str, max_width: f64) -> Vec<WrappedLine> {
        if text.is_empty() {
            return Vec::new();
        }

        let space_w = self.measure_text(" ");

        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut current_w = 0.0f64;

        for word in text.split_whitespace() {
            let word_w = self.measure_text(word);

            if current.is_empty() {
                current.push_str(word);
                current_w = word_w;
                continue;
            }

            let candidate_w = current_w + space_w + word_w;
            if candidate_w > max_width {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
                current_w = word_w;
            } else {
                current.push(' ');
                current.push_str(word);
                current_w = candidate_w;
            }
        }
        lines.push(current);

        let mut line_top = 0.0f64;
        lines
            .into_iter()
            .map(|line_text| {
                let glyphs = self.measure_text_glyphs(&line_text, font);
                let bounds = self.text_bounds(&line_text, font);
                let baseline_y = line_top + bounds.ascent;
                let line = WrappedLine {
                    glyphs,
                    line_top,
                    baseline_y,
                    width: bounds.w,
                };
                line_top += bounds.h.max(1.0);
                line
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal `TextMetrics` impl exercising only the default `measure_text`/
    /// `text_bounds` — leaves `measure_text_glyphs`/`measure_text_wrapped` at
    /// their default impls, matching the canvas2d/wgpu-instanced backends'
    /// real shape (no shaper feature, defaults all the way down).
    struct FakeMetrics;

    const CHAR_W: f64 = 10.0;
    const ASCENT: f64 = 9.0;
    const DESCENT: f64 = 3.0;

    impl TextMetrics for FakeMetrics {
        fn measure_text(&self, text: &str) -> f64 {
            text.chars().count() as f64 * CHAR_W
        }

        fn text_bounds(&self, text: &str, _font: &str) -> TextBounds {
            let w = self.measure_text(text);
            TextBounds {
                x: 0.0,
                y: -ASCENT,
                w,
                h: ASCENT + DESCENT,
                ascent: ASCENT,
                descent: DESCENT,
            }
        }
    }

    #[test]
    fn default_wrap_empty_text_is_empty() {
        let fm = FakeMetrics;
        assert!(fm.measure_text_wrapped("", "12px Test", 100.0).is_empty());
    }

    #[test]
    fn default_wrap_wide_enough_is_a_single_line_matching_unwrapped() {
        let fm = FakeMetrics;
        let text = "short text";
        let lines = fm.measure_text_wrapped(text, "12px Test", 10_000.0);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].width, fm.measure_text(text));
        assert_eq!(lines[0].line_top, 0.0);
        assert_eq!(lines[0].baseline_y, ASCENT);
    }

    #[test]
    fn default_wrap_narrow_width_wraps_into_multiple_lines() {
        let fm = FakeMetrics;
        // Each word is 3 chars = 30px; "word word" = 30 + 10(space) + 30 = 70px.
        let text = "one two three four five six seven eight";
        let max_width = 65.0; // fits ~2 short words per line at most

        let lines = fm.measure_text_wrapped(text, "12px Test", max_width);
        assert!(
            lines.len() > 1,
            "expected wrap into multiple lines, got {}",
            lines.len()
        );

        let line_height = ASCENT + DESCENT;
        let mut prev_top = -1.0f64;
        for (i, line) in lines.iter().enumerate() {
            assert!(line.line_top > prev_top, "line_top must strictly increase");
            prev_top = line.line_top;

            let expected_top = i as f64 * line_height;
            assert!(
                (line.line_top - expected_top).abs() < 1e-9,
                "line {i} top {} != expected {expected_top}",
                line.line_top
            );

            // No word in this fixture is unbreakably wide, so every wrapped
            // line must fit within max_width — no overflow tolerance needed.
            assert!(
                line.width <= max_width,
                "line {i} width {} exceeds max_width {max_width}",
                line.width
            );
        }

        let total_height = prev_top + line_height;
        let expected_total = lines.len() as f64 * line_height;
        assert!((total_height - expected_total).abs() < 1e-9);
    }
}
