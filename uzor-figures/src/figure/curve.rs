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
use crate::figure::{resolve_tick_count, FigureOverlay, MarginPolicy, TickCountPolicy, YDomainPolicy};
use crate::guide::annotation::{draw_annotation_overlays, draw_annotation_underlays, Annotation};
use crate::guide::axis::AxisTickWeightStyle;
use crate::guide::grid::GridTickWeightStyle;
use crate::guide::legend::{self, LegendEntry, LegendPosition};
use crate::guide::{axis, crosshair, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::interact::viewport::Viewport;
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
    /// Reference lines/bands/callouts drawn over this figure's marks — set
    /// via [`CurveFigure::with_annotations`]. Empty (the default)
    /// reproduces the original behavior exactly (see
    /// [`crate::guide::annotation`]).
    annotations: Vec<Annotation>,
    /// This figure's own Y-domain zero-baseline policy — see
    /// [`CurveFigure::with_y_domain_policy`].
    y_domain_policy: YDomainPolicy,
    /// This figure's own left-margin sizing policy — see
    /// [`CurveFigure::with_margin_policy`].
    margin_policy: MarginPolicy,
    /// This figure's own X tick-count policy — see
    /// [`CurveFigure::with_x_tick_policy`].
    x_tick_policy: TickCountPolicy,
    /// This figure's own Y tick-count policy — see
    /// [`CurveFigure::with_y_tick_policy`].
    y_tick_policy: TickCountPolicy,
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
        Self {
            series,
            title: None,
            fill: false,
            x_scale_override: None,
            legend_position: None,
            downsample_max: None,
            annotations: Vec::new(),
            y_domain_policy: YDomainPolicy::default(),
            margin_policy: MarginPolicy::default(),
            x_tick_policy: TickCountPolicy::Fixed(TARGET_X_TICKS),
            y_tick_policy: TickCountPolicy::Fixed(TARGET_Y_TICKS),
        }
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

    /// Reference lines/bands/callouts drawn over this figure's marks — see
    /// [`crate::guide::annotation`]. Same additive-builder shape as every
    /// other optional capability on this figure.
    pub fn with_annotations(mut self, annotations: Vec<Annotation>) -> Self {
        self.annotations = annotations;
        self
    }

    /// This figure's Y-domain zero-baseline policy — see
    /// [`YDomainPolicy`]. Default (unset) is [`YDomainPolicy::ForceZero`],
    /// byte-identical to this figure's own pre-existing behavior (the Y
    /// domain always folded its `fold` seed at `(0.0, 0.0)`, unconditionally
    /// widening to include zero — a real audit finding: a plain, unfilled
    /// line chart got the SAME forced-zero treatment a bar/filled-area
    /// legitimately needs, with no opt-out, unlike
    /// [`crate::figure::ScatterFigure`]/[`crate::figure::BoxplotFigure`],
    /// which both deliberately never force zero). Set
    /// [`YDomainPolicy::FitData`] to fit the domain to the data's own
    /// extent only — the conventional line-chart behavior every reference
    /// library (D3, Highcharts, TradingView) uses by default, reserving
    /// forced zero-baselining for bars/filled areas.
    pub fn with_y_domain_policy(mut self, policy: YDomainPolicy) -> Self {
        self.y_domain_policy = policy;
        self
    }

    /// Override this figure's left-margin sizing policy — see
    /// [`MarginPolicy`]'s own docs. Default (unset) is
    /// [`MarginPolicy::Measured`].
    pub fn with_margin_policy(mut self, policy: MarginPolicy) -> Self {
        self.margin_policy = policy;
        self
    }

    /// Override this figure's X tick-count policy — see
    /// [`TickCountPolicy`]'s own docs. Default (unset) is
    /// `TickCountPolicy::Fixed(6)`, byte-identical to this figure's
    /// pre-existing constant.
    pub fn with_x_tick_policy(mut self, policy: TickCountPolicy) -> Self {
        self.x_tick_policy = policy;
        self
    }

    /// Override this figure's Y tick-count policy — see
    /// [`TickCountPolicy`]'s own docs. Default (unset) is
    /// `TickCountPolicy::Fixed(5)`, byte-identical to this figure's
    /// pre-existing constant.
    pub fn with_y_tick_policy(mut self, policy: TickCountPolicy) -> Self {
        self.y_tick_policy = policy;
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
            .map(|(i, s)| LegendEntry {
                label: s.name.clone(),
                color: theme.palette[i % theme.palette.len()].clone(),
                symbol: crate::guide::legend::LegendSymbol::Line,
            })
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
    /// **Does not account for a legend, nor for [`MarginPolicy::Measured`]
    /// widening the left OR right margin past [`MARGIN_LEFT`]/
    /// [`MARGIN_RIGHT`]** (the latter now ALSO grows for a wide X-axis
    /// extreme-tick label overhang, see
    /// [`crate::guide::axis::measure_x_axis_extreme_overhang`]) — same
    /// ctx-less-accessor reasoning documented on
    /// [`crate::figure::BarFigure::plot_area`] (both need live text
    /// metrics from a real `&mut dyn RenderContext`, which this accessor
    /// doesn't have). `render_with` measures + widens the SAME base rect
    /// internally via its own `ctx`, so this figure's own hover/tooltip
    /// stay mutually consistent within one `render_with` call regardless.
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
        // Seed the fold at `(0.0, 0.0)` under `ForceZero` (unconditionally
        // widens the domain to include zero) or at the data's own extreme
        // extent under `FitData` (never pulls zero in) — see
        // `with_y_domain_policy`'s own doc comment.
        let seed = match self.y_domain_policy {
            YDomainPolicy::ForceZero => (0.0_f64, 0.0_f64),
            YDomainPolicy::FitData => (f64::INFINITY, f64::NEG_INFINITY),
        };
        let (y_min, y_max) =
            self.series.iter().flat_map(|s| s.points.iter()).fold(seed, |(mn, mx), &(_, py)| (mn.min(py), mx.max(py)));
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
    ///
    /// Equivalent to `render_with_viewport(ctx, rect, theme, overlay,
    /// None)` — see that method's own doc comment for the opt-in viewport
    /// (pan/zoom/fit) entry point this figure gained in Wave 5. Passing
    /// `None` here reproduces this method's pre-Wave-5 output byte-for-byte.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        self.render_with_viewport(ctx, rect, theme, overlay, None);
    }

    /// Same as [`CurveFigure::render_with`], but additionally resolves this
    /// figure's X domain through `x_viewport`'s own visible WINDOW (see
    /// [`crate::interact::viewport::Viewport`]'s own module docs for the
    /// window model) instead of the auto-computed/overridden scale's own
    /// FULL extent. `x_viewport: None` (what [`CurveFigure::render_with`]
    /// always passes) reproduces this method's output byte-for-byte —
    /// proven by
    /// `render_with_viewport_with_no_viewport_matches_render_with` below.
    ///
    /// Windowing only takes effect when this figure's own X scale (the
    /// auto-computed [`LinearScale`], or a [`CurveFigure::with_x_scale`]
    /// override) itself supports it via [`crate::scale::Scale::windowed`]
    /// — every scale kind this crate ships DOES (`LinearScale`/`LogScale`/
    /// `SymlogScale`/`PowScale`/[`crate::scale::TimeScale`]); a
    /// hypothetical `Scale` implementor that doesn't override `windowed`
    /// simply renders UNWINDOWED (`x_viewport` has no effect for it),
    /// never panics.
    ///
    /// Ticks/grid/axis labels re-derive from the WINDOWED scale
    /// automatically — every draw call below already takes `&dyn Scale`,
    /// so nothing downstream needed to change to "know about" a viewport
    /// (design law #1: one shared transform/scale interface). A viewport
    /// window can legitimately place some of this figure's own points
    /// outside the visible plot rect (the un-panned/zoomed remainder of
    /// the series) — this method clips the series draw pass to the plot
    /// rect whenever windowing is actually active, so those points' own
    /// stroke/fill never bleed past the plot rect into the axis-label
    /// margin (never clipped when `x_viewport` is `None`, so
    /// `render_with`'s own call path pays zero extra cost and renders
    /// identically to before this method existed).
    ///
    /// **The Y domain is NOT windowed** — [`CurveFigure::y_scale`] always
    /// computes the FULL data extent regardless of `x_viewport`'s state.
    /// Auto-fitting Y to only the currently-visible (post-X-window) points
    /// is a `ScaleMode::Auto`/`Focus`-shaped concern (Wave 3 of
    /// `nemo/docs/uzor-engines/plans/engine-strengthening-arc-2026-07-24.md`,
    /// not built yet) layered ON TOP of a windowed X — this method
    /// deliberately leaves that seam alone rather than half-building it.
    ///
    /// **The caller owns keeping `x_viewport`'s own
    /// [`Viewport::data_domain`] in sync** with this figure's real X
    /// extent (e.g. call `x_viewport.set_data_domain(figure.x_scale()
    /// .unwrap().domain())` whenever the underlying series changes) — the
    /// same "recomputed fresh every render, from whatever `self.series`
    /// currently holds" contract [`CurveFigure::x_scale`] already places
    /// on any external caller reacting to new data. This method never
    /// mutates a borrowed `&Viewport` itself (single-writer discipline,
    /// matching [`crate::interact::viewport`]'s own design rule).
    pub fn render_with_viewport(
        &self,
        ctx: &mut dyn RenderContext,
        rect: Rect,
        theme: &FigureTheme,
        overlay: &FigureOverlay<'_>,
        x_viewport: Option<&Viewport>,
    ) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        // Resolve this render's own left margin + tick counts BEFORE
        // building the plot rect (`MarginPolicy::Measured`/
        // `TickCountPolicy::Adaptive` both need the plot's own geometry,
        // but neither depends on anything the plot rect itself depends on
        // beyond the fixed constants below — never circular). `y_scale`
        // is `Copy` (`LinearScale`), so computing it once here and reusing
        // it below costs nothing extra.
        let y_scale_for_layout = self.y_scale();
        // The auto-computed nice-linear X domain only needs to exist when
        // no override was supplied. Resolved HERE (before margins) rather
        // than after the plot rect is built (as it used to be) — neither
        // `self.x_scale_override` nor `self.x_scale()` depends on the
        // plot's own pixel geometry (`x_scale()` sizes its own nice()
        // domain off `TARGET_X_TICKS`, the fixed constant, same
        // already-documented "domain sizing stays off the fixed constant,
        // only the DRAWN tick count adapts" convention `TickCountPolicy::
        // Adaptive`'s own doc comment establishes) — so computing it once
        // here and reusing it below (both for the new X-extreme-overhang
        // margin measurement AND the render loop's own X scale) costs
        // nothing extra and removes a duplicate computation.
        let computed_x_scale = if self.x_scale_override.is_none() { self.x_scale() } else { None };
        let base_x_scale: Option<&dyn Scale> = match (&self.x_scale_override, &computed_x_scale) {
            (Some(s), _) => Some(s.as_ref()),
            (None, Some(s)) => Some(s),
            (None, None) => None,
        };
        // Apply an optional viewport WINDOW on top of the full-domain
        // scale — see this method's own doc comment for the seam. Every
        // margin/tick/grid/axis/hover computation below reads `x_scale`
        // exactly as it did before `x_viewport` existed; it just may now
        // be a windowed scale rather than the full-domain one.
        let windowed_x_scale: Option<Box<dyn Scale>> =
            x_viewport.zip(base_x_scale).and_then(|(vp, scale)| crate::interact::viewport::windowed_scale(scale, vp));
        let x_scale: Option<&dyn Scale> = windowed_x_scale.as_deref().or(base_x_scale);

        let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
        let plot_height_estimate = (rect.height - title_h - MARGIN_BOTTOM).max(0.0);
        let target_y_ticks = resolve_tick_count(self.y_tick_policy, plot_height_estimate);
        // X-extreme overhang: how far the X axis's OWN leftmost/rightmost
        // tick label (center-aligned under its own tick) extends past its
        // own tick position — measured at `TARGET_X_TICKS` (the same
        // fixed constant `x_scale()`'s own nice-domain sizing uses, not
        // the plot-width-dependent adaptive count — computing THAT here
        // would be circular, since it needs the margin this measurement
        // itself feeds into). See `axis::measure_x_axis_extreme_overhang`'s
        // own doc comment for why this is a DIFFERENT gap than
        // `measure_y_axis_gutter` already covers.
        let x_overhang = x_scale.map(|s| axis::measure_x_axis_extreme_overhang(ctx, s, theme, TARGET_X_TICKS)).unwrap_or((0.0, 0.0));
        let margin_left = match (&y_scale_for_layout, self.margin_policy) {
            (Some(y_scale), MarginPolicy::Measured) => {
                MARGIN_LEFT.max(axis::measure_y_axis_gutter(ctx, y_scale, theme, target_y_ticks)).max(x_overhang.0)
            }
            _ => MARGIN_LEFT,
        };
        let margin_right = match self.margin_policy {
            MarginPolicy::Measured => MARGIN_RIGHT.max(x_overhang.1),
            MarginPolicy::Fixed => MARGIN_RIGHT,
        };
        let plot_width_estimate = (rect.width - margin_left - margin_right).max(0.0);
        let target_x_ticks = resolve_tick_count(self.x_tick_policy, plot_width_estimate);

        let base_rect = Rect::new(rect.x + margin_left, rect.y + title_h, plot_width_estimate, plot_height_estimate);
        let legend_position = self.resolved_legend_position();
        let legend_entries = if legend_position.is_some() { self.legend_entries(theme) } else { Vec::new() };

        let (plot_rect, legend_rect) = match legend_position {
            Some(pos) if !legend_entries.is_empty() => {
                let size = legend::measure_legend(ctx, theme, &legend_entries, pos, base_rect.width, base_rect.height);
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

        if let (Some(x_scale), Some(y_scale)) = (x_scale, y_scale_for_layout) {
            // Weighted entry points: a real render-output change ONLY when
            // `x_scale` is a `TimeScale` (`Scale::tick_weight` reports
            // `Some`) — every other X scale (the auto-computed
            // `LinearScale`) renders byte-identically to the unweighted
            // `draw_x_grid`/`draw_x_axis`, see `guide::grid`/`guide::axis`'s
            // own regression tests for the proof. Closes the "TimeScale's
            // tested tick-weight hierarchy never reaches the shared axis/
            // grid guides" audit finding for this figure's own X axis.
            grid::draw_x_grid_weighted(ctx, &area, x_scale, theme, target_x_ticks, &GridTickWeightStyle::default());
            grid::draw_y_grid(ctx, &area, &y_scale, theme, target_y_ticks);

            // Annotation FILLS (the only underlay: `HBand`'s own shaded
            // rect) paint UNDER the series — the typical "shaded zone sits
            // behind the line" convention, over the grid. Reference
            // LINES/LABELS/`Callout` paint AFTER the series below (see
            // `guide::annotation`'s own "Layer contract" doc comment) —
            // matches `ScatterFigure`'s own fix for the SAME class of
            // "dense marks swallow annotation text" defect.
            draw_annotation_underlays(ctx, &area, &y_scale, theme, &self.annotations);

            // Per-series LTTB-downsampled (or borrowed verbatim) point
            // set — the SAME set drawn below AND hit-tested against
            // (see `with_downsample`'s own docs: one-transform law, a
            // hover never resolves to a point that isn't actually drawn).
            let rendered: Vec<std::borrow::Cow<'_, [(f64, f64)]>> = self.series.iter().map(|s| self.rendered_points(s)).collect();

            // A viewport window can legitimately place points OUTSIDE the
            // visible plot rect — clip the series draw pass so those
            // points' own stroke/fill never bleed past the plot rect into
            // the axis-label margin. `windowed_x_scale.is_some()` is
            // `false` whenever `x_viewport` is `None` (`render_with`'s own
            // call path), so this is a complete no-op there — proven by
            // `render_with_viewport_with_no_viewport_matches_render_with`.
            let clip_active = windowed_x_scale.is_some();
            if clip_active {
                ctx.save();
                ctx.clip_rect(area.rect.x, area.rect.y, area.rect.width, area.rect.height);
            }
            for (i, s) in rendered.iter().enumerate() {
                let color = theme.palette[i % theme.palette.len()].clone();
                let style = MarkStyle { color, ..Default::default() };
                if self.fill {
                    let fill_style = MarkStyle { fill_alpha: FILL_ALPHA, ..style.clone() };
                    draw_area(ctx, &area, x_scale, &y_scale, s, &fill_style);
                }
                draw_polyline(ctx, &area, x_scale, &y_scale, s, &style);
            }
            if clip_active {
                ctx.restore();
            }

            draw_annotation_overlays(ctx, &area, x_scale, &y_scale, theme, &self.annotations);

            axis::draw_x_axis_weighted(ctx, &area, x_scale, theme, target_x_ticks, &AxisTickWeightStyle::default());
            axis::draw_y_axis(ctx, &area, &y_scale, theme, target_y_ticks);

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
                        let y_step = nice_step(y_scale.max - y_scale.min, target_y_ticks as f64);
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
    fn default_y_domain_policy_forces_a_zero_baseline_unchanged_from_before() {
        // Every point sits well above zero — the default (unset) policy
        // must still pull the domain down to include 0.0, exactly the
        // pre-existing behavior every caller today already relies on.
        let figure = CurveFigure::new(vec![(0.0, 100.0), (1.0, 110.0)]);
        let y = figure.y_scale().expect("2 points");
        assert!(y.min <= 0.0, "default policy must force a zero baseline, got min={}", y.min);
    }

    #[test]
    fn fit_data_y_domain_policy_never_forces_a_zero_baseline() {
        let figure = CurveFigure::new(vec![(0.0, 100.0), (1.0, 110.0)]).with_y_domain_policy(YDomainPolicy::FitData);
        let y = figure.y_scale().expect("2 points");
        assert!(y.min > 0.0, "FitData policy must fit the data, never force a zero baseline, got min={}", y.min);
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
    fn with_annotations_default_is_empty_and_render_still_succeeds() {
        use uzor_export::{render_to_png, ExportSpec};

        let figure = CurveFigure::new(vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5)]).with_annotations(vec![
            crate::guide::annotation::Annotation::HLine { value: 0.5, color: None, label: Some("mid".to_owned()) },
            crate::guide::annotation::Annotation::Callout { x: 1.0, y: 1.0, text: "peak".to_owned() },
        ]);
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 300.0, 200.0), &theme);
        });
        assert!(result.is_ok());
        assert!(CurveFigure::new(vec![(0.0, 0.0), (1.0, 1.0)]).annotations.is_empty());
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

    // ── MarginPolicy / TickCountPolicy (items 2 + 3) ────────────────────

    #[test]
    fn default_margin_policy_is_measured_and_default_tick_policies_match_the_pre_existing_constants() {
        let figure = CurveFigure::new(vec![(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(figure.margin_policy, MarginPolicy::Measured);
        assert_eq!(figure.x_tick_policy, TickCountPolicy::Fixed(TARGET_X_TICKS));
        assert_eq!(figure.y_tick_policy, TickCountPolicy::Fixed(TARGET_Y_TICKS));
    }

    #[test]
    fn measured_margin_grows_the_right_margin_to_protect_the_rightmost_x_label_from_clipping() {
        // Superseded claim, corrected: this fixture's short Y labels never
        // needed the LEFT margin to grow, but `MARGIN_RIGHT` (8.0px) is
        // narrower than ANY real numeric label's own half-width — before
        // `guide::axis::measure_x_axis_extreme_overhang` existed, NOTHING
        // protected the rightmost X tick's own label from clipping past
        // the plot rect's right edge, for even this ordinary,
        // unremarkable dataset (the owner-reported defect: a symlog
        // axis's own extreme label clipping in a narrow proof panel was
        // the visible symptom, but the underlying gap was general, not
        // symlog-specific). `Measured` must now render differently from
        // `Fixed` to reserve that room.
        use uzor_export::{render_to_png, ExportSpec};

        let points = vec![(0.0, 1.0), (1.0, 5.0), (2.0, 3.0), (3.0, 8.0)];
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);

        let fixed = CurveFigure::new(points.clone()).with_margin_policy(MarginPolicy::Fixed);
        let measured = CurveFigure::new(points).with_margin_policy(MarginPolicy::Measured);
        let fixed_png = render_to_png(&spec, |ctx| fixed.render(ctx, rect, &theme)).expect("fixed render");
        let measured_png = render_to_png(&spec, |ctx| measured.render(ctx, rect, &theme)).expect("measured render");
        assert_ne!(
            fixed_png, measured_png,
            "Measured must widen the right margin to protect the rightmost X label from the clipping Fixed's own tiny MARGIN_RIGHT allows"
        );
    }


    #[test]
    fn measured_margin_widens_and_shifts_the_plot_for_a_deliberately_wide_y_label() {
        use uzor_export::{render_to_png, ExportSpec};

        // A huge value forces a wide formatted tick label (many grouped
        // digits) that would not fit the pre-existing fixed MARGIN_LEFT.
        let points = vec![(0.0, 1.0), (1.0, 999_999_999.0)];
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);

        let fixed = CurveFigure::new(points.clone()).with_margin_policy(MarginPolicy::Fixed);
        let measured = CurveFigure::new(points).with_margin_policy(MarginPolicy::Measured);
        let fixed_png = render_to_png(&spec, |ctx| fixed.render(ctx, rect, &theme)).expect("fixed render");
        let measured_png = render_to_png(&spec, |ctx| measured.render(ctx, rect, &theme)).expect("measured render");
        assert_ne!(fixed_png, measured_png, "Measured must render differently once a wide Y label would otherwise clip under Fixed");
    }

    #[test]
    fn adaptive_tick_policy_requests_more_ticks_for_a_wider_plot() {
        assert_eq!(
            crate::figure::resolve_tick_count(TickCountPolicy::Fixed(6), 100.0),
            6,
            "Fixed must ignore available_px entirely"
        );
        let narrow = crate::figure::resolve_tick_count(TickCountPolicy::Adaptive { min: 2, max: 20 }, 140.0);
        let wide = crate::figure::resolve_tick_count(TickCountPolicy::Adaptive { min: 2, max: 20 }, 1400.0);
        assert!(wide > narrow, "a wider plot must resolve to MORE adaptive ticks (narrow={narrow}, wide={wide})");
    }

    #[test]
    fn adaptive_tick_policy_renders_without_panicking_and_a_default_fixed_render_stays_unaffected() {
        use uzor_export::{render_to_png, ExportSpec};

        let points: Vec<(f64, f64)> = (0..50).map(|i| (i as f64, (i as f64 * 1.7).sin() * 10.0 + 20.0)).collect();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 600, height_px: 300, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 600.0, 300.0);

        let fixed_default = CurveFigure::new(points.clone());
        let adaptive = CurveFigure::new(points.clone())
            .with_x_tick_policy(TickCountPolicy::Adaptive { min: 2, max: 30 })
            .with_y_tick_policy(TickCountPolicy::Adaptive { min: 2, max: 30 });

        let default_png = render_to_png(&spec, |ctx| fixed_default.render(ctx, rect, &theme)).expect("default render");
        let adaptive_result = render_to_png(&spec, |ctx| adaptive.render(ctx, rect, &theme));
        assert!(adaptive_result.is_ok(), "an adaptive tick policy must render without panicking");

        // A plain `CurveFigure::new` (Fixed at the pre-existing constants)
        // must be completely unaffected by this item's own new fields.
        let unaffected_png = render_to_png(&spec, |ctx| CurveFigure::new(points).render(ctx, rect, &theme)).expect("render");
        assert_eq!(default_png, unaffected_png);
    }

    // ── Wave 5 (Viewport pan/zoom/fit) ──────────────────────────────────

    use crate::interact::viewport::Viewport;

    fn viewport_fixture_points() -> Vec<(f64, f64)> {
        (0..100).map(|i| (i as f64, ((i as f64) * 0.3).sin() * 20.0 + 50.0)).collect()
    }

    #[test]
    fn render_with_viewport_with_no_viewport_matches_render_with() {
        // The binding doctrine's own gate: `x_viewport: None` must
        // reproduce `render_with`'s output byte-for-byte.
        use uzor_export::{render_to_png, ExportSpec};

        let figure = CurveFigure::new(viewport_fixture_points()).with_title("viewport-none");
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 250, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 400.0, 250.0);

        let via_render_with = render_to_png(&spec, |ctx| figure.render_with(ctx, rect, &theme, &FigureOverlay::default())).expect("render_with");
        let via_viewport_none =
            render_to_png(&spec, |ctx| figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), None)).expect("render_with_viewport(None)");
        assert_eq!(via_render_with, via_viewport_none, "render_with_viewport(.., None) must be byte-identical to render_with");

        // And `render()` itself (the plain V1 entry point) must ALSO be
        // completely unaffected by this figure gaining viewport support.
        let via_render = render_to_png(&spec, |ctx| figure.render(ctx, rect, &theme)).expect("render");
        assert_eq!(via_render_with, via_render);
    }

    #[test]
    fn a_zoomed_viewport_narrows_the_rendered_x_domain() {
        // Not just "renders without panicking" — actually resolves a
        // NARROWER windowed scale than the figure's own full auto domain.
        let figure = CurveFigure::new(viewport_fixture_points());
        let full_domain = figure.x_scale().expect("100 points").domain();

        let mut vp = Viewport::new(full_domain);
        let (full_min, full_max) = full_domain;
        let focal = (full_min + full_max) / 2.0;
        vp.zoom_at(focal, 5.0);
        let window = vp.window();

        assert!(window.1 - window.0 < full_max - full_min, "a zoomed viewport must produce a window narrower than the full domain");

        use uzor_export::{render_to_png, ExportSpec};
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 250, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 400.0, 250.0);
        let zoomed = render_to_png(&spec, |ctx| {
            figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp));
        })
        .expect("zoomed render");
        let unzoomed = render_to_png(&spec, |ctx| figure.render(ctx, rect, &theme)).expect("unzoomed render");
        assert_ne!(zoomed, unzoomed, "a genuinely zoomed viewport must render visibly differently from the unwindowed full-domain render");
    }

    #[test]
    fn a_panned_viewport_renders_differently_from_the_unpanned_window() {
        let figure = CurveFigure::new(viewport_fixture_points());
        let full_domain = figure.x_scale().expect("100 points").domain();

        let mut vp = Viewport::new(full_domain);
        let (full_min, full_max) = full_domain;
        vp.zoom_at((full_min + full_max) / 2.0, 4.0); // narrow first, so there's room to pan
        let before_pan_window = vp.window();

        use uzor_export::{render_to_png, ExportSpec};
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 250, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 400.0, 250.0);
        let pre_pan = render_to_png(&spec, |ctx| figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp))).expect("pre-pan render");

        vp.pan((full_max - full_min) * 0.1);
        assert_ne!(vp.window(), before_pan_window, "pan must actually move the window for this test to prove anything");
        let post_pan = render_to_png(&spec, |ctx| figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp))).expect("post-pan render");

        assert_ne!(pre_pan, post_pan, "panning the viewport must change the rendered output");
    }

    #[test]
    fn fit_to_data_after_zoom_restores_the_exact_full_domain_and_renders_correctly() {
        // `fit_to_data`'s own domain-level correctness claim: after a real
        // zoom, it restores `window() == data_domain()` EXACTLY (not
        // "close to"), and the windowed scale built from that window
        // resolves to the SAME (min, max) the figure's own unwindowed
        // scale would. NOT asserted as a byte-identical PNG: even with an
        // identical domain, `render_with_viewport` still wraps the series
        // draw pass in `save()/clip_rect()/restore()` whenever a viewport
        // is present at all (see that method's own doc comment) — a
        // rasterizer's own edge-antialiasing at a clip boundary can
        // legitimately differ by a few subpixel values from a completely
        // unclipped draw of the identical geometry, the same class of
        // "real backend differs at the pixel level despite being logically
        // correct" this crate's own multi-backend proof section already
        // documents (see `lib.rs`'s "Divergence policy" doctrine) — a
        // stricter byte-equality claim here would be an overclaim.
        let figure = CurveFigure::new(viewport_fixture_points());
        let full_domain = figure.x_scale().expect("100 points").domain();

        let mut vp = Viewport::new(full_domain);
        vp.zoom_at((full_domain.0 + full_domain.1) / 2.0, 6.0);
        assert_ne!(vp.window(), full_domain, "the zoom must actually narrow the window for this test to prove anything");
        vp.fit_to_data();
        assert_eq!(vp.window(), full_domain, "fit_to_data must restore the exact full data domain");

        let windowed = full_scale_domain_via_windowed(&figure, &vp);
        assert_eq!(windowed, full_domain, "the windowed scale built from a fit-to-data viewport must resolve the SAME domain as the unwindowed figure");

        use uzor_export::{render_to_png, ExportSpec};
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 250, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 400.0, 250.0);
        let result = render_to_png(&spec, |ctx| figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp)));
        assert!(result.is_ok(), "a viewport fit back to the full data domain must still render without panicking");
    }

    /// Resolve the SAME windowed `Scale` `render_with_viewport` would build
    /// for `figure`'s auto-computed X domain under `vp` — a direct,
    /// non-rendering proof that the windowing math itself is correct,
    /// independent of any rasterizer-level pixel comparison.
    fn full_scale_domain_via_windowed(figure: &CurveFigure, vp: &Viewport) -> (f64, f64) {
        let base = figure.x_scale().expect("figure must have a domain");
        let (window_min, window_max) = vp.window();
        Scale::windowed(&base, window_min, window_max).expect("LinearScale supports windowing").domain()
    }

    #[test]
    fn a_deeply_zoomed_viewport_re_derives_a_denser_tick_set() {
        // Ticks must re-derive for the visible window (never bypass the
        // shared tick machinery) — a deep zoom into a small sub-range
        // must produce ticks with a visibly finer step than the full,
        // unzoomed domain's own ticks.
        let figure = CurveFigure::new(viewport_fixture_points());
        let full_domain = figure.x_scale().expect("100 points").domain();
        let full_scale = LinearScale::new(full_domain.0, full_domain.1);
        let full_ticks = full_scale.ticks(TARGET_X_TICKS);
        let full_step = if full_ticks.len() >= 2 { (full_ticks[1].value - full_ticks[0].value).abs() } else { 1.0 };

        let mut vp = Viewport::new(full_domain);
        vp.zoom_at((full_domain.0 + full_domain.1) / 2.0, 20.0);
        let (w_min, w_max) = vp.window();
        let windowed_scale = LinearScale::new(w_min, w_max);
        let windowed_ticks = windowed_scale.ticks(TARGET_X_TICKS);
        let windowed_step = if windowed_ticks.len() >= 2 { (windowed_ticks[1].value - windowed_ticks[0].value).abs() } else { 1.0 };

        assert!(
            windowed_step < full_step,
            "a deeply zoomed window must re-derive a finer tick step than the full domain (full_step={full_step}, windowed_step={windowed_step})"
        );

        // And the real render path must actually route through this
        // windowed (finer) tick set, not the full domain's own.
        use uzor_export::{render_to_png, ExportSpec};
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 250, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 400.0, 250.0);
        let result = render_to_png(&spec, |ctx| {
            figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp));
        });
        assert!(result.is_ok(), "a deeply zoomed viewport must still render without panicking");
    }

    #[test]
    fn viewport_with_a_time_scale_override_windows_in_real_calendar_time() {
        // The exact claim the MLC-harvest doc's own domain-vs-bar-index
        // distinction is about: a Viewport pans/zooms a TimeScale-backed
        // CurveFigure in real UTC seconds, never an array/bar position.
        use crate::scale::TimeScale;

        const ANCHOR: f64 = 1_704_067_200.0; // 2024-01-01T00:00:00Z
        const DAY_SECS: f64 = 86_400.0;
        let points: Vec<(f64, f64)> = (0..90).map(|i| (ANCHOR + i as f64 * DAY_SECS, ((i as f64) * 0.2).sin() * 10.0 + 30.0)).collect();
        let x_min = points[0].0;
        let x_max = points[points.len() - 1].0;
        let figure = CurveFigure::new(points).with_x_scale(TimeScale::new(x_min, x_max));

        let mut vp = Viewport::new((x_min, x_max));
        // Zoom into a real 10-day window, anchored at day 30.
        let focal = ANCHOR + 30.0 * DAY_SECS;
        vp.zoom_at(focal, 9.0);
        let (w_min, w_max) = vp.window();
        assert!(w_max - w_min < 15.0 * DAY_SECS, "a zoomed TimeScale viewport must window down to roughly a 10-day span, got {} seconds", w_max - w_min);

        use uzor_export::{render_to_png, ExportSpec};
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 500, height_px: 300, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 500.0, 300.0);
        let zoomed = render_to_png(&spec, |ctx| figure.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp))).expect("zoomed time render");
        let full = render_to_png(&spec, |ctx| figure.render(ctx, rect, &theme)).expect("full time render");
        assert_ne!(zoomed, full, "a real calendar-time zoom must render visibly differently from the full 90-day view");
    }

    #[test]
    fn viewport_on_a_figure_with_fewer_than_two_points_renders_without_panicking() {
        // Degenerate case: an empty/single-point figure never resolves an
        // X scale at all (`x_scale()` returns `None`), so a viewport must
        // be a harmless no-op rather than panicking on a missing domain.
        use uzor_export::{render_to_png, ExportSpec};

        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 200, height_px: 150, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 200.0, 150.0);
        let vp = Viewport::new((0.0, 1.0));

        let empty = CurveFigure::new(Vec::new());
        let result = render_to_png(&spec, |ctx| empty.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp)));
        assert!(result.is_ok(), "an empty figure with a viewport must render without panicking");

        let single = CurveFigure::new(vec![(5.0, 5.0)]);
        let result = render_to_png(&spec, |ctx| single.render_with_viewport(ctx, rect, &theme, &FigureOverlay::default(), Some(&vp)));
        assert!(result.is_ok(), "a single-point figure with a viewport must render without panicking");
    }
}
