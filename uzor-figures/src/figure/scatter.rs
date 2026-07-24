//! `ScatterFigure` — an x/y point cloud: continuous X (or a caller-
//! supplied override, e.g. [`crate::scale::TimeScale`]) and Y linear
//! domains, point markers sized either fixed or value-mapped
//! ([`PointRadius`]), deterministic uniform-stride thinning at high N
//! ([`uniform_thin_indices`] — see its own docs for why this is NOT LTTB),
//! a nearest-marker hover tooltip, and an opt-in `annotations` overlay
//! ([`crate::guide::annotation`]).
//!
//! ## Why thinning here is straight subsampling, not LTTB
//!
//! [`crate::transform::lttb`] (Largest-Triangle-Three-Buckets) is built on
//! two assumptions that hold for a TIME SERIES / line and do NOT hold for
//! an unordered scatter cloud:
//!
//! 1. **Sequential order carries meaning.** LTTB compares each candidate
//!    point's triangle area against its immediate NEIGHBORS in the array
//!    (the previously-chosen point and the next bucket's centroid) — this
//!    is a proxy for "how much does removing this point change the LINE'S
//!    visual shape." A scatter cloud draws no line between points, and
//!    array-adjacent points are not assumed to be spatially adjacent —
//!    there is no "shape" a triangle-area heuristic is preserving.
//! 2. **The first and last points are privileged anchors.** For a time
//!    series, index 0 and index N-1 are genuinely special (the start/end
//!    of the series). For an arbitrary point cloud, `points[0]` and
//!    `points[n-1]` are just whichever two the caller happened to push
//!    first/last — anchoring on them introduces an arbitrary bias LTTB
//!    was never designed to justify.
//!
//! [`uniform_thin_indices`] instead picks `max_points` INDICES evenly
//! spaced across `0..n` (still keeping the first and last index, for the
//! same "never drop the caller's own edge data" reason every other
//! downsampler in this crate keeps its edges) — a fair, deterministic,
//! `O(max_points)` sample of the array's own order, with no claim about
//! WHICH points are visually "more important." This preserves the overall
//! spatial DENSITY distribution of the cloud (a dense region keeps
//! proportionally more samples than a sparse one) without LTTB's
//! line-shape bias, which is the right property for an unordered cloud.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::guide::annotation::{draw_annotation_overlays, draw_annotation_underlays, Annotation};
use crate::guide::axis::AxisTickWeightStyle;
use crate::guide::grid::GridTickWeightStyle;
use crate::guide::{axis, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::mark::point::draw_points_sized;
use crate::mark::MarkStyle;
use crate::scale::linear::{format_value, nice_step};
use crate::scale::{LinearScale, Scale};
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 56.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_X_TICKS: usize = 6;
const TARGET_Y_TICKS: usize = 5;
const DEFAULT_POINT_RADIUS: f64 = 3.5;
const DEFAULT_FILL_ALPHA: f64 = 0.85;
/// Extra px tolerance (beyond a point's own drawn radius) hover still
/// snaps within — a scatter marker has a real, bounded visible footprint
/// (unlike a curve's own always-nearest hover), so hover only activates
/// near an actual drawn point, never anywhere in the empty plot area.
const HOVER_TOLERANCE_PX: f64 = 4.0;
const HOVER_HIGHLIGHT_ALPHA: f64 = 0.35;
const HOVER_HIGHLIGHT_EXTRA_RADIUS: f64 = 2.0;

/// One data point — `value` optionally drives per-point radius under
/// [`PointRadius::ValueMapped`] (ignored under [`PointRadius::Fixed`]).
#[derive(Debug, Clone, Copy)]
pub struct ScatterPoint {
    pub x: f64,
    pub y: f64,
    pub value: Option<f64>,
}

impl ScatterPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y, value: None }
    }

    pub fn with_value(x: f64, y: f64, value: f64) -> Self {
        Self { x, y, value: Some(value) }
    }
}

/// How a point's own drawn radius is resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointRadius {
    /// Every point draws at the same radius, regardless of its own
    /// [`ScatterPoint::value`].
    Fixed(f64),
    /// A point's radius is linearly interpolated between `min_radius`
    /// (at the lowest [`ScatterPoint::value`] across the WHOLE point set)
    /// and `max_radius` (at the highest). A point with `value: None`
    /// falls back to `min_radius`.
    ValueMapped { min_radius: f64, max_radius: f64 },
}

