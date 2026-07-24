//! `WaterfallFigure` — a running-total "bridge" chart: discrete signed
//! deltas flow into (and out of) a cumulative total, with periodic
//! checkpoint bars.
//!
//! FT hygiene (business-chart research doc §6): a waterfall is a
//! FLOW-category chart, not a stacked-bar variant — every [`WaterfallItem`]
//! contributes its `value` (a signed delta) to a running sum
//! ([`compute_steps`]), but [`WaterfallKind::Delta`] bars FLOAT between the
//! previous and new running total while [`WaterfallKind::Subtotal`]/
//! [`WaterfallKind::Total`] bars drop to the zero baseline (a full
//! ground-to-value bar, visually marking "this is a checkpoint/total, not
//! a delta") — the professional-read rule this figure bakes in as a
//! default, not an option. A thin connector line bridges each bar's
//! arrival level to the next bar's own edge (FT convention); positive
//! deltas paint in [`crate::theme::FigureTheme::positive`], negative in
//! `negative`, checkpoints in the theme's neutral `label_color`.
//!
//! Reuses the SAME [`crate::scale::BandScale`] x-axis + nice-rounded
//! [`crate::scale::LinearScale`] y-axis + [`crate::guide::axis`]/
//! [`crate::guide::grid`] machinery [`crate::figure::BarFigure`] already
//! established — a waterfall's own geometry ([`layout_bars`]/
//! [`layout_connectors`]) is genuinely new (running-sum bar extents +
//! inter-bar connectors), but the axis/grid painting underneath it is not
//! reinvented.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::{resolve_tick_count, FigureOverlay, MarginPolicy, TickCountPolicy};
use crate::guide::{axis, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::mark::text::draw_label_centered;
use crate::scale::linear::{format_value, nice_step};
use crate::scale::{BandScale, LinearScale, Scale};
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 52.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_Y_TICKS: usize = 5;
const BAND_PADDING: f64 = 0.25;
const CONNECTOR_GAP: f64 = 0.0;
const VALUE_LABEL_GAP: f64 = 6.0;
const HOVER_HIGHLIGHT_ALPHA: f64 = 0.22;

/// Which running-sum role an item plays — see the module docs for exactly
/// how each kind's own bar extent differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterfallKind {
    /// Floats between the previous and new running total.
    Delta,
    /// Drops to the zero baseline (a checkpoint, still contributes `value`
    /// to the running sum — pass `0.0` for a pure "re-anchor here, no
    /// change" checkpoint).
    Subtotal,
    /// Same visual treatment as [`WaterfallKind::Subtotal`] (ground-to-
    /// value bar) — kept as a distinct variant purely so a caller/theme
    /// can tell "the grand total" apart from an intermediate subtotal if
    /// it ever wants to (this figure itself paints both identically).
    Total,
}

/// One item in the bridge: a signed `value` delta contributing to the
/// running sum, and a [`WaterfallKind`] controlling how its OWN bar is
/// drawn (float vs. drop-to-baseline).
#[derive(Debug, Clone)]
pub struct WaterfallItem {
    pub label: String,
    pub value: f64,
    pub kind: WaterfallKind,
}

/// One item's own resolved running-sum step — `start_value`/`end_value`
/// are the bar's own domain-space vertical extent (unordered: for a
/// negative [`WaterfallKind::Delta`], `start_value > end_value`).
#[derive(Debug, Clone, Copy)]
pub struct WaterfallStep {
    pub running_before: f64,
    pub running_after: f64,
    pub start_value: f64,
    pub end_value: f64,
}

/// Walk `items` left to right, accumulating a running sum — EVERY item's
/// `value` (regardless of `kind`) adds algebraically
/// (`running_after = running_before + value`); only the BAR's own visual
/// extent differs by `kind` (see the module docs).
pub fn compute_steps(items: &[WaterfallItem]) -> Vec<WaterfallStep> {
    let mut running = 0.0_f64;
    items
        .iter()
        .map(|item| {
            let running_before = running;
            let running_after = running_before + item.value;
            running = running_after;
            let (start_value, end_value) = match item.kind {
                WaterfallKind::Delta => (running_before, running_after),
                WaterfallKind::Subtotal | WaterfallKind::Total => (0.0, running_after),
            };
            WaterfallStep { running_before, running_after, start_value, end_value }
        })
        .collect()
}

fn y_domain(steps: &[WaterfallStep]) -> (f64, f64) {
    steps
        .iter()
        .fold((0.0_f64, 0.0_f64), |(mn, mx), s| (mn.min(s.start_value).min(s.end_value), mx.max(s.start_value).max(s.end_value)))
}

