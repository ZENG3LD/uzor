//! `CurveFigure` — a line (optionally area-filled) series over two nice
//! linear domains. Covers plain line, cumulative curve, and filled-area
//! use cases — same mark composition, `fill` just toggles whether
//! [`crate::mark::area::draw_area`] runs before the stroke.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
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

/// A line series over `(x, y)` domain points.
pub struct CurveFigure {
    pub points: Vec<(f64, f64)>,
    pub title: Option<String>,
    pub fill: bool,
    /// Caller-supplied X scale (e.g. [`crate::scale::TimeScale`]) used in
    /// place of the auto-computed nice [`LinearScale`] over `points`' x
    /// values — set via [`CurveFigure::with_x_scale`]. `None` (the
    /// default) reproduces the original behavior exactly.
    x_scale_override: Option<Box<dyn Scale>>,
}

impl CurveFigure {
    pub fn new(points: Vec<(f64, f64)>) -> Self {
        Self { points, title: None, fill: false, x_scale_override: None }
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
    /// UTC-second `points` x values) instead of the auto-computed nice
    /// [`LinearScale`] domain. Follows this figure's existing builder
    /// style (`with_title`/`with_fill`) — the minimal option needed for a
    /// caller to render a time-axis curve without this figure knowing
    /// anything trading/time-specific itself: every draw call below
    /// already takes `&dyn Scale`, so any [`Scale`] impl slots in.
    pub fn with_x_scale(mut self, scale: impl Scale + 'static) -> Self {
        self.x_scale_override = Some(Box::new(scale));
        self
    }

    fn plot_rect(&self, rect: Rect) -> Rect {
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
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.plot_rect(rect))
    }

    /// This figure's auto-computed nice-linear X domain scale, `None` when
    /// there are fewer than 2 points (nothing to plot — `render`/
    /// `render_with` draw nothing either). Exposed for the same reason as
    /// [`CurveFigure::plot_area`]: a caller driving an external brush drag
    /// needs to invert its own pixel positions through the EXACT scale
    /// this figure renders with.
    ///
    /// **Does not reflect [`CurveFigure::with_x_scale`]** — when a custom
    /// X scale override is set, `render`/`render_with` draw through THAT
    /// scale instead of this one; this accessor always returns the
    /// auto-computed linear domain regardless. Re-pointing external brush
    /// callers onto an overridden scale is a Phase B (full V2
    /// generalization) concern, out of scope for this scale-math port.
    pub fn x_scale(&self) -> Option<LinearScale> {
        if self.points.len() < 2 {
            return None;
        }
        let (x_min, x_max) = self
            .points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &(px, _)| (mn.min(px), mx.max(px)));
        Some(LinearScale::nice(x_min, x_max, TARGET_X_TICKS))
    }

    fn y_scale(&self) -> Option<LinearScale> {
        if self.points.len() < 2 {
            return None;
        }
        let (y_min, y_max) = self
            .points
            .iter()
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
    /// plot draws a crosshair + nearest-point marker + an (x, y) tooltip.
    /// `overlay.brush`/`overlay.focus` are not consumed by this figure —
    /// linked-brush highlighting is a bars/histogram concern in V2 (see
    /// [`crate::figure::FigureOverlay`]).
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let area = self.plot_area(rect);

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

            let style = MarkStyle { color: theme.palette[0].clone(), ..Default::default() };
            if self.fill {
                let fill_style = MarkStyle { fill_alpha: FILL_ALPHA, ..style.clone() };
                draw_area(ctx, &area, x_scale, &y_scale, &self.points, &fill_style);
            }
            draw_polyline(ctx, &area, x_scale, &y_scale, &self.points, &style);

            axis::draw_x_axis(ctx, &area, x_scale, theme, TARGET_X_TICKS);
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);

            if let Some((hx, hy)) = overlay.hover_px {
                if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                    if let Some(idx) = hit::nearest_point_x(&area, x_scale, &y_scale, &self.points, hx) {
                        let (data_x, data_y) = self.points[idx];
                        crosshair::draw_crosshair(ctx, &area, theme, data_x, data_y, x_scale, &y_scale);

                        let marker_style = MarkStyle { color: theme.palette[1].clone(), ..Default::default() };
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
                        let lines = vec![
                            ("x".to_owned(), x_scale.format_value(data_x)),
                            ("y".to_owned(), format_value(data_y, y_step)),
                        ];
                        let anchor = (area.x(x_scale, data_x), area.y(&y_scale, data_y));
                        tooltip::draw_tooltip(ctx, theme, anchor, &lines, area.rect);
                    }
                }
            }
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}