impl Default for PointRadius {
    fn default() -> Self {
        PointRadius::Fixed(DEFAULT_POINT_RADIUS)
    }
}

/// Deterministic, unbiased index thinning — see the module docs for why
/// this is straight subsampling, not LTTB. `max_points >= n` (or `n == 0`)
/// is identity (every index). Always keeps index `0` and `n - 1` (when
/// `max_points >= 2`). Strictly ascending, no duplicate indices — rounding
/// collisions between adjacent target positions simply COLLAPSE (the
/// output may be shorter than `max_points`, never longer, never a
/// duplicate point).
pub fn uniform_thin_indices(n: usize, max_points: usize) -> Vec<usize> {
    if n == 0 || max_points == 0 {
        return Vec::new();
    }
    if max_points >= n {
        return (0..n).collect();
    }
    if max_points == 1 {
        return vec![0];
    }

    let mut indices = Vec::with_capacity(max_points);
    let mut last: Option<usize> = None;
    for i in 0..max_points {
        let t = i as f64 / (max_points - 1) as f64;
        let idx = ((t * (n - 1) as f64).round() as usize).min(n - 1);
        if last != Some(idx) {
            indices.push(idx);
            last = Some(idx);
        }
    }
    indices
}

/// An x/y point cloud over one or more [`ScatterPoint`]s — see the module
/// docs.
pub struct ScatterFigure {
    pub points: Vec<ScatterPoint>,
    pub title: Option<String>,
    radius: PointRadius,
    x_scale_override: Option<Box<dyn Scale>>,
    thin_max: Option<usize>,
    annotations: Vec<Annotation>,
}

