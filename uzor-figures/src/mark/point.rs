//! `draw_points` — a scatter of circles at `(x, y)` domain points, via
//! `BatchPainter::draw_circle_batch` (one draw call per style/color).

use uzor::render::{CircleBatch, RenderContext};

use crate::coord::PlotArea;
use crate::scale::Scale;

use super::MarkStyle;

/// Draw one filled circle of `radius` px per point.
pub fn draw_points(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    x: &dyn Scale,
    y: &dyn Scale,
    points: &[(f64, f64)],
    radius: f64,
    style: &MarkStyle,
) {
    if points.is_empty() {
        return;
    }
    let circles: Vec<CircleBatch> =
        points.iter().map(|&(px, py)| CircleBatch { cx: area.x(x, px), cy: area.y(y, py), r: radius }).collect();
    ctx.set_global_alpha(style.fill_alpha);
    ctx.draw_circle_batch(&circles, &style.color);
    ctx.set_global_alpha(1.0);
}
