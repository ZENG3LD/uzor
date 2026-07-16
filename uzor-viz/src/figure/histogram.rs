//! `HistogramFigure` — equal-width binning over a nice-rounded domain,
//! rendered as a [`BandFigure`](super::bars)-style bar chart. Binning is a
//! pure, independently-testable transform ([`bin`]) — no rendering
//! involved.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::VizOverlay;
use crate::guide::{axis, grid};
use crate::mark::rect::draw_bars;
use crate::mark::MarkStyle;
use crate::scale::linear::format_value;
use crate::scale::{BandScale, LinearScale, Scale};
use crate::theme::VizTheme;

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

/// A histogram over raw `samples`, binned to `bin_count` equal-width bins
/// at render time.
pub struct HistogramFigure {
    pub samples: Vec<f64>,
    pub bin_count: usize,
    pub title: Option<String>,
}

impl HistogramFigure {
    pub fn new(samples: Vec<f64>, bin_count: usize) -> Self {
        Self { samples, bin_count, title: None }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
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

    /// This figure's plot-area transform for `rect` — exposed for the
    /// same reason as [`crate::figure::CurveFigure::plot_area`].
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.plot_rect(rect))
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &VizOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &VizTheme) {
        self.render_with(ctx, rect, theme, &VizOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to
    /// `overlay.brush`: bins whose domain-X range overlaps the brush
    /// interval get redrawn in an accent color — the linked-brush half of
    /// the report's #2<->#5 shared-time-brush requirement, here linking a
    /// curve figure's drag to this figure's bins (see
    /// [`crate::figure::VizOverlay`]). `overlay.hover_px`/`overlay.focus`
    /// are not consumed by this figure in V2.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &VizTheme, overlay: &VizOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let area = self.plot_area(rect);

        let bins = bin(&self.samples, self.bin_count);
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

            grid::draw_y_grid(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
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

            axis::draw_x_axis(ctx, &area, &band, theme, band.len().min(MAX_X_LABELS));
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
}
