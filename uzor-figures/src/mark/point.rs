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

/// Per-point-radius counterpart of [`draw_points`] — one filled circle per
/// `(x, y, radius)` domain triple, radius already resolved by the caller
/// (e.g. [`crate::figure::ScatterFigure`]'s value-mapped marker sizing).
/// [`draw_points`] takes ONE shared radius for the whole batch; this
/// function exists because `CircleBatch` itself already carries a
/// per-circle `r` field — [`draw_points`] just never threaded a
/// per-point value through to it.
pub fn draw_points_sized(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    x: &dyn Scale,
    y: &dyn Scale,
    points: &[(f64, f64, f64)],
    style: &MarkStyle,
) {
    if points.is_empty() {
        return;
    }
    let circles: Vec<CircleBatch> =
        points.iter().map(|&(px, py, r)| CircleBatch { cx: area.x(x, px), cy: area.y(y, py), r }).collect();
    ctx.set_global_alpha(style.fill_alpha);
    ctx.draw_circle_batch(&circles, &style.color);
    ctx.set_global_alpha(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;
    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    #[test]
    fn draw_points_sized_renders_without_panicking_for_varied_radii() {
        let spec = ExportSpec { width_px: 200, height_px: 100, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            let area = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
            let x = LinearScale::new(0.0, 10.0);
            let y = LinearScale::new(0.0, 10.0);
            let style = MarkStyle::default();
            draw_points_sized(ctx, &area, &x, &y, &[(1.0, 1.0, 2.0), (5.0, 5.0, 8.0), (9.0, 2.0, 0.5)], &style);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn draw_points_sized_empty_points_is_a_no_op() {
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            let area = PlotArea::new(Rect::new(0.0, 0.0, 10.0, 10.0));
            let x = LinearScale::new(0.0, 10.0);
            let y = LinearScale::new(0.0, 10.0);
            draw_points_sized(ctx, &area, &x, &y, &[], &MarkStyle::default());
        });
        assert!(result.is_ok());
    }
}
