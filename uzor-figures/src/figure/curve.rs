//! `CurveFigure` — a line (optionally area-filled) series over two nice
//! linear domains. Covers plain line, cumulative curve, and filled-area
//! use cases — same mark composition, `fill` just toggles whether
//! [`crate::mark::area::draw_area`] runs before the stroke.
//!
//! Multi-series (`CurveSeries`) is additive: [`CurveFigure::new`] is now
//! sugar for a single-series figure via [`CurveFigure::with_series`], and
//! its render output is byte-identical to the pre-multi-series version —
//! the single-series hover marker/tooltip path below is the SAME
//! accent-colored (`theme.palette[1]`) marker that existed before, gated
//! on `self.series.len() <= 1`. A shared X/Y domain (union of every
//! series' points) and per-series color (`theme.palette[i]`) activate
//! once `series.len() > 1`; a legend ([`crate::guide::legend`])
//! auto-appears at [`LegendPosition::Right`] then, or on any figure via
//! [`CurveFigure::with_legend`].

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::guide::legend::{self, LegendEntry, LegendPosition};
use crate::guide::{axis, crosshair, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::mark::area::draw_area;
use crate::mark::line::draw_polyline;
use crate::mark::point::draw_points;
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
/// Fill alpha applied under the stroked line when `fill` is enabled.
const FILL_ALPHA: f64 = 0.25;
/// Radius (px) of the nearest-point hover marker.
const HOVER_MARKER_RADIUS: f64 = 4.0;
/// Gap (px) between a measured legend band and the plot rect it shrinks.
const LEGEND_GAP: f64 = 8.0;

/// One named line series — `points` are `(x, y)` domain pairs, same shape
/// [`CurveFigure::new`]'s original single-series `points` field always
/// used.
#[derive(Debug, Clone)]
pub struct CurveSeries {
    pub name: String,
    pub points: Vec<(f64, f64)>,
}

/// A line series over `(x, y)` domain points — one or more [`CurveSeries`],
/// sharing one X/Y domain (the union of every series' points).
pub struct CurveFigure {
    pub series: Vec<CurveSeries>,
    pub title: Option<String>,
    pub fill: bool,
    /// Caller-supplied X scale (e.g. [`crate::scale::TimeScale`]) used in
    /// place of the auto-computed nice [`LinearScale`] over every series'
    /// x values — set via [`CurveFigure::with_x_scale`]. `None` (the
    /// default) reproduces the original behavior exactly.
    x_scale_override: Option<Box<dyn Scale>>,
    legend_position: Option<LegendPosition>,
    /// Per-series LTTB downsample budget — set via
    /// [`CurveFigure::with_downsample`]. `None` (the default) reproduces
    /// the original behavior exactly (every raw point drawn/hit-tested).
    downsample_max: Option<usize>,
}

impl CurveFigure {
    /// Single-series constructor — sugar for
    /// `with_series(vec![CurveSeries { name: String::new(), points }])`. A
    /// single series never shows a legend on its own (see
    /// [`CurveFigure::resolved_legend_position`]), so this reproduces the
    /// pre-multi-series render exactly.
    pub fn new(points: Vec<(f64, f64)>) -> Self {
        Self::with_series(vec![CurveSeries { name: String::new(), points }])
    }

    /// Multi-series constructor — one line per [`CurveSeries`], each its
    /// own `theme.palette[i]` color, sharing one X/Y domain (the union of
    /// every series' points). `series.len() > 1` auto-shows a legend at
    /// [`LegendPosition::Right`] unless overridden via
    /// [`CurveFigure::with_legend`].
    pub fn with_series(series: Vec<CurveSeries>) -> Self {
        Self { series, title: None, fill: false, x_scale_override: None, legend_position: None, downsample_max: None }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Supply a custom X scale (e.g. a [`crate::scale::TimeScale`] over
    /// UTC-second x values) instead of the auto-computed nice
    /// [`LinearScale`] domain. Follows this figure's existing builder
    /// style (`with_title`/`with_fill`) — the minimal option needed for a
    /// caller to render a time-axis curve without this figure knowing
    /// anything trading/time-specific itself: every draw call below
    /// already takes `&dyn Scale`, so any [`Scale`] impl slots in.
    pub fn with_x_scale(mut self, scale: impl Scale + 'static) -> Self {
        self.x_scale_override = Some(Box::new(scale));
        self
    }

    /// Force a legend at `position` regardless of series count (default:
    /// auto-shown at [`LegendPosition::Right`] only when there's more than
    /// one series — see [`CurveFigure::resolved_legend_position`]).
    pub fn with_legend(mut self, position: LegendPosition) -> Self {
        self.legend_position = Some(position);
        self
    }

    /// Downsample each series independently via LTTB
    /// ([`crate::transform::lttb`]) whenever its own point count exceeds
    /// `max_points`, applied at RENDER time — a series at or under the
    /// budget is untouched (byte-identical to not calling this at all).
    ///
    /// **Downsampling is a VISUAL transform only.** The auto-computed X/Y
    /// domain ([`CurveFigure::x_scale`]/`y_scale`) always folds over the
    /// FULL raw series regardless of this setting, so downsampling never
    /// shrinks or reflows this figure's own layout/axes.
    ///
    /// **Hit-testing decision (one-transform law, design law #1):**
    /// crosshair/tooltip nearest-point hit-testing resolves against the
    /// SAME downsampled point set the render pass actually draws — never
    /// the raw, undrawn points. The alternative (hit-test on raw points,
    /// draw downsampled ones) would let a hover snap to a point that
    /// isn't even on screen, which is a strictly worse and more confusing
    /// failure mode than "hover resolves to whichever nearby point LTTB
    /// kept" — the same principle [`crate::coord::PlotArea`]'s own docs
    /// state for paint vs. hit-test geometry, applied one level up (data
    /// selection) instead of screen-space transform.
    pub fn with_downsample(mut self, max_points: usize) -> Self {
        self.downsample_max = Some(max_points);
        self
    }

    /// The exact point set this figure draws AND hit-tests for `s` this
    /// render — LTTB-downsampled to [`CurveFigure::downsample_max`] when
    /// `s` exceeds that budget, borrowed verbatim otherwise (see
    /// [`CurveFigure::with_downsample`]'s own docs for why render and
    /// hit-test always agree).
    fn rendered_points<'a>(&self, s: &'a CurveSeries) -> std::borrow::Cow<'a, [(f64, f64)]> {
        match self.downsample_max {
            Some(max) if s.points.len() > max => std::borrow::Cow::Owned(crate::transform::lttb(&s.points, max)),
            _ => std::borrow::Cow::Borrowed(&s.points),
        }
    }

    /// Resolved legend position for this render: an explicit
    /// [`CurveFigure::with_legend`] override, or auto-[`LegendPosition::Right`]
    /// when there's more than one series, or `None` otherwise.
    fn resolved_legend_position(&self) -> Option<LegendPosition> {
        self.legend_position.or(if self.series.len() > 1 { Some(LegendPosition::Right) } else { None })
    }

    fn legend_entries(&self, theme: &FigureTheme) -> Vec<LegendEntry> {
        self.series
            .iter()
            .enumerate()
            .map(|(i, s)| LegendEntry { label: s.name.clone(), color: theme.palette[i % theme.palette.len()].clone() })
            .collect()
    }

    fn total_points(&self) -> usize {
        self.series.iter().map(|s| s.points.len()).sum()
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

    /// This figure's plot-area transform for `rect` — exposed so a caller
    /// driving an external gesture (e.g. a demo's live brush drag) can
    /// draw in exact pixel alignment with this figure's own render pass
    /// (design law #1: one transform, shared, never recomputed
    /// independently elsewhere).
    ///
    /// **Does not account for a legend** — same ctx-less-accessor
    /// reasoning documented on [`crate::figure::BarFigure::plot_area`].
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.base_plot_rect(rect))
    }

    /// This figure's auto-computed nice-linear X domain scale (the union
    /// of every series' points), `None` when there are fewer than 2 points
    /// total (nothing to plot — `render`/`render_with` draw nothing
    /// either). Exposed for the same reason as [`CurveFigure::plot_area`]:
    /// a caller driving an external brush drag needs to invert its own
    /// pixel positions through the EXACT scale this figure renders with.
    ///
    /// **Does not reflect [`CurveFigure::with_x_scale`]** — when a custom
    /// X scale override is set, `render`/`render_with` draw through THAT
    /// scale instead of this one; this accessor always returns the
    /// auto-computed linear domain regardless. Re-pointing external brush
    /// callers onto an overridden scale is a Phase B (full V2
    /// generalization) concern, out of scope for this scale-math port.
    pub fn x_scale(&self) -> Option<LinearScale> {
        if self.total_points() < 2 {
            return None;
        }
        let (x_min, x_max) = self
            .series
            .iter()
            .flat_map(|s| s.points.iter())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &(px, _)| (mn.min(px), mx.max(px)));
        Some(LinearScale::nice(x_min, x_max, TARGET_X_TICKS))
    }

    fn y_scale(&self) -> Option<LinearScale> {
        if self.total_points() < 2 {
            return None;
        }
        let (y_min, y_max) = self
            .series
            .iter()
            .flat_map(|s| s.points.iter())
            .fold((0.0_f64, 0.0_f64), |(mn, mx), &(_, py)| (mn.min(py), mx.max(py)));
        Some(LinearScale::nice(y_min, y_max, TARGET_Y_TICKS))
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position inside the
    /// plot draws a crosshair + nearest-point marker (across every series)
    /// + an (x, y[, series]) tooltip. `overlay.brush`/`overlay.focus` are
    /// not consumed by this figure — linked-brush highlighting is a
    /// bars/histogram concern in V2 (see [`crate::figure::FigureOverlay`]).
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let base_rect = self.base_plot_rect(rect);
        let legend_position = self.resolved_legend_position();
        let legend_entries = if legend_position.is_some() { self.legend_entries(theme) } else { Vec::new() };

        let (plot_rect, legend_rect) = match legend_position {
            Some(pos) if !legend_entries.is_empty() => {
                let size = legend::measure_legend(ctx, theme, &legend_entries, pos, base_rect.width);
                match pos {
                    LegendPosition::Top => {
                        let reserved = size.height + LEGEND_GAP;
                        (
                            Rect::new(base_rect.x, base_rect.y + reserved, base_rect.width, (base_rect.height - reserved).max(0.0)),
                            Some((Rect::new(base_rect.x, base_rect.y, base_rect.width, size.height), pos)),
                        )
                    }
                    LegendPosition::Bottom => {
                        let reserved = size.height + LEGEND_GAP;
                        (
                            Rect::new(base_rect.x, base_rect.y, base_rect.width, (base_rect.height - reserved).max(0.0)),
                            Some((Rect::new(base_rect.x, base_rect.bottom() - size.height, base_rect.width, size.height), pos)),
                        )
                    }
                    LegendPosition::Right => {
                        let reserved = size.width + LEGEND_GAP;
                        (
                            Rect::new(base_rect.x, base_rect.y, (base_rect.width - reserved).max(0.0), base_rect.height),
                            Some((Rect::new(base_rect.right() - size.width, base_rect.y, size.width, base_rect.height), pos)),
                        )
                    }
                }
            }
            _ => (base_rect, None),
        };

        let area = PlotArea::new(plot_rect);

        // The auto-computed nice-linear X domain only needs to exist when
        // no override was supplied — skip computing it otherwise (it would
        // be thrown away unused).
        let computed_x_scale = if self.x_scale_override.is_none() { self.x_scale() } else { None };
        let x_scale: Option<&dyn Scale> = match (&self.x_scale_override, &computed_x_scale) {
            (Some(s), _) => Some(s.as_ref()),
            (None, Some(s)) => Some(s),
            (None, None) => None,
        };

        if let (Some(x_scale), Some(y_scale)) = (x_scale, self.y_scale()) {
            grid::draw_x_grid(ctx, &area, x_scale, theme, TARGET_X_TICKS);
            grid::draw_y_grid(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);

            // Per-series LTTB-downsampled (or borrowed verbatim) point
            // set — the SAME set drawn below AND hit-tested against
            // (see `with_downsample`'s own docs: one-transform law, a
            // hover never resolves to a point that isn't actually drawn).
            let rendered: Vec<std::borrow::Cow<'_, [(f64, f64)]>> = self.series.iter().map(|s| self.rendered_points(s)).collect();

            for (i, s) in rendered.iter().enumerate() {
                let color = theme.palette[i % theme.palette.len()].clone();
                let style = MarkStyle { color, ..Default::default() };
                if self.fill {
                    let fill_style = MarkStyle { fill_alpha: FILL_ALPHA, ..style.clone() };
                    draw_area(ctx, &area, x_scale, &y_scale, s, &fill_style);
                }
                draw_polyline(ctx, &area, x_scale, &y_scale, s, &style);
            }

            axis::draw_x_axis(ctx, &area, x_scale, theme, TARGET_X_TICKS);
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);

            if let Some((hx, hy)) = overlay.hover_px {
                if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                    let series_points: Vec<&[(f64, f64)]> = rendered.iter().map(|s| s.as_ref()).collect();
                    if let Some((si, pi)) = hit::nearest_point_x_multi(&area, x_scale, &y_scale, &series_points, hx) {
                        let (data_x, data_y) = rendered[si][pi];
                        crosshair::draw_crosshair(ctx, &area, theme, data_x, data_y, x_scale, &y_scale);

                        // Single-series: the ORIGINAL accent marker color
                        // (`theme.palette[1]`, distinct from the line's own
                        // `theme.palette[0]`) — byte-compatible with the
                        // pre-multi-series render. Multi-series: the
                        // hovered series' OWN color, so the marker visually
                        // corresponds to the line/legend swatch it belongs to.
                        let marker_color =
                            if self.series.len() <= 1 { theme.palette[1].clone() } else { theme.palette[si % theme.palette.len()].clone() };
                        let marker_style = MarkStyle { color: marker_color, ..Default::default() };
                        draw_points(ctx, &area, x_scale, &y_scale, &[(data_x, data_y)], HOVER_MARKER_RADIUS, &marker_style);

                        // `x` goes through `x_scale.format_value` (the
                        // `Scale`-provided formatter, `Scale::format_value`)
                        // rather than the raw numeric `format_value` `y`
                        // still uses below — correct whether this is the
                        // auto-computed LinearScale or an overridden scale
                        // (e.g. TimeScale, whose domain is Unix seconds; a
                        // raw-timestamp tooltip label was a known Phase B
                        // gap, closed by `TimeScale`'s own override).
                        let y_step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
                        let mut lines = vec![("x".to_owned(), x_scale.format_value(data_x))];
                        if self.series.len() > 1 {
                            lines.push(("series".to_owned(), self.series[si].name.clone()));
                        }
                        lines.push(("y".to_owned(), format_value(data_y, y_step)));
                        let anchor = (area.x(x_scale, data_x), area.y(&y_scale, data_y));
                        tooltip::draw_tooltip(ctx, theme, anchor, &lines, area.rect);
                    }
                }
            }
        }

        if let Some((legend_rect, pos)) = legend_rect {
            legend::draw_legend(ctx, legend_rect, theme, &legend_entries, pos);
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
    fn new_single_series_never_auto_shows_a_legend() {
        let figure = CurveFigure::new(vec![(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(figure.resolved_legend_position(), None);
    }

    #[test]
    fn with_series_multi_series_auto_shows_a_right_legend() {
        let series = vec![
            CurveSeries { name: "a".to_owned(), points: vec![(0.0, 0.0), (1.0, 1.0)] },
            CurveSeries { name: "b".to_owned(), points: vec![(0.0, 2.0), (1.0, 3.0)] },
        ];
        let figure = CurveFigure::with_series(series);
        assert_eq!(figure.resolved_legend_position(), Some(LegendPosition::Right));
    }

    #[test]
    fn with_legend_overrides_the_auto_default_even_for_a_single_series() {
        let figure = CurveFigure::new(vec![(0.0, 0.0), (1.0, 1.0)]).with_legend(LegendPosition::Top);
        assert_eq!(figure.resolved_legend_position(), Some(LegendPosition::Top));
    }

    #[test]
    fn legend_entries_assign_distinct_theme_palette_colors_by_series_index() {
        let series = vec![
            CurveSeries { name: "a".to_owned(), points: vec![(0.0, 0.0)] },
            CurveSeries { name: "b".to_owned(), points: vec![(0.0, 1.0)] },
            CurveSeries { name: "c".to_owned(), points: vec![(0.0, 2.0)] },
        ];
        let figure = CurveFigure::with_series(series);
        let theme = FigureTheme::dark();
        let entries = figure.legend_entries(&theme);
        assert_eq!(entries.len(), 3);
        let colors: Vec<&str> = entries.iter().map(|e| e.color.as_str()).collect();
        let mut unique = colors.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), colors.len(), "every series must get its own distinct swatch color");
    }

    #[test]
    fn multi_series_x_and_y_domain_is_the_union_of_every_series() {
        let series = vec![
            CurveSeries { name: "a".to_owned(), points: vec![(0.0, -5.0), (10.0, 2.0)] },
            CurveSeries { name: "b".to_owned(), points: vec![(-20.0, 1.0), (5.0, 40.0)] },
        ];
        let figure = CurveFigure::with_series(series);
        let x = figure.x_scale().expect("non-empty fixture");
        let y = figure.y_scale().expect("non-empty fixture");
        assert!(x.min <= -20.0, "x domain must contain series b's leftmost point");
        assert!(x.max >= 10.0, "x domain must contain series a's rightmost point");
        assert!(y.min <= -5.0, "y domain must contain series a's lowest point");
        assert!(y.max >= 40.0, "y domain must contain series b's highest point");
    }

    #[test]
    fn single_series_x_scale_matches_the_pre_multi_series_domain() {
        // `new()`'s domain over ONE series must be identical to the domain
        // over that series' points directly — no union widening from a
        // series that doesn't exist.
        let points = vec![(2.0, 4.0), (8.0, -3.0), (5.0, 10.0)];
        let figure = CurveFigure::new(points.clone());
        let x = figure.x_scale().expect("non-empty fixture");
        let (x_min, x_max) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &(px, _)| (mn.min(px), mx.max(px)));
        assert!(x.min <= x_min && x.max >= x_max);
    }

    #[test]
    fn single_series_render_uses_the_original_accent_marker_color_path() {
        // A single series must still render under hover (no panic, valid
        // dimensions) — exercises the `self.series.len() <= 1` branch that
        // keeps `CurveFigure::new`'s output byte-compatible with the
        // pre-multi-series render (accent marker color, no "series" tooltip
        // line).
        use uzor_export::{render_to_png, ExportSpec};

        let figure = CurveFigure::new(vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5)]).with_title("single series");
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let overlay = FigureOverlay { hover_px: Some((150.0, 100.0)), brush: None, focus: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        });
        assert!(result.is_ok());
    }
}
