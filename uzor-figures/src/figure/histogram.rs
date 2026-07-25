//! `HistogramFigure` — equal-width binning over a nice-rounded domain,
//! rendered as a [`BandFigure`](super::bars)-style bar chart. Binning
//! itself (Engine-strengthening WAVE 4b) is now [`crate::transform::bin`]
//! — a pure, independently-testable, REUSABLE transform (not entangled
//! with painting, and no longer private to this figure) — re-exported
//! here (`bin`/`resolve_bin_count`/`Bin`/`BinPolicy`) so every existing
//! `uzor_figures::figure::histogram::{...}`/`uzor_figures::{...}` import
//! path keeps resolving unchanged. See `transform::bin`'s own module doc
//! for the one disclosed signature change this move made (`bin`'s own
//! free function now takes a [`BinPolicy`] directly, not a raw
//! `bin_count: usize` — that raw-count entry point is [`crate::transform::
//! bin::bin_by_count`] now).

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::{resolve_tick_count, FigureOverlay, MarginPolicy, TickCountPolicy};
use crate::guide::axis::LabelOverflow;
use crate::guide::{axis, grid};
use crate::mark::rect::draw_bars;
use crate::mark::MarkStyle;
use crate::scale::linear::format_value;
use crate::scale::{BandScale, LinearScale, Scale};
use crate::theme::FigureTheme;

pub use crate::transform::bin::{bin, resolve_bin_count, Bin, BinPolicy};

const MARGIN_LEFT: f64 = 48.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_Y_TICKS: usize = 5;
/// Cap on how many of a wide bin count get x-axis labels — a 50-bin
/// histogram with one label per bar is unreadable regardless of
/// available width.
const MAX_X_LABELS: usize = 10;

/// A histogram over raw `samples`, binned at render time via its own
/// [`BinPolicy`] (default: the caller-supplied manual count — see
/// [`HistogramFigure::new`]).
pub struct HistogramFigure {
    pub samples: Vec<f64>,
    bin_policy: BinPolicy,
    pub title: Option<String>,
    /// This figure's own left-margin sizing policy — see
    /// [`HistogramFigure::with_margin_policy`].
    margin_policy: MarginPolicy,
    /// This figure's own Y tick-count policy — see
    /// [`HistogramFigure::with_y_tick_policy`].
    y_tick_policy: TickCountPolicy,
    /// This figure's own X-axis label-collision policy — see
    /// [`HistogramFigure::with_label_overflow`].
    label_overflow: LabelOverflow,
}

impl HistogramFigure {
    pub fn new(samples: Vec<f64>, bin_count: usize) -> Self {
        Self {
            samples,
            bin_policy: BinPolicy::Manual(bin_count),
            title: None,
            margin_policy: MarginPolicy::default(),
            y_tick_policy: TickCountPolicy::Fixed(TARGET_Y_TICKS),
            label_overflow: LabelOverflow::default(),
        }
    }

    /// Construct with an automatic bin-count rule instead of a
    /// caller-chosen count — see [`BinPolicy`]'s own docs for each rule.
    pub fn with_auto_bins(samples: Vec<f64>, policy: BinPolicy) -> Self {
        Self {
            samples,
            bin_policy: policy,
            title: None,
            margin_policy: MarginPolicy::default(),
            y_tick_policy: TickCountPolicy::Fixed(TARGET_Y_TICKS),
            label_overflow: LabelOverflow::default(),
        }
    }