/// One bar's screen-pixel geometry — `top_px`/`bottom_px` already ordered
/// (`top_px <= bottom_px`), ready to `fill_rect` directly.
#[derive(Debug, Clone, Copy)]
pub struct WaterfallBar {
    pub item_index: usize,
    pub x0: f64,
    pub x1: f64,
    pub top_px: f64,
    pub bottom_px: f64,
}

/// Pure per-bar pixel layout over `steps` (from [`compute_steps`]) through
/// `area`/`band`/`y` — the SAME geometry [`WaterfallFigure::render_with`]
/// paints with and a caller's own hover routing hit-tests through (design
/// law #1).
pub fn layout_bars(steps: &[WaterfallStep], area: &PlotArea, band: &BandScale, y: &dyn Scale) -> Vec<WaterfallBar> {
    steps
        .iter()
        .enumerate()
        .take(band.len())
        .map(|(i, s)| {
            let (x0, x1) = area.x_band(band, i);
            let top_px = area.y(y, s.start_value).min(area.y(y, s.end_value));
            let bottom_px = area.y(y, s.start_value).max(area.y(y, s.end_value));
            WaterfallBar { item_index: i, x0, x1, top_px, bottom_px }
        })
        .collect()
}

/// One connector segment: a horizontal line at `y_px` bridging bar `i`'s
/// own right edge to bar `i+1`'s own left edge, at the pixel level of the
/// running total flowing between them (`steps[i].running_after ==
/// steps[i+1].running_before` by construction — see [`compute_steps`]).
#[derive(Debug, Clone, Copy)]
pub struct WaterfallConnector {
    pub x0: f64,
    pub x1: f64,
    pub y_px: f64,
}

/// Pure connector layout — one entry per adjacent item pair, same
/// design-law-#1 split as [`layout_bars`].
pub fn layout_connectors(steps: &[WaterfallStep], area: &PlotArea, band: &BandScale, y: &dyn Scale) -> Vec<WaterfallConnector> {
    if steps.len() < 2 || band.len() < 2 {
        return Vec::new();
    }
    (0..steps.len().min(band.len()) - 1)
        .map(|i| {
            let (_, x0) = area.x_band(band, i);
            let (x1, _) = area.x_band(band, i + 1);
            let y_px = area.y(y, steps[i].running_after);
            WaterfallConnector { x0: x0 + CONNECTOR_GAP, x1: x1 - CONNECTOR_GAP, y_px }
        })
        .collect()
}

/// A running-total bridge chart — see the module docs.
pub struct WaterfallFigure {
    pub items: Vec<WaterfallItem>,
    title: Option<String>,
    /// Inner padding (fraction of each band's width) — see
    /// [`WaterfallFigure::with_band_padding`]. Defaults to [`BAND_PADDING`].
    band_padding: f64,
    /// This figure's own left-margin sizing policy — see
    /// [`WaterfallFigure::with_margin_policy`].
    margin_policy: MarginPolicy,
    /// This figure's own Y tick-count policy — see
    /// [`WaterfallFigure::with_y_tick_policy`].
    y_tick_policy: TickCountPolicy,
}

