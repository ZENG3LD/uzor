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
}
