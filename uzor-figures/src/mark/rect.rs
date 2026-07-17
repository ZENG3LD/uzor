//! `draw_bars` — vertical bars over a [`BandScale`] x-axis and any `y`
//! [`Scale`]. `draw_bars_grouped`/`draw_bars_stacked` are the
//! multi-series counterparts (`crate::figure::BarFigure`'s own
//! `BarMode::{Grouped, Stacked}`) — both still draw through the SAME
//! `[`PlotArea`]`/`[`BandScale`] transform `draw_bars` uses, just with an
//! extra per-category sub-division ([`sub_band_range`]) or a cumulative
//! running-sum walk layered on top.

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

/// Inner padding fraction between series sub-bands within one category
/// band (grouped bars) — same role [`BandScale`]'s own `padding` plays one
/// level up, between categories.
const SUB_BAND_PADDING: f64 = 0.15;

/// Pixel `(left, right)` extent of series `series_index` (of
/// `series_count`) within a single category band's own pixel extent
/// `[x0, x1]` — [`crate::figure::BarFigure::render_with`] and
/// [`crate::interact::hit::bar_series_at`] both call this SAME function
/// (design law #1: one nested-band geometry formula for paint AND
/// hit-test).
///
/// Implemented by nesting a fresh [`BandScale`] of `series_count` (unnamed)
/// bands inside `[x0, x1]` and reusing its own already-tested
/// [`BandScale::band_range`] — the identical symmetric-inner-padding
/// convention a top-level category [`BandScale`] already uses, just one
/// level deeper. `series_count <= 1` returns `[x0, x1]` UNCHANGED (no
/// sub-padding applied) — a lone series has no adjacent sub-band to pad
/// against, so it fills the whole category band exactly, which is what
/// keeps [`crate::figure::BarFigure::new`]'s single-series render
/// byte-identical to its pre-multi-series output.
pub fn sub_band_range(x0: f64, x1: f64, series_count: usize, series_index: usize) -> (f64, f64) {
    if series_count <= 1 {
        return (x0, x1);
    }
    let sub = BandScale::new(vec![String::new(); series_count], SUB_BAND_PADDING);
    let (t0, t1) = sub.band_range(series_index);
    (x0 + t0 * (x1 - x0), x0 + t1 * (x1 - x0))
}

/// Draw grouped multi-series bars: within each category band, one
/// sub-band per series side by side (via [`sub_band_range`]), each series
/// its own color. `series_values[s][i]` is series `s`'s value for
/// category `i`; a category index missing from a shorter series is simply
/// skipped for that series (no panic, no synthetic zero bar drawn).
pub fn draw_bars_grouped(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    band: &BandScale,
    y: &dyn Scale,
    series_values: &[&[f64]],
    colors: &[&str],
) {
    if band.is_empty() || series_values.is_empty() {
        return;
    }

    let (y_min, y_max) = y.domain();
    let baseline_value = 0.0_f64.clamp(y_min.min(y_max), y_min.max(y_max));
    let baseline_px = area.y(y, baseline_value);
    let series_count = series_values.len();

    ctx.set_global_alpha(1.0);
    for i in 0..band.len() {
        let (x0, x1) = area.x_band(band, i);
        for (si, values) in series_values.iter().enumerate() {
            let Some(&value) = values.get(i) else { continue };
            let (sx0, sx1) = sub_band_range(x0, x1, series_count, si);
            let value_px = area.y(y, value);
            let (top, height) =
                if value_px <= baseline_px { (value_px, baseline_px - value_px) } else { (baseline_px, value_px - baseline_px) };
            ctx.set_fill_color(colors.get(si).copied().unwrap_or("#888888"));
            ctx.fill_rect(sx0, top, (sx1 - sx0).max(0.0), height);
        }
    }
}

