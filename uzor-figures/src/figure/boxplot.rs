//! `BoxplotFigure` — a quartile summary per category: a box spanning
//! `[Q1, Q3]`, a median line, whiskers at the Tukey `1.5 * IQR` convention,
//! and individual points for outliers beyond that. Quartile math
//! ([`quartile`]/[`boxplot_stats`]) is pure and independently tested from
//! painting (design law #1, same split every other figure's own geometry
//! function uses, e.g. [`crate::figure::layout_bars`]).
//!
//! ## Quartile method
//!
//! [`quartile`] is the linear-interpolation percentile (numpy's default
//! `'linear'` method, R's "type 7") — sort the samples, then linearly
//! interpolate between the two bracketing order statistics at fractional
//! rank `p * (n - 1)`. This is a real, well-known, precisely-defined
//! convention (not an ad hoc choice) and — unlike Tukey's original
//! "median-of-halves" hinge method — degrades gracefully and unambiguously
//! for tiny `n` (`n == 1`: every quartile equals the single sample; `n ==
//! 2`/`3`: a real fractional interpolation between real order statistics,
//! never an undefined "median of an empty half").
//!
//! **Engine-strengthening WAVE 4b**: [`quartile`] now DELEGATES its own
//! Q1/median/Q3 computation to [`crate::transform::quantile::quantile`]
//! (this crate's ONE canonical percentile implementation — the exact same
//! method, previously duplicated independently here) via
//! [`MissingDataPolicy::Skip`]. Behavior-preserving for every EXISTING
//! test/caller (none exercise a non-finite sample — confirmed by reading
//! every test below before this change) — this crate's OWN `transform`
//! layer sits BENEATH `figure` (`CurveFigure::with_downsample` already
//! established that direction via `transform::lttb`), so `figure` code
//! depending on it is the correct direction, unlike the reverse. This
//! ALSO closes a real, previously-undocumented gap: the pre-existing
//! local `percentile`/`quartile` had NO non-finite handling at all — an
//! all-`NaN` `samples` used to silently return `Some((NaN, NaN, NaN))`
//! (via ordinary NaN-arithmetic propagation through the old
//! `unwrap_or(Equal)`-sorted interpolation) instead of the honest "no
//! usable data" `None` every OTHER degenerate case (incl. plain empty
//! `samples`) already returns — a genuine defect, fixed outright per this
//! crate's own binding doctrine (not a silent behavior change: no
//! existing test ever exercised this path, and the new behavior is
//! strictly more honest). [`boxplot_stats`]'s own whisker/outlier scan
//! stays over the ORIGINAL (unfiltered) sorted sample list — untouched,
//! since a `NaN` there already, implicitly, never satisfies any of its
//! `>=`/`<`/`>` fence comparisons (so it was already excluded from
//! whisker/outlier results before this change, just not from Q1/median/Q3
//! computation).
//!
//! ## Whisker convention (documented — the task's own explicit ask)
//!
//! Whiskers use the standard Tukey (1977) `1.5 * IQR` fence, exactly as
//! matplotlib/`ggplot2`'s own default boxplot draws it: `IQR = Q3 - Q1`,
//! the fences sit at `Q1 - 1.5 * IQR` / `Q3 + 1.5 * IQR`, and each whisker
//! extends to the MOST EXTREME ACTUAL SAMPLE still inside its own fence
//! (never literally at the fence value itself, which may not correspond to
//! any real data point) — every sample strictly beyond its own whisker is
//! drawn individually as an outlier point, never absorbed into the
//! whisker's own reach.

use uzor::render::{CircleBatch, RenderContext};
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::transform::quantile::quantile as transform_quantile;
use crate::transform::MissingDataPolicy;
use crate::figure::{resolve_tick_count, FigureOverlay, MarginPolicy, TickCountPolicy};
use crate::guide::{axis, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::scale::linear::{format_value, nice_step};
use crate::scale::{BandScale, LinearScale};
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 52.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_Y_TICKS: usize = 5;
const BAND_PADDING: f64 = 0.3;
/// Fraction of each category band's own width the box itself occupies —
/// narrower than the band so adjacent categories never touch.
const BOX_WIDTH_FRACTION: f64 = 0.6;
/// Fraction of the box's own width the whisker cap ticks span.
const WHISKER_CAP_FRACTION: f64 = 0.4;
const OUTLIER_RADIUS: f64 = 2.5;
const HOVER_HIGHLIGHT_ALPHA: f64 = 0.18;
const SELECTED_STROKE_WIDTH: f64 = 2.0;

/// The Tukey 1.5x-IQR whisker multiplier — see the module docs.
pub const WHISKER_IQR_MULTIPLIER: f64 = 1.5;

/// One category's fully-resolved quartile summary.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxplotStats {
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub whisker_low: f64,
    pub whisker_high: f64,
    pub outliers: Vec<f64>,
    pub sample_count: usize,
}

