//! `BarFigure` — a categorical bar chart: nice-rounded linear Y domain,
//! band X, grid + axes + bars, optional per-bar value labels.
//!
//! Multi-series (`BarSeries`/`BarMode::{Grouped, Stacked}`) is additive:
//! [`BarFigure::new`] is now sugar for a single-series
//! [`BarMode::Grouped`] figure via [`BarFigure::with_series`], and its
//! render output is byte-identical to the pre-multi-series version — the
//! single-series draw/hover/tooltip path below is the SAME code that
//! existed before ([`crate::mark::rect::draw_bars`], not the new
//! `draw_bars_grouped`), gated on `self.series.len() <= 1`; grouped/
//! stacked geometry only activates for a genuinely multi-series figure. A
//! legend ([`crate::guide::legend`]) auto-appears at [`LegendPosition::Top`]
//! once `series.len() > 1`, or on any figure via [`BarFigure::with_legend`].

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::guide::annotation::{draw_annotations, Annotation};
use crate::guide::legend::{self, LegendEntry, LegendPosition};
use crate::guide::{axis, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::mark::rect::{draw_bars, draw_bars_grouped, draw_bars_stacked};
use crate::mark::text::draw_label_centered;
use crate::mark::MarkStyle;
use crate::scale::linear::{format_value, nice_step};
use crate::scale::{BandScale, LinearScale};
use crate::theme::FigureTheme;

/// Left margin for the y-axis tick labels; bottom margin for the x-axis
/// tick labels; top margin reserved for an optional title.
const MARGIN_LEFT: f64 = 48.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_Y_TICKS: usize = 5;
/// Inner padding (fraction of each band's width) used by [`BarFigure::band_scale`].
const BAND_PADDING: f64 = 0.2;
/// Fill alpha of the translucent "brighter" overlay drawn over a hovered bar.
const HOVER_HIGHLIGHT_ALPHA: f64 = 0.22;
/// Stroke width of the persistent-selection outline drawn around a
/// [`crate::interact::focus::FocusSet`]-selected bar.
const SELECTED_STROKE_WIDTH: f64 = 2.0;
/// Gap (px) between a measured legend band and the plot rect it shrinks.
const LEGEND_GAP: f64 = 8.0;

/// One named value series — a set of grouped sub-bars or one stacked
/// segment, per [`BarMode`]. `values[i]` is this series' value for
/// category `i` (a category index missing from a shorter series is
/// skipped, never treated as a synthetic zero).
#[derive(Debug, Clone)]
pub struct BarSeries {
    pub name: String,
    pub values: Vec<f64>,
}

/// How multiple [`BarSeries`] combine within one category band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarMode {
    /// Sub-bands side by side within each category band (band-within-band,
    /// via [`crate::mark::rect::sub_band_range`]).
    Grouped,
    /// Cumulative y-stacking — positive values stack upward from the zero
    /// baseline, negative values stack downward from it (standard
    /// finance-chart convention; positives and negatives never combine
    /// into one running total).
    Stacked,
}

/// One category = one category band; each band holds one or more
/// [`BarSeries`] values, combined per [`BarMode`].
pub struct BarFigure {
    pub categories: Vec<String>,
    pub series: Vec<BarSeries>,
    pub mode: BarMode,
    pub title: Option<String>,
    pub show_value_labels: bool,
    legend_position: Option<LegendPosition>,
    /// Reference lines/bands/callouts drawn over this figure's bars — set
    /// via [`BarFigure::with_annotations`]. Empty (the default) reproduces
    /// the original behavior exactly (see [`crate::guide::annotation`]).
    annotations: Vec<Annotation>,
}

impl BarFigure {
    /// Single-series constructor — sugar for
    /// `with_series(categories, vec![BarSeries { name: String::new(), values }], BarMode::Grouped)`.
    /// A single series never shows a sub-band or a legend on its own (see
    /// [`BarFigure::resolved_legend_position`]), so this reproduces the
    /// pre-multi-series render exactly.
    pub fn new(categories: Vec<String>, values: Vec<f64>) -> Self {
        Self::with_series(categories, vec![BarSeries { name: String::new(), values }], BarMode::Grouped)
    }

