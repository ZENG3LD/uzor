//! [`CosmicShaper`] — [`LineShaper`] backed by `cosmic-text` (via
//! `uzor::shaper` / [`TextMetrics::measure_text_wrapped`]).

use uzor::render::{TextMetrics, WrappedLine};

use super::LineShaper;
use crate::model::FontSpec;

/// [`LineShaper`] implementation backed by cosmic-text.
///
/// Two constructors select *where* shaping happens — both produce
/// identical results (the four shaper-backed render contexts' own
/// `measure_text_wrapped` override delegates to the same
/// `uzor::shaper::measure_glyphs_wrapped` this calls directly):
///
/// - [`CosmicShaper::from_ctx`] — delegates to an already-live render
///   context's [`TextMetrics::measure_text_wrapped`]. Use this when a
///   `RenderContext` is already on hand (e.g. inside a widget's draw
///   pass) so no second font system is touched.
/// - [`CosmicShaper::headless`] — calls
///   `uzor::shaper::measure_glyphs_wrapped` directly. No render
///   context/window needed — for `uzor-export`-driven headless layout,
///   or plain unit tests.
pub enum CosmicShaper<'a> {
    /// Delegates to a live render context's `TextMetrics::measure_text_wrapped`.
    FromCtx(&'a dyn TextMetrics),
    /// Calls `uzor::shaper::measure_glyphs_wrapped` directly — no context needed.
    Headless,
}

impl<'a> CosmicShaper<'a> {
    /// Shape through an already-live render context.
    pub fn from_ctx(ctx: &'a dyn TextMetrics) -> Self {
        CosmicShaper::FromCtx(ctx)
    }

    /// Shape directly via `uzor::shaper`, no render context needed.
    pub fn headless() -> CosmicShaper<'static> {
        CosmicShaper::Headless
    }
}

impl<'a> LineShaper for CosmicShaper<'a> {
    fn shape_wrapped(&self, text: &str, font: &FontSpec, max_width: f64) -> Vec<WrappedLine> {
        let font_str = font.to_css_font();
        match self {
            CosmicShaper::FromCtx(ctx) => ctx.measure_text_wrapped(text, &font_str, max_width),
            CosmicShaper::Headless => uzor::shaper::measure_glyphs_wrapped(text, &font_str, max_width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

    /// `from_ctx` (real shaper-backed render context) and `headless` (direct
    /// `uzor::shaper` call) must agree on the same input — both paths
    /// bottom out in `uzor::shaper::measure_glyphs_wrapped`.
    #[test]
    fn from_ctx_and_headless_agree_on_a_short_line() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let ctx = TinySkiaCpuRenderContext::new(200, 100, 1.0);

        let from_ctx = CosmicShaper::from_ctx(&ctx);
        let headless = CosmicShaper::headless();

        let a = from_ctx.shape_wrapped("Hello uzor-text", &font, 1000.0);
        let b = headless.shape_wrapped("Hello uzor-text", &font, 1000.0);

        assert_eq!(a.len(), 1);
        assert_eq!(a.len(), b.len());
        assert_eq!(a[0].glyphs.len(), b[0].glyphs.len());
        for (ga, gb) in a[0].glyphs.iter().zip(b[0].glyphs.iter()) {
            assert_eq!(ga.cluster, gb.cluster);
            assert!((ga.x_offset - gb.x_offset).abs() < 0.01);
        }
    }
}
