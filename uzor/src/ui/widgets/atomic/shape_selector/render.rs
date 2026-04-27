//! ShapeSelector render entry point — dispatches over `ShapeSelectorRenderKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};

use super::settings::ShapeSelectorSettings;
use super::style::SelectorButtonStyle;
use super::theme::ShapeSelectorTheme;
use super::types::{ShapeSelectorRenderKind, ShapeSelectorView, ThemePresetView, UIStyleView};

/// Render a shape selector widget, dispatching on `kind`.
///
/// For `Shape` kind: use `draw_shape_selector_button` directly — it requires
/// the `draw_shape` closure which cannot be stored in `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect.
/// - `state`    — interaction state from the coordinator.
/// - `settings` — visual configuration.
/// - `kind`     — which render variant to use.
pub fn draw_shape_selector(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    settings: &ShapeSelectorSettings,
    kind:     &ShapeSelectorRenderKind<'_>,
) {
    match kind {
        ShapeSelectorRenderKind::Shape
        | ShapeSelectorRenderKind::ThemePreset
        | ShapeSelectorRenderKind::UIStyle => {
            // Callers should use the dedicated draw_* functions which carry view data.
            let _ = (ctx, rect, state, settings);
        }
        ShapeSelectorRenderKind::Custom(f) => {
            f(ctx, rect, state, settings);
        }
    }
}

/// Render a shape selector button (section 34 — signals tab shape row).
///
/// # Visual
/// - Background: active bg when selected, hover bg on hover, idle bg otherwise.
/// - Border: selected/hover/idle selector border.
/// - Shape content: drawn by `draw_shape` closure into an inset rect (3 px each side).
/// - Label: drawn below the button in `selector_label_text` at `label_font_size`.
///
/// # Arguments
/// - `rect`       — button bounds (typically `style.width() × style.height()` square).
/// - `draw_shape` — closure: `(ctx, inner_rect, color)`.
pub fn draw_shape_selector_button<F>(
    ctx:        &mut dyn RenderContext,
    rect:       Rect,
    view:       &ShapeSelectorView<'_>,
    font:       &str,
    style:      &dyn SelectorButtonStyle,
    theme:      &dyn ShapeSelectorTheme,
    draw_shape: F,
)
where
    F: FnOnce(&mut dyn RenderContext, Rect, &str),
{
    let r = style.radius();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.selected {
        theme.selector_active_bg()
    } else if view.hovered {
        theme.selector_hover_bg()
    } else {
        theme.selector_idle_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Border ─────────────────────────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        (theme.selector_selected_border(), style.selected_border_width())
    } else if view.hovered {
        (theme.selector_hover_border(), style.border_width())
    } else {
        (theme.selector_idle_border(), style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Shape content (inset by 3 px) ──────────────────────────────────────────
    let inset = 3.0_f64;
    let inner = Rect::new(
        rect.x + inset,
        rect.y + inset,
        (rect.width  - inset * 2.0).max(0.0),
        (rect.height - inset * 2.0).max(0.0),
    );
    let shape_color = if view.selected {
        theme.selector_active_text()
    } else {
        theme.selector_idle_text()
    };
    draw_shape(ctx, inner, shape_color);

    // ── Label ─────────────────────────────────────────────────────────────────
    if let Some(label) = view.label {
        ctx.set_font(font);
        ctx.set_fill_color(theme.selector_label_text());
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(
            label,
            rect.center_x(),
            rect.y + rect.height + style.label_gap(),
        );
    }
}

/// Render a theme preset selector button (section 39 — appearance tab).
///
/// # Visual
/// - Background: active/hover/idle selector bg.
/// - Border: selected/hover/idle outline.
/// - 18×18 preview color swatch at left edge.
/// - Theme name text to the right of the swatch.
pub fn draw_theme_preset_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ThemePresetView<'_>,
    font:  &str,
    style: &dyn SelectorButtonStyle,
    theme: &dyn ShapeSelectorTheme,
) {
    let r = style.radius();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.selected {
        theme.selector_active_bg()
    } else if view.hovered {
        theme.selector_hover_bg()
    } else {
        theme.selector_idle_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Selection / hover border ───────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        (theme.selector_selected_border(), style.selected_border_width())
    } else if view.hovered {
        (theme.selector_hover_border(), style.border_width())
    } else {
        (theme.selector_idle_border(), style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Preview color swatch (18×18) ───────────────────────────────────────────
    const SWATCH_SIZE: f64 = 18.0;
    let swatch_x = rect.x + 6.0;
    let swatch_y = rect.center_y() - SWATCH_SIZE / 2.0;

    ctx.set_fill_color(view.preview_color);
    ctx.fill_rect(swatch_x, swatch_y, SWATCH_SIZE, SWATCH_SIZE);

    ctx.set_stroke_color(view.swatch_border_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.stroke_rect(swatch_x, swatch_y, SWATCH_SIZE, SWATCH_SIZE);

    // ── Theme name text ────────────────────────────────────────────────────────
    let text_color = if view.selected {
        theme.selector_active_text()
    } else {
        theme.selector_label_text()
    };
    ctx.set_font(font);
    ctx.set_fill_color(text_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.label, rect.x + 30.0, rect.center_y());
}

/// Render a UI style selector button (section 40 — appearance tab).
///
/// # Visual
/// - Background: active/hover/idle selector bg.
/// - Border: selected/hover/idle outline.
/// - Label text centered inside the button rect.
pub fn draw_ui_style_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &UIStyleView<'_>,
    font:  &str,
    style: &dyn SelectorButtonStyle,
    theme: &dyn ShapeSelectorTheme,
) {
    let r = style.radius();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.selected {
        theme.selector_active_bg()
    } else if view.hovered {
        theme.selector_hover_bg()
    } else {
        theme.selector_idle_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Border ─────────────────────────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        (theme.selector_selected_border(), style.selected_border_width())
    } else if view.hovered {
        (theme.selector_hover_border(), style.border_width())
    } else {
        (theme.selector_idle_border(), style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Label text ─────────────────────────────────────────────────────────────
    let text_color = if view.selected {
        theme.selector_active_text()
    } else {
        theme.selector_label_text()
    };
    ctx.set_font(font);
    ctx.set_fill_color(text_color);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.label, rect.center_x(), rect.center_y());
}
