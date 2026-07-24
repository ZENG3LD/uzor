//! Grid guides — faint reference lines at tick positions, drawn behind
//! the marks. Style convention (solid, low-contrast, thin) follows
//! `mylittlechart`'s `chart_render/grid.rs::draw_grid` — only the two
//! straight-line passes are ported; the price/time-scale-specific margin
//! extension and data-gap-band drawing there are chart-domain concerns
//! this crate has no business harvesting.

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::scale::time::TickMarkWeight;
use crate::scale::Scale;
use crate::theme::FigureTheme;

/// Visual policy for [`draw_x_grid_weighted`]/[`draw_y_grid_weighted`] —
/// same "only affects a scale whose [`crate::scale::Scale::tick_weight`]
/// returns `Some`" contract as [`crate::guide::axis::AxisTickWeightStyle`]
/// ([`crate::scale::TimeScale`] today; every other scale is unaffected
/// regardless of this style). A configurable field set — every
/// behavioral knob is a real option: [`GridTickWeightStyle::default`]
/// draws a visibly heavier line at a major/medium calendar boundary (the
/// SAME "major boundary reads stronger" convention
/// [`crate::figure::TimelineFigure`]'s own bespoke `draw_time_grid`
/// already established locally — this is the shared, reusable
/// counterpart for `CurveFigure`/`ScatterFigure`'s own generic X grid);
/// [`GridTickWeightStyle::flat`] reproduces the unweighted
/// [`draw_x_grid`]/[`draw_y_grid`]'s own single stroke width for every
/// tick regardless of weight.
#[derive(Debug, Clone, PartialEq)]
pub struct GridTickWeightStyle {
    /// Stroke width (px) for a major tick's own gridline
    /// ([`TickMarkWeight::is_major`] — Year/Month boundaries).
    pub major_stroke_width: f64,
    /// Stroke width (px) for a medium tick's own gridline
    /// ([`TickMarkWeight::is_medium`] — Day boundaries).
    pub medium_stroke_width: f64,
    /// Stroke width (px) for every other (minor) tick's own gridline —
    /// matches the unweighted draw's own fixed `1.0`.
    pub minor_stroke_width: f64,
}

impl Default for GridTickWeightStyle {
    fn default() -> Self {
        Self { major_stroke_width: 1.5, medium_stroke_width: 1.2, minor_stroke_width: 1.0 }
    }
}

impl GridTickWeightStyle {
    /// Every gridline draws identically regardless of its own weight —
    /// the SAME output [`draw_x_grid`]/[`draw_y_grid`] (the unweighted
    /// draw) already produce.
    pub fn flat() -> Self {
        Self { major_stroke_width: 1.0, medium_stroke_width: 1.0, minor_stroke_width: 1.0 }
    }
}

fn resolve_stroke_width(weight: Option<TickMarkWeight>, style: &GridTickWeightStyle) -> f64 {
    match weight {
        Some(w) if w.is_major() => style.major_stroke_width,
        Some(w) if w.is_medium() => style.medium_stroke_width,
        _ => style.minor_stroke_width,
    }
}

/// Vertical grid lines at each x-axis tick.
pub fn draw_x_grid(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_x_grid_impl(ctx, area, scale, theme, target_ticks, None);
}

/// Same as [`draw_x_grid`], but styles each gridline's own stroke width
/// per `scale.tick_weight(tick.value)` under `style` — see
/// [`GridTickWeightStyle`]'s own doc comment. A `scale` that never
/// reports a weight (every scale except [`crate::scale::TimeScale`])
/// renders byte-identically to [`draw_x_grid`] regardless of `style`.
pub fn draw_x_grid_weighted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, style: &GridTickWeightStyle) {
    draw_x_grid_impl(ctx, area, scale, theme, target_ticks, Some(style));
}

fn draw_x_grid_impl(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, weight_style: Option<&GridTickWeightStyle>) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    ctx.set_stroke_color(&theme.grid_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    for tick in &ticks {
        let x = area.x(scale, tick.value);
        if let Some(style) = weight_style {
            ctx.set_stroke_width(resolve_stroke_width(scale.tick_weight(tick.value), style));
        }
        ctx.begin_path();
        ctx.move_to(x, area.rect.y);
        ctx.line_to(x, area.rect.bottom());
        ctx.stroke();
    }
}

