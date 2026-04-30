//! Checkbox render entry point — dispatches over `CheckboxRenderKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};

use super::settings::CheckboxSettings;
use super::types::{CheckboxRenderKind, CheckboxView};

/// Render a checkbox widget, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (box origin; `width == height == style.size()` by convention).
/// - `state`    — interaction state from the coordinator.
/// - `view`     — per-frame data (checked, label).
/// - `settings` — visual configuration.
/// - `kind`     — which render variant to use.
/// - `font`     — font string for the optional label.
pub fn draw_checkbox(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    view:     &CheckboxView<'_>,
    settings: &CheckboxSettings,
    kind:     &CheckboxRenderKind<'_>,
    font:     &str,
) {
    match kind {
        CheckboxRenderKind::Standard => draw_standard(ctx, rect, view, settings, font),
        CheckboxRenderKind::Visibility => draw_standard(ctx, rect, view, settings, font),
        CheckboxRenderKind::LevelVisibility => draw_standard(ctx, rect, view, settings, font),
        CheckboxRenderKind::Cross => draw_cross(ctx, rect, view, settings, font),
        CheckboxRenderKind::CircleCheck => draw_circle_check(ctx, rect, view, settings, font),
        CheckboxRenderKind::Notification => draw_notification(ctx, rect, view, settings, font),
        CheckboxRenderKind::Custom(f) => f(ctx, rect, state, view, settings),
    }
}

// =============================================================================
// Standard / Visibility / LevelVisibility (sections 21-23)
// =============================================================================

fn draw_standard(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &CheckboxView<'_>,
    settings: &CheckboxSettings,
    font:     &str,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();
    let r = style.radius();

    // Fixed-size box (16×16) at left of rect, vertically centred.
    let box_size = style.size();
    let box_x = rect.x;
    let box_y = rect.y + (rect.height - box_size) / 2.0;

    // Background fill
    let bg = if view.checked {
        theme.checkbox_bg_checked()
    } else {
        theme.checkbox_bg_unchecked()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(box_x, box_y, box_size, box_size, r);

    // Border stroke
    ctx.set_stroke_color(theme.checkbox_border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(box_x, box_y, box_size, box_size, r);

    // Checkmark path
    if view.checked {
        let inset = style.checkmark_inset();
        ctx.set_stroke_color(theme.checkbox_checkmark());
        ctx.set_stroke_width(style.checkmark_width());
        ctx.set_line_dash(&[]);
        ctx.begin_path();
        ctx.move_to(box_x + 3.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + 6.0, box_y + box_size - inset);
        ctx.line_to(box_x + box_size - 3.0, box_y + inset);
        ctx.stroke();
    }

    // Label to the right of the box
    let box_rect = Rect::new(box_x, box_y, box_size, box_size);
    draw_label(ctx, box_rect, view, settings, font);
}

// =============================================================================
// Notification (section 24)
// =============================================================================

fn draw_notification(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &CheckboxView<'_>,
    settings: &CheckboxSettings,
    font:     &str,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();
    let r = style.radius();

    let box_size = style.size();
    let box_x = rect.x;
    let box_y = rect.y + (rect.height - box_size) / 2.0;

    // Outer square — stroke only
    ctx.set_stroke_color(theme.checkbox_border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(box_x, box_y, box_size, box_size, r);

    // Inner filled rect when enabled
    if view.checked {
        let inset = 3.0_f64;
        ctx.set_fill_color(theme.checkbox_notification_inner());
        ctx.fill_rounded_rect(
            box_x + inset,
            box_y + inset,
            box_size - inset * 2.0,
            box_size - inset * 2.0,
            1.0,
        );
    }

    let box_rect = Rect::new(box_x, box_y, box_size, box_size);
    draw_label(ctx, box_rect, view, settings, font);
}

// =============================================================================
// Cross variant (reserve)
// =============================================================================

fn draw_cross(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &CheckboxView<'_>,
    settings: &CheckboxSettings,
    font:     &str,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();
    let r = style.radius();

    let box_size = style.size();
    let box_x = rect.x;
    let box_y = rect.y + (rect.height - box_size) / 2.0;

    let bg = if view.checked {
        theme.checkbox_bg_checked()
    } else {
        theme.checkbox_bg_unchecked()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(box_x, box_y, box_size, box_size, r);

    ctx.set_stroke_color(theme.checkbox_border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(box_x, box_y, box_size, box_size, r);

    if view.checked {
        let inset = 4.0_f64;
        let x1 = box_x + inset;
        let y1 = box_y + inset;
        let x2 = box_x + box_size - inset;
        let y2 = box_y + box_size - inset;
        ctx.set_stroke_color(theme.checkbox_checkmark());
        ctx.set_stroke_width(style.checkmark_width());
        ctx.set_line_dash(&[]);
        ctx.begin_path();
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        ctx.stroke();
        ctx.begin_path();
        ctx.move_to(x2, y1);
        ctx.line_to(x1, y2);
        ctx.stroke();
    }

    let box_rect = Rect::new(box_x, box_y, box_size, box_size);
    draw_label(ctx, box_rect, view, settings, font);
}

// =============================================================================
// CircleCheck variant (reserve)
// =============================================================================

fn draw_circle_check(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &CheckboxView<'_>,
    settings: &CheckboxSettings,
    font:     &str,
) {
    use std::f64::consts::TAU;

    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let box_size = style.size();
    let box_x = rect.x;
    let box_y = rect.y + (rect.height - box_size) / 2.0;
    let box_rect_inner = Rect::new(box_x, box_y, box_size, box_size);
    let r = box_size / 2.0;
    let cx = box_rect_inner.center_x();
    let cy = box_rect_inner.center_y();

    let bg = if view.checked {
        theme.checkbox_bg_checked()
    } else {
        theme.checkbox_bg_unchecked()
    };

    ctx.begin_path();
    ctx.arc(cx, cy, r, 0.0, TAU);
    ctx.set_fill_color(bg);
    ctx.fill();

    ctx.begin_path();
    ctx.arc(cx, cy, r, 0.0, TAU);
    ctx.set_stroke_color(theme.checkbox_border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke();

    if view.checked {
        let inner_r = r * 0.5;
        ctx.begin_path();
        ctx.arc(cx, cy, inner_r, 0.0, TAU);
        ctx.set_fill_color(theme.checkbox_checkmark());
        ctx.fill();
    }

    draw_label(ctx, box_rect_inner, view, settings, font);
}

// =============================================================================
// Shared label helper
// =============================================================================

fn draw_label(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &CheckboxView<'_>,
    settings: &CheckboxSettings,
    font:     &str,
) {
    if let Some(label) = view.label {
        let style = settings.style.as_ref();
        let theme = settings.theme.as_ref();
        ctx.set_font(font);
        ctx.set_fill_color(theme.checkbox_label_text());
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            label,
            rect.x + rect.width + style.label_gap(),
            rect.y + rect.height / 2.0,
        );
    }
}
