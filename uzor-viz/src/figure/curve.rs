//! `CurveFigure` — a line (optionally area-filled) series over two nice
//! linear domains. Covers plain line, cumulative curve, and filled-area
//! use cases — same mark composition, `fill` just toggles whether
//! [`crate::mark::area::draw_area`] runs before the stroke.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::guide::{axis, grid};
use crate::mark::area::draw_area;
use crate::mark::line::draw_polyline;
use crate::mark::MarkStyle;
use crate::scale::LinearScale;
use crate::theme::VizTheme;

const MARGIN_LEFT: f64 = 56.0;
const MARGIN_RIGHT: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_X_TICKS: usize = 6;
const TARGET_Y_TICKS: usize = 5;
/// Fill alpha applied under the stroked line when `fill` is enabled.
const FILL_ALPHA: f64 = 0.25;

/// A line series over `(x, y)` domain points.
pub struct CurveFigure {
    pub points: Vec<(f64, f64)>,
    pub title: Option<String>,
    pub fill: bool,
}

impl CurveFigure {
    pub fn new(points: Vec<(f64, f64)>) -> Self {
        Self { points, title: None, fill: false }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

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

        if self.points.len() >= 2 {
            let (x_min, x_max) = self
                .points
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &(px, _)| (mn.min(px), mx.max(px)));
            let (y_min, y_max) = self
                .points
                .iter()
                .fold((0.0_f64, 0.0_f64), |(mn, mx), &(_, py)| (mn.min(py), mx.max(py)));

            let x_scale = LinearScale::nice(x_min, x_max, TARGET_X_TICKS);
            let y_scale = LinearScale::nice(y_min, y_max, TARGET_Y_TICKS);

            grid::draw_x_grid(ctx, &area, &x_scale, theme, TARGET_X_TICKS);
            grid::draw_y_grid(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);

            let style = MarkStyle { color: theme.palette[0].clone(), ..Default::default() };
            if self.fill {
                let fill_style = MarkStyle { fill_alpha: FILL_ALPHA, ..style.clone() };
                draw_area(ctx, &area, &x_scale, &y_scale, &self.points, &fill_style);
            }
            draw_polyline(ctx, &area, &x_scale, &y_scale, &self.points, &style);

            axis::draw_x_axis(ctx, &area, &x_scale, theme, TARGET_X_TICKS);
            axis::draw_y_axis(ctx, &area, &y_scale, theme, TARGET_Y_TICKS);
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}
