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
use crate::scale::time::TickMarkWeight;
use crate::scale::{NumberFormat, Scale, Tick};
use crate::theme::FigureTheme;

const TICK_LENGTH: f64 = 4.0;
const LABEL_GAP: f64 = 4.0;

/// Visual policy for [`draw_x_axis_weighted`]/[`draw_y_axis_weighted`] —
/// how a tick's own [`Scale::tick_weight`] classification (only
/// [`crate::scale::TimeScale`] reports one today; every other scale's
/// ticks stay `None` and render EXACTLY like the unweighted
/// [`draw_x_axis`]/[`draw_y_axis`], regardless of this style) changes its
/// drawn tick length, stroke width, and (for a major tick only) label
/// color. A configurable field set, not a hardcoded constant — every
/// behavioral knob here is a real option: [`AxisTickWeightStyle::default`]
/// renders a visibly distinguishable major > medium > minor hierarchy
/// (the "tested hierarchy computed and thrown away" audit finding this
/// closes — [`crate::scale::time::TickMarkWeight`] was fully built and
/// unit-tested but never consulted by this shared guide);
/// [`AxisTickWeightStyle::flat`] reproduces the unweighted draw's own
/// single style for every tick regardless of weight, reachable through
/// the weighted entry point without switching call sites back to
/// `draw_x_axis`/`draw_y_axis`.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTickWeightStyle {
    /// Tick-mark length (px) for a major tick ([`TickMarkWeight::is_major`]
    /// — Year/Month calendar boundaries).
    pub major_tick_length: f64,
    /// Tick-mark length (px) for a medium tick ([`TickMarkWeight::
    /// is_medium`] — Day boundaries).
    pub medium_tick_length: f64,
    /// Tick-mark length (px) for every other (minor) tick — matches
    /// [`TICK_LENGTH`], the unweighted draw's own fixed value.
    pub minor_tick_length: f64,
    /// Stroke width (px) for a major tick's own mark.
    pub major_stroke_width: f64,
    /// Stroke width (px) for a medium tick's own mark.
    pub medium_stroke_width: f64,
    /// Stroke width (px) for a minor tick's own mark — matches the
    /// unweighted draw's own fixed `1.0`.
    pub minor_stroke_width: f64,
    /// Label color for a major tick's own text — `None` falls back to
    /// `theme.label_color` (same as every tick under the unweighted
    /// draw; medium/minor ticks always use `theme.label_color`).
    pub major_label_color: Option<String>,
}

impl Default for AxisTickWeightStyle {
    /// A visibly distinguishable major > medium > minor hierarchy —
    /// roughly double tick length + heavier stroke for a major boundary,
    /// a modest bump for a medium one, unchanged for minor.
    fn default() -> Self {
        Self {
            major_tick_length: TICK_LENGTH * 2.0,
            medium_tick_length: TICK_LENGTH * 1.5,
            minor_tick_length: TICK_LENGTH,
            major_stroke_width: 1.75,
            medium_stroke_width: 1.25,
            minor_stroke_width: 1.0,
            major_label_color: None,
        }
    }
}

impl AxisTickWeightStyle {
    /// Every tick draws identically regardless of its own weight — the
    /// SAME output [`draw_x_axis`]/[`draw_y_axis`] (the unweighted draw)
    /// already produce, reachable through the weighted entry point
    /// without switching call sites.
    pub fn flat() -> Self {
        Self {
            major_tick_length: TICK_LENGTH,
            medium_tick_length: TICK_LENGTH,
            minor_tick_length: TICK_LENGTH,
            major_stroke_width: 1.0,
            medium_stroke_width: 1.0,
            minor_stroke_width: 1.0,
            major_label_color: None,
        }
    }
}

/// Resolve `(tick_length, stroke_width)` for `weight` under `style` — a
/// tick with no weight (`None`, every non-[`crate::scale::TimeScale`]
/// scale, or a `TimeScale` tick this crate's own classifier never marks
/// major/medium) resolves to the MINOR tier.
pub(crate) fn resolve_tick_style(weight: Option<TickMarkWeight>, style: &AxisTickWeightStyle) -> (f64, f64) {
    match weight {
        Some(w) if w.is_major() => (style.major_tick_length, style.major_stroke_width),
        Some(w) if w.is_medium() => (style.medium_tick_length, style.medium_stroke_width),
        _ => (style.minor_tick_length, style.minor_stroke_width),
    }
}

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
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, None, None);
}