    /// Override this figure's bin-count policy — see [`BinPolicy`]'s own
    /// docs. Same additive-builder shape as every other optional
    /// capability on this figure.
    pub fn with_bin_policy(mut self, policy: BinPolicy) -> Self {
        self.bin_policy = policy;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
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

    /// Override this figure's X-axis label-collision policy — see
    /// [`LabelOverflow`]'s own docs. Default (unset) is
    /// [`LabelOverflow::Skip`], byte-identical to this figure's
    /// pre-existing greedy-skip behavior. A wide bin count is the
    /// audit's own named example for this item.
    pub fn with_label_overflow(mut self, overflow: LabelOverflow) -> Self {
        self.label_overflow = overflow;
        self
    }

    /// This figure's resolved bin count — [`resolve_bin_count`] over its
    /// own `samples` and [`BinPolicy`].
    pub fn bin_count(&self) -> usize {
        resolve_bin_count(self.bin_policy, &self.samples)
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

    /// This figure's plot-area transform for `rect` — exposed for the
    /// same reason as [`crate::figure::CurveFigure::plot_area`]. **Does
    /// not account for [`MarginPolicy::Measured`] widening the left
    /// margin** — same ctx-less-accessor caveat documented on
    /// [`crate::figure::BarFigure::plot_area`].
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.plot_rect(rect))
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to
    /// `overlay.brush`: bins whose domain-X range overlaps the brush
    /// interval get redrawn in an accent color — the linked-brush half of
    /// the report's #2<->#5 shared-time-brush requirement, here linking a
    /// curve figure's drag to this figure's bins (see
    /// [`crate::figure::FigureOverlay`]). `overlay.hover_px`/`overlay.focus`
    /// are not consumed by this figure in V2.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let bins = bin(&self.samples, self.bin_policy);
        if !bins.is_empty() {
            // Bin edges labeled at the bin width's own precision — a
            // histogram over small fractional data shouldn't collapse
            // every label to "0.0" just because it uses a fixed decimal
            // count.
            let bin_width = (bins[0].range.1 - bins[0].range.0).max(f64::EPSILON);
            let labels: Vec<String> = bins.iter().map(|b| format_value(b.range.0, bin_width)).collect();
            let values: Vec<f64> = bins.iter().map(|b| b.count as f64).collect();

            let band = BandScale::new(labels, 0.05);
            let max_count = values.iter().copied().fold(0.0_f64, f64::max);
            let y_scale = LinearScale::nice(0.0, max_count, TARGET_Y_TICKS);

            // Resolve this render's own margins + Y tick count BEFORE
            // building the plot rect — see `CurveFigure::render_with`'s
            // own identical non-circularity reasoning; `BarFigure::
            // render_with`'s own comment covers why `margin_bottom` is
            // resolved first (it doesn't depend on `margin_left`).
            let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
            let x_target = band.len().min(MAX_X_LABELS);
            let margin_bottom = match self.label_overflow {
                LabelOverflow::Skip => MARGIN_BOTTOM,
                LabelOverflow::Rotate(degrees) => MARGIN_BOTTOM.max(axis::measure_rotated_x_axis_gutter(ctx, &band, theme, x_target, degrees)),
                LabelOverflow::Auto => MARGIN_BOTTOM.max(axis::measure_rotated_x_axis_gutter(ctx, &band, theme, x_target, axis::AUTO_ROTATE_DEGREES)),
            };
            let plot_height_estimate = (rect.height - title_h - margin_bottom).max(0.0);
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

            grid::draw_y_grid(ctx, &area, &y_scale, theme, target_y_ticks);
            let style = MarkStyle { color: theme.palette[2].clone(), ..Default::default() };
            draw_bars(ctx, &area, &band, &y_scale, &values, &style);

            if let Some((b0, b1)) = overlay.brush {
                let (lo, hi) = (b0.min(b1), b0.max(b1));
                let (y_min, y_max) = y_scale.domain();
                let baseline_value = 0.0_f64.clamp(y_min.min(y_max), y_min.max(y_max));
                let baseline_px = area.y(&y_scale, baseline_value);
                let accent = &theme.palette[3];
                ctx.set_fill_color(accent);
                for (i, b) in bins.iter().enumerate() {
                    if b.range.1 > lo && b.range.0 < hi {
                        let (x0, x1) = area.x_band(&band, i);
                        let value_px = area.y(&y_scale, values[i]);
                        let (top, height) =
                            if value_px <= baseline_px { (value_px, baseline_px - value_px) } else { (baseline_px, value_px - baseline_px) };
                        ctx.fill_rect(x0, top, (x1 - x0).max(0.0), height);
                    }
                }
            }

            axis::draw_x_axis_overflow(ctx, &area, &band, theme, x_target, self.label_overflow);
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

    // Low-level `bin_by_count`/`resolve_bin_count`/`BinPolicy` golden
    // tests moved verbatim to `transform::bin`'s own test module (Engine-
    // strengthening WAVE 4b — see that module's own doc comment for the
    // full "why" of the move). This module keeps only `HistogramFigure`-
    // LEVEL behavior: the byte-identical-default proof and the
    // render-without-panicking sweep across every `BinPolicy` variant.

    #[test]
    fn new_defaults_to_manual_bin_policy_byte_identical_to_pre_existing_behavior() {
        let figure = HistogramFigure::new(vec![1.0, 2.0, 3.0, 4.0, 5.0], 5);
        assert_eq!(figure.bin_count(), 5, "HistogramFigure::new must still resolve to exactly the caller-supplied bin_count");
    }

    #[test]
    fn with_auto_bins_and_with_bin_policy_render_without_panicking_for_every_rule() {
        use uzor_export::{render_to_png, ExportSpec};

        let samples: Vec<f64> = (0..200).map(|i| ((i * 37) % 97) as f64 * 0.5).collect();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        for policy in [BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott, BinPolicy::Manual(12)] {
            let figure = HistogramFigure::with_auto_bins(samples.clone(), policy).with_title("auto bins");
            let result = render_to_png(&spec, |ctx| {
                figure.render(ctx, Rect::new(0.0, 0.0, 300.0, 200.0), &theme);
            });
            assert!(result.is_ok(), "{policy:?} must render without panicking");

            let via_builder = HistogramFigure::new(samples.clone(), 5).with_bin_policy(policy);
            assert_eq!(via_builder.bin_count(), figure.bin_count(), "with_bin_policy must resolve identically to with_auto_bins for the same policy");
        }
    }

    // ── MarginPolicy / TickCountPolicy (items 2 + 3) ────────────────────

    #[test]
    fn default_policies_match_the_pre_existing_constant() {
        let figure = HistogramFigure::new(vec![1.0, 2.0, 3.0], 5);
        assert_eq!(figure.margin_policy, MarginPolicy::Measured);
        assert_eq!(figure.y_tick_policy, TickCountPolicy::Fixed(TARGET_Y_TICKS));
    }

    #[test]
    fn measured_margin_widens_for_a_deliberately_wide_y_count_label() {
        use uzor_export::{render_to_png, ExportSpec};

        // A single bin with an enormous count forces a wide Y tick label.
        let samples: Vec<f64> = std::iter::repeat(1.0).take(2000).collect();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);

        let fixed = HistogramFigure::new(samples.clone(), 4).with_margin_policy(MarginPolicy::Fixed);
        let measured = HistogramFigure::new(samples, 4).with_margin_policy(MarginPolicy::Measured);
        let fixed_png = render_to_png(&spec, |ctx| fixed.render(ctx, rect, &theme)).expect("fixed render");
        let measured_png = render_to_png(&spec, |ctx| measured.render(ctx, rect, &theme)).expect("measured render");
        assert_ne!(fixed_png, measured_png, "Measured must render differently once a wide Y count label would otherwise clip under Fixed");
    }

    #[test]
    fn adaptive_y_tick_policy_renders_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};