impl ScatterFigure {
    pub fn new(points: Vec<ScatterPoint>) -> Self {
        Self { points, title: None, radius: PointRadius::default(), x_scale_override: None, thin_max: None, annotations: Vec::new() }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_radius(mut self, radius: PointRadius) -> Self {
        self.radius = radius;
        self
    }

    /// Supply a custom X scale (e.g. a [`crate::scale::TimeScale`] over
    /// UTC-second x values) instead of the auto-computed nice
    /// [`LinearScale`] domain — same builder shape as
    /// [`crate::figure::CurveFigure::with_x_scale`].
    pub fn with_x_scale(mut self, scale: impl Scale + 'static) -> Self {
        self.x_scale_override = Some(Box::new(scale));
        self
    }

    /// Thin to at most `max_points` via [`uniform_thin_indices`], applied
    /// at RENDER time only — same "auto-computed domain always folds over
    /// the FULL raw point set, downsampling never reflows layout/axes, and
    /// hit-testing resolves against the SAME thinned set the render pass
    /// draws" one-transform law [`crate::figure::CurveFigure::
    /// with_downsample`]'s own docs establish, applied here to an
    /// unordered cloud instead of a line.
    pub fn with_thinning(mut self, max_points: usize) -> Self {
        self.thin_max = Some(max_points);
        self
    }

    /// Reference lines/bands/callouts drawn over this figure's marks — see
    /// [`crate::guide::annotation`].
    pub fn with_annotations(mut self, annotations: Vec<Annotation>) -> Self {
        self.annotations = annotations;
        self
    }

    fn base_plot_rect(&self, rect: Rect) -> Rect {
        let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
        Rect::new(
            rect.x + MARGIN_LEFT,
            rect.y + title_h,
            (rect.width - MARGIN_LEFT - MARGIN_RIGHT).max(0.0),
            (rect.height - title_h - MARGIN_BOTTOM).max(0.0),
        )
    }

    /// This figure's plot-area transform for `rect` — same reason as every
    /// other figure's `plot_area` accessor (external hover routing needs
    /// the EXACT transform this figure renders with).
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.base_plot_rect(rect))
    }

    /// This figure's auto-computed nice-linear X domain (the FULL point
    /// set's own extent, regardless of [`ScatterFigure::with_thinning`]),
    /// `None` for fewer than 2 points. Exposed for the same reason as
    /// [`crate::figure::CurveFigure::x_scale`].
    ///
    /// **Does not reflect [`ScatterFigure::with_x_scale`]** — same
    /// documented divergence [`crate::figure::CurveFigure::x_scale`]'s own
    /// docs carry.
    pub fn x_scale(&self) -> Option<LinearScale> {
        if self.points.len() < 2 {
            return None;
        }
        let (mn, mx) = self.points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), p| (mn.min(p.x), mx.max(p.x)));
        Some(LinearScale::nice(mn, mx, TARGET_X_TICKS))
    }

    /// This figure's auto-computed nice-linear Y domain. **Never forces a
    /// zero baseline** (unlike [`crate::figure::BarFigure`]/
    /// [`crate::figure::CurveFigure`]'s own Y domains, which always include
    /// `0.0`) — a bar/filled-area's height encodes AREA FROM ZERO, a
    /// scatter marker's position does not, so this figure fits the data's
    /// own extent only, matching the standard scatter-plot convention
    /// (matplotlib/ggplot2 auto-scale to data, never zero-anchored).
    fn y_scale(&self) -> Option<LinearScale> {
        if self.points.len() < 2 {
            return None;
        }
        let (mn, mx) = self.points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), p| (mn.min(p.y), mx.max(p.y)));
        Some(LinearScale::nice(mn, mx, TARGET_Y_TICKS))
    }

    /// `(min, max)` across every point's own [`ScatterPoint::value`] —
    /// ALWAYS over the full point set (never just the thinned/rendered
    /// subset), so a drawn point's own size stays stable regardless of
    /// [`ScatterFigure::with_thinning`]'s own budget. `None` when no point
    /// carries a value.
    fn value_domain(&self) -> Option<(f64, f64)> {
        let mut found = false;
        let (mn, mx) = self.points.iter().filter_map(|p| p.value).fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), v| {
            found = true;
            (mn.min(v), mx.max(v))
        });
        if found {
            Some((mn, mx))
        } else {
            None
        }
    }

    fn point_radius(&self, p: &ScatterPoint, value_domain: Option<(f64, f64)>) -> f64 {
        match self.radius {
            PointRadius::Fixed(r) => r,
            PointRadius::ValueMapped { min_radius, max_radius } => {
                let (Some(v), Some((lo, hi))) = (p.value, value_domain) else { return min_radius };
                let t = if (hi - lo).abs() < f64::EPSILON { 0.5 } else { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) };
                min_radius + t * (max_radius - min_radius)
            }
        }
    }

    /// The original-index set this render pass draws AND hit-tests — every
    /// index when [`ScatterFigure::with_thinning`] is unset or the point
    /// count is already at/under budget, [`uniform_thin_indices`]'s output
    /// otherwise.
    fn rendered_indices(&self) -> Vec<usize> {
        match self.thin_max {
            Some(max) if self.points.len() > max => uniform_thin_indices(self.points.len(), max),
            _ => (0..self.points.len()).collect(),
        }
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position within a
    /// marker's own visible radius (+ [`HOVER_TOLERANCE_PX`]) brightens it
    /// and shows an (x, y[, value]) tooltip. `overlay.brush`/`overlay.focus`
    /// are not consumed by this figure (no V2 consumer needs linked-brush/
    /// persistent-selection on a scatter cloud yet — see
    /// [`crate::figure::FigureOverlay`]).
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let base_rect = self.base_plot_rect(rect);
        let area = PlotArea::new(base_rect);

        let computed_x_scale = if self.x_scale_override.is_none() { self.x_scale() } else { None };
        let x_scale: Option<&dyn Scale> = match (&self.x_scale_override, &computed_x_scale) {
            (Some(s), _) => Some(s.as_ref()),
            (None, Some(s)) => Some(s),
            (None, None) => None,
        };

        if let (Some(x_scale), Some(y_scale)) = (x_scale, self.y_scale()) {
            // Weighted entry point: a real render-output change ONLY when
            // `x_scale` is a `TimeScale` — see `CurveFigure::render_with`'s
            // own identical comment for the full reasoning + regression
            // proof.
            grid::draw_x_grid_weighted(ctx, &area, x_scale, theme, TARGET_X_TICKS, &GridTickWeightStyle::default());
            grid::draw_y_grid(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);

            // Annotation FILLS (the only underlay: `HBand`'s own shaded
            // rect) paint UNDER the data points — the typical "shaded zone
            // sits behind the cloud" convention, over the grid. Reference
            // LINES/LABELS/`Callout` paint AFTER the points below (see
            // `guide::annotation`'s own "Layer contract" doc comment) — an
            // owner defect report found a dense point cloud swallowing the
            // `HBand` label/`Callout` box when the whole annotation pass
            // drew in one shot before marks.
            draw_annotation_underlays(ctx, &area, &y_scale, theme, &self.annotations);

            let indices = self.rendered_indices();
            let value_domain = self.value_domain();
            let sized: Vec<(f64, f64, f64)> = indices
                .iter()
                .map(|&i| {
                    let p = &self.points[i];
                    (p.x, p.y, self.point_radius(p, value_domain))
                })
                .collect();
            let style = MarkStyle { color: theme.palette[0].clone(), fill_alpha: DEFAULT_FILL_ALPHA, ..Default::default() };
            draw_points_sized(ctx, &area, x_scale, &y_scale, &sized, &style);

            draw_annotation_overlays(ctx, &area, x_scale, &y_scale, theme, &self.annotations);

            if let Some((hx, hy)) = overlay.hover_px {
                if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                    let screen_points: Vec<(f64, f64)> = indices.iter().map(|&i| (self.points[i].x, self.points[i].y)).collect();
                    if let Some(local_i) = hit::nearest_point_xy(&area, x_scale, &y_scale, &screen_points, hx, hy) {
                        let orig_i = indices[local_i];
                        let p = &self.points[orig_i];
                        let sx = area.x(x_scale, p.x);
                        let sy = area.y(&y_scale, p.y);
                        let r = self.point_radius(p, value_domain);
                        let dist = ((sx - hx).powi(2) + (sy - hy).powi(2)).sqrt();
                        if dist <= r + HOVER_TOLERANCE_PX {
                            draw_points_sized(
                                ctx,
                                &area,
                                x_scale,
                                &y_scale,
                                &[(p.x, p.y, r + HOVER_HIGHLIGHT_EXTRA_RADIUS)],
                                &MarkStyle { color: theme.highlight.clone(), fill_alpha: HOVER_HIGHLIGHT_ALPHA, ..Default::default() },
                            );

                            let y_step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
                            let mut lines = vec![("x".to_owned(), x_scale.format_value(p.x)), ("y".to_owned(), format_value(p.y, y_step))];
                            if let Some(v) = p.value {
                                let v_step = value_domain.map(|(lo, hi)| nice_step(hi - lo, TARGET_Y_TICKS as f64)).unwrap_or(1.0);
                                lines.push(("value".to_owned(), format_value(v, v_step)));
                            }
                            tooltip::draw_tooltip(ctx, theme, (sx, sy), &lines, area.rect);
                        }
                    }
                }
            }

            axis::draw_x_axis_weighted(ctx, &area, x_scale, theme, TARGET_X_TICKS, &AxisTickWeightStyle::default());
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_and_y_domain_span_the_full_point_set() {
        let points = vec![ScatterPoint::new(-5.0, 100.0), ScatterPoint::new(20.0, -10.0), ScatterPoint::new(8.0, 40.0)];
        let figure = ScatterFigure::new(points);
        let x = figure.x_scale().expect("3 points");
        let y = figure.y_scale().expect("3 points");
        assert!(x.min <= -5.0 && x.max >= 20.0);
        assert!(y.min <= -10.0 && y.max >= 100.0);
    }

    #[test]
    fn y_domain_never_forces_a_zero_baseline() {
        // Every point sits well above zero — unlike BarFigure/CurveFigure,
        // a scatter's own Y domain must NOT be pulled down to include 0.
        let points = vec![ScatterPoint::new(0.0, 500.0), ScatterPoint::new(1.0, 520.0)];
        let figure = ScatterFigure::new(points);
        let y = figure.y_scale().expect("2 points");
        assert!(y.min > 0.0, "scatter Y domain must fit the data, never force a zero baseline (got min={})", y.min);
    }

    #[test]
    fn fewer_than_two_points_has_no_domain() {
        assert!(ScatterFigure::new(vec![ScatterPoint::new(1.0, 1.0)]).x_scale().is_none());
        assert!(ScatterFigure::new(Vec::new()).y_scale().is_none());
    }

    #[test]
    fn point_radius_fixed_ignores_value() {
        let figure = ScatterFigure::new(Vec::new()).with_radius(PointRadius::Fixed(6.0));
        let with_value = ScatterPoint::with_value(0.0, 0.0, 999.0);
        let without_value = ScatterPoint::new(0.0, 0.0);
        assert_eq!(figure.point_radius(&with_value, Some((0.0, 999.0))), 6.0);
        assert_eq!(figure.point_radius(&without_value, None), 6.0);
    }

    #[test]
    fn point_radius_value_mapped_interpolates_and_falls_back_for_missing_value() {
        let figure = ScatterFigure::new(Vec::new()).with_radius(PointRadius::ValueMapped { min_radius: 2.0, max_radius: 10.0 });
        let lo = ScatterPoint::with_value(0.0, 0.0, 0.0);
        let hi = ScatterPoint::with_value(0.0, 0.0, 100.0);
        let mid = ScatterPoint::with_value(0.0, 0.0, 50.0);
        let missing = ScatterPoint::new(0.0, 0.0);
        let domain = Some((0.0, 100.0));
        assert!((figure.point_radius(&lo, domain) - 2.0).abs() < 1e-9);
        assert!((figure.point_radius(&hi, domain) - 10.0).abs() < 1e-9);
        assert!((figure.point_radius(&mid, domain) - 6.0).abs() < 1e-9);
        assert_eq!(figure.point_radius(&missing, domain), 2.0, "a point with no value must fall back to min_radius");
    }

    #[test]
    fn value_domain_spans_only_points_that_actually_carry_a_value() {
        let points = vec![ScatterPoint::new(0.0, 0.0), ScatterPoint::with_value(1.0, 1.0, 5.0), ScatterPoint::with_value(2.0, 2.0, 15.0)];
        let figure = ScatterFigure::new(points);
        let (lo, hi) = figure.value_domain().expect("2 of 3 points carry a value");
        assert!((lo - 5.0).abs() < 1e-9);
        assert!((hi - 15.0).abs() < 1e-9);
    }

    #[test]
    fn value_domain_is_none_when_no_point_carries_a_value() {
        let points = vec![ScatterPoint::new(0.0, 0.0), ScatterPoint::new(1.0, 1.0)];
        assert!(ScatterFigure::new(points).value_domain().is_none());
    }

    #[test]
    fn uniform_thin_indices_identity_when_max_at_or_above_len() {
        assert_eq!(uniform_thin_indices(5, 5), vec![0, 1, 2, 3, 4]);
        assert_eq!(uniform_thin_indices(5, 50), vec![0, 1, 2, 3, 4]);
        assert_eq!(uniform_thin_indices(0, 10), Vec::<usize>::new());
        assert_eq!(uniform_thin_indices(10, 0), Vec::<usize>::new());
    }

    #[test]
    fn uniform_thin_indices_keeps_first_and_last_and_is_strictly_ascending() {
        let out = uniform_thin_indices(1000, 37);
        assert_eq!(out[0], 0);
        assert_eq!(*out.last().unwrap(), 999);
        for w in out.windows(2) {
            assert!(w[1] > w[0], "indices must be strictly ascending (never duplicated), got {out:?}");
        }
        assert!(out.len() <= 37);
    }

    #[test]
    fn uniform_thin_indices_is_deterministic() {
        let a = uniform_thin_indices(733, 40);
        let b = uniform_thin_indices(733, 40);
        assert_eq!(a, b);
    }

    #[test]
    fn uniform_thin_indices_single_target_keeps_only_the_first_index() {
        assert_eq!(uniform_thin_indices(10, 1), vec![0]);
    }

    #[test]
    fn render_smoke_with_hover_thinning_value_mapped_radius_and_annotations() {
        use uzor_export::{render_to_png, ExportSpec};

        let points: Vec<ScatterPoint> = (0..200)
            .map(|i| {
                let x = i as f64;
                let y = 10.0 + ((i * 37) % 53) as f64;
                ScatterPoint::with_value(x, y, (i % 20) as f64)
            })
            .collect();
        let figure = ScatterFigure::new(points)
            .with_title("smoke")
            .with_radius(PointRadius::ValueMapped { min_radius: 2.0, max_radius: 8.0 })
            .with_thinning(60)
            .with_annotations(vec![
                Annotation::HBand { low: 20.0, high: 40.0, color: None, label: Some("range".to_owned()) },
                Annotation::Callout { x: 100.0, y: 30.0, text: "note".to_owned() },
            ]);
        let theme = FigureTheme::dark();
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let overlay = FigureOverlay { hover_px: Some((200.0, 150.0)), brush: None, focus: None };
        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn empty_figure_renders_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};
        let figure = ScatterFigure::new(Vec::new());
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 200, height_px: 150, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 150.0), &theme);
        });
        assert!(result.is_ok());
    }
}