    /// Multi-series constructor. `series.len() > 1` auto-shows a legend at
    /// [`LegendPosition::Top`] unless overridden via
    /// [`BarFigure::with_legend`].
    pub fn with_series(categories: Vec<String>, series: Vec<BarSeries>, mode: BarMode) -> Self {
        Self { categories, series, mode, title: None, show_value_labels: false, legend_position: None, annotations: Vec::new() }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_value_labels(mut self, show: bool) -> Self {
        self.show_value_labels = show;
        self
    }

    /// Force a legend at `position` regardless of series count (default:
    /// auto-shown at [`LegendPosition::Top`] only when there's more than
    /// one series — see [`BarFigure::resolved_legend_position`]).
    pub fn with_legend(mut self, position: LegendPosition) -> Self {
        self.legend_position = Some(position);
        self
    }

    /// Reference lines/bands/callouts drawn over this figure's bars — see
    /// [`crate::guide::annotation`]. Only [`crate::guide::annotation::
    /// Annotation::HLine`] makes real sense here (this figure's X axis is
    /// a [`crate::scale::BandScale`], not a continuous domain — a
    /// `VLine`/`Callout`'s own `x` would map through the band's own
    /// fractional-index domain, which is a valid but unusual call); the
    /// draw function itself doesn't forbid it.
    pub fn with_annotations(mut self, annotations: Vec<Annotation>) -> Self {
        self.annotations = annotations;
        self
    }

    /// Resolved legend position for this render: an explicit
    /// [`BarFigure::with_legend`] override, or auto-[`LegendPosition::Top`]
    /// when there's more than one series, or `None` otherwise.
    fn resolved_legend_position(&self) -> Option<LegendPosition> {
        self.legend_position.or(if self.series.len() > 1 { Some(LegendPosition::Top) } else { None })
    }

    fn legend_entries(&self, theme: &FigureTheme) -> Vec<LegendEntry> {
        self.series
            .iter()
            .enumerate()
            .map(|(i, s)| LegendEntry { label: s.name.clone(), color: theme.palette[i % theme.palette.len()].clone() })
            .collect()
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

    /// This figure's plot-area transform for `rect` — exposed for the same
    /// reason as [`crate::figure::CurveFigure::plot_area`]: a caller
    /// driving hover/click routing from outside needs to hit-test through
    /// the EXACT same transform this figure renders with.
    ///
    /// **Does not account for a legend.** Measuring one needs live text
    /// metrics (a `&mut dyn RenderContext`), which this ctx-less accessor
    /// doesn't have — kept this way so every existing single-series,
    /// legend-less external caller stays byte-compatible.
    /// [`BarFigure::render_with`] measures + shrinks the SAME base rect
    /// internally via its own `ctx`, so a multi-series figure's own
    /// hover/tooltip/legend stay mutually consistent within one
    /// `render_with` call regardless.
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.base_plot_rect(rect))
    }

    /// This figure's own category band scale (always constructible, even
    /// for zero categories — an empty band trivially hit-tests to
    /// nothing). Exposed for the same reason as [`BarFigure::plot_area`].
    pub fn band_scale(&self) -> BandScale {
        BandScale::new(self.categories.clone(), BAND_PADDING)
    }

    /// `(sum of positive values, sum of negative values)` for category `i`
    /// across every series — the two cumulative extremes
    /// [`BarMode::Stacked`] actually reaches (positives stack up from `0`,
    /// negatives stack down from it, never combined into one running
    /// total).
    fn stack_totals(&self, i: usize) -> (f64, f64) {
        self.series.iter().fold((0.0_f64, 0.0_f64), |(pos, neg), s| match s.values.get(i) {
            Some(&v) if v >= 0.0 => (pos + v, neg),
            Some(&v) => (pos, neg + v),
            None => (pos, neg),
        })
    }

