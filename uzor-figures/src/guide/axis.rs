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
use crate::scale::Scale;
use crate::theme::FigureTheme;

const TICK_LENGTH: f64 = 4.0;
const LABEL_GAP: f64 = 4.0;

/// Draw the bottom x-axis: baseline, downward tick marks, and tick labels
/// centered under each tick.
pub fn draw_x_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }

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

        let half_w = ctx.measure_text(&tick.label) / 2.0;
        if x - half_w < last_label_right + LABEL_GAP {
            continue; // would collide with the previously drawn label
        }
        ctx.set_fill_color(&theme.label_color);
        ctx.fill_text(&tick.label, x, axis_y + TICK_LENGTH + LABEL_GAP);
        last_label_right = x + half_w;
    }
}

/// Draw the left y-axis: baseline, leftward tick marks, and right-aligned
/// tick labels.
pub fn draw_y_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }

    let axis_x = area.rect.x;
    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(axis_x, area.rect.y);
    ctx.line_to(axis_x, area.rect.bottom());
    ctx.stroke();

    // Row height for the vertical greedy-skip check — text_bounds (not
    // just measure_text) because the y-axis's collision axis is height,
    // not width.
    let row_height = ticks
        .iter()
        .map(|t| ctx.text_bounds(&t.label, &theme.label_font).h)
        .fold(0.0_f64, f64::max)
        .max(1.0);

    // `ticks` is ascending by value, and `area.y` is INVERTED (larger
    // value -> smaller y) — so screen y monotonically DECREASES as this
    // loop advances. The greedy-skip tracker must follow that direction:
    // it remembers the TOP edge of the last label actually drawn (the
    // smallest y so far), and a candidate collides when its own BOTTOM
    // edge would reach up into that already-claimed space.
    let mut last_label_top: Option<f64> = None;
    for tick in &ticks {
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
        draw_label_right_aligned(ctx, &tick.label, axis_x - TICK_LENGTH - LABEL_GAP, y, &theme.label_color, &theme.label_font);
        last_label_top = Some(y - row_height / 2.0);
    }
}