/// `(Q1, median, Q3)` over `samples` via [`crate::transform::quantile::
/// quantile`] (this crate's ONE canonical percentile implementation —
/// see this module's own top doc comment for the full delegation
/// reasoning). `None` for empty `samples`, OR for `samples` that contain
/// no FINITE values at all (an all-non-finite input has no usable data
/// either — a genuine defect fix, not a silent behavior change: see this
/// module's own top doc comment).
pub fn quartile(samples: &[f64]) -> Option<(f64, f64, f64)> {
    let q1 = transform_quantile(samples, 0.25, MissingDataPolicy::Skip).ok()?;
    let median = transform_quantile(samples, 0.5, MissingDataPolicy::Skip).ok()?;
    let q3 = transform_quantile(samples, 0.75, MissingDataPolicy::Skip).ok()?;
    Some((q1, median, q3))
}

/// Full [`BoxplotStats`] over `samples` — quartiles via [`quartile`]'s own
/// delegation to [`crate::transform::quantile::quantile`], whiskers/
/// outliers via the Tukey `1.5 * IQR` convention (see the module docs).
/// `None` for empty `samples`, OR for all-non-finite `samples` (see
/// [`quartile`]'s own doc comment for why that's a defect fix, not a
/// silent change — no existing test exercised this path).
pub fn boxplot_stats(samples: &[f64]) -> Option<BoxplotStats> {
    if samples.is_empty() {
        return None;
    }
    let (q1, median, q3) = quartile(samples)?;
    // The whisker-fence scan below stays over the ORIGINAL (unfiltered)
    // sorted sample list — a `NaN` here already, implicitly, never
    // satisfies any `>=`/`<`/`>` fence comparison below, so it was
    // already excluded from whisker/outlier results before this
    // refactor; only Q1/median/Q3's own computation changed.
    let mut sorted: Vec<f64> = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let iqr = q3 - q1;
    let low_fence = q1 - WHISKER_IQR_MULTIPLIER * iqr;
    let high_fence = q3 + WHISKER_IQR_MULTIPLIER * iqr;

    let whisker_low = sorted.iter().copied().find(|&v| v >= low_fence).unwrap_or(q1);
    let whisker_high = sorted.iter().copied().rev().find(|&v| v <= high_fence).unwrap_or(q3);
    let outliers: Vec<f64> = sorted.iter().copied().filter(|&v| v < whisker_low || v > whisker_high).collect();

    Some(BoxplotStats { q1, median, q3, whisker_low, whisker_high, outliers, sample_count: sorted.len() })
}

/// A quartile-summary chart over one or more named categories — see the
/// module docs.
pub struct BoxplotFigure {
    pub categories: Vec<String>,
    pub samples: Vec<Vec<f64>>,
    pub title: Option<String>,
    /// Inner padding (fraction of each band's width) — see
    /// [`BoxplotFigure::with_band_padding`]. Defaults to [`BAND_PADDING`].
    band_padding: f64,
    /// This figure's own left-margin sizing policy — see
    /// [`BoxplotFigure::with_margin_policy`].
    margin_policy: MarginPolicy,
    /// This figure's own Y tick-count policy — see
    /// [`BoxplotFigure::with_y_tick_policy`].
    y_tick_policy: TickCountPolicy,
}

impl BoxplotFigure {
    /// `categories[i]` labels `samples[i]`'s own distribution — mismatched
    /// lengths are not an error: a category beyond `samples.len()` simply
    /// draws no box (still gets its own band + axis label), and a `samples`
    /// entry beyond `categories.len()` is never reached (the band scale is
    /// built from `categories` alone).
    pub fn new(categories: Vec<String>, samples: Vec<Vec<f64>>) -> Self {
        Self {
            categories,
            samples,
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
    /// other figure's `plot_area` accessor. **Does not account for
    /// [`MarginPolicy::Measured`] widening the left margin** — same
    /// ctx-less-accessor caveat documented on
    /// [`crate::figure::BarFigure::plot_area`].
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.base_plot_rect(rect))
    }

