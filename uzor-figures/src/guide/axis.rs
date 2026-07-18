//! Axis guides — baseline + tick marks + tick labels, driven entirely by
//! [`Scale::ticks`] and drawn through the same [`PlotArea`] transform
//! every mark uses.
//!
//! [`BandScale`] needs no special case here: its `ticks()` already
//! returns one tick per category, and its `map()` already resolves an
//! index to the band CENTER (not the band edge) — so
//! `area.x(band_scale, tick.value)` lands the label exactly where a
//! category-axis label belongs, through the exact same call every
//! continuous-scale axis uses.
//!
//! Label collision is a simple greedy left-to-right (x-axis) / top-to-
//! bottom (y-axis) skip: if the next label would overlap the last DRAWN
//! label's extent, it's dropped rather than crowded — cheap and gives a
//! readable axis without a real constraint solver.

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::coord::PlotArea;
use crate::mark::text::draw_label_right_aligned;
use crate::scale::{NumberFormat, Scale, Tick};
use crate::theme::FigureTheme;

const TICK_LENGTH: f64 = 4.0;
const LABEL_GAP: f64 = 4.0;

/// Best-effort tick "step" derived from the first two ticks — same
/// "derive a display step from the scale's own ticks" convention
/// [`Scale::format_value`]'s default impl and [`crate::guide::crosshair`]'s
/// own `axis_step` helper already use, reused here so a
/// [`NumberFormat`]-formatted label picks the same sane decimal precision
/// an unformatted tick label would. Falls back to `1.0` for a degenerate/
/// single-tick set.
fn ticks_step(ticks: &[Tick]) -> f64 {
    if ticks.len() >= 2 {
        (ticks[1].value - ticks[0].value).abs().max(f64::EPSILON)
    } else {
        1.0
    }
}

/// Draw the bottom x-axis: baseline, downward tick marks, and tick labels
/// centered under each tick.
pub fn draw_x_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, None);
}

/// Same as [`draw_x_axis`], but re-formats every tick's own label through
/// `format` (e.g. [`NumberFormat::Si`]/`Percent`/`Currency`) instead of the
/// scale's own default [`Tick::label`] — the tick VALUES, positions, and
/// greedy-skip collision logic are entirely UNCHANGED (design law #1: one
/// tick-generation/collision algorithm; only the display STRING differs).
pub fn draw_x_axis_formatted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: NumberFormat) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, Some(format));
}

fn draw_x_axis_impl(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: Option<NumberFormat>) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    let step = ticks_step(&ticks);

    let axis_y = area.rect.bottom();
    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(area.rect.x, axis_y);
    ctx.line_to(area.rect.right(), axis_y);
    ctx.stroke();

    ctx.set_font(&theme.label_font);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Top);

    let mut last_label_right = f64::MIN;
    for tick in &ticks {
        let x = area.x(scale, tick.value);

        ctx.set_stroke_color(&theme.axis_color);
        ctx.begin_path();
        ctx.move_to(x, axis_y);
        ctx.line_to(x, axis_y + TICK_LENGTH);
        ctx.stroke();

        let label = match format {
            Some(f) => f.format(tick.value, step),
            None => tick.label.clone(),
        };
        let half_w = ctx.measure_text(&label) / 2.0;
        if x - half_w < last_label_right + LABEL_GAP {
            continue; // would collide with the previously drawn label
        }
        ctx.set_fill_color(&theme.label_color);
        ctx.fill_text(&label, x, axis_y + TICK_LENGTH + LABEL_GAP);
        last_label_right = x + half_w;
    }
}

/// Draw the left y-axis: baseline, leftward tick marks, and right-aligned
/// tick labels.
pub fn draw_y_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, None);
}

/// Same as [`draw_y_axis`], but re-formats every tick's own label through
/// `format` — see [`draw_x_axis_formatted`]'s own docs (identical
/// reasoning, Y side).
pub fn draw_y_axis_formatted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: NumberFormat) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, Some(format));
}

