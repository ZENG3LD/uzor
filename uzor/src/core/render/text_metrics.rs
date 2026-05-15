//! [`TextMetrics`] — stateless text measurement queries.
//!
//! Paint `expand()` calls and layout passes take `&dyn TextMetrics` directly
//! rather than the full `&dyn RenderContext`, keeping dependency surfaces small.

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

    // Phase 3 additions (declared here, implemented in Phase 3):
    // fn text_bounds(&self, text: &str) -> TextBounds;
    // fn measure_text_glyphs(&self, text: &str) -> Vec<GlyphMetric>;
}
