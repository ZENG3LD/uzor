//! Radio render entry point — dispatches over `RadioRenderKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};

use super::settings::RadioSettings;
use super::types::{DotShape, RadioRenderKind};

/// Render a radio widget, dispatching on `kind`.
pub fn draw_radio(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    settings: &RadioSettings,
    kind:     &RadioRenderKind<'_>,
) {
    match kind {
        RadioRenderKind::Group { x, y, width, view } => {
            draw_radio_group(ctx, *x, *y, *width, view, settings);
        }
        RadioRenderKind::Pair { use_ring_dot, x, cy, between_gap, view } => {
            draw_radio_pair(ctx, *x, *cy, *between_gap, view, settings, *use_ring_dot);
        }
        RadioRenderKind::Dot { shape, cx, cy, view } => {
            draw_radio_dot(ctx, *cx, *cy, view.selected, *shape, settings);
        }
        RadioRenderKind::Custom(f) => {
            f(ctx, rect, state, settings);
        }
    }
}

// =============================================================================
// Group (section 35)
// =============================================================================

fn draw_radio_group(
    ctx:      &mut dyn RenderContext,
    x:        f64,
    y:        f64,
    width:    f64,
    view:     &super::types::RadioGroupView<'_>,
    settings: &RadioSettings,
) {
    use std::f64::consts::TAU;

    let style = settings.group_style.as_ref();
    let theme = settings.theme.as_ref();

    let mut current_y = y;

    for (i, opt) in view.options.iter().enumerate() {
        let is_selected = i == view.selected;

        // Hover background
        if opt.hovered {
            ctx.set_fill_color(theme.radio_row_bg_hover());
            ctx.fill_rounded_rect(x, current_y, width, style.row_height(), style.row_corner_radius());
        }

        // Outer ring
        let circle_cx = x + style.circle_offset_x();
        let circle_cy = current_y + style.circle_offset_y();

        ctx.begin_path();
        ctx.arc(circle_cx, circle_cy, style.outer_radius(), 0.0, TAU);
        ctx.set_stroke_color(if is_selected {
            theme.radio_outer_border_selected()
        } else {
            theme.radio_outer_border()
        });
        ctx.set_stroke_width(style.ring_stroke_width());
        ctx.set_line_dash(&[]);
        ctx.stroke();

        // Inner dot (selected only)
        if is_selected {
            ctx.begin_path();
            ctx.arc(circle_cx, circle_cy, style.inner_radius(), 0.0, TAU);
            ctx.set_fill_color(theme.radio_inner_dot());
            ctx.fill();
        }

        // Label
        ctx.set_fill_color(if is_selected {
            theme.radio_label_text_selected()
        } else {
            theme.radio_label_text()
        });
        ctx.set_font(&format!("{}px sans-serif", style.label_font_size()));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(opt.label, x + style.label_offset_x(), current_y + style.label_offset_y());

        // Description
        if !opt.description.is_empty() {
            ctx.set_fill_color(theme.radio_description_text());
            ctx.set_font(&format!("{}px sans-serif", style.desc_font_size()));
            ctx.fill_text(opt.description, x + style.label_offset_x(), current_y + style.desc_offset_y());
        }

        current_y += style.row_height() + style.gap();
    }
}

// =============================================================================
// Pair (sections 36-37)
// =============================================================================

