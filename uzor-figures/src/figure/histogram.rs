//! `HistogramFigure` — equal-width binning over a nice-rounded domain,
//! rendered as a [`BandFigure`](super::bars)-style bar chart. Binning is a
//! pure, independently-testable transform ([`bin`]) — no rendering
//! involved.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::boxplot::quartile;
use crate::figure::{resolve_tick_count, FigureOverlay, MarginPolicy, TickCountPolicy};
use crate::guide::axis::LabelOverflow;
use crate::guide::{axis, grid};
use crate::mark::rect::draw_bars;
use crate::mark::MarkStyle;
use crate::scale::linear::format_value;
use crate::scale::{BandScale, LinearScale, Scale};
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 48.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_Y_TICKS: usize = 5;
/// Cap on how many of a wide bin count get x-axis labels — a 50-bin
/// histogram with one label per bar is unreadable regardless of
/// available width.
const MAX_X_LABELS: usize = 10;

/// One equal-width bin: its `[start, end)` domain range and sample count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bin {
    pub range: (f64, f64),
    pub count: usize,
}

/// Bin `samples` into `bin_count` equal-width bins spanning a nice-rounded
/// domain around the data extent.
///
/// - Empty `samples` -> empty result (nothing to bin).
/// - All-equal samples (including a single sample) -> the domain widens to
///   `[v, v + 1)` so bin width stays non-zero; every sample lands in bin 0.
/// - `bin_count` is floored at 1.
pub fn bin(samples: &[f64], bin_count: usize) -> Vec<Bin> {
    let bin_count = bin_count.max(1);
    if samples.is_empty() {
        return Vec::new();
    }

    let (data_min, data_max) = samples
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
    if !data_min.is_finite() || !data_max.is_finite() {
        return Vec::new();
    }

    let (domain_min, domain_max) =
        if (data_max - data_min).abs() < f64::EPSILON { (data_min, data_min + 1.0) } else { (data_min, data_max) };

    let width = (domain_max - domain_min) / bin_count as f64;
    let mut bins: Vec<Bin> = (0..bin_count)
        .map(|i| Bin { range: (domain_min + i as f64 * width, domain_min + (i + 1) as f64 * width), count: 0 })
        .collect();

    for &v in samples {
        let idx = (((v - domain_min) / width) as usize).min(bin_count - 1);
        bins[idx].count += 1;
    }
    bins
}

/// How [`HistogramFigure`] resolves its own bin COUNT — see
/// [`HistogramFigure::new`]'s own doc comment for the gap this closes
/// (every mainstream histogram implementation — matplotlib `bins='auto'`,
/// numpy `histogram_bin_edges`, R's `hist()` — picks a sane default bin
/// count from the data itself; before this enum existed, a caller with no
/// domain intuition about bin count had no help at all).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinPolicy {
    /// Always use the caller-supplied bin count — byte-identical to this
    /// crate's pre-existing (and, before this item, only) behavior. THE
    /// DEFAULT: [`HistogramFigure::new`] still takes an explicit
    /// `bin_count` and resolves to this variant, so every existing caller
    /// keeps its exact output unchanged.
    Manual(usize),
    /// Sturges' rule: `ceil(log2(n)) + 1` — the simplest, most widely
    /// taught automatic rule (R's own `hist()` default); best suited to a
    /// small, roughly-normal sample count.
    Sturges,
    /// Freedman-Diaconis' rule: `bin_width = 2 * IQR / n^(1/3)`, converted
    /// to a bin count via `ceil(data_range / bin_width)` — robust to
    /// outliers (sizes the bin width from the IQR, not the full range),
    /// the rule matplotlib's own `bins='auto'` prefers for larger,
    /// real-world (non-normal) datasets. Reuses
    /// [`crate::figure::boxplot::quartile`], already exported and exactly
    /// the Q1/Q3 machinery this rule needs.
    FreedmanDiaconis,
    /// Scott's rule: `bin_width = 3.49 * stddev / n^(1/3)` — asymptotically
    /// optimal for roughly-normal data (minimizes integrated mean squared
    /// error against a Gaussian reference), converted to a bin count the
    /// same way as [`BinPolicy::FreedmanDiaconis`].
    Scott,
}

