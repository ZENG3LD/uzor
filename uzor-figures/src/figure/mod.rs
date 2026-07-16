//! Composed figures — thin, in-crate compositions of scale + coord + mark
//! + guide. Each figure is a pure `render(&self, ctx, rect, theme)` (V1)
//! or `render_with(&self, ctx, rect, theme, overlay: &FigureOverlay)` (V2)
//! over borrowed data; a figure never retains interaction state itself —
//! [`FigureOverlay`] is borrowed per-frame input, not owned (design law #3).
//! A real registry/IR (mlc's `ChartTypeDef`+`DrawOps` two-table pattern)
//! is a later milestone — see the crate-root docs.

pub mod bars;
pub mod curve;
pub mod histogram;

pub use bars::BarFigure;
pub use curve::CurveFigure;
pub use histogram::{bin, Bin, HistogramFigure};

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::interact::focus::FocusSet;
use crate::theme::FigureTheme;

/// Borrowed per-frame interaction state a figure may optionally react to.
/// The default (`hover_px: None, brush: None, focus: None`) reproduces
/// exactly the old stateless V1 render — `render(ctx, rect, theme)` is
/// literally `render_with(ctx, rect, theme, &FigureOverlay::default())`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FigureOverlay<'a> {
    /// Cursor position in the SAME absolute pixel space as the `rect`
    /// passed to `render_with`. A figure hit-tests this against its own
    /// [`crate::coord::PlotArea`] internally (via
    /// [`crate::interact::hit::hit_zone`]) — a position outside that
    /// figure's own plot rect is simply ignored, so fanning the same
    /// value out to every panel in a multi-figure layout is harmless.
    pub hover_px: Option<(f64, f64)>,
    /// Domain-space `(min, max)` interval — e.g. the committed value from
    /// [`crate::interact::brush::BrushState::domain_interval`] on
    /// whichever figure owns the drag. A figure that supports
    /// brush-linked highlighting re-derives ITS OWN screen position for
    /// this interval through its own scale — figures may live in entirely
    /// different domains (see the linked-brush demo), so the dragging
    /// figure's raw pixels are never threaded through directly.
    pub brush: Option<(f64, f64)>,
    /// Optional persistent hover/selection set (click-to-pin, future
    /// cross-figure linking). `None` for figures that don't use it.
    pub focus: Option<&'a FocusSet>,
}

/// Shared title-bar draw shared by every figure in this module — top-left,
/// one line, `theme.label_font`/`theme.label_color`.
pub(crate) fn draw_title(ctx: &mut dyn RenderContext, rect: Rect, title: &str, theme: &FigureTheme) {
    ctx.set_font(&theme.label_font);
    ctx.set_fill_color(&theme.label_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);
    ctx.fill_text(title, rect.x + 8.0, rect.y + 6.0);
}