fn draw_radio_pair(
    ctx:          &mut dyn RenderContext,
    x:            f64,
    cy:           f64,
    between_gap:  f64,
    view:         &super::types::RadioPairView<'_>,
    settings:     &RadioSettings,
    use_ring_dot: bool,
) {
    use std::f64::consts::TAU;

    let style = settings.pair_style.as_ref();
    let theme = settings.theme.as_ref();
    let r        = style.radio_radius();
    let sw       = style.ring_stroke_width();
    let font_str = format!("{}px sans-serif", style.label_font_size());

    // Draw one entry; returns x-coordinate past the label.
    let draw_entry = |ctx: &mut dyn RenderContext, ex: f64, label: &str, is_active: bool| -> f64 {
        let ccx = ex + r;

        if use_ring_dot {
            ctx.begin_path();
            ctx.arc(ccx, cy, r, 0.0, TAU);
            ctx.set_stroke_color(if is_active {
                theme.radio_outer_border_selected()
            } else {
                theme.radio_outer_border()
            });
            ctx.set_stroke_width(sw);
            ctx.set_line_dash(&[]);
            ctx.stroke();

            if is_active {
                ctx.begin_path();
                ctx.arc(ccx, cy, style.inner_dot_radius(), 0.0, TAU);
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.fill();
            }
        } else {
            ctx.begin_path();
            ctx.arc(ccx, cy, r, 0.0, TAU);
            if is_active {
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.fill();
            } else {
                ctx.set_stroke_color(theme.radio_label_text());
                ctx.set_stroke_width(sw);
                ctx.set_line_dash(&[]);
                ctx.stroke();
            }
        }

        let label_x = ccx + r + style.label_gap();
        ctx.set_font(&font_str);
        ctx.set_fill_color(if is_active {
            theme.radio_label_text_selected()
        } else {
            theme.radio_label_text()
        });
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, label_x, cy);

        // Approximate advance
        label_x + (label.len() as f64 * style.label_font_size() * 0.6)
    };

    let left_end = draw_entry(ctx, x, view.left_label, view.selected_left);
    draw_entry(ctx, left_end + between_gap, view.right_label, !view.selected_left);
}

// =============================================================================
// Dot (section 37 — single circle)
// =============================================================================

fn draw_radio_dot(
    ctx:      &mut dyn RenderContext,
    cx:       f64,
    cy:       f64,
    selected: bool,
    shape:    DotShape,
    settings: &RadioSettings,
) {
    use std::f64::consts::TAU;

    let style = settings.pair_style.as_ref();
    let theme = settings.theme.as_ref();
    let r  = style.radio_radius();
    let sw = style.ring_stroke_width();

    match shape {
        DotShape::Circle => {
            // Outer ring
            ctx.begin_path();
            ctx.arc(cx, cy, r, 0.0, TAU);
            ctx.set_stroke_color(if selected {
                theme.radio_outer_border_selected()
            } else {
                theme.radio_outer_border()
            });
            ctx.set_stroke_width(sw);
            ctx.set_line_dash(&[]);
            ctx.stroke();

            // Inner dot (selected only)
            if selected {
                ctx.begin_path();
                ctx.arc(cx, cy, style.inner_dot_radius(), 0.0, TAU);
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.fill();
            }
        }
        DotShape::Square => {
            let half = r;
            let color = if selected {
                theme.radio_outer_border_selected()
            } else {
                theme.radio_outer_border()
            };
            ctx.set_stroke_color(color);
            ctx.set_stroke_width(sw);
            ctx.set_line_dash(&[]);
            ctx.stroke_rect(cx - half, cy - half, half * 2.0, half * 2.0);
            if selected {
                let ir = style.inner_dot_radius();
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.fill_rect(cx - ir, cy - ir, ir * 2.0, ir * 2.0);
            }
        }
        DotShape::Pill => {
            let half_w = r * 1.5;
            let half_h = r * 0.6;
            let pill_r  = half_h;
            let color = if selected {
                theme.radio_outer_border_selected()
            } else {
                theme.radio_outer_border()
            };
            ctx.set_stroke_color(color);
            ctx.set_stroke_width(sw);
            ctx.set_line_dash(&[]);
            ctx.stroke_rounded_rect(cx - half_w, cy - half_h, half_w * 2.0, half_h * 2.0, pill_r);
            if selected {
                ctx.set_fill_color(theme.radio_inner_dot());
                let ir = style.inner_dot_radius() * 0.8;
                ctx.fill_rounded_rect(cx - ir, cy - ir * 0.5, ir * 2.0, ir, ir * 0.5);
            }
        }
        DotShape::Star => {
            // Approximate star with a thick circle + cross strokes
            ctx.begin_path();
            ctx.arc(cx, cy, r, 0.0, TAU);
            ctx.set_stroke_color(if selected {
                theme.radio_outer_border_selected()
            } else {
                theme.radio_outer_border()
            });
            ctx.set_stroke_width(sw);
            ctx.set_line_dash(&[]);
            ctx.stroke();
            if selected {
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.begin_path();
                ctx.arc(cx, cy, style.inner_dot_radius(), 0.0, TAU);
                ctx.fill();
            }
        }
    }
}