    /// Nice-rounded Y domain. [`BarMode::Grouped`]: the extent of every
    /// individual value across every series (baseline `0.0` always
    /// included, same convention the pre-multi-series single-series
    /// domain already used). [`BarMode::Stacked`]: the extent of each
    /// category's own cumulative positive/negative sums (a stacked
    /// column's visible height is the SUM of its segments, not any one
    /// segment's own value).
    fn y_scale(&self) -> Option<LinearScale> {
        if self.categories.is_empty() || self.series.is_empty() {
            return None;
        }
        let (data_min, data_max) = match self.mode {
            BarMode::Grouped => self
                .series
                .iter()
                .flat_map(|s| s.values.iter())
                .fold((0.0_f64, 0.0_f64), |(mn, mx), &v| (mn.min(v), mx.max(v))),
            BarMode::Stacked => (0..self.categories.len()).fold((0.0_f64, 0.0_f64), |(mn, mx), i| {
                let (pos, neg) = self.stack_totals(i);
                (mn.min(neg), mx.max(pos))
            }),
        };
        Some(LinearScale::nice(data_min, data_max, TARGET_Y_TICKS))
    }

    /// Per-series cumulative segment `(top_px, bottom_px)` pixel pairs for
    /// category `i`, in series order — the SAME running-sum walk
    /// [`crate::mark::rect::draw_bars_stacked`] uses internally, exposed
    /// so `render_with`'s own hover resolves a stacked segment through the
    /// EXACT geometry that was painted (design law #1, via
    /// [`crate::interact::hit::stacked_series_at`]).
    fn stacked_segments_px(&self, area: &PlotArea, y: &LinearScale, i: usize) -> Vec<(f64, f64)> {
        let mut pos_acc = 0.0_f64;
        let mut neg_acc = 0.0_f64;
        self.series
            .iter()
            .map(|s| {
                let value = s.values.get(i).copied().unwrap_or(0.0);
                let (bottom_v, top_v) = if value >= 0.0 {
                    let bottom = pos_acc;
                    pos_acc += value;
                    (bottom, pos_acc)
                } else {
                    let top = neg_acc;
                    neg_acc += value;
                    (neg_acc, top)
                };
                (area.y(y, top_v), area.y(y, bottom_v))
            })
            .collect()
    }

