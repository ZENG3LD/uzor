//! Hit-testing — every function here resolves a screen pixel through
//! [`PlotArea`] (design law #1: one transform for render AND hit-test).
//! Generalized from mlc's `HitResult`/`ChartHitTester`
//! (`engine/input/handler/traits.rs:32-236`): the dozen chart-specific
//! zones (sub-panes, pane separators, drawing primitives/control points,
//! toolbar, scale-corner) collapse into the four zones a composed-figure
//! engine actually has — a rectangular plot area with an x-axis strip
//! below it and a y-axis strip to its left.
//!
//! [`nearest_point_x_multi`]/[`bar_series_at`]/[`stacked_series_at`] are
//! the multi-series extensions (`crate::figure::CurveFigure::with_series`/
//! `crate::figure::BarFigure::with_series`) — a hover now resolves not just
//! WHICH category/point but also WHICH series, through the exact same
//! [`PlotArea`]-driven geometry the single-series functions above already
//! established.

use crate::coord::PlotArea;
use crate::mark::rect::sub_band_range;
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
        if !dx.is_finite() || !dy.is_finite() {
            continue; // a missing (NaN/±inf) sample was never actually drawn — see mark::GapPolicy
        }
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

/// 2D counterpart of [`nearest_point_x`] — the index of the `points` entry
/// whose SCREEN position (both x AND y, not just x) is nearest `(px, py)`.
/// [`nearest_point_x`] snaps along X only, which is correct for a curve
/// (a line has exactly one point per X, so the crosshair only ever needs
/// to disambiguate horizontally) — a scatter cloud has no such ordering
/// (many points can share nearly the same X at very different Y), so
/// [`crate::figure::ScatterFigure`]'s own hover needs a real 2D nearest-
/// neighbor instead. No distance cap here (matches [`nearest_point_x`]'s
/// own unbounded-nearest convention) — a caller wanting "only within this
/// marker's own visible radius" (which [`ScatterFigure`] does want) applies
/// that check itself against the returned index's own resolved screen
/// position, the same "hit-test resolves the candidate, the caller decides
/// whether it's close enough to react to" split [`hit_zone`] already uses
/// one level up. Returns `None` for an empty `points`.
///
/// [`ScatterFigure`]: crate::figure::ScatterFigure
pub fn nearest_point_xy(area: &PlotArea, xscale: &dyn Scale, yscale: &dyn Scale, points: &[(f64, f64)], px: f64, py: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, &(dx, dy)) in points.iter().enumerate() {
        if !dx.is_finite() || !dy.is_finite() {
            continue; // a missing (NaN/±inf) sample was never actually drawn — see mark::GapPolicy
        }
        let sx = area.x(xscale, dx);
        let sy = area.y(yscale, dy);
        let dist = ((sx - px).powi(2) + (sy - py).powi(2)).sqrt();
        if best.map_or(true, |(_, best_dist)| dist < best_dist) {
            best = Some((i, dist));
        }
    }
    best.map(|(i, _)| i)
}

/// Multi-series counterpart of [`nearest_point_x`] — the `(series_index,
/// point_index)` whose mapped screen-x is nearest `px`, picked GLOBALLY
/// across every series (never per-series-then-merged — a point in a
/// "later" series can still win over a farther point in the first one).
/// Same y-visibility filter as [`nearest_point_x`]. Returns `None` when
/// every series is empty (or every point is filtered out by the
/// y-visibility check).
pub fn nearest_point_x_multi(area: &PlotArea, xscale: &dyn Scale, yscale: &dyn Scale, series: &[&[(f64, f64)]], px: f64) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, f64)> = None;
    for (si, points) in series.iter().enumerate() {
        for (pi, &(dx, dy)) in points.iter().enumerate() {
            if !dx.is_finite() || !dy.is_finite() {
                continue; // a missing (NaN/±inf) sample was never actually drawn — see mark::GapPolicy
            }
            let screen_y = area.y(yscale, dy);
            if screen_y < area.rect.y - 1e-6 || screen_y > area.rect.bottom() + 1e-6 {
                continue;
            }
            let screen_x = area.x(xscale, dx);
            let dist = (screen_x - px).abs();
            if best.map_or(true, |(_, _, best_dist)| dist < best_dist) {
                best = Some((si, pi, dist));
            }
        }
    }
    best.map(|(si, pi, _)| (si, pi))
}

/// Which grouped-mode series sub-band (of `series_count`, within one
/// category band's own pixel extent `[x0, x1]`) contains `px` — reuses
/// [`sub_band_range`], the SAME nested band geometry
/// [`crate::mark::rect::draw_bars_grouped`] paints with (design law #1:
/// paint and hit-test share one geometry formula).
pub fn bar_series_at(x0: f64, x1: f64, series_count: usize, px: f64) -> Option<usize> {
    (0..series_count).find(|&i| {
        let (sx0, sx1) = sub_band_range(x0, x1, series_count, i);
        px >= sx0 && px <= sx1
    })
}