/// Draw stacked multi-series bars: within each category band, series
/// segments stack cumulatively — positive values stack UPWARD from the
/// zero baseline, negative values stack DOWNWARD from it (standard
/// finance-chart convention; a category with both positive and negative
/// series never mixes them into one running total). `series_values[s][i]`
/// is series `s`'s value for category `i`, same missing-index-skips
/// convention as [`draw_bars_grouped`].
pub fn draw_bars_stacked(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    band: &BandScale,
    y: &dyn Scale,
    series_values: &[&[f64]],
    colors: &[&str],
) {
    if band.is_empty() || series_values.is_empty() {
        return;
    }

    ctx.set_global_alpha(1.0);
    for i in 0..band.len() {
        let (x0, x1) = area.x_band(band, i);
        let mut pos_acc = 0.0_f64;
        let mut neg_acc = 0.0_f64;
        for (si, values) in series_values.iter().enumerate() {
            let Some(&value) = values.get(i) else { continue };
            let (bottom_value, top_value) = if value >= 0.0 {
                let bottom = pos_acc;
                pos_acc += value;
                (bottom, pos_acc)
            } else {
                let top = neg_acc;
                neg_acc += value;
                (neg_acc, top)
            };
            let top_px = area.y(y, top_value);
            let bottom_px = area.y(y, bottom_value);
            let (top, height) = (top_px.min(bottom_px), (bottom_px - top_px).abs());
            ctx.set_fill_color(colors.get(si).copied().unwrap_or("#888888"));
            ctx.fill_rect(x0, top, (x1 - x0).max(0.0), height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_band_range_single_series_fills_the_whole_band_unchanged() {
        assert_eq!(sub_band_range(10.0, 90.0, 1, 0), (10.0, 90.0));
        assert_eq!(sub_band_range(10.0, 90.0, 0, 0), (10.0, 90.0));
    }

    #[test]
    fn sub_band_range_divides_band_into_equal_padded_slots() {
        // 4 equal series slots over a 100px-wide band: each raw slot is
        // 25px wide; padding shrinks it symmetrically, never widens it.
        let (x0, x1) = (0.0, 100.0);
        for i in 0..4 {
            let (sx0, sx1) = sub_band_range(x0, x1, 4, i);
            let raw_slot = 25.0;
            assert!(sx1 - sx0 < raw_slot, "padded sub-band must be narrower than its raw equal slot");
            assert!(sx1 - sx0 > 0.0);
        }
    }

    #[test]
    fn sub_band_range_slots_do_not_overlap_and_stay_in_stable_order() {
        let (x0, x1) = (0.0, 300.0);
        let n = 5;
        let mut prev_right = x0;
        for i in 0..n {
            let (sx0, sx1) = sub_band_range(x0, x1, n, i);
            assert!(sx0 >= prev_right - 1e-9, "series {i} slot must not start before the previous one ends");
            assert!(sx1 <= x1 + 1e-9, "series {i} slot must stay inside the category band");
            prev_right = sx1;
        }
    }

    #[test]
    fn draw_bars_grouped_ignores_a_series_missing_this_category_index() {
        // A shorter series (fewer values than categories) must not panic —
        // its missing index is simply skipped, not treated as a zero bar.
        // Real `RenderContext` via `uzor-export` (dev-dep), same headless
        // convention this crate's own proof tests use — no hand-rolled mock.
        use uzor_export::{render_to_png, ExportSpec};

        let spec = ExportSpec { width_px: 300, height_px: 100, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            let area = PlotArea::new(uzor::types::Rect::new(0.0, 0.0, 300.0, 100.0));
            let band = BandScale::new(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()], 0.2);
            let y = crate::scale::LinearScale::new(0.0, 10.0);
            let short: Vec<f64> = vec![5.0]; // only category 0 has a value
            let full: Vec<f64> = vec![2.0, 4.0, 6.0];
            draw_bars_grouped(ctx, &area, &band, &y, &[&short, &full], &["#111111", "#222222"]);
        });
        assert!(result.is_ok(), "a shorter series must be skipped past its missing index, never panic");
    }
}