/// Same as [`draw_x_axis`], but re-formats every tick's own label through
/// `format` (e.g. [`NumberFormat::Si`]/`Percent`/`Currency`) instead of the
/// scale's own default [`Tick::label`] — the tick VALUES, positions, and
/// greedy-skip collision logic are entirely UNCHANGED (design law #1: one
/// tick-generation/collision algorithm; only the display STRING differs).
pub fn draw_x_axis_formatted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: NumberFormat) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, Some(format), None);
}

/// Same as [`draw_x_axis`], but styles each tick's own mark/label per
/// `scale.tick_weight(tick.value)` under `style` — see
/// [`AxisTickWeightStyle`]'s own doc comment. A `scale` that never
/// reports a weight (every scale except [`crate::scale::TimeScale`])
/// renders byte-identically to [`draw_x_axis`] regardless of `style`.
pub fn draw_x_axis_weighted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, style: &AxisTickWeightStyle) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, None, Some(style));
}

fn draw_x_axis_impl(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    scale: &dyn Scale,
    theme: &FigureTheme,
    target_ticks: usize,
    format: Option<NumberFormat>,
    weight_style: Option<&AxisTickWeightStyle>,
) {
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

        // Only a weighted call site ever changes the tick's own stroke
        // width — the unweighted path leaves it at whatever the baseline
        // above already set (`1.0`), byte-identical to before this fix.
        let (tick_len, major_label_color) = match weight_style {
            Some(style) => {
                let weight = scale.tick_weight(tick.value);
                let (len, stroke_w) = resolve_tick_style(weight, style);
                ctx.set_stroke_color(&theme.axis_color);
                ctx.set_stroke_width(stroke_w);
                let major_color = weight.filter(|w| w.is_major()).and_then(|_| style.major_label_color.as_deref());
                (len, major_color)
            }
            None => {
                ctx.set_stroke_color(&theme.axis_color);
                (TICK_LENGTH, None)
            }
        };
        ctx.begin_path();
        ctx.move_to(x, axis_y);
        ctx.line_to(x, axis_y + tick_len);
        ctx.stroke();

        let label = match format {
            Some(f) => f.format(tick.value, step),
            None => tick.label.clone(),
        };
        let half_w = ctx.measure_text(&label) / 2.0;
        if x - half_w < last_label_right + LABEL_GAP {
            continue; // would collide with the previously drawn label
        }
        ctx.set_fill_color(major_label_color.unwrap_or(&theme.label_color));
        ctx.fill_text(&label, x, axis_y + tick_len + LABEL_GAP);
        last_label_right = x + half_w;
    }
}

/// Draw the left y-axis: baseline, leftward tick marks, and right-aligned
/// tick labels.
pub fn draw_y_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, None, None);
}

/// Same as [`draw_y_axis`], but re-formats every tick's own label through
/// `format` — see [`draw_x_axis_formatted`]'s own docs (identical
/// reasoning, Y side).
pub fn draw_y_axis_formatted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: NumberFormat) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, Some(format), None);
}

/// Same as [`draw_y_axis`], but styles each tick's own mark/label per
/// `scale.tick_weight(tick.value)` under `style` — see
/// [`draw_x_axis_weighted`]'s own docs (identical reasoning, Y side).
pub fn draw_y_axis_weighted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, style: &AxisTickWeightStyle) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, None, Some(style));
}

