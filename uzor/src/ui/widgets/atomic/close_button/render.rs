//! Close button rendering — X glyph with optional hover background.
//!
//! Ported from `button/render.rs` section 41.

use crate::render::RenderContext;
use crate::types::{Rect, WidgetState};

use super::settings::CloseButtonSettings;
use super::style::{DefaultCloseButtonStyle, LargeCloseButtonStyle};
use super::types::CloseButtonRenderKind;

/// Per-instance data for `draw_close_button`.
pub struct CloseButtonView {
    /// Whether the pointer is over the button.
    pub hovered: bool,
}

/// Interaction result returned by `draw_close_button`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CloseButtonResult {
    pub clicked: bool,
    pub hovered: bool,
}

/// Render a close button, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (caller sizes to `style.size() × style.size()`).
/// - `state`    — interaction state from the coordinator.
/// - `view`     — per-frame state (hovered flag).
/// - `settings` — visual configuration (theme + style override).
/// - `kind`     — which preset to use or `Custom` escape hatch.
pub fn draw_close_button(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    view:     &CloseButtonView,
    settings: &CloseButtonSettings,
    kind:     &CloseButtonRenderKind,
) -> CloseButtonResult {
    match kind {
        CloseButtonRenderKind::Default => {
            draw_close_button_inner(ctx, rect, view, settings)
        }
        CloseButtonRenderKind::Large => {
            // Large uses LargeCloseButtonStyle geometry; theme from settings.
            let large_style = LargeCloseButtonStyle;
            draw_close_button_with_style(ctx, rect, view, &large_style, settings.theme.as_ref())
        }
        CloseButtonRenderKind::Custom(f) => {
            f(ctx, rect, state, view, settings);
            CloseButtonResult {
                clicked: false,
                hovered: view.hovered,
            }
        }
    }
}

fn draw_close_button_inner(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &CloseButtonView,
    settings: &CloseButtonSettings,
) -> CloseButtonResult {
    draw_close_button_with_style(ctx, rect, view, settings.style.as_ref(), settings.theme.as_ref())
}

fn draw_close_button_with_style(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &CloseButtonView,
    style: &dyn super::style::CloseButtonStyle,
    theme: &dyn super::theme::CloseButtonTheme,
) -> CloseButtonResult {
    if view.hovered {
        ctx.set_fill_color(theme.close_button_bg_hover());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.hover_bg_radius());
    }

    let color = if view.hovered {
        theme.close_button_x_color_hover()
    } else {
        theme.close_button_x_color()
    };

    let inset = style.x_inset();
    let x1 = rect.x + inset;
    let y1 = rect.y + inset;
    let x2 = rect.x + rect.width  - inset;
    let y2 = rect.y + rect.height - inset;

    ctx.set_stroke_color(color);
    ctx.set_stroke_width(style.x_stroke_width());
    ctx.set_line_dash(&[]);

    ctx.begin_path();
    ctx.move_to(x1, y1);
    ctx.line_to(x2, y2);
    ctx.stroke();

    ctx.begin_path();
    ctx.move_to(x2, y1);
    ctx.line_to(x1, y2);
    ctx.stroke();

    CloseButtonResult {
        clicked: false,
        hovered: view.hovered,
    }
}

/// Build default `CloseButtonSettings` using the `Default` preset geometry.
pub fn default_settings() -> CloseButtonSettings {
    CloseButtonSettings {
        theme: Box::new(super::theme::DefaultCloseButtonTheme),
        style: Box::new(DefaultCloseButtonStyle),
    }
}