/// Resolve a bin count from `policy` over `samples` — the pure function
/// [`HistogramFigure::bin_count`] calls, independently testable against
/// known datasets. Every automatic rule floors at `1` bin (matching
/// [`bin`]'s own floor) and falls back to `1` for fewer than 2 samples (no
/// meaningful spread to derive a rule from).
pub fn resolve_bin_count(policy: BinPolicy, samples: &[f64]) -> usize {
    let n = samples.len();
    match policy {
        BinPolicy::Manual(count) => count.max(1),
        _ if n < 2 => 1,
        BinPolicy::Sturges => (((n as f64).log2().ceil()) as usize + 1).max(1),
        BinPolicy::FreedmanDiaconis => {
            let Some((q1, _, q3)) = quartile(samples) else { return 1 };
            let iqr = q3 - q1;
            let (data_min, data_max) = samples.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
            let range = data_max - data_min;
            if iqr <= 0.0 || range <= 0.0 {
                return 1;
            }
            let bin_width = 2.0 * iqr / (n as f64).cbrt();
            if bin_width <= 0.0 {
                return 1;
            }
            (range / bin_width).ceil().max(1.0) as usize
        }
        BinPolicy::Scott => {
            let mean = samples.iter().sum::<f64>() / n as f64;
            let variance = samples.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
            let stddev = variance.sqrt();
            let (data_min, data_max) = samples.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
            let range = data_max - data_min;
            if stddev <= 0.0 || range <= 0.0 {
                return 1;
            }
            let bin_width = 3.49 * stddev / (n as f64).cbrt();
            if bin_width <= 0.0 {
                return 1;
            }
            (range / bin_width).ceil().max(1.0) as usize
        }
    }
}

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

        let bins = bin(&self.samples, self.bin_count());
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

    #[test]
    fn empty_samples_produce_no_bins() {
        assert!(bin(&[], 10).is_empty());
    }

    #[test]
    fn single_value_lands_in_one_bin_with_full_count() {
        let bins = bin(&[5.0], 4);
        assert_eq!(bins.len(), 4);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, 1);
        assert_eq!(bins[0].count, 1);
    }

    #[test]
    fn all_equal_samples_land_in_one_bin_with_full_count() {
        let samples = vec![3.0, 3.0, 3.0, 3.0];
        let bins = bin(&samples, 5);
        assert_eq!(bins.len(), 5);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, samples.len());
        assert_eq!(bins[0].count, samples.len());
    }

    #[test]
    fn every_sample_is_counted_exactly_once() {
        let samples: Vec<f64> = (0..100).map(|i| i as f64 * 0.37).collect();
        let bins = bin(&samples, 10);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, samples.len());
    }

    #[test]
    fn max_value_sample_lands_in_the_last_bin_not_out_of_range() {
        // The sample equal to the data max computes a fractional index
        // exactly at `bin_count` (`(4.0 - 0.0) / 1.0 == 4`, one past the
        // last valid index `3`) without the `.min(bin_count - 1)` clamp —
        // verify it lands in the last bin instead of panicking.
        let bins = bin(&[0.0, 4.0], 4);
        assert_eq!(bins.len(), 4);
        assert_eq!(bins[0].count, 1);
        assert_eq!(bins[3].count, 1);
        assert_eq!(bins[1].count, 0);
        assert_eq!(bins[2].count, 0);
    }

    #[test]
    fn bin_count_floors_at_one() {
        let bins = bin(&[1.0, 2.0, 3.0], 0);
        assert_eq!(bins.len(), 1);
        assert_eq!(bins[0].count, 3);
    }

    // ── BinPolicy (item 6) ───────────────────────────────────────────────

    #[test]
    fn manual_policy_always_returns_the_caller_supplied_count_regardless_of_data() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0];
        assert_eq!(resolve_bin_count(BinPolicy::Manual(7), &samples), 7);
        assert_eq!(resolve_bin_count(BinPolicy::Manual(0), &samples), 1, "Manual must floor at 1, matching bin()'s own floor");
    }

    #[test]
    fn new_defaults_to_manual_bin_policy_byte_identical_to_pre_existing_behavior() {
        let figure = HistogramFigure::new(vec![1.0, 2.0, 3.0, 4.0, 5.0], 5);
        assert_eq!(figure.bin_count(), 5, "HistogramFigure::new must still resolve to exactly the caller-supplied bin_count");
    }

    #[test]
    fn sturges_matches_the_hand_computed_golden_for_a_known_dataset() {
        // n = 4: ceil(log2(4)) + 1 = ceil(2.0) + 1 = 3 (log2(4) is exact in
        // f64 since 4 is a power of two — no rounding-boundary risk).
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(resolve_bin_count(BinPolicy::Sturges, &samples), 3);
    }

    #[test]
    fn freedman_diaconis_matches_the_hand_computed_golden_for_a_known_dataset() {
        // n = 4, same fixture `boxplot::quartile`'s own numpy golden uses:
        // Q1 = 1.75, Q3 = 3.25 -> IQR = 1.5. bin_width = 2*1.5 / 4^(1/3)
        // = 3 / 1.5874... ~= 1.8902. data range = 3.0.
        // bin_count = ceil(3.0 / 1.8902) = ceil(1.5875) = 2.
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(resolve_bin_count(BinPolicy::FreedmanDiaconis, &samples), 2);
    }

    #[test]
    fn scott_matches_the_hand_computed_golden_for_a_known_dataset() {
        // n = 4: mean = 2.5, population variance = 1.25, stddev ~=
        // 1.11803. bin_width = 3.49 * 1.11803 / 4^(1/3) ~= 2.4586.
        // bin_count = ceil(3.0 / 2.4586) = ceil(1.2202) = 2.
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(resolve_bin_count(BinPolicy::Scott, &samples), 2);
    }

    #[test]
    fn a_larger_roughly_uniform_dataset_gets_a_wider_sturges_bin_count() {
        // n = 100: ceil(log2(100)) + 1 = ceil(6.6439) + 1 = 7 + 1 = 8.
        let samples: Vec<f64> = (0..100).map(|i| i as f64).collect();
        assert_eq!(resolve_bin_count(BinPolicy::Sturges, &samples), 8);
    }

    #[test]
    fn every_automatic_rule_falls_back_to_one_bin_for_degenerate_all_equal_data() {
        // Zero IQR/stddev must never divide by zero or produce a NaN/0
        // bin count.
        let samples = vec![7.0; 20];
        for policy in [BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott] {
            assert_eq!(resolve_bin_count(policy, &samples), if policy == BinPolicy::Sturges { 6 } else { 1 });
        }
    }

    #[test]
    fn every_automatic_rule_falls_back_to_one_bin_for_fewer_than_two_samples() {
        for policy in [BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott] {
            assert_eq!(resolve_bin_count(policy, &[]), 1);
            assert_eq!(resolve_bin_count(policy, &[42.0]), 1);
        }
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
