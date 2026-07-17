//! Colorbar guide — a vertical gradient swatch + min/max tick labels for a
//! [`ColorScale`]-driven figure (e.g. [`crate::figure::HeatmapFigure`]'s
//! cell fill). The continuous-scale counterpart of [`crate::guide::legend`]'s
//! discrete swatch list — measured BEFORE plot layout so a figure can
//! shrink its own plot rect by the exact reserved size (same design law #1
//! discipline `legend` already documents).
//!
//! The gradient itself is built from [`ColorScale::stops`] fed straight
//! into [`uzor::render::GradientPainter::fill_linear_gradient`] — no pixel
//! sampling or manual rect-striping, a real smooth gradient on backends
//! that support one (falls back to a flat first-stop fill on backends that
//! don't, same degrade `GradientPainter`'s own default impl documents).

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::scale::color::ColorScale;
use crate::scale::linear::format_value;
use crate::theme::FigureTheme;

const BAR_WIDTH: f64 = 14.0;
const PAD: f64 = 6.0;
const TICK_GAP: f64 = 6.0;
/// Sample density for the gradient's own stop list — coarse enough to be
/// cheap, fine enough that the OKLCH ramp reads as continuous rather than
/// banded.
const GRADIENT_STOPS: usize = 12;

/// A colorbar's measured reserved size — a figure shrinks its plot rect by
/// this (plus its own gap constant) on whichever side it reserves the
/// colorbar (this guide is always a fixed-width RIGHT-side band).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColorbarSize {
    pub width: f64,
}

/// Measure a colorbar for `scale`'s own domain — call BEFORE painting; the
/// figure then shrinks its plot rect by the result (design law #1).
pub fn measure_colorbar(ctx: &mut dyn RenderContext, theme: &FigureTheme, scale: &ColorScale) -> ColorbarSize {
    ctx.set_font(&theme.label_font);
    let (min, max) = scale.domain();
    let label_w = ctx.measure_text(&format_value(min, 1.0)).max(ctx.measure_text(&format_value(max, 1.0)));
    ColorbarSize { width: PAD + BAR_WIDTH + TICK_GAP + label_w }
}

/// Paint a vertical gradient bar (MAX at the top, MIN at the bottom —
/// matches [`crate::coord::PlotArea::y`]'s own "larger value plots higher"
/// convention) into `rect`, with min/max tick labels beside it.
pub fn draw_colorbar(ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, scale: &ColorScale) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let (min, max) = scale.domain();
    let bar_x = rect.x;
    let bar_y = rect.y;
    let bar_h = rect.height;

    // `stops()` is ascending by domain value (min -> max); the gradient
    // LINE runs top (bar_y, wants MAX) -> bottom (bar_y + bar_h, wants
    // MIN), so the stop list is fed in REVERSE (descending value = 0.0 at
    // the top) to land max's color at offset 0.0.
    let raw_stops = scale.stops(GRADIENT_STOPS);
    let n = raw_stops.len().max(1);
    let gradient_stops: Vec<(f32, String)> = raw_stops
        .iter()
        .rev()
        .enumerate()
        .map(|(i, (_, hex))| (i as f32 / (n - 1).max(1) as f32, hex.clone()))
        .collect();
    let stop_refs: Vec<(f32, &str)> = gradient_stops.iter().map(|(o, c)| (*o, c.as_str())).collect();

    ctx.begin_path();
    ctx.rect(bar_x, bar_y, BAR_WIDTH, bar_h);
    ctx.fill_linear_gradient(&stop_refs, bar_x, bar_y, bar_x, bar_y + bar_h);

    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.stroke_rect(bar_x, bar_y, BAR_WIDTH, bar_h);

    ctx.set_font(&theme.label_font);
    ctx.set_fill_color(&theme.label_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);
    ctx.fill_text(&format_value(max, 1.0), bar_x + BAR_WIDTH + TICK_GAP, bar_y);
    ctx.set_text_baseline(TextBaseline::Bottom);
    ctx.fill_text(&format_value(min, 1.0), bar_x + BAR_WIDTH + TICK_GAP, bar_y + bar_h);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_export::{render_to_png, ExportSpec};

    #[test]
    fn measure_colorbar_reserves_a_positive_width() {
        let theme = FigureTheme::dark();
        let scale = ColorScale::sequential(0.0, 100.0);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = ColorbarSize::default();
        render_to_png(&spec, |ctx| {
            size = measure_colorbar(ctx, &theme, &scale);
        })
        .expect("render");
        assert!(size.width > BAR_WIDTH);
    }

    #[test]
    fn draw_colorbar_renders_without_panicking_including_a_degenerate_rect() {
        let theme = FigureTheme::dark();
        let scale = ColorScale::sequential(0.0, 100.0);
        let spec = ExportSpec { width_px: 200, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_colorbar(ctx, Rect::new(10.0, 10.0, 60.0, 150.0), &theme, &scale);
            draw_colorbar(ctx, Rect::new(0.0, 0.0, 0.0, 0.0), &theme, &scale);
        });
        assert!(result.is_ok());
    }
}