    fn hover_tooltip_lines(&self, i: usize, si: usize, y_scale: &LinearScale) -> Vec<(String, String)> {
        let category = self.categories.get(i).cloned().unwrap_or_default();
        let series_name = self.series.get(si).map(|s| s.name.clone()).unwrap_or_default();
        let value = self.series.get(si).and_then(|s| s.values.get(i)).copied().unwrap_or(0.0);
        let step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
        vec![("category".to_owned(), category), ("series".to_owned(), series_name), ("value".to_owned(), format_value(value, step))]
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a bar
    /// brightens its fill and shows a category/(series/)value tooltip;
    /// `overlay.focus`-selected categories get a persistent accent
    /// outline spanning the whole category band. `overlay.brush` is not
    /// consumed by this figure (bars in V2 have no brush-linked
    /// highlighting — see [`crate::figure::FigureOverlay`]).
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

        if let Some(y_scale) = self.y_scale() {
            let band = self.band_scale();

            grid::draw_y_grid(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);

            // Reference lines/bands paint UNDER the bars (over the grid,
            // under the data) — same ordering `CurveFigure::render_with`
            // uses.
            draw_annotations(ctx, &area, &band, &y_scale, theme, &self.annotations);

            if self.series.len() <= 1 {
                if let Some(single) = self.series.first() {
                    let style = MarkStyle { color: theme.palette[0].clone(), ..Default::default() };
                    draw_bars(ctx, &area, &band, &y_scale, &single.values, &style);
                }
            } else {
                let series_values: Vec<&[f64]> = self.series.iter().map(|s| s.values.as_slice()).collect();
                let colors: Vec<&str> = (0..self.series.len()).map(|i| theme.palette[i % theme.palette.len()].as_str()).collect();
                match self.mode {
                    BarMode::Grouped => draw_bars_grouped(ctx, &area, &band, &y_scale, &series_values, &colors),
                    BarMode::Stacked => draw_bars_stacked(ctx, &area, &band, &y_scale, &series_values, &colors),
                }
            }

            if self.show_value_labels {
                if let Some(single) = self.series.first().filter(|_| self.series.len() == 1) {
                    let step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
                    for (i, &value) in single.values.iter().enumerate().take(band.len()) {
                        let (x0, x1) = area.x_band(&band, i);
                        let label_y = area.y(&y_scale, value) - 6.0;
                        draw_label_centered(ctx, &format_value(value, step), (x0 + x1) / 2.0, label_y, &theme.label_color, &theme.label_font);
                    }
                }
            }

            if let Some(focus) = overlay.focus {
                let accent = &theme.palette[1];
                for i in 0..band.len() {
                    if focus.is_selected(i as u64) {
                        let (x0, x1) = area.x_band(&band, i);
                        ctx.set_stroke_color(accent);
                        ctx.set_stroke_width(SELECTED_STROKE_WIDTH);
                        ctx.stroke_rect(x0, area.rect.y, (x1 - x0).max(0.0), area.rect.height);
                    }
                }
            }

            if let Some((hx, hy)) = overlay.hover_px {
                if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                    if let Some(i) = hit::bar_index_at(&area, &band, hx) {
                        if self.series.len() <= 1 {
                            let (x0, x1) = area.x_band(&band, i);
                            ctx.set_fill_color("#ffffff");
                            ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                            ctx.fill_rect(x0, area.rect.y, (x1 - x0).max(0.0), area.rect.height);
                            ctx.set_global_alpha(1.0);

                            let category = self.categories.get(i).cloned().unwrap_or_default();
                            let value = self.series.first().and_then(|s| s.values.get(i)).copied().unwrap_or(0.0);
                            let step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
                            let lines = vec![("category".to_owned(), category), ("value".to_owned(), format_value(value, step))];
                            tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, area.rect);
                        } else {
                            match self.mode {
                                BarMode::Grouped => {
                                    let (x0, x1) = area.x_band(&band, i);
                                    if let Some(si) = hit::bar_series_at(x0, x1, self.series.len(), hx) {
                                        let (sx0, sx1) = crate::mark::rect::sub_band_range(x0, x1, self.series.len(), si);
                                        ctx.set_fill_color("#ffffff");
                                        ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                                        ctx.fill_rect(sx0, area.rect.y, (sx1 - sx0).max(0.0), area.rect.height);
                                        ctx.set_global_alpha(1.0);

                                        let lines = self.hover_tooltip_lines(i, si, &y_scale);
                                        tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, area.rect);
                                    }
                                }
                                BarMode::Stacked => {
                                    let segments = self.stacked_segments_px(&area, &y_scale, i);
                                    if let Some(si) = hit::stacked_series_at(&segments, hy) {
                                        let (x0, x1) = area.x_band(&band, i);
                                        let (top, bottom) = segments[si];
                                        ctx.set_fill_color("#ffffff");
                                        ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                                        ctx.fill_rect(x0, top.min(bottom), (x1 - x0).max(0.0), (bottom - top).abs());
                                        ctx.set_global_alpha(1.0);

                                        let lines = self.hover_tooltip_lines(i, si, &y_scale);
                                        tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, area.rect);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            axis::draw_x_axis(ctx, &area, &band, theme, band.len());
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
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

    fn cats(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("cat-{i}")).collect()
    }

    #[test]
    fn new_single_series_never_auto_shows_a_legend() {
        let figure = BarFigure::new(cats(3), vec![1.0, 2.0, 3.0]);
        assert_eq!(figure.resolved_legend_position(), None);
    }

    #[test]
    fn with_series_multi_series_auto_shows_a_top_legend() {
        let series = vec![
            BarSeries { name: "a".to_owned(), values: vec![1.0, 2.0] },
            BarSeries { name: "b".to_owned(), values: vec![3.0, 4.0] },
        ];
        let figure = BarFigure::with_series(cats(2), series, BarMode::Grouped);
        assert_eq!(figure.resolved_legend_position(), Some(LegendPosition::Top));
    }

    #[test]
    fn with_legend_overrides_the_auto_default_even_for_a_single_series() {
        let figure = BarFigure::new(cats(2), vec![1.0, 2.0]).with_legend(LegendPosition::Right);
        assert_eq!(figure.resolved_legend_position(), Some(LegendPosition::Right));
    }

    #[test]
    fn legend_entries_assign_distinct_theme_palette_colors_by_series_index() {
        let series = vec![
            BarSeries { name: "a".to_owned(), values: vec![1.0] },
            BarSeries { name: "b".to_owned(), values: vec![2.0] },
            BarSeries { name: "c".to_owned(), values: vec![3.0] },
        ];
        let figure = BarFigure::with_series(cats(1), series, BarMode::Grouped);
        let theme = FigureTheme::dark();
        let entries = figure.legend_entries(&theme);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].label, "a");
        let colors: Vec<&str> = entries.iter().map(|e| e.color.as_str()).collect();
        let mut unique = colors.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), colors.len(), "every series must get its own distinct swatch color");
    }