impl WaterfallFigure {
    pub fn new(items: Vec<WaterfallItem>) -> Self {
        Self {
            items,
            title: None,
            band_padding: BAND_PADDING,
            margin_policy: MarginPolicy::default(),
            y_tick_policy: TickCountPolicy::Fixed(TARGET_Y_TICKS),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Override this figure's band inner padding (fraction of each band's
    /// own width used as the gap between bands, clamped `0.0..=0.9` by
    /// [`BandScale::new`]) — the "bar width ratio" a caller couldn't
    /// previously reach despite [`BandScale::new`] already accepting an
    /// arbitrary value. Default (unset) is [`BAND_PADDING`], byte-identical
    /// to this figure's own pre-existing constant.
    pub fn with_band_padding(mut self, padding: f64) -> Self {
        self.band_padding = padding;
        self
    }

    /// Override this figure's left-margin sizing policy — see
    /// [`MarginPolicy`]'s own docs. Default (unset) is
    /// [`MarginPolicy::Measured`].
    pub fn with_margin_policy(mut self, policy: MarginPolicy) -> Self {
        self.margin_policy = policy;
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
    /// the EXACT transform this figure renders with). **Does not account
    /// for [`MarginPolicy::Measured`] widening the left margin** — same
    /// ctx-less-accessor caveat documented on
    /// [`crate::figure::BarFigure::plot_area`].
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.base_plot_rect(rect))
    }

    /// This figure's own category band scale, one band per item.
    pub fn band_scale(&self) -> BandScale {
        BandScale::new(self.items.iter().map(|i| i.label.clone()).collect(), self.band_padding)
    }

    /// This figure's own resolved running-sum steps.
    pub fn steps(&self) -> Vec<WaterfallStep> {
        compute_steps(&self.items)
    }

    /// Nice-rounded Y domain over every step's own bar extent (always
    /// includes `0.0` implicitly — every [`WaterfallKind::Subtotal`]/
    /// [`WaterfallKind::Total`] bar starts there, and [`compute_steps`]
    /// begins its own running sum at `0.0`).
    fn y_scale(&self) -> Option<LinearScale> {
        if self.items.is_empty() {
            return None;
        }
        let steps = self.steps();
        let (mn, mx) = y_domain(&steps);
        Some(LinearScale::nice(mn, mx, TARGET_Y_TICKS))
    }

    fn bar_color<'a>(&self, item: &WaterfallItem, theme: &'a FigureTheme) -> &'a str {
        match item.kind {
            WaterfallKind::Delta if item.value >= 0.0 => &theme.positive,
            WaterfallKind::Delta => &theme.negative,
            WaterfallKind::Subtotal | WaterfallKind::Total => &theme.label_color,
        }
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a bar
    /// brightens it and shows a label/delta/cumulative tooltip.
    /// `overlay.focus`/`overlay.brush` are not consumed by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        if let Some(y_scale) = self.y_scale() {
            let band = self.band_scale();
            let steps = self.steps();

            // Resolve this render's own left margin + Y tick count BEFORE
            // building the plot rect — see `CurveFigure::render_with`'s
            // own identical non-circularity reasoning.
            let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
            let plot_height_estimate = (rect.height - title_h - MARGIN_BOTTOM).max(0.0);
            let target_y_ticks = resolve_tick_count(self.y_tick_policy, plot_height_estimate);
            let margin_left = match self.margin_policy {
                MarginPolicy::Measured => MARGIN_LEFT.max(axis::measure_y_axis_gutter(ctx, &y_scale, theme, target_y_ticks)),
                MarginPolicy::Fixed => MARGIN_LEFT,
            };
            let area = PlotArea::new(Rect::new(
                rect.x + margin_left,
                rect.y + title_h,
                (rect.width - margin_left - MARGIN_RIGHT).max(0.0),
                plot_height_estimate,
            ));

            let step_for_fmt = nice_step(y_scale.max - y_scale.min, target_y_ticks as f64);

            grid::draw_y_grid(ctx, &area, &y_scale, theme, target_y_ticks);

            let bars = layout_bars(&steps, &area, &band, &y_scale);
            for bar in &bars {
                let Some(item) = self.items.get(bar.item_index) else { continue };
                ctx.set_fill_color(self.bar_color(item, theme));
                ctx.fill_rect(bar.x0, bar.top_px, (bar.x1 - bar.x0).max(0.0), (bar.bottom_px - bar.top_px).max(0.0));
            }

            let connectors = layout_connectors(&steps, &area, &band, &y_scale);
            ctx.set_stroke_color(&theme.axis_color);
            ctx.set_stroke_width(1.0);
            ctx.set_line_dash(&[]);
            for c in &connectors {
                ctx.begin_path();
                ctx.move_to(c.x0, c.y_px);
                ctx.line_to(c.x1, c.y_px);
                ctx.stroke();
            }

            for (bar, item) in bars.iter().zip(self.items.iter()) {
                let text = match item.kind {
                    WaterfallKind::Delta => {
                        let sign = if item.value >= 0.0 { "+" } else { "" };
                        format!("{sign}{}", format_value(item.value, step_for_fmt))
                    }
                    WaterfallKind::Subtotal | WaterfallKind::Total => {
                        format_value(steps[bar.item_index].running_after, step_for_fmt)
                    }
                };
                draw_label_centered(ctx, &text, (bar.x0 + bar.x1) / 2.0, bar.top_px - VALUE_LABEL_GAP, &theme.label_color, &theme.label_font);
            }

