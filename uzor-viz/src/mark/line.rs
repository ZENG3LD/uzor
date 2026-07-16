//! `draw_polyline` — a line series through `(x, y)` domain points.

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::scale::Scale;

use super::MarkStyle;

/// Stroke a polyline through `points` (domain-space `(x, y)` pairs),
/// mapped through `x`/`y` and `area` — the same transform every other
/// mark in this crate uses.
///
/// Fewer than 2 points draw nothing (a single point has no line to draw —
/// use [`super::point::draw_points`] for that case).
pub fn draw_polyline(
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
    let screen: Vec<(f64, f64)> = points.iter().map(|&(px, py)| (area.x(x, px), area.y(y, py))).collect();
    ctx.stroke_polyline(&screen, &style.color, style.stroke_width);
}
