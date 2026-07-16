//! `BarFigure` — a categorical bar chart: nice-rounded linear Y domain,
//! band X, grid + axes + bars, optional per-bar value labels.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::guide::{axis, grid, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::mark::rect::draw_bars;
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

/// One category = one bar. `categories.len()` must equal `values.len()` —
/// [`crate::mark::rect::draw_bars`] takes the shorter of the two if they
/// differ, so a mismatch degrades gracefully rather than panicking.
pub struct BarFigure {
    pub categories: Vec<String>,
    pub values: Vec<f64>,
    pub title: Option<String>,
    pub show_value_labels: bool,
}

impl BarFigure {
    pub fn new(categories: Vec<String>, values: Vec<f64>) -> Self {
        Self { categories, values, title: None, show_value_labels: false }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_value_labels(mut self, show: bool) -> Self {
        self.show_value_labels = show;
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
    /// same reason as [`crate::figure::CurveFigure::plot_area`]: a caller
    /// driving hover/click routing from outside needs to hit-test through
    /// the EXACT same transform this figure renders with.
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.plot_rect(rect))
    }

    /// This figure's own category band scale (always constructible, even
    /// for zero categories — an empty band trivially hit-tests to
    /// nothing). Exposed for the same reason as [`BarFigure::plot_area`].
    pub fn band_scale(&self) -> BandScale {
        BandScale::new(self.categories.clone(), BAND_PADDING)
    }

    fn y_scale(&self) -> Option<LinearScale> {
        if self.categories.is_empty() || self.values.is_empty() {
            return None;
        }
        let (data_min, data_max) = self
            .values
            .iter()
            .fold((0.0_f64, 0.0_f64), |(mn, mx), &v| (mn.min(v), mx.max(v)));
        Some(LinearScale::nice(data_min, data_max, TARGET_Y_TICKS))
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a bar
    /// brightens its fill and shows a category/value tooltip;
    /// `overlay.focus`-selected bars get a persistent accent outline.
    /// `overlay.brush` is not consumed by this figure (bars in V2 have no
    /// brush-linked highlighting — see [`crate::figure::FigureOverlay`]).
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let area = self.plot_area(rect);

        if let Some(y_scale) = self.y_scale() {
            let band = self.band_scale();

            grid::draw_y_grid(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
            let style = MarkStyle { color: theme.palette[0].clone(), ..Default::default() };
            draw_bars(ctx, &area, &band, &y_scale, &self.values, &style);

            if self.show_value_labels {
                let step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
                for (i, &value) in self.values.iter().enumerate().take(band.len()) {
                    let (x0, x1) = area.x_band(&band, i);
                    let label_y = area.y(&y_scale, value) - 6.0;
                    draw_label_centered(ctx, &format_value(value, step), (x0 + x1) / 2.0, label_y, &theme.label_color, &theme.label_font);
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
                        let (x0, x1) = area.x_band(&band, i);
                        ctx.set_fill_color("#ffffff");
                        ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                        ctx.fill_rect(x0, area.rect.y, (x1 - x0).max(0.0), area.rect.height);
                        ctx.set_global_alpha(1.0);

                        let category = self.categories.get(i).cloned().unwrap_or_default();
                        let value = self.values.get(i).copied().unwrap_or(0.0);
                        let step = nice_step(y_scale.max - y_scale.min, TARGET_Y_TICKS as f64);
                        let lines = vec![("category".to_owned(), category), ("value".to_owned(), format_value(value, step))];
                        tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, area.rect);
                    }
                }
            }

            axis::draw_x_axis(ctx, &area, &band, theme, band.len());
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}
