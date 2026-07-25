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
use crate::scale::ClassScale;
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

/// Measure a discrete colorbar for `scale`'s own classes — call BEFORE
/// painting, same discipline as [`measure_colorbar`]. Reserves space for
/// the WIDEST class label (`scale.class_label(i)` over every class), not
/// just the two domain endpoints a continuous [`measure_colorbar`] reserves
/// for.
pub fn measure_discrete_colorbar(ctx: &mut dyn RenderContext, theme: &FigureTheme, scale: &dyn ClassScale) -> ColorbarSize {
    ctx.set_font(&theme.label_font);
    let n = scale.class_count();
    let widest = (0..n).map(|i| ctx.measure_text(&scale.class_label(i))).fold(0.0_f64, f64::max);
    ColorbarSize { width: PAD + BAR_WIDTH + TICK_GAP + widest }
}

/// The discrete-class counterpart of [`draw_colorbar`] — `scale.
/// class_count()` STACKED SOLID swatches (never a gradient — a binning
/// scale's whole point is that its output is a CLASS, not a continuously
/// interpolated ramp position) with each class's own [`ClassScale::
/// class_label`] beside it, wired into [`crate::guide::legend`]/this
/// module the same way [`crate::guide::legend::entries_from_class_scale`]
/// wires a binning scale into the discrete swatch LIST — this is the
/// vertical-bar sibling of that same "present a binning scale as discrete
/// classes, not a ramp" capability.
///
/// Class `0` (the LOWEST class) draws at the BOTTOM, the highest class at
/// the TOP — the same "max at top, min at bottom" convention
/// [`draw_colorbar`] already uses (matches [`crate::coord::PlotArea::y`]'s
/// own "larger value plots higher" convention). `colors` supplies each
/// class's own swatch fill, cycling via `index % colors.len()` once
/// `scale.class_count()` exceeds `colors.len()` — same modulo policy
/// [`crate::guide::legend::entries_from_class_scale`] documents; an empty
/// `colors` slice falls back to `theme.palette`.
pub fn draw_discrete_colorbar(ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, scale: &dyn ClassScale, colors: &[String]) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let n = scale.class_count().max(1);
    let bar_x = rect.x;
    let bar_y = rect.y;
    let swatch_h = rect.height / n as f64;
    let color_source: &[String] = if colors.is_empty() { &theme.palette } else { colors };
    let len = color_source.len().max(1);

    ctx.set_font(&theme.label_font);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    for i in 0..n {
        // Class `i == 0` is the LOWEST class -> bottom row on screen;
        // class `n - 1` is the highest -> top row.
        let row_from_top = n - 1 - i;
        let y0 = bar_y + row_from_top as f64 * swatch_h;
        let color = color_source.get(i % len).map(String::as_str).unwrap_or("#808080");

        ctx.set_fill_color(color);
        ctx.fill_rect(bar_x, y0, BAR_WIDTH, swatch_h);
        ctx.set_stroke_color(&theme.axis_color);
        ctx.set_stroke_width(1.0);
        ctx.stroke_rect(bar_x, y0, BAR_WIDTH, swatch_h);

        ctx.set_fill_color(&theme.label_color);
        ctx.fill_text(&scale.class_label(i), bar_x + BAR_WIDTH + TICK_GAP, y0 + swatch_h / 2.0);
    }
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

    // ── discrete colorbar (binning scale) ───────────────────────────────

    #[test]
    fn measure_discrete_colorbar_reserves_a_positive_width() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(0.0, 100.0, 5);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = ColorbarSize::default();
        render_to_png(&spec, |ctx| {
            size = measure_discrete_colorbar(ctx, &theme, &scale);
        })
        .expect("render");
        assert!(size.width > BAR_WIDTH);
    }

    #[test]
    fn draw_discrete_colorbar_renders_without_panicking_including_a_degenerate_rect() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(-50.0, 150.0, 5);
        let colors: Vec<String> = vec!["#111111".to_owned(), "#222222".to_owned(), "#333333".to_owned(), "#444444".to_owned(), "#555555".to_owned()];
        let spec = ExportSpec { width_px: 200, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_discrete_colorbar(ctx, Rect::new(10.0, 10.0, 60.0, 150.0), &theme, &scale, &colors);
            draw_discrete_colorbar(ctx, Rect::new(0.0, 0.0, 0.0, 0.0), &theme, &scale, &colors);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn draw_discrete_colorbar_is_visibly_banded_not_a_smooth_ramp() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(0.0, 100.0, 3);
        let colors: Vec<String> = vec!["#000000".to_owned(), "#808080".to_owned(), "#ffffff".to_owned()];
        let spec = ExportSpec { width_px: 100, height_px: 90, dpr: 1.0, background: None };
        let discrete = render_to_png(&spec, |ctx| {
            ctx.set_fill_color(&theme.background);
            ctx.fill_rect(0.0, 0.0, 100.0, 90.0);
            draw_discrete_colorbar(ctx, Rect::new(10.0, 10.0, 60.0, 70.0), &theme, &scale, &colors);
        })
        .expect("discrete render");
        let continuous = render_to_png(&spec, |ctx| {
            ctx.set_fill_color(&theme.background);
            ctx.fill_rect(0.0, 0.0, 100.0, 90.0);
            let ramp = ColorScale::sequential_colors(0.0, 100.0, &["#000000", "#808080", "#ffffff"]);
            draw_colorbar(ctx, Rect::new(10.0, 10.0, 60.0, 70.0), &theme, &ramp);
        })
        .expect("continuous render");
        assert_ne!(discrete, continuous, "a discrete banded colorbar must render visibly differently from a smooth gradient ramp over comparable colors");
    }

    #[test]
    fn discrete_colorbar_falls_back_to_theme_palette_for_empty_colors() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(0.0, 100.0, 3);
        let spec = ExportSpec { width_px: 100, height_px: 90, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_discrete_colorbar(ctx, Rect::new(10.0, 10.0, 60.0, 70.0), &theme, &scale, &[]);
        });
        assert!(result.is_ok());
    }
}
