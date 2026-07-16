//! Crosshair guide — dashed hair-lines through a resolved domain point,
//! clipped to the plot rect, plus small axis-cursor labels. Drawing idiom
//! lifted from `mylittlechart`'s `chart_render/crosshair.rs::draw_crosshair`
//! (vertical + horizontal line pair through the cursor position) —
//! generalized off any two [`Scale`]s instead of a bar-index viewport +
//! price scale. mlc-specific concerns dropped: magnet-to-OHLC snapping,
//! multi-window/sub-pane Y re-derivation, and drag-clamped positioning —
//! V2's crosshair always tracks an already-resolved domain point (the
//! caller picks it via [`crate::interact::hit::nearest_point_x`]) with no
//! notion of sub-panes.

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::coord::PlotArea;
use crate::scale::Scale;
use crate::theme::FigureTheme;

const DASH_PATTERN: [f64; 2] = [4.0, 3.0];
const LABEL_PAD: f64 = 3.0;

/// Best-effort tick "step" at the current domain — reused only to pick a
/// sane decimal precision for the cursor label via
/// [`crate::scale::linear::format_value`]. Falls back to `1.0` for a
/// degenerate/near-empty tick set (e.g. a [`crate::scale::BandScale`],
/// which always returns one tick per category regardless of magnitude).
fn axis_step(scale: &dyn Scale) -> f64 {
    let ticks = scale.ticks(5);
    if ticks.len() >= 2 {
        (ticks[1].value - ticks[0].value).abs().max(f64::EPSILON)
    } else {
        1.0
    }
}

/// Draw a crosshair at domain point `(data_x, data_y)`: a dashed vertical
/// line spanning the plot's full height, a dashed horizontal line spanning
/// its full width, and small cursor labels on each axis. No-op if the
/// resolved screen position falls outside `area`'s own plot rect.
pub fn draw_crosshair(ctx: &mut dyn RenderContext, area: &PlotArea, theme: &FigureTheme, data_x: f64, data_y: f64, xscale: &dyn Scale, yscale: &dyn Scale) {
    let x = area.x(xscale, data_x);
    let y = area.y(yscale, data_y);
    if x < area.rect.x || x > area.rect.right() || y < area.rect.y || y > area.rect.bottom() {
        return;
    }

    ctx.set_stroke_color(&theme.label_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&DASH_PATTERN);

    ctx.begin_path();
    ctx.move_to(x, area.rect.y);
    ctx.line_to(x, area.rect.bottom());
    ctx.stroke();

    ctx.begin_path();
    ctx.move_to(area.rect.x, y);
    ctx.line_to(area.rect.right(), y);
    ctx.stroke();

    ctx.set_line_dash(&[]); // restore solid — every other guide in this crate assumes solid by default

    // X goes through `xscale.format_value` (the `Scale`-provided
    // formatter) rather than the raw numeric `format_value`/`axis_step`
    // pair `y` still uses below: `xscale` is whatever a figure passed in
    // (e.g. `CurveFigure::with_x_scale`'s `TimeScale` override), and only
    // the scale itself knows how to format its own domain correctly (a
    // `TimeScale`'s domain is Unix seconds — formatting it as a plain
    // number was a known gap, closed by `Scale::format_value`'s default +
    // `TimeScale`'s calendar override).
    let x_label = xscale.format_value(data_x);
    let y_label = crate::scale::linear::format_value(data_y, axis_step(yscale));

    ctx.set_font(&theme.label_font);
    ctx.set_fill_color(&theme.label_color);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Top);
    ctx.fill_text(&x_label, x, area.rect.bottom() + LABEL_PAD);

    ctx.set_text_align(TextAlign::Right);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&y_label, area.rect.x - LABEL_PAD, y);
}
