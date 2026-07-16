//! `BarFigure` — a categorical bar chart: nice-rounded linear Y domain,
//! band X, grid + axes + bars, optional per-bar value labels.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::guide::{axis, grid};
use crate::mark::rect::draw_bars;
use crate::mark::text::draw_label_centered;
use crate::mark::MarkStyle;
use crate::scale::linear::{format_value, nice_step};
use crate::scale::{BandScale, LinearScale};
use crate::theme::VizTheme;

/// Left margin for the y-axis tick labels; bottom margin for the x-axis
/// tick labels; top margin reserved for an optional title.
const MARGIN_LEFT: f64 = 48.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_Y_TICKS: usize = 5;

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

    /// Render into `rect` of `ctx` using `theme`. Pure over `&self` —
    /// no state is retained between calls.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &VizTheme) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
        let plot_rect = Rect::new(
            rect.x + MARGIN_LEFT,
            rect.y + title_h,
            (rect.width - MARGIN_LEFT - MARGIN_RIGHT).max(0.0),
            (rect.height - title_h - MARGIN_BOTTOM).max(0.0),
        );
        let area = PlotArea::new(plot_rect);

        if !self.categories.is_empty() && !self.values.is_empty() {
            let (data_min, data_max) = self
                .values
                .iter()
                .fold((0.0_f64, 0.0_f64), |(mn, mx), &v| (mn.min(v), mx.max(v)));
            let y_scale = LinearScale::nice(data_min, data_max, TARGET_Y_TICKS);
            let band = BandScale::new(self.categories.clone(), 0.2);

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

            axis::draw_x_axis(ctx, &area, &band, theme, band.len());
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}