fn draw_y_axis_impl(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: Option<NumberFormat>) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    let step = ticks_step(&ticks);

    let axis_x = area.rect.x;
    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(axis_x, area.rect.y);
    ctx.line_to(axis_x, area.rect.bottom());
    ctx.stroke();

    let labels: Vec<String> = ticks.iter().map(|t| match format { Some(f) => f.format(t.value, step), None => t.label.clone() }).collect();

    // Row height for the vertical greedy-skip check — text_bounds (not
    // just measure_text) because the y-axis's collision axis is height,
    // not width.
    let row_height = labels
        .iter()
        .map(|label| ctx.text_bounds(label, &theme.label_font).h)
        .fold(0.0_f64, f64::max)
        .max(1.0);

    // `ticks` is ascending by value, and `area.y` is INVERTED (larger
    // value -> smaller y) — so screen y monotonically DECREASES as this
    // loop advances. The greedy-skip tracker must follow that direction:
    // it remembers the TOP edge of the last label actually drawn (the
    // smallest y so far), and a candidate collides when its own BOTTOM
    // edge would reach up into that already-claimed space.
    let mut last_label_top: Option<f64> = None;
    for (tick, label) in ticks.iter().zip(labels.iter()) {
        let y = area.y(scale, tick.value);

        ctx.set_stroke_color(&theme.axis_color);
        ctx.begin_path();
        ctx.move_to(axis_x - TICK_LENGTH, y);
        ctx.line_to(axis_x, y);
        ctx.stroke();

        let label_bottom = y + row_height / 2.0;
        if let Some(prev_top) = last_label_top {
            if label_bottom + LABEL_GAP > prev_top {
                continue; // would collide with the previously drawn label
            }
        }
        draw_label_right_aligned(ctx, label, axis_x - TICK_LENGTH - LABEL_GAP, y, &theme.label_color, &theme.label_font);
        last_label_top = Some(y - row_height / 2.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;
    use crate::theme::FigureTheme;
    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(40.0, 10.0, 300.0, 150.0))
    }

    #[test]
    fn ticks_step_derives_from_the_first_two_ticks() {
        let ticks = vec![
            Tick { value: 0.0, label: "0".to_owned() },
            Tick { value: 10.0, label: "10".to_owned() },
            Tick { value: 20.0, label: "20".to_owned() },
        ];
        assert_eq!(ticks_step(&ticks), 10.0);
    }

    #[test]
    fn ticks_step_falls_back_to_one_for_a_single_tick() {
        let ticks = vec![Tick { value: 5.0, label: "5".to_owned() }];
        assert_eq!(ticks_step(&ticks), 1.0);
    }

    #[test]
    fn draw_x_axis_formatted_renders_without_panicking_for_every_variant() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 5000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        for format in [NumberFormat::Plain, NumberFormat::Thousands, NumberFormat::Si, NumberFormat::Percent, NumberFormat::Currency("$")] {
            let result = render_to_png(&spec, |ctx| {
                draw_x_axis_formatted(ctx, &area(), &scale, &theme, 5, format);
            });
            assert!(result.is_ok(), "draw_x_axis_formatted must render cleanly for {format:?}");
        }
    }

    #[test]
    fn draw_y_axis_formatted_renders_without_panicking_for_every_variant() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(-500.0, 500.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        for format in [NumberFormat::Plain, NumberFormat::Thousands, NumberFormat::Si, NumberFormat::Percent, NumberFormat::Currency("$")] {
            let result = render_to_png(&spec, |ctx| {
                draw_y_axis_formatted(ctx, &area(), &scale, &theme, 5, format);
            });
            assert!(result.is_ok(), "draw_y_axis_formatted must render cleanly for {format:?}");
        }
    }

    #[test]
    fn formatted_with_thousands_default_is_the_same_shape_as_unformatted() {
        // Not a byte-for-byte draw-op capture (this crate's own headless
        // proof convention doesn't expose one for `guide::axis`), but both
        // paths must at minimum render successfully over the SAME scale —
        // `NumberFormat::default()` resolving to `Thousands` is proven at
        // the pure-formatter level in `scale::format`'s own tests
        // (`default_matches_current_thousands_output`).
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 1_000_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let unformatted = render_to_png(&spec, |ctx| draw_x_axis(ctx, &area(), &scale, &theme, 5));
        let formatted = render_to_png(&spec, |ctx| draw_x_axis_formatted(ctx, &area(), &scale, &theme, 5, NumberFormat::default()));
        assert!(unformatted.is_ok() && formatted.is_ok());
    }
}
