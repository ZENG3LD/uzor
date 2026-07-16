//! `draw_bars` — vertical bars over a [`BandScale`] x-axis and any `y`
//! [`Scale`].

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::scale::{BandScale, Scale};

use super::MarkStyle;

/// Draw one vertical bar per `values[i]` (values beyond `band.len()` are
/// ignored — caller keeps `band`/`values` the same length).
///
/// The baseline is domain value `0.0`, clamped into `y`'s own domain — a
/// value range that never crosses zero (e.g. `[10, 50]`) still gets a
/// valid, on-scale baseline instead of extrapolating off-screen.
pub fn draw_bars(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    band: &BandScale,
    y: &dyn Scale,
    values: &[f64],
    style: &MarkStyle,
) {
    if band.is_empty() || values.is_empty() {
        return;
    }

    let (y_min, y_max) = y.domain();
    let baseline_value = 0.0_f64.clamp(y_min.min(y_max), y_min.max(y_max));
    let baseline_px = area.y(y, baseline_value);

    ctx.set_fill_color(&style.color);
    ctx.set_global_alpha(style.fill_alpha);
    for (i, &value) in values.iter().enumerate().take(band.len()) {
        let (x0, x1) = area.x_band(band, i);
        let value_px = area.y(y, value);
        let (top, height) =
            if value_px <= baseline_px { (value_px, baseline_px - value_px) } else { (baseline_px, value_px - baseline_px) };
        ctx.fill_rect(x0, top, (x1 - x0).max(0.0), height);
    }
    ctx.set_global_alpha(1.0);
}
