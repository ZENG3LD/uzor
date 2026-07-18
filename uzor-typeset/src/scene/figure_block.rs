//! [`TypesetFigure`]/[`BlockSizing`]/[`FigureBlock`] — the figure-block
//! adapter (design doc §2.2). A figure reports no intrinsic content size
//! (`uzor-figures`'s figures paint into whatever `rect` they're handed,
//! they never measure themselves first) — [`BlockSizing`] is therefore a
//! REQUIRED, explicit field on [`FigureBlock`], never a default that
//! silently stretches/squashes a figure to fill a leftover rect (design
//! doc §2.2/§7 P1 risk note).
//!
//! Figures are ATOMIC: [`crate::compose::compose`] never splits one across
//! two regions — if a figure's resolved height doesn't fit the remaining
//! space of its current region, the whole figure defers to the next one
//! (same all-or-nothing placement [`crate::scene::ImageBlock`]/
//! [`crate::scene::ListBlock`] use).

use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_figures::FigureTheme;

/// How a [`FigureBlock`]/[`crate::scene::ImageBlock`] resolves its own
/// height within the region it's placed into (design doc §2.2). Width is
/// never part of this enum — a flow figure/image always fills its
/// region's own width, exactly like a `Block::Paragraph`'s box width
/// (`crate::compose::flow`'s "regions own the width" convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockSizing {
    /// Height = the region's own remaining height (whatever is left below
    /// the current cursor) — a `FillRegion` block therefore always
    /// "fits," by construction, wherever it's placed.
    FillRegion,
    /// A fixed height in the same units as every other block's rect.
    FixedHeight(f64),
    /// Height = `region_width / aspect` — keeps a caller-chosen aspect
    /// ratio regardless of how wide the region turns out to be.
    AspectRatio(f64),
}

impl BlockSizing {
    /// Resolve this sizing to a concrete height for a block placed at
    /// `region_width`, with `remaining_height` left in the current
    /// region. The ONLY seam every atomic-placement call site (real
    /// placement AND the keep-with-next lookahead, `compose::flow`) uses
    /// to answer "how tall is this block" — never a second ad hoc height
    /// formula (design law 1).
    pub fn resolve_height(&self, region_width: f64, remaining_height: f64) -> f64 {
        match self {
            BlockSizing::FillRegion => remaining_height.max(0.0),
            BlockSizing::FixedHeight(h) => h.max(0.0),
            BlockSizing::AspectRatio(aspect) if *aspect > 0.0 => region_width / aspect,
            BlockSizing::AspectRatio(_) => 0.0,
        }
    }
}

/// The figure-block adapter: figures render into a rect, they don't shape
/// glyphs or compute their own layout — `uzor-typeset` never invents a new
/// figure-drawing primitive (design law 4), it only erases over the
/// concrete `uzor-figures` figure kinds at this one call site (bar/curve/
/// histogram/timeline/sankey, plus — typography-gap WAVE 4 — scatter/
/// boxplot/kpi).
pub trait TypesetFigure {
    /// Paint this figure into `rect` of `ctx` using `theme`.
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme);

    /// `Some(w/h)` if this figure kind wants to keep an aspect ratio
    /// (e.g. a future square heatmap); `None` (the default) means "fill
    /// whatever rect [`BlockSizing`] computes," which is what every
    /// current figure kind wants.
    fn preferred_aspect(&self) -> Option<f64> {
        None
    }
}

impl TypesetFigure for uzor_figures::BarFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::CurveFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::HistogramFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::TimelineFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::SankeyFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::ScatterFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::BoxplotFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

impl TypesetFigure for uzor_figures::KpiFigure {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &uzor_figures::FigureOverlay::default());
    }
}

/// A flow-participating figure: the erased figure + its required sizing
/// (design doc §2.2's "required, explicit field" — no `Default` impl on
/// this struct on purpose, a caller must always name a [`BlockSizing`]).
pub struct FigureBlock<'a> {
    pub figure: &'a dyn TypesetFigure,
    pub sizing: BlockSizing,
}

impl<'a> FigureBlock<'a> {
    pub fn new(figure: &'a dyn TypesetFigure, sizing: BlockSizing) -> Self {
        Self { figure, sizing }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_height_resolves_verbatim_regardless_of_region_width_or_remaining_height() {
        let sizing = BlockSizing::FixedHeight(120.0);
        assert_eq!(sizing.resolve_height(500.0, 10.0), 120.0);
        assert_eq!(sizing.resolve_height(50.0, 1000.0), 120.0);
    }

    #[test]
    fn aspect_ratio_resolves_to_width_over_aspect_exactly() {
        let sizing = BlockSizing::AspectRatio(2.0);
        assert_eq!(sizing.resolve_height(400.0, 1000.0), 200.0, "height must be width/aspect exactly, no silent stretch");
    }

    #[test]
    fn fill_region_resolves_to_the_remaining_height_and_never_negative() {
        let sizing = BlockSizing::FillRegion;
        assert_eq!(sizing.resolve_height(400.0, 250.0), 250.0);
        assert_eq!(sizing.resolve_height(400.0, -5.0), 0.0, "never a negative resolved height");
    }

    struct StubFigure;
    impl TypesetFigure for StubFigure {
        fn render(&self, _ctx: &mut dyn RenderContext, _rect: Rect, _theme: &FigureTheme) {}
    }

    #[test]
    fn preferred_aspect_defaults_to_none() {
        let stub = StubFigure;
        assert_eq!(stub.preferred_aspect(), None);
    }
}