/// Horizontal grid lines at each y-axis tick.
pub fn draw_y_grid(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_y_grid_impl(ctx, area, scale, theme, target_ticks, None);
}

/// Same as [`draw_y_grid`], but styles each gridline's own stroke width
/// per `scale.tick_weight(tick.value)` under `style` — see
/// [`draw_x_grid_weighted`]'s own docs (identical reasoning, Y side).
pub fn draw_y_grid_weighted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, style: &GridTickWeightStyle) {
    draw_y_grid_impl(ctx, area, scale, theme, target_ticks, Some(style));
}

fn draw_y_grid_impl(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, weight_style: Option<&GridTickWeightStyle>) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    ctx.set_stroke_color(&theme.grid_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    for tick in &ticks {
        let y = area.y(scale, tick.value);
        if let Some(style) = weight_style {
            ctx.set_stroke_width(resolve_stroke_width(scale.tick_weight(tick.value), style));
        }
        ctx.begin_path();
        ctx.move_to(area.rect.x, y);
        ctx.line_to(area.rect.right(), y);
        ctx.stroke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::{LinearScale, TimeScale};
    use crate::theme::FigureTheme;
    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(40.0, 10.0, 300.0, 150.0))
    }

    #[test]
    fn resolve_stroke_width_distinguishes_every_weight_tier() {
        let style = GridTickWeightStyle::default();
        let major = resolve_stroke_width(Some(TickMarkWeight::Year), &style);
        let medium = resolve_stroke_width(Some(TickMarkWeight::Day), &style);
        let minor = resolve_stroke_width(Some(TickMarkWeight::Minute1), &style);
        let none = resolve_stroke_width(None, &style);
        assert!(major > medium && medium > minor);
        assert_eq!(minor, none, "no weight (every non-TimeScale scale) resolves the SAME as an explicit minor tick");
    }

    #[test]
    fn flat_style_matches_minor_for_every_weight_tier() {
        let flat = GridTickWeightStyle::flat();
        assert_eq!(resolve_stroke_width(Some(TickMarkWeight::Year), &flat), resolve_stroke_width(None, &flat));
        assert_eq!(resolve_stroke_width(None, &flat), 1.0);
    }

    #[test]
    fn weighted_grid_over_a_non_time_scale_renders_byte_identical_to_the_unweighted_grid() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 1_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };

        let unweighted = render_to_png(&spec, |ctx| draw_x_grid(ctx, &area(), &scale, &theme, 6)).expect("unweighted x-grid render");
        let weighted = render_to_png(&spec, |ctx| draw_x_grid_weighted(ctx, &area(), &scale, &theme, 6, &GridTickWeightStyle::default()))
            .expect("weighted x-grid render over a non-TimeScale scale");
        assert_eq!(unweighted, weighted, "a non-TimeScale x-grid must render identically through the weighted entry point");
    }

    #[test]
    fn a_month_spanning_time_axis_resolves_distinguishable_major_and_minor_gridline_widths() {
        let jan1_2024 = 1_704_067_200.0; // 2024-01-01T00:00:00Z
        const DAY_SECS: f64 = 86_400.0;
        let scale = TimeScale::new(jan1_2024, jan1_2024 + 62.0 * DAY_SECS);
        let ticks = scale.ticks(6);
        assert!(ticks.len() >= 2, "fixture must produce a real multi-tick set");

        let style = GridTickWeightStyle::default();
        let mut widths: Vec<f64> = ticks.iter().map(|t| resolve_stroke_width(scale.tick_weight(t.value), &style)).collect();
        widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        widths.dedup();
        assert!(widths.len() >= 2, "a month-spanning time axis must resolve at least 2 distinct gridline widths");

        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 600, height_px: 250, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| draw_x_grid_weighted(ctx, &area(), &scale, &theme, 6, &style));
        assert!(result.is_ok());
    }
}
