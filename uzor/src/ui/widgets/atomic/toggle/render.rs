//! Toggle render entry point — dispatches over `ToggleRenderKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{IconId, Rect, WidgetState};

use super::settings::ToggleSettings;
use super::types::{ToggleRenderKind, ToggleView};

/// Render a toggle widget, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (origin of the widget).
/// - `state`    — interaction state from the coordinator.
/// - `view`     — per-frame data (toggled, label, disabled).
/// - `settings` — visual configuration.
/// - `kind`     — which render variant to use.
pub fn draw_toggle<F>(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    view:     &ToggleView<'_>,
    settings: &ToggleSettings,
    kind:     &ToggleRenderKind<'_>,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match kind {
        ToggleRenderKind::Switch => {
            draw_toggle_switch_inner(ctx, rect, view, settings, false, draw_icon);
        }
        ToggleRenderKind::SwitchWide => {
            draw_toggle_switch_inner(ctx, rect, view, settings, true, draw_icon);
        }
        ToggleRenderKind::IconSwap { icon_off, icon_on } => {
            draw_icon_swap(ctx, rect, state, view, settings, icon_off, icon_on, draw_icon);
        }
        ToggleRenderKind::Custom(f) => {
            f(ctx, rect, state, view, settings);
            let _ = draw_icon; // consumed by caller's closure if needed
        }
    }
}

// =============================================================================
// Switch variant (sections 25-26)
// =============================================================================

fn draw_toggle_switch_inner<F>(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ToggleView<'_>,
    settings: &ToggleSettings,
    wide:     bool,
    _draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let style = if wide {
        // SwitchWide uses SignalsToggleStyle dims; but we access via switch_style
        // which defaults to IndicatorToggleStyle. Callers that want SwitchWide
        // should pass SignalsToggleStyle in settings.switch_style.
        settings.switch_style.as_ref()
    } else {
        settings.switch_style.as_ref()
    };
    let theme = settings.theme.as_ref();

    let tw = style.track_width();
    let th = style.track_height();
    let tr = th / 2.0;
    let kr = style.thumb_radius();
    let kp = style.thumb_padding();

    // Track
    let track_color = if view.toggled {
        theme.toggle_track_on()
    } else {
        theme.toggle_track_off()
    };
    ctx.set_fill_color(track_color);
    ctx.fill_rounded_rect(rect.x, rect.y, tw, th, tr);

    // Thumb
    let thumb_color = if view.toggled {
        theme.toggle_thumb_on()
    } else {
        theme.toggle_thumb_off()
    };
    let thumb_center_x = if view.toggled {
        rect.x + tw - kr - kp
    } else {
        rect.x + kr + kp
    };
    let thumb_center_y = rect.y + th / 2.0;

    ctx.set_fill_color(thumb_color);
    ctx.fill_rounded_rect(
        thumb_center_x - kr,
        thumb_center_y - kr,
        kr * 2.0,
        kr * 2.0,
        kr,
    );

    // Disabled overlay
    if view.disabled {
        ctx.set_fill_color(theme.toggle_disabled_overlay());
        ctx.fill_rounded_rect(rect.x, rect.y, tw, th, tr);
    }

    // Label
    if let Some(label) = view.label {
        let label_color = if view.disabled {
            theme.toggle_label_text_disabled()
        } else {
            theme.toggle_label_text()
        };
        let font = "13px sans-serif";
        ctx.set_font(font);
        ctx.set_fill_color(label_color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, rect.x + tw + style.label_gap(), rect.y + th / 2.0);
    }
}

// =============================================================================
// IconSwap variant (Eye/EyeOff, Lock/Unlock)
// =============================================================================

fn draw_icon_swap<F>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    state:     WidgetState,
    view:      &ToggleView<'_>,
    settings:  &ToggleSettings,
    icon_off:  &IconId,
    icon_on:   &IconId,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let theme = settings.theme.as_ref();
    let icon_size = settings.icon_style.icon_size();

    let color = if view.disabled {
        theme.toggle_label_text_disabled()
    } else if view.toggled || matches!(state, WidgetState::Active | WidgetState::Toggled) {
        theme.toggle_icon_active()
    } else {
        theme.toggle_icon_normal()
    };

    let icon = if view.toggled { icon_on } else { icon_off };

    let icon_rect = Rect::new(
        rect.center_x() - icon_size / 2.0,
        rect.center_y() - icon_size / 2.0,
        icon_size,
        icon_size,
    );
    draw_icon(ctx, icon, icon_rect, color);
}
