//! `draw_area` — filled polygon under a line series, down to the y
//! baseline (domain value `0.0`, clamped into `y`'s domain).

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::scale::Scale;

use super::MarkStyle;

/// Fill the region between the `(x, y)` polyline and the y baseline.
///
/// Fewer than 2 points draw nothing (mirrors [`super::line::draw_polyline`]
/// — an area needs at least a line segment to close a polygon around).
pub fn draw_area(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    x: &dyn Scale,
    y: &dyn Scale,
    points: &[(f64, f64)],
    style: &MarkStyle,
) {
    if points.len() < 2 {
        return;
    }

    let (y_min, y_max) = y.domain();
    let baseline_value = 0.0_f64.clamp(y_min.min(y_max), y_min.max(y_max));
    let baseline_px = area.y(y, baseline_value);

    let first_x_px = area.x(x, points[0].0);
    let first_y_px = area.y(y, points[0].1);
    let last_x_px = area.x(x, points[points.len() - 1].0);

    ctx.set_fill_color(&style.color);
    ctx.set_global_alpha(style.fill_alpha);
    ctx.begin_path();
    ctx.move_to(first_x_px, baseline_px);
    ctx.line_to(first_x_px, first_y_px);
    for &(px, py) in &points[1..] {
        ctx.line_to(area.x(x, px), area.y(y, py));
    }
    ctx.line_to(last_x_px, baseline_px);
    ctx.close_path();
    ctx.fill();
    ctx.set_global_alpha(1.0);
}
