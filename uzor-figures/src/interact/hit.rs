//! Hit-testing — every function here resolves a screen pixel through
//! [`PlotArea`] (design law #1: one transform for render AND hit-test).
//! Generalized from mlc's `HitResult`/`ChartHitTester`
//! (`engine/input/handler/traits.rs:32-236`): the dozen chart-specific
//! zones (sub-panes, pane separators, drawing primitives/control points,
//! toolbar, scale-corner) collapse into the four zones a composed-figure
//! engine actually has — a rectangular plot area with an x-axis strip
//! below it and a y-axis strip to its left.

use crate::coord::PlotArea;
use crate::scale::{BandScale, Scale};

/// Which region of a figure's rendered area a screen pixel falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    /// Inside the plot rect itself (marks live here).
    Plot,
    /// Below the plot rect, within its horizontal extent — the x-axis
    /// tick/label strip.
    AxisX,
    /// Left of the plot rect, within its vertical extent — the y-axis
    /// tick/label strip.
    AxisY,
    /// Anywhere else (title bar, outer margins, outside the figure).
    Outside,
}

/// Classify `(x, y)` (screen px) against `area`'s plot rect. Corners and
/// edges of the rect itself count as [`HitZone::Plot`] (inclusive bounds)
/// — matches [`PlotArea::x`]/[`PlotArea::y`], which place domain extremes
/// exactly on the rect's edges.
pub fn hit_zone(area: &PlotArea, x: f64, y: f64) -> HitZone {
    let r = area.rect;
    let within_x = x >= r.x && x <= r.right();
    let within_y = y >= r.y && y <= r.bottom();
    match (within_x, within_y) {
        (true, true) => HitZone::Plot,
        (true, false) if y > r.bottom() => HitZone::AxisX,
        (false, true) if x < r.x => HitZone::AxisY,
        _ => HitZone::Outside,
    }
}

/// Index of the `points` entry whose mapped screen-x is nearest `px` — for
/// curve hover (crosshair snapping to the nearest data point along X).
///
/// `yscale` is applied through the exact same [`PlotArea::y`] transform
/// every mark uses (design law #1): a candidate whose mapped y falls
/// outside the plot's own visible range is skipped — it can't be a fair
/// "nearest visible point" if it wouldn't actually be drawn on screen.
/// Returns `None` for an empty `points` (or if every point is filtered
/// out this way).
pub fn nearest_point_x(area: &PlotArea, xscale: &dyn Scale, yscale: &dyn Scale, points: &[(f64, f64)], px: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, &(dx, dy)) in points.iter().enumerate() {
        let screen_y = area.y(yscale, dy);
        if screen_y < area.rect.y - 1e-6 || screen_y > area.rect.bottom() + 1e-6 {
            continue;
        }
        let screen_x = area.x(xscale, dx);
        let dist = (screen_x - px).abs();
        if best.map_or(true, |(_, best_dist)| dist < best_dist) {
            best = Some((i, dist));
        }
    }
    best.map(|(i, _)| i)
}

/// Index of the band whose `[left, right)` pixel extent (via
/// [`PlotArea::x_band`]) contains `px` — for bar/histogram hover. Returns
/// `None` when `px` falls in an inter-band padding gap or outside every
/// band (an empty `band` trivially returns `None`).
pub fn bar_index_at(area: &PlotArea, band: &BandScale, px: f64) -> Option<usize> {
    (0..band.len()).find(|&i| {
        let (x0, x1) = area.x_band(band, i);
        px >= x0 && px <= x1
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;
    use uzor::types::Rect;

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(50.0, 50.0, 100.0, 80.0))
    }

    #[test]
    fn hit_zone_corners_are_inclusive_plot() {
        let a = area();
        assert_eq!(hit_zone(&a, 50.0, 50.0), HitZone::Plot); // top-left
        assert_eq!(hit_zone(&a, 150.0, 50.0), HitZone::Plot); // top-right
        assert_eq!(hit_zone(&a, 50.0, 130.0), HitZone::Plot); // bottom-left
        assert_eq!(hit_zone(&a, 150.0, 130.0), HitZone::Plot); // bottom-right
    }

    #[test]
    fn hit_zone_below_and_left_of_plot_are_axis_strips() {
        let a = area();
        assert_eq!(hit_zone(&a, 75.0, 140.0), HitZone::AxisX);
        assert_eq!(hit_zone(&a, 10.0, 75.0), HitZone::AxisY);
    }

    #[test]
    fn hit_zone_elsewhere_is_outside() {
        let a = area();
        assert_eq!(hit_zone(&a, 10.0, 10.0), HitZone::Outside); // above-left
        assert_eq!(hit_zone(&a, 75.0, 10.0), HitZone::Outside); // straight above
        assert_eq!(hit_zone(&a, 200.0, 75.0), HitZone::Outside); // right of plot
    }

    #[test]
    fn nearest_point_x_picks_exact_index() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        let points = [(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)];
        let px = a.x(&xscale, 10.0);
        assert_eq!(nearest_point_x(&a, &xscale, &yscale, &points, px), Some(1));
    }

    #[test]
    fn nearest_point_x_picks_closer_of_two_at_a_mid_position() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        let points = [(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)];
        // Slightly closer to domain x=10 (index 1) than to x=0 (index 0).
        let px = a.x(&xscale, 6.0);
        assert_eq!(nearest_point_x(&a, &xscale, &yscale, &points, px), Some(1));
    }

    #[test]
    fn nearest_point_x_empty_points_is_none() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        assert_eq!(nearest_point_x(&a, &xscale, &yscale, &[], 50.0), None);
    }

    #[test]
    fn bar_index_at_inside_and_outside_bands() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 300.0, 100.0));
        let band = BandScale::new(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()], 0.3);
        // Center of band 1 is inside band 1.
        let (x0, x1) = a.x_band(&band, 1);
        let center = (x0 + x1) / 2.0;
        assert_eq!(bar_index_at(&a, &band, center), Some(1));
        // Just left of band 1's left edge falls in the padding gap.
        assert_eq!(bar_index_at(&a, &band, x0 - 1.0), None);
        // Off the whole scale entirely.
        assert_eq!(bar_index_at(&a, &band, 10_000.0), None);
    }

    #[test]
    fn bar_index_at_empty_band_is_always_none() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 300.0, 100.0));
        let band = BandScale::new(Vec::new(), 0.1);
        assert_eq!(bar_index_at(&a, &band, 50.0), None);
    }
}
