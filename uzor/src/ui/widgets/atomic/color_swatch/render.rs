//! ColorSwatch render entry point — dispatches over `ColorSwatchRenderKind`.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::ColorSwatchSettings;
use super::style::{ColorSwatchStyle, FillToggleStyle};
use super::theme::ColorSwatchTheme;
use super::types::{ColorSwatchRenderKind, ColorSwatchView, FillToggleView};

/// Convert a `[r, g, b, a]` byte tuple to a CSS `rgba(…)` string.
#[inline]
fn rgba_css(c: [u8; 4]) -> String {
    let alpha = c[3] as f64 / 255.0;
    format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], alpha)
}

/// Render a color swatch widget, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (origin of the widget).
/// - `state`    — interaction state from the coordinator.
/// - `view`     — per-frame data (color, hovered, selected, transparency).
/// - `settings` — visual configuration.
/// - `kind`     — which render variant to use.
pub fn draw_color_swatch(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    view:     &ColorSwatchView<'_>,
    settings: &ColorSwatchSettings,
    kind:     &ColorSwatchRenderKind<'_>,
) {
    match kind {
        ColorSwatchRenderKind::Simple => {
            draw_swatch_inner(ctx, rect, view, settings.style.as_ref(), settings.theme.as_ref());
        }
        ColorSwatchRenderKind::WithTransparency => {
            // Force show_transparency on regardless of view field
            let view_with_checker = ColorSwatchView {
                show_transparency: true,
                ..*view
            };
            draw_swatch_inner(ctx, rect, &view_with_checker, settings.style.as_ref(), settings.theme.as_ref());
        }
        ColorSwatchRenderKind::Indicator => {
            draw_swatch_inner(ctx, rect, view, settings.style.as_ref(), settings.theme.as_ref());
        }
        ColorSwatchRenderKind::Primitive => {
            draw_swatch_inner(ctx, rect, view, settings.style.as_ref(), settings.theme.as_ref());
        }
        ColorSwatchRenderKind::FillToggle => {
            // FillToggle uses FillToggleView data but ColorSwatchView carries color + disabled
            let fill_view = FillToggleView {
                filled: view.selected,
                color:  view.color,
                disabled: false, // caller can use Custom if disabled is needed
            };
            draw_fill_toggle_inner(ctx, rect, &fill_view, settings.fill_toggle_style.as_ref(), settings.theme.as_ref());
        }
        ColorSwatchRenderKind::Custom(f) => {
            f(ctx, rect, state, view, settings);
        }
    }
}

/// Render a fill-toggle button directly (bypass dispatcher).
///
/// # Arguments
/// - `ctx`   — render context.
/// - `rect`  — pixel rect for the toggle square.
/// - `view`  — per-frame data (filled, color, disabled).
/// - `style` — geometry (radius, border width).
/// - `theme` — color slots.
pub fn draw_fill_toggle(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &FillToggleView,
    style: &dyn FillToggleStyle,
    theme: &dyn ColorSwatchTheme,
) {
    draw_fill_toggle_inner(ctx, rect, view, style, theme);
}

// =============================================================================
// Internal helpers
// =============================================================================

fn draw_swatch_inner(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ColorSwatchView<'_>,
    style: &dyn ColorSwatchStyle,
    theme: &dyn ColorSwatchTheme,
) {
    let r = style.radius();

    // ── 1. Hover expand rect ───────────────────────────────────────────────────
    let expand = style.hover_expand();
    if expand > 0.0 && (view.hovered || view.selected) {
        ctx.set_fill_color(theme.color_swatch_hover_outline());
        ctx.fill_rect(
            rect.x      - expand,
            rect.y      - expand,
            rect.width  + expand * 2.0,
            rect.height + expand * 2.0,
        );
    }

    // ── 2. Checkerboard background ─────────────────────────────────────────────
    if view.show_transparency {
        let tile = style.checker_tile_size();
        ctx.set_fill_color(theme.transparency_checker_a());
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        ctx.set_fill_color(theme.transparency_checker_b());
        ctx.fill_rect(rect.x,        rect.y,        tile, tile);
        ctx.fill_rect(rect.x + tile, rect.y + tile, tile, tile);
    }

    // ── 3. Color fill ──────────────────────────────────────────────────────────
    let color_css = rgba_css(view.color);
    ctx.set_fill_color(&color_css);
    if r > 0.0 {
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    } else {
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
    }

    // ── 4. Border stroke ───────────────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        let c = view.border_color_override
            .unwrap_or_else(|| theme.color_swatch_selected_border());
        (c, style.selected_border_width())
    } else {
        let c = view.border_color_override
            .unwrap_or_else(|| theme.color_swatch_border());
        (c, style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    if r > 0.0 {
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    } else {
        ctx.stroke_rect(rect.x, rect.y, rect.width, rect.height);
    }
}

fn draw_fill_toggle_inner(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &FillToggleView,
    style: &dyn FillToggleStyle,
    theme: &dyn ColorSwatchTheme,
) {
    let r = style.radius();

    // ── Base background ────────────────────────────────────────────────────────
    ctx.set_fill_color(theme.fill_toggle_background());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Fill when enabled ──────────────────────────────────────────────────────
    if view.filled {
        let color_css = rgba_css(view.color);
        ctx.set_fill_color(&color_css);
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    }

    // ── Border ─────────────────────────────────────────────────────────────────
    let border_color = if view.filled {
        theme.fill_toggle_active_border()
    } else {
        theme.color_swatch_border()
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Diagonal strikethrough when fill disabled ──────────────────────────────
    if !view.filled {
        ctx.set_stroke_color(theme.fill_toggle_off_pattern_color());
        ctx.set_stroke_width(1.0);
        ctx.set_line_dash(&[]);
        ctx.begin_path();
        ctx.move_to(rect.x + 2.0,              rect.y + rect.height - 2.0);
        ctx.line_to(rect.x + rect.width - 2.0, rect.y + 2.0);
        ctx.stroke();
    }

    // ── Disabled overlay ───────────────────────────────────────────────────────
    if view.disabled {
        // Inline constant: semi-transparent black overlay for disabled state.
        // Value mirrors toggle_disabled_overlay from mlc: "rgba(0,0,0,0.35)".
        ctx.set_fill_color("rgba(0,0,0,0.35)");
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    }
}