/// Which stacked-mode segment contains screen-y `py`, given each series'
/// own already-resolved `(top_px, bottom_px)` pixel pair for one category
/// (in series order) — e.g. [`crate::figure::BarFigure`]'s own
/// `stacked_segments_px`, which walks the EXACT SAME cumulative running
/// sum [`crate::mark::rect::draw_bars_stacked`] paints with. Returns
/// `None` when `py` falls outside every segment (e.g. the padding above/
/// below a short column).
pub fn stacked_series_at(segments: &[(f64, f64)], py: f64) -> Option<usize> {
    segments.iter().position(|&(top, bottom)| {
        let (lo, hi) = (top.min(bottom), top.max(bottom));
        py >= lo && py <= hi
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

    #[test]
    fn nearest_point_x_multi_picks_the_globally_nearest_point_across_series() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        let series_a: Vec<(f64, f64)> = vec![(0.0, 0.0), (20.0, 0.0)];
        let series_b: Vec<(f64, f64)> = vec![(10.0, 5.0)];
        let series: Vec<&[(f64, f64)]> = vec![&series_a, &series_b];
        // domain x=10 (series 1's only point) is nearest to px at domain x=10.
        let px = a.x(&xscale, 10.0);
        assert_eq!(nearest_point_x_multi(&a, &xscale, &yscale, &series, px), Some((1, 0)));
    }

    #[test]
    fn nearest_point_x_multi_empty_series_is_none() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        assert_eq!(nearest_point_x_multi(&a, &xscale, &yscale, &[], 50.0), None);
        let empty: Vec<(f64, f64)> = Vec::new();
        assert_eq!(nearest_point_x_multi(&a, &xscale, &yscale, &[&empty], 50.0), None);
    }

    #[test]
    fn nearest_point_xy_picks_the_true_2d_nearest_not_just_nearest_by_x() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        // Two points share nearly the same X (10 and 11) but very
        // different Y (0 and 9) — the true nearest 2D neighbor to
        // (x=10.5, y=9) must be the point with the CLOSE Y, even though a
        // 1D-by-X-only test (`nearest_point_x`) would treat them as
        // roughly equidistant.
        let points = [(10.0, 0.0), (11.0, 9.0)];
        let px = a.x(&xscale, 10.5);
        let py = a.y(&yscale, 9.0);
        assert_eq!(nearest_point_xy(&a, &xscale, &yscale, &points, px, py), Some(1));
    }

    #[test]
    fn nearest_point_xy_empty_points_is_none() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        assert_eq!(nearest_point_xy(&a, &xscale, &yscale, &[], 50.0, 50.0), None);
    }

    #[test]
    fn nearest_point_x_skips_a_non_finite_sample_instead_of_corrupting_the_distance_comparison() {
        // A NaN sample first in the array must never "win" the greedy
        // `best.map_or(true, ...)` comparison (every subsequent `dist <
        // NaN` compares false, which would otherwise pin the NaN
        // candidate forever) — the real bug B1 names.
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        let points = [(f64::NAN, f64::NAN), (10.0, 5.0), (0.0, 0.0)];
        let px = a.x(&xscale, 10.0);
        assert_eq!(nearest_point_x(&a, &xscale, &yscale, &points, px), Some(1), "the finite nearest point must win, never the leading NaN");
    }

    #[test]
    fn nearest_point_xy_skips_non_finite_samples() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        let points = [(f64::NAN, f64::NAN), (10.0, 5.0)];
        let px = a.x(&xscale, 10.0);
        let py = a.y(&yscale, 5.0);
        assert_eq!(nearest_point_xy(&a, &xscale, &yscale, &points, px, py), Some(1));
    }

    #[test]
    fn nearest_point_x_multi_skips_non_finite_samples_across_series() {
        let a = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let xscale = LinearScale::new(0.0, 20.0);
        let yscale = LinearScale::new(0.0, 10.0);
        let series_a: Vec<(f64, f64)> = vec![(f64::NAN, f64::NAN), (0.0, 0.0)];
        let series_b: Vec<(f64, f64)> = vec![(10.0, 5.0)];
        let series: Vec<&[(f64, f64)]> = vec![&series_a, &series_b];
        let px = a.x(&xscale, 10.0);
        assert_eq!(nearest_point_x_multi(&a, &xscale, &yscale, &series, px), Some((1, 0)));
    }

    #[test]
    fn bar_series_at_resolves_the_correct_sub_band_and_none_in_the_gap() {
        let (x0, x1) = (0.0, 300.0);
        let n = 3;
        // Sub-band 1's center must resolve to series index 1.
        let (sx0, sx1) = sub_band_range(x0, x1, n, 1);
        let center = (sx0 + sx1) / 2.0;
        assert_eq!(bar_series_at(x0, x1, n, center), Some(1));
        // A point between sub-bands (in the padding gap) resolves to none.
        let (_, prev_end) = sub_band_range(x0, x1, n, 0);
        let (next_start, _) = sub_band_range(x0, x1, n, 1);
        if next_start > prev_end {
            let gap_mid = (prev_end + next_start) / 2.0;
            assert_eq!(bar_series_at(x0, x1, n, gap_mid), None);
        }
    }

    #[test]
    fn stacked_series_at_resolves_the_segment_containing_py_and_none_outside_all() {
        // Segment pixel pairs in series order — top < bottom in screen
        // space (smaller y = higher on screen), matching how `y` maps a
        // larger domain value to a smaller pixel.
        let segments = [(80.0, 100.0), (50.0, 80.0), (20.0, 50.0)];
        assert_eq!(stacked_series_at(&segments, 90.0), Some(0));
        assert_eq!(stacked_series_at(&segments, 65.0), Some(1));
        assert_eq!(stacked_series_at(&segments, 30.0), Some(2));
        assert_eq!(stacked_series_at(&segments, 10.0), None);
    }
}