            if let Some((hx, hy)) = overlay.hover_px {
                if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                    if let Some(i) = hit::bar_index_at(&area, &band, hx) {
                        if let (Some(bar), Some(item), Some(step)) = (bars.get(i), self.items.get(i), steps.get(i)) {
                            ctx.set_fill_color(&theme.highlight);
                            ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                            ctx.fill_rect(bar.x0, bar.top_px, (bar.x1 - bar.x0).max(0.0), (bar.bottom_px - bar.top_px).max(0.0));
                            ctx.set_global_alpha(1.0);

                            let lines = vec![
                                ("label".to_owned(), item.label.clone()),
                                ("delta".to_owned(), format_value(item.value, step_for_fmt)),
                                ("cumulative".to_owned(), format_value(step.running_after, step_for_fmt)),
                            ];
                            tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, area.rect);
                        }
                    }
                }
            }

            axis::draw_x_axis(ctx, &area, &band, theme, band.len());
            axis::draw_y_axis(ctx, &area, &y_scale, theme, target_y_ticks);
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, value: f64, kind: WaterfallKind) -> WaterfallItem {
        WaterfallItem { label: label.to_owned(), value, kind }
    }

    #[test]
    fn compute_steps_running_sums_are_correct_including_negatives() {
        let items = vec![
            item("start", 100.0, WaterfallKind::Total),
            item("gain-a", 20.0, WaterfallKind::Delta),
            item("gain-b", 15.0, WaterfallKind::Delta),
            item("loss", -30.0, WaterfallKind::Delta),
            item("end", 0.0, WaterfallKind::Total),
        ];
        let steps = compute_steps(&items);
        assert!((steps[0].running_after - 100.0).abs() < 1e-9);
        assert!((steps[1].running_after - 120.0).abs() < 1e-9);
        assert!((steps[2].running_after - 135.0).abs() < 1e-9);
        assert!((steps[3].running_after - 105.0).abs() < 1e-9, "negative delta must subtract from the running total");
        assert!((steps[4].running_after - 105.0).abs() < 1e-9, "a zero-value Total checkpoint must not change the running sum");
    }

    #[test]
    fn delta_bar_floats_between_previous_and_new_running_total() {
        let items = vec![item("start", 50.0, WaterfallKind::Total), item("delta", 20.0, WaterfallKind::Delta)];
        let steps = compute_steps(&items);
        assert!((steps[1].start_value - 50.0).abs() < 1e-9);
        assert!((steps[1].end_value - 70.0).abs() < 1e-9);
    }

    #[test]
    fn negative_delta_bar_still_floats_with_start_above_end() {
        let items = vec![item("start", 50.0, WaterfallKind::Total), item("delta", -20.0, WaterfallKind::Delta)];
        let steps = compute_steps(&items);
        assert!((steps[1].start_value - 50.0).abs() < 1e-9);
        assert!((steps[1].end_value - 30.0).abs() < 1e-9);
    }

    #[test]
    fn subtotal_bar_drops_to_the_zero_baseline_regardless_of_running_before() {
        let items = vec![
            item("start", 50.0, WaterfallKind::Total),
            item("gain", 30.0, WaterfallKind::Delta),
            item("checkpoint", 0.0, WaterfallKind::Subtotal),
        ];
        let steps = compute_steps(&items);
        assert!((steps[2].running_before - 80.0).abs() < 1e-9, "sanity: the checkpoint's own arrival level is 80");
        assert_eq!(steps[2].start_value, 0.0, "a Subtotal bar's own start_value must be the zero baseline, never running_before");
        assert!((steps[2].end_value - 80.0).abs() < 1e-9);
    }

    #[test]
    fn total_bar_also_drops_to_the_zero_baseline() {
        let items = vec![item("start", 10.0, WaterfallKind::Total), item("end", 0.0, WaterfallKind::Total)];
        let steps = compute_steps(&items);
        assert_eq!(steps[1].start_value, 0.0);
    }

    #[test]
    fn connectors_join_the_exact_touching_edges_of_two_adjacent_delta_bars() {
        // Two positive deltas back to back: bar 0 spans [0,10], bar 1
        // spans [10,25] — the connector between them must sit at pixel
        // level for domain value 10, which is BOTH bar 0's own bottom-most
        // pixel-space edge value... more precisely: bar 0's own
        // start_value (0) is the baseline; bar 0's END (10, running_after)
        // is the shared touching level with bar 1's START (10).
        let items = vec![item("a", 10.0, WaterfallKind::Delta), item("b", 15.0, WaterfallKind::Delta)];
        let steps = compute_steps(&items);
        let band = BandScale::new(vec!["a".to_owned(), "b".to_owned()], 0.1);
        let y = LinearScale::nice(0.0, 25.0, 5);
        let area = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));

        let bars = layout_bars(&steps, &area, &band, &y);
        let connectors = layout_connectors(&steps, &area, &band, &y);
        assert_eq!(connectors.len(), 1);

        let expected_y = area.y(&y, 10.0);
        assert!((connectors[0].y_px - expected_y).abs() < 1e-6);
        // That level must be exactly bar 0's own top edge (its end_value,
        // the larger of the pair for a positive delta)...
        assert!((bars[0].top_px - expected_y).abs() < 1e-6);
        // ...and exactly bar 1's own bottom edge (its start_value).
        assert!((bars[1].bottom_px - expected_y).abs() < 1e-6);
    }

    #[test]
    fn connectors_span_from_the_left_bars_right_edge_to_the_right_bars_left_edge() {
        let items = vec![item("a", 10.0, WaterfallKind::Delta), item("b", 5.0, WaterfallKind::Delta)];
        let steps = compute_steps(&items);
        let band = BandScale::new(vec!["a".to_owned(), "b".to_owned()], 0.1);
        let y = LinearScale::nice(0.0, 15.0, 5);
        let area = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));

        let connectors = layout_connectors(&steps, &area, &band, &y);
        let (_, x0_expected) = area.x_band(&band, 0);
        let (x1_expected, _) = area.x_band(&band, 1);
        assert!((connectors[0].x0 - x0_expected).abs() < 1e-6);
        assert!((connectors[0].x1 - x1_expected).abs() < 1e-6);
    }

    #[test]
    fn empty_or_single_item_produces_no_connectors_without_panicking() {
        assert!(layout_connectors(&[], &PlotArea::new(Rect::new(0.0, 0.0, 10.0, 10.0)), &BandScale::new(Vec::new(), 0.1), &LinearScale::new(0.0, 1.0))
            .is_empty());
        let items = vec![item("only", 5.0, WaterfallKind::Total)];
        let steps = compute_steps(&items);
        let band = BandScale::new(vec!["only".to_owned()], 0.1);
        let y = LinearScale::nice(0.0, 5.0, 5);
        let area = PlotArea::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(layout_connectors(&steps, &area, &band, &y).is_empty());
    }

    #[test]
    fn default_band_padding_matches_the_pre_existing_constant() {
        let items = vec![item("only", 5.0, WaterfallKind::Total)];
        let figure = WaterfallFigure::new(items);
        assert!((figure.band_scale().padding - BAND_PADDING).abs() < 1e-9);
    }

    #[test]
    fn with_band_padding_overrides_the_default_and_is_reflected_in_the_band_scale() {
        let items = vec![item("only", 5.0, WaterfallKind::Total)];
        let figure = WaterfallFigure::new(items).with_band_padding(0.5);
        assert!((figure.band_scale().padding - 0.5).abs() < 1e-9);
    }

    #[test]
    fn empty_figure_renders_without_panicking() {
        let figure = WaterfallFigure::new(Vec::new());
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 300.0, 200.0), &theme);
        });
        assert!(result.is_ok(), "an empty waterfall figure must render without panicking");
    }

    // ── MarginPolicy / TickCountPolicy (items 2 + 3) ────────────────────

    #[test]
    fn default_policies_match_the_pre_existing_constant() {
        let figure = WaterfallFigure::new(vec![item("only", 5.0, WaterfallKind::Total)]);
        assert_eq!(figure.margin_policy, MarginPolicy::Measured);
        assert_eq!(figure.y_tick_policy, TickCountPolicy::Fixed(TARGET_Y_TICKS));
    }

    #[test]
    fn measured_margin_widens_for_a_deliberately_wide_y_label() {
        use uzor_export::{render_to_png, ExportSpec};

        let items = vec![item("start", 999_999_999.0, WaterfallKind::Total)];
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);

        let fixed = WaterfallFigure::new(items.clone()).with_margin_policy(MarginPolicy::Fixed);
        let measured = WaterfallFigure::new(items).with_margin_policy(MarginPolicy::Measured);
        let fixed_png = render_to_png(&spec, |ctx| fixed.render(ctx, rect, &theme)).expect("fixed render");
        let measured_png = render_to_png(&spec, |ctx| measured.render(ctx, rect, &theme)).expect("measured render");
        assert_ne!(fixed_png, measured_png, "Measured must render differently once a wide Y label would otherwise clip under Fixed");
    }

    #[test]
    fn adaptive_y_tick_policy_renders_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};

        let items = vec![
            item("start", 100.0, WaterfallKind::Total),
            item("gain", 20.0, WaterfallKind::Delta),
            item("loss", -15.0, WaterfallKind::Delta),
            item("end", 0.0, WaterfallKind::Total),
        ];
        let figure = WaterfallFigure::new(items).with_y_tick_policy(TickCountPolicy::Adaptive { min: 2, max: 15 });
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| figure.render(ctx, Rect::new(0.0, 0.0, 400.0, 300.0), &theme));
        assert!(result.is_ok());
    }
}
