//! `draw_polyline` — a line series through `(x, y)` domain points.

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::scale::Scale;

use super::{gap_runs, MarkStyle};

/// Stroke a polyline through `points` (domain-space `(x, y)` pairs),
/// mapped through `x`/`y` and `area` — the same transform every other
/// mark in this crate uses.
///
/// `points` is first split into runs per `style.gap_policy` (see
/// [`super::GapPolicy`]'s own docs — the default, [`super::GapPolicy::
/// Break`], strokes each non-finite-free run as its own separate segment,
/// a visible gap exactly where a sample is missing). Each run shorter
/// than 2 points draws nothing (a single point has no line to draw — use
/// [`super::point::draw_points`] for that case). `style.line_cap`/
/// `line_join` are applied before every stroke.
pub fn draw_polyline(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    x: &dyn Scale,
    y: &dyn Scale,
    points: &[(f64, f64)],
    style: &MarkStyle,
) {
    ctx.set_line_cap(style.line_cap.as_str());
    ctx.set_line_join(style.line_join.as_str());
    for run in gap_runs(points, style.gap_policy) {
        if run.len() < 2 {
            continue;
        }
        let screen: Vec<(f64, f64)> = run.iter().map(|&(px, py)| (area.x(x, px), area.y(y, py))).collect();
        ctx.stroke_polyline(&screen, &style.color, style.stroke_width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::GapPolicy;
    use crate::scale::LinearScale;
    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0))
    }

    fn scales() -> (LinearScale, LinearScale) {
        (LinearScale::new(0.0, 10.0), LinearScale::new(0.0, 10.0))
    }

    fn render_ok(points: &[(f64, f64)], style: &MarkStyle) -> bool {
        let spec = ExportSpec { width_px: 200, height_px: 100, dpr: 1.0, background: None };
        render_to_png(&spec, |ctx| {
            let (x, y) = scales();
            draw_polyline(ctx, &area(), &x, &y, points, style);
        })
        .is_ok()
    }

    #[test]
    fn nan_in_the_middle_renders_without_panicking_under_every_gap_policy() {
        let points = vec![(0.0, 0.0), (2.0, 2.0), (f64::NAN, f64::NAN), (6.0, 6.0), (8.0, 8.0)];
        for policy in [GapPolicy::Break, GapPolicy::Connect, GapPolicy::Skip] {
            let style = MarkStyle { gap_policy: policy, ..Default::default() };
            assert!(render_ok(&points, &style), "{policy:?} must render a NaN-in-the-middle series without panicking");
        }
    }

    #[test]
    fn nan_at_the_ends_renders_without_panicking_under_every_gap_policy() {
        let points = vec![(f64::NAN, 0.0), (2.0, 2.0), (4.0, 4.0), (6.0, 6.0), (f64::NAN, f64::NAN)];
        for policy in [GapPolicy::Break, GapPolicy::Connect, GapPolicy::Skip] {
            let style = MarkStyle { gap_policy: policy, ..Default::default() };
            assert!(render_ok(&points, &style), "{policy:?} must render a leading/trailing-NaN series without panicking");
        }
    }

    #[test]
    fn all_nan_renders_without_panicking_under_every_gap_policy() {
        let points = vec![(f64::NAN, f64::NAN); 5];
        for policy in [GapPolicy::Break, GapPolicy::Connect, GapPolicy::Skip] {
            let style = MarkStyle { gap_policy: policy, ..Default::default() };
            assert!(render_ok(&points, &style), "{policy:?} must render an all-NaN series without panicking");
        }
    }

    #[test]
    fn single_valid_point_among_gaps_renders_without_panicking_under_every_gap_policy() {
        let points = vec![(f64::NAN, f64::NAN), (5.0, 5.0), (f64::NAN, f64::NAN)];
        for policy in [GapPolicy::Break, GapPolicy::Connect, GapPolicy::Skip] {
            let style = MarkStyle { gap_policy: policy, ..Default::default() };
            assert!(render_ok(&points, &style), "{policy:?} must render a single-valid-point series without panicking");
        }
    }

    #[test]
    fn break_and_connect_render_differently_across_a_real_gap() {
        // Break draws 2 disconnected segments; Connect bridges them into
        // one continuous stroke — the two must NOT produce byte-identical
        // output for a fixture with a real interior gap.
        let points = vec![(0.0, 0.0), (2.0, 8.0), (f64::NAN, f64::NAN), (6.0, 2.0), (8.0, 9.0)];
        let spec = ExportSpec { width_px: 200, height_px: 100, dpr: 1.0, background: None };
        let (x, y) = scales();
        let break_png = render_to_png(&spec, |ctx| {
            let style = MarkStyle { gap_policy: GapPolicy::Break, ..Default::default() };
            draw_polyline(ctx, &area(), &x, &y, &points, &style);
        })
        .expect("break render");
        let connect_png = render_to_png(&spec, |ctx| {
            let style = MarkStyle { gap_policy: GapPolicy::Connect, ..Default::default() };
            draw_polyline(ctx, &area(), &x, &y, &points, &style);
        })
        .expect("connect render");
        assert_ne!(break_png, connect_png, "Break (visible gap) must render differently from Connect (bridged gap)");
    }

    #[test]
    fn skip_draws_nothing_when_any_point_is_non_finite() {
        let with_gap = vec![(0.0, 0.0), (2.0, 2.0), (f64::NAN, f64::NAN), (6.0, 6.0)];
        let spec = ExportSpec { width_px: 200, height_px: 100, dpr: 1.0, background: None };
        let (x, y) = scales();
        let skip_png = render_to_png(&spec, |ctx| {
            let style = MarkStyle { gap_policy: GapPolicy::Skip, ..Default::default() };
            draw_polyline(ctx, &area(), &x, &y, &with_gap, &style);
        })
        .expect("skip render");
        let blank_png = render_to_png(&spec, |ctx| {
            let style = MarkStyle::default();
            draw_polyline(ctx, &area(), &x, &y, &[], &style);
        })
        .expect("blank render");
        assert_eq!(skip_png, blank_png, "GapPolicy::Skip must draw nothing at all when the series has any gap");
    }

    #[test]
    fn fewer_than_two_points_draws_nothing() {
        let spec = ExportSpec { width_px: 50, height_px: 50, dpr: 1.0, background: None };
        let (x, y) = scales();
        let one_point = render_to_png(&spec, |ctx| draw_polyline(ctx, &area(), &x, &y, &[(1.0, 1.0)], &MarkStyle::default())).expect("render");
        let empty = render_to_png(&spec, |ctx| draw_polyline(ctx, &area(), &x, &y, &[], &MarkStyle::default())).expect("render");
        assert_eq!(one_point, empty, "a single point must draw nothing, same as an empty slice");
    }
}
