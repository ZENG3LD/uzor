//! Composed V1 figures — thin, in-crate compositions of scale + coord +
//! mark + guide. Each figure is a pure `render(&self, ctx, rect, theme)`
//! over borrowed data; no interaction, no retained state. A real
//! registry/IR (mlc's `ChartTypeDef`+`DrawOps` two-table pattern) is a
//! later milestone — see the crate-root docs.

pub mod bars;
pub mod curve;
pub mod histogram;

pub use bars::BarFigure;
pub use curve::CurveFigure;
pub use histogram::{bin, Bin, HistogramFigure};

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::theme::VizTheme;

/// Shared title-bar draw shared by every figure in this module — top-left,
/// one line, `theme.label_font`/`theme.label_color`.
pub(crate) fn draw_title(ctx: &mut dyn RenderContext, rect: Rect, title: &str, theme: &VizTheme) {
    ctx.set_font(&theme.label_font);
    ctx.set_fill_color(&theme.label_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);
    ctx.fill_text(title, rect.x + 8.0, rect.y + 6.0);
}