        let samples: Vec<f64> = (0..300).map(|i| ((i * 37) % 97) as f64 * 0.5).collect();
        let figure = HistogramFigure::new(samples, 10).with_y_tick_policy(TickCountPolicy::Adaptive { min: 2, max: 20 });
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 400, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| figure.render(ctx, Rect::new(0.0, 0.0, 300.0, 400.0), &theme));
        assert!(result.is_ok());
    }

    // ── LabelOverflow (item 4) ───────────────────────────────────────────

    #[test]
    fn default_label_overflow_is_skip() {
        let figure = HistogramFigure::new(vec![1.0, 2.0, 3.0], 5);
        assert_eq!(figure.label_overflow, LabelOverflow::Skip);
    }

    #[test]
    fn with_label_overflow_rotate_renders_without_panicking_and_differs_from_skip() {
        use uzor_export::{render_to_png, ExportSpec};

        let samples: Vec<f64> = (0..500).map(|i| ((i * 41) % 199) as f64 * 3.0).collect();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);

        let skip = HistogramFigure::new(samples.clone(), 20);
        let rotated = HistogramFigure::new(samples, 20).with_label_overflow(LabelOverflow::Rotate(45.0));
        let skip_png = render_to_png(&spec, |ctx| skip.render(ctx, rect, &theme)).expect("skip render");
        let rotated_png = render_to_png(&spec, |ctx| rotated.render(ctx, rect, &theme)).expect("rotate render");
        assert_ne!(skip_png, rotated_png, "LabelOverflow::Rotate must render visibly differently from the default Skip for a wide bin count");
    }
}