    #[test]
    fn grouped_y_scale_spans_every_series_own_value_extent() {
        let series = vec![
            BarSeries { name: "a".to_owned(), values: vec![10.0, -5.0] },
            BarSeries { name: "b".to_owned(), values: vec![40.0, 2.0] },
        ];
        let figure = BarFigure::with_series(cats(2), series, BarMode::Grouped);
        let scale = figure.y_scale().expect("non-empty fixture");
        assert!(scale.min <= -5.0, "grouped domain must contain the most negative individual value");
        assert!(scale.max >= 40.0, "grouped domain must contain the largest individual value");
    }

    #[test]
    fn stacked_totals_split_positive_and_negative_sums_never_mixed() {
        let series = vec![
            BarSeries { name: "revenue".to_owned(), values: vec![20.0] },
            BarSeries { name: "cost".to_owned(), values: vec![-8.0] },
            BarSeries { name: "adjustment".to_owned(), values: vec![5.0] },
        ];
        let figure = BarFigure::with_series(cats(1), series, BarMode::Stacked);
        let (pos, neg) = figure.stack_totals(0);
        assert!((pos - 25.0).abs() < 1e-9, "positive sum must be 20 + 5, cost excluded");
        assert!((neg - (-8.0)).abs() < 1e-9, "negative sum must be -8 alone");
    }

    #[test]
    fn stacked_y_scale_spans_the_cumulative_extremes_not_any_single_value() {
        let series = vec![
            BarSeries { name: "revenue".to_owned(), values: vec![20.0, 15.0] },
            BarSeries { name: "cost".to_owned(), values: vec![-8.0, -12.0] },
            BarSeries { name: "adjustment".to_owned(), values: vec![5.0, -3.0] },
        ];
        let figure = BarFigure::with_series(cats(2), series, BarMode::Stacked);
        let scale = figure.y_scale().expect("non-empty fixture");
        // Category 0: positive total 20+5=25, negative total -8.
        // Category 1: positive total 15, negative total -12-3=-15.
        assert!(scale.max >= 25.0, "domain must contain the largest cumulative POSITIVE total, not any single series value");
        assert!(scale.min <= -15.0, "domain must contain the most negative cumulative total, not any single series value");
    }