fn draw_y_axis_impl(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    scale: &dyn Scale,
    theme: &FigureTheme,
    target_ticks: usize,
    format: Option<NumberFormat>,
    weight_style: Option<&AxisTickWeightStyle>,
) {
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

        // Only a weighted call site ever changes the tick's own stroke
        // width — the unweighted path leaves it at whatever the baseline
        // above already set (`1.0`), byte-identical to before this fix.
        let (tick_len, major_label_color) = match weight_style {
            Some(style) => {
                let weight = scale.tick_weight(tick.value);
                let (len, stroke_w) = resolve_tick_style(weight, style);
                ctx.set_stroke_color(&theme.axis_color);
                ctx.set_stroke_width(stroke_w);
                let major_color = weight.filter(|w| w.is_major()).and_then(|_| style.major_label_color.as_deref());
                (len, major_color)
            }
            None => {
                ctx.set_stroke_color(&theme.axis_color);
                (TICK_LENGTH, None)
            }
        };
        ctx.begin_path();
        ctx.move_to(axis_x - tick_len, y);
        ctx.line_to(axis_x, y);
        ctx.stroke();

        let label_bottom = y + row_height / 2.0;
        if let Some(prev_top) = last_label_top {
            if label_bottom + LABEL_GAP > prev_top {
                continue; // would collide with the previously drawn label
            }
        }
        draw_label_right_aligned(ctx, label, axis_x - tick_len - LABEL_GAP, y, major_label_color.unwrap_or(&theme.label_color), &theme.label_font);
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

    // ── TimeScale tick-weight wiring (item A5) ──────────────────────────

    #[test]
    fn resolve_tick_style_distinguishes_every_weight_tier() {
        let style = AxisTickWeightStyle::default();
        let major = resolve_tick_style(Some(TickMarkWeight::Year), &style);
        let medium = resolve_tick_style(Some(TickMarkWeight::Day), &style);
        let minor = resolve_tick_style(Some(TickMarkWeight::Minute1), &style);
        let none = resolve_tick_style(None, &style);

        assert_ne!(major, medium, "a major tick must resolve differently from a medium one");
        assert_ne!(medium, minor, "a medium tick must resolve differently from a minor one");
        assert_eq!(minor, none, "no weight (every non-TimeScale scale) resolves the SAME as an explicit minor tick");
        assert!(major.0 > medium.0 && medium.0 > minor.0, "tick length must strictly increase major > medium > minor");
        assert!(major.1 > medium.1 && medium.1 > minor.1, "stroke width must strictly increase major > medium > minor");
    }

    #[test]
    fn flat_style_matches_minor_for_every_weight_tier() {
        let flat = AxisTickWeightStyle::flat();
        let major = resolve_tick_style(Some(TickMarkWeight::Year), &flat);
        let minor = resolve_tick_style(None, &flat);
        assert_eq!(major, minor, "AxisTickWeightStyle::flat must draw every tick identically regardless of weight");
        assert_eq!(major, (TICK_LENGTH, 1.0), "flat must match the unweighted draw's own fixed tick length/stroke width");
    }

    #[test]
    fn weighted_draw_over_a_non_time_scale_renders_byte_identical_to_the_unweighted_draw() {
        // The safety property the whole wiring depends on: a scale that
        // never reports a tick weight (every scale except TimeScale) must
        // be COMPLETELY unaffected by switching a call site from
        // `draw_x_axis`/`draw_y_axis` to the weighted entry point.
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 1_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };

        let unweighted = render_to_png(&spec, |ctx| draw_x_axis(ctx, &area(), &scale, &theme, 6)).expect("unweighted x-axis render");
        let weighted = render_to_png(&spec, |ctx| draw_x_axis_weighted(ctx, &area(), &scale, &theme, 6, &AxisTickWeightStyle::default()))
            .expect("weighted x-axis render over a non-TimeScale scale");
        assert_eq!(unweighted, weighted, "a non-TimeScale x-axis must render identically through the weighted entry point");

        let unweighted_y = render_to_png(&spec, |ctx| draw_y_axis(ctx, &area(), &scale, &theme, 6)).expect("unweighted y-axis render");
        let weighted_y = render_to_png(&spec, |ctx| draw_y_axis_weighted(ctx, &area(), &scale, &theme, 6, &AxisTickWeightStyle::default()))
            .expect("weighted y-axis render over a non-TimeScale scale");
        assert_eq!(unweighted_y, weighted_y, "a non-TimeScale y-axis must render identically through the weighted entry point");
    }

    #[test]
    fn a_month_spanning_time_axis_resolves_distinguishable_major_and_minor_tick_styles() {
        // Item A5's own explicit gate: "test that a month-spanning time
        // axis renders distinguishable major vs. minor ticks." Builds a
        // real `TimeScale` over ~2 months (the same regime
        // `scale::time`'s own `ticks_over_two_months_pick_day_scale_
        // cadence` fixture uses — day-cadence ticks, some of which land on
        // a real month/day-1 boundary) and proves the RESOLVED per-tick
        // style genuinely differs across the generated tick set, not just
        // in isolated unit fixtures above.
        use crate::scale::TimeScale;

        let jan1_2024 = 1_704_067_200.0; // 2024-01-01T00:00:00Z
        const DAY_SECS: f64 = 86_400.0;
        let scale = TimeScale::new(jan1_2024, jan1_2024 + 62.0 * DAY_SECS);
        let ticks = scale.ticks(6);
        assert!(ticks.len() >= 2, "fixture must produce a real multi-tick set");

        let style = AxisTickWeightStyle::default();
        let resolved: Vec<(f64, f64)> =
            ticks.iter().map(|t| resolve_tick_style(scale.tick_weight(t.value), &style)).collect();
        let mut unique = resolved.clone();
        unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
        unique.dedup();
        assert!(
            unique.len() >= 2,
            "a month-spanning time axis must resolve at least 2 distinct tick styles (major vs. minor), got {resolved:?}"
        );

        // And the actual render must succeed end to end.
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 600, height_px: 250, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| draw_x_axis_weighted(ctx, &area(), &scale, &theme, 6, &style));
        assert!(result.is_ok());
    }
}