    /// This figure's own category band scale.
    pub fn band_scale(&self) -> BandScale {
        BandScale::new(self.categories.clone(), self.band_padding)
    }

    /// This category's own resolved stats, `None` for an empty/missing
    /// sample set.
    fn stats_for(&self, i: usize) -> Option<BoxplotStats> {
        self.samples.get(i).and_then(|s| boxplot_stats(s))
    }

    /// Nice-rounded Y domain over every category's own whisker/outlier
    /// extent. **Never forces a zero baseline** — same reasoning
    /// [`crate::figure::ScatterFigure`]'s own Y domain docs give (a box's
    /// position, not an area-from-zero, is what a boxplot encodes).
    /// `None` when every category is empty.
    fn y_scale(&self) -> Option<LinearScale> {
        let mut found = false;
        let (mn, mx) = (0..self.categories.len()).filter_map(|i| self.stats_for(i)).fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), s| {
            found = true;
            let lo = s.outliers.iter().copied().fold(s.whisker_low, f64::min);
            let hi = s.outliers.iter().copied().fold(s.whisker_high, f64::max);
            (mn.min(lo), mx.max(hi))
        });
        if found {
            Some(LinearScale::nice(mn, mx, TARGET_Y_TICKS))
        } else {
            None
        }
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a
    /// category's own band brightens its box and shows a quartile/
    /// whisker/outlier-count tooltip; `overlay.focus`-selected categories
    /// get a persistent accent outline spanning the whole band (same
    /// convention [`crate::figure::BarFigure`] uses). `overlay.brush` is
    /// not consumed by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        if let Some(y_scale) = self.y_scale() {
            let band = self.band_scale();

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

            grid::draw_y_grid(ctx, &area, &y_scale, theme, target_y_ticks);

            let box_color = theme.palette[0].clone();
            let outlier_color = theme.palette[1].clone();
            let step = nice_step(y_scale.max - y_scale.min, target_y_ticks as f64);

            for i in 0..band.len() {
                let Some(stats) = self.stats_for(i) else { continue };
                let (x0, x1) = area.x_band(&band, i);
                let box_half = (x1 - x0) * BOX_WIDTH_FRACTION / 2.0;
                let cx = (x0 + x1) / 2.0;
                let (bx0, bx1) = (cx - box_half, cx + box_half);
                let cap_half = box_half * WHISKER_CAP_FRACTION;

                let y_q1 = area.y(&y_scale, stats.q1);
                let y_q3 = area.y(&y_scale, stats.q3);
                let y_median = area.y(&y_scale, stats.median);
                let y_low = area.y(&y_scale, stats.whisker_low);
                let y_high = area.y(&y_scale, stats.whisker_high);
                let (box_top, box_bottom) = (y_q3.min(y_q1), y_q3.max(y_q1));

                // Whisker stems + caps.
                ctx.set_stroke_color(&theme.axis_color);
                ctx.set_stroke_width(1.0);
                ctx.set_line_dash(&[]);
                ctx.begin_path();
                ctx.move_to(cx, y_high);
                ctx.line_to(cx, box_top);
                ctx.stroke();
                ctx.begin_path();
                ctx.move_to(cx, box_bottom);
                ctx.line_to(cx, y_low);
                ctx.stroke();
                ctx.begin_path();
                ctx.move_to(cx - cap_half, y_high);
                ctx.line_to(cx + cap_half, y_high);
                ctx.stroke();
                ctx.begin_path();
                ctx.move_to(cx - cap_half, y_low);
                ctx.line_to(cx + cap_half, y_low);
                ctx.stroke();

                // Box.
                ctx.set_fill_color(&box_color);
                ctx.set_global_alpha(0.55);
                ctx.fill_rect(bx0, box_top, (bx1 - bx0).max(0.0), (box_bottom - box_top).max(0.0));
                ctx.set_global_alpha(1.0);
                ctx.set_stroke_color(&box_color);
                ctx.stroke_rect(bx0, box_top, (bx1 - bx0).max(0.0), (box_bottom - box_top).max(0.0));

                // Median line.
                ctx.set_stroke_color(&theme.label_color);
                ctx.set_stroke_width(2.0);
                ctx.begin_path();
                ctx.move_to(bx0, y_median);
                ctx.line_to(bx1, y_median);
                ctx.stroke();

                // Outliers — this figure has no continuous X scale (only
                // the category `BandScale`), so an outlier's own screen
                // position is resolved directly (`cx`, `area.y(&y_scale,
                // v)`) rather than forcing it through `mark::point::
                // draw_points`'s own 2-scale signature.
                if !stats.outliers.is_empty() {
                    let circles: Vec<CircleBatch> =
                        stats.outliers.iter().map(|&v| CircleBatch { cx, cy: area.y(&y_scale, v), r: OUTLIER_RADIUS }).collect();
                    ctx.draw_circle_batch(&circles, &outlier_color);
                }

                if let Some(focus) = overlay.focus {
                    if focus.is_selected(i as u64) {
                        ctx.set_stroke_color(&theme.palette[1]);
                        ctx.set_stroke_width(SELECTED_STROKE_WIDTH);
                        ctx.stroke_rect(x0, area.rect.y, (x1 - x0).max(0.0), area.rect.height);
                    }
                }

                if let Some((hx, hy)) = overlay.hover_px {
                    if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                        if hit::bar_index_at(&area, &band, hx) == Some(i) {
                            ctx.set_fill_color(&theme.highlight);
                            ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                            ctx.fill_rect(x0, area.rect.y, (x1 - x0).max(0.0), area.rect.height);
                            ctx.set_global_alpha(1.0);

                            let category = self.categories.get(i).cloned().unwrap_or_default();
                            let lines = vec![
                                ("category".to_owned(), category),
                                ("q1".to_owned(), format_value(stats.q1, step)),
                                ("median".to_owned(), format_value(stats.median, step)),
                                ("q3".to_owned(), format_value(stats.q3, step)),
                                ("whisker low".to_owned(), format_value(stats.whisker_low, step)),
                                ("whisker high".to_owned(), format_value(stats.whisker_high, step)),
                                ("outliers".to_owned(), stats.outliers.len().to_string()),
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

    #[test]
    fn quartile_matches_the_numpy_linear_method_golden() {
        // numpy.percentile([1,2,3,4], [25,50,75], method="linear") ==
        // [1.75, 2.5, 3.25] — the well-known textbook golden this method
        // is defined to reproduce.
        let (q1, median, q3) = quartile(&[1.0, 2.0, 3.0, 4.0]).expect("non-empty");
        assert!((q1 - 1.75).abs() < 1e-9);
        assert!((median - 2.5).abs() < 1e-9);
        assert!((q3 - 3.25).abs() < 1e-9);
    }

    #[test]
    fn quartile_of_empty_samples_is_none() {
        assert!(quartile(&[]).is_none());
        assert!(boxplot_stats(&[]).is_none());
    }

    #[test]
    fn quartile_n_equals_1_collapses_every_quartile_to_the_single_sample() {
        let (q1, median, q3) = quartile(&[42.0]).expect("1 sample");
        assert_eq!(q1, 42.0);
        assert_eq!(median, 42.0);
        assert_eq!(q3, 42.0);
        let stats = boxplot_stats(&[42.0]).expect("1 sample");
        assert_eq!(stats.whisker_low, 42.0);
        assert_eq!(stats.whisker_high, 42.0);
        assert!(stats.outliers.is_empty());
    }

    #[test]
    fn quartile_n_equals_2_interpolates_between_the_two_real_samples() {
        let (q1, median, q3) = quartile(&[1.0, 2.0]).expect("2 samples");
        assert!((q1 - 1.25).abs() < 1e-9);
        assert!((median - 1.5).abs() < 1e-9);
        assert!((q3 - 1.75).abs() < 1e-9);
    }

    #[test]
    fn quartile_n_equals_3_interpolates_between_the_correct_bracketing_pair() {
        let (q1, median, q3) = quartile(&[1.0, 2.0, 3.0]).expect("3 samples");
        assert!((q1 - 1.5).abs() < 1e-9);
        assert!((median - 2.0).abs() < 1e-9);
        assert!((q3 - 2.5).abs() < 1e-9);
    }

    #[test]
    fn quartile_is_order_independent_sorts_internally() {
        let (q1a, ma, q3a) = quartile(&[4.0, 1.0, 3.0, 2.0]).expect("4 samples");
        let (q1b, mb, q3b) = quartile(&[1.0, 2.0, 3.0, 4.0]).expect("4 samples");
        assert_eq!((q1a, ma, q3a), (q1b, mb, q3b));
    }

    // ── Engine-strengthening WAVE 4b — `quartile`/`boxplot_stats` now
    // delegate to `transform::quantile::quantile` ──────────────────────

    #[test]
    fn quartile_all_non_finite_samples_is_now_none_the_documented_defect_fix() {
        // Before this wave's refactor, an all-NaN input silently returned
        // `Some((NaN, NaN, NaN))` via ordinary NaN-arithmetic propagation
        // through the old locally-sorted interpolation — no test ever
        // relied on that, and it's a strictly less honest answer than the
        // `None` every other "no usable data" case already returns.
        assert!(quartile(&[f64::NAN, f64::NAN, f64::NAN]).is_none());
        assert!(boxplot_stats(&[f64::NAN, f64::NAN]).is_none());
    }

    #[test]
    fn quartile_skips_non_finite_samples_mixed_with_finite_ones() {
        let with_nan = quartile(&[1.0, f64::NAN, 2.0, 3.0, 4.0]).expect("3 finite samples remain");
        let clean = quartile(&[1.0, 2.0, 3.0, 4.0]).expect("4 finite samples");
        assert_eq!(with_nan, clean, "a stray NaN mixed into otherwise-finite samples must be skipped, not corrupt the whole result");
    }

    #[test]
    fn quartile_matches_the_transform_quantile_function_directly_proving_the_delegation_is_real() {
        let samples = [3.0, 7.0, 1.0, 9.0, 4.0, 2.0, 8.0];
        let (q1, median, q3) = quartile(&samples).expect("non-empty");
        assert_eq!(q1, crate::transform::quantile::quantile(&samples, 0.25, crate::transform::MissingDataPolicy::Skip).unwrap());
        assert_eq!(median, crate::transform::quantile::quantile(&samples, 0.5, crate::transform::MissingDataPolicy::Skip).unwrap());
        assert_eq!(q3, crate::transform::quantile::quantile(&samples, 0.75, crate::transform::MissingDataPolicy::Skip).unwrap());
    }

    #[test]
    fn boxplot_stats_whisker_extends_to_the_real_sample_inside_the_fence_not_the_fence_itself() {
        // A dense cluster [10..20] plus one deliberate high outlier (100) —
        // the fence math must exclude 100 from the whisker's own reach and
        // report it as a real outlier, while the whisker itself lands on a
        // REAL sample (20), never on the fence value.
        let samples: Vec<f64> = (10..=20).map(|v| v as f64).collect();
        let mut with_outlier = samples.clone();
        with_outlier.push(100.0);
        let stats = boxplot_stats(&with_outlier).expect("non-empty");
        assert_eq!(stats.outliers, vec![100.0], "the far outlier must be reported individually");
        assert!(with_outlier.contains(&stats.whisker_high), "the whisker's own high end must land on a REAL sample");
        assert!(stats.whisker_high < 100.0, "the whisker must never reach all the way to the outlier itself");
    }

    #[test]
    fn boxplot_stats_no_outliers_when_data_is_tight() {
        let samples: Vec<f64> = (0..50).map(|i| 10.0 + (i as f64 * 0.2)).collect();
        let stats = boxplot_stats(&samples).expect("non-empty");
        assert!(stats.outliers.is_empty(), "a smooth, tight distribution must report zero outliers");
        assert!(stats.sample_count == 50);
    }

    #[test]
    fn boxplot_stats_low_outlier_is_detected_symmetrically() {
        let mut samples: Vec<f64> = (50..=60).map(|v| v as f64).collect();
        samples.push(-200.0);
        let stats = boxplot_stats(&samples).expect("non-empty");
        assert_eq!(stats.outliers, vec![-200.0]);
        assert!(stats.whisker_low > -200.0);
    }

    #[test]
    fn default_band_padding_matches_the_pre_existing_constant() {
        let figure = BoxplotFigure::new(vec!["a".to_owned()], vec![vec![1.0, 2.0, 3.0]]);
        assert!((figure.band_scale().padding - BAND_PADDING).abs() < 1e-9);
    }

    #[test]
    fn with_band_padding_overrides_the_default_and_is_reflected_in_the_band_scale() {
        let figure = BoxplotFigure::new(vec!["a".to_owned()], vec![vec![1.0, 2.0, 3.0]]).with_band_padding(0.45);
        assert!((figure.band_scale().padding - 0.45).abs() < 1e-9);
    }

    #[test]
    fn y_scale_never_forces_a_zero_baseline() {
        let figure = BoxplotFigure::new(vec!["a".to_owned()], vec![vec![500.0, 510.0, 520.0, 530.0, 540.0]]);
        let y = figure.y_scale().expect("non-empty fixture");
        assert!(y.min > 0.0, "boxplot Y domain must fit the data, never force a zero baseline (got min={})", y.min);
    }

    #[test]
    fn a_category_with_no_samples_is_skipped_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};

        let figure = BoxplotFigure::new(
            vec!["has-data".to_owned(), "empty".to_owned()],
            vec![vec![1.0, 2.0, 3.0, 4.0, 5.0], Vec::new()],
        )
        .with_title("mixed");
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 300.0, 200.0), &theme);
        });
        assert!(result.is_ok(), "a category with zero samples must skip its own box, never panic");
    }

    #[test]
    fn render_smoke_with_hover_and_focus() {
        use crate::interact::focus::FocusSet;
        use uzor_export::{render_to_png, ExportSpec};

        let categories: Vec<String> = (0..4).map(|i| format!("group-{i}")).collect();
        let samples: Vec<Vec<f64>> = vec![
            (0..30).map(|i| 40.0 + ((i * 7) % 25) as f64).collect(),
            (0..25).map(|i| 55.0 + ((i * 11) % 30) as f64).collect(),
            {
                let mut v: Vec<f64> = (0..28).map(|i| 35.0 + ((i * 5) % 20) as f64).collect();
                v.push(140.0);
                v
            },
            (0..3).map(|i| 60.0 + i as f64 * 5.0).collect(),
        ];
        let figure = BoxplotFigure::new(categories, samples).with_title("smoke");
        let theme = FigureTheme::dark();
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let mut focus = FocusSet::empty();
        focus.select(1);
        let overlay = FigureOverlay { hover_px: Some((150.0, 200.0)), brush: None, focus: Some(&focus) };
        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        });
        assert!(result.is_ok());
    }

    // ── MarginPolicy / TickCountPolicy (items 2 + 3) ────────────────────

    #[test]
    fn default_policies_match_the_pre_existing_constant() {
        let figure = BoxplotFigure::new(vec!["a".to_owned()], vec![vec![1.0, 2.0, 3.0]]);
        assert_eq!(figure.margin_policy, MarginPolicy::Measured);
        assert_eq!(figure.y_tick_policy, TickCountPolicy::Fixed(TARGET_Y_TICKS));
    }

    #[test]
    fn measured_margin_widens_for_a_deliberately_wide_y_label() {
        use uzor_export::{render_to_png, ExportSpec};

        let samples = vec![vec![1.0, 500_000_000.0, 999_999_999.0, 2.0, 3.0]];
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);

        let fixed = BoxplotFigure::new(vec!["a".to_owned()], samples.clone()).with_margin_policy(MarginPolicy::Fixed);
        let measured = BoxplotFigure::new(vec!["a".to_owned()], samples).with_margin_policy(MarginPolicy::Measured);
        let fixed_png = render_to_png(&spec, |ctx| fixed.render(ctx, rect, &theme)).expect("fixed render");
        let measured_png = render_to_png(&spec, |ctx| measured.render(ctx, rect, &theme)).expect("measured render");
        assert_ne!(fixed_png, measured_png, "Measured must render differently once a wide Y label would otherwise clip under Fixed");
    }

    #[test]
    fn adaptive_y_tick_policy_renders_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};

        let categories: Vec<String> = (0..3).map(|i| format!("group-{i}")).collect();
        let samples: Vec<Vec<f64>> = (0..3).map(|g| (0..20).map(|i| (g * 10 + i) as f64).collect()).collect();
        let figure = BoxplotFigure::new(categories, samples).with_y_tick_policy(TickCountPolicy::Adaptive { min: 2, max: 15 });
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| figure.render(ctx, Rect::new(0.0, 0.0, 400.0, 300.0), &theme));
        assert!(result.is_ok());
    }
}
