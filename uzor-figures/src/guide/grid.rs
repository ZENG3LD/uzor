//! Grid guides — faint reference lines at tick positions, drawn behind
//! the marks. Style convention (solid, low-contrast, thin) follows
//! `mylittlechart`'s `chart_render/grid.rs::draw_grid` — only the two
//! straight-line passes are ported; the price/time-scale-specific margin
//! extension and data-gap-band drawing there are chart-domain concerns
//! this crate has no business harvesting.

use uzor::render::RenderContext;

use crate::coord::PlotArea;
use crate::scale::Scale;
use crate::theme::FigureTheme;

/// Vertical grid lines at each x-axis tick.
pub fn draw_x_grid(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    ctx.set_stroke_color(&theme.grid_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    for tick in &ticks {
        let x = area.x(scale, tick.value);
        ctx.begin_path();
        ctx.move_to(x, area.rect.y);
        ctx.line_to(x, area.rect.bottom());
        ctx.stroke();
    }
}

/// Horizontal grid lines at each y-axis tick.
pub fn draw_y_grid(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    ctx.set_stroke_color(&theme.grid_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    for tick in &ticks {
        let y = area.y(scale, tick.value);
        ctx.begin_path();
        ctx.move_to(area.rect.x, y);
        ctx.line_to(area.rect.right(), y);
        ctx.stroke();
    }
}