    #[test]
    fn stacked_segments_px_top_equals_cumulative_sum_and_total_height_matches_the_full_sum() {
        let series = vec![
            BarSeries { name: "a".to_owned(), values: vec![10.0] },
            BarSeries { name: "b".to_owned(), values: vec![20.0] },
            BarSeries { name: "c".to_owned(), values: vec![5.0] },
        ];
        let figure = BarFigure::with_series(cats(1), series, BarMode::Stacked);
        let y_scale = figure.y_scale().expect("non-empty fixture");
        let area = PlotArea::new(Rect::new(0.0, 0.0, 100.0, 200.0));
        let segments = figure.stacked_segments_px(&area, &y_scale, 0);
        assert_eq!(segments.len(), 3);

        // Segment 0 spans [0, 10]; segment 1 spans [10, 30] (cumulative);
        // segment 2 spans [30, 35] (cumulative) — the WHOLE column's own
        // top pixel must equal the pixel for domain value 35 (10+20+5),
        // the full sum, not any single segment's own value.
        let expected_total_top_px = area.y(&y_scale, 35.0);
        let (seg2_top, _seg2_bottom) = segments[2];
        assert!((seg2_top - expected_total_top_px).abs() < 1e-6, "the last segment's own top must land at the FULL cumulative sum");

        // The whole stacked column's pixel height must equal the sum of
        // every segment's own pixel height (no overlap, no gap).
        let column_top_px = segments.iter().map(|&(t, _)| t).fold(f64::INFINITY, f64::min);
        let column_bottom_px = segments.iter().map(|&(_, b)| b).fold(f64::NEG_INFINITY, f64::max);
        let column_height = column_bottom_px - column_top_px;
        let summed_height: f64 = segments.iter().map(|&(t, b)| (b - t).abs()).sum();
        assert!((column_height - summed_height).abs() < 1e-6, "the whole column's height must equal the sum of its own segment heights");
    }

    #[test]
    fn stacked_segments_px_negative_values_stack_downward_from_the_baseline() {
        let series = vec![
            BarSeries { name: "a".to_owned(), values: vec![-10.0] },
            BarSeries { name: "b".to_owned(), values: vec![-20.0] },
        ];
        let figure = BarFigure::with_series(cats(1), series, BarMode::Stacked);
        let y_scale = figure.y_scale().expect("non-empty fixture");
        let area = PlotArea::new(Rect::new(0.0, 0.0, 100.0, 200.0));
        let segments = figure.stacked_segments_px(&area, &y_scale, 0);

        let baseline_px = area.y(&y_scale, 0.0);
        // Both segments must sit AT OR BELOW the zero baseline on screen
        // (larger y = lower on screen) — negative values never stack
        // upward into positive territory.
        for &(top, bottom) in &segments {
            let lower = top.max(bottom);
            let upper = top.min(bottom);
            assert!(upper >= baseline_px - 1e-6, "a negative segment must never extend above the zero baseline");
            assert!(lower > baseline_px - 1e-6);
        }
        // Segment 1 (cumulative -30) must sit further from the baseline
        // (further down the screen) than segment 0 (cumulative -10 alone).
        let seg0_bottom = segments[0].1.max(segments[0].0);
        let seg1_bottom = segments[1].1.max(segments[1].0);
        assert!(seg1_bottom > seg0_bottom, "later negative segments must stack FURTHER from the baseline, not overlap the first");
    }

    #[test]
    fn with_annotations_default_is_empty_and_render_still_succeeds() {
        use uzor_export::{render_to_png, ExportSpec};

        let figure = BarFigure::new(cats(3), vec![5.0, 10.0, 15.0])
            .with_annotations(vec![crate::guide::annotation::Annotation::HLine { value: 8.0, color: None, label: Some("target".to_owned()) }]);
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 300.0, 200.0), &theme);
        });
        assert!(result.is_ok());
        assert!(BarFigure::new(cats(2), vec![1.0, 2.0]).annotations.is_empty());
    }

    #[test]
    fn single_series_render_uses_the_original_draw_bars_path_not_grouped_geometry() {
        // A single series must still render (no panic, valid dimensions) —
        // this exercises the `self.series.len() <= 1` branch that keeps
        // `BarFigure::new`'s output byte-compatible with the
        // pre-multi-series render.
        use uzor_export::{render_to_png, ExportSpec};

        let figure = BarFigure::new(cats(3), vec![5.0, 10.0, 15.0]).with_title("single series");
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let result = render_to_png(&spec, |ctx| {
            figure.render(ctx, rect, &theme);
        });
        assert!(result.is_ok());
    }
}
