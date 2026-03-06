//! Popup factory rendering for desktop

use super::{RenderContext, rgba_to_hex};
use uzor_core::types::{Rect, WidgetState};
use uzor_core::widgets::popup::types::PopupType;
use uzor_core::widgets::popup::theme::PopupTheme;

/// Render popup with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    popup: &PopupType,
    rect: Rect,
    _state: WidgetState,
    theme: &dyn PopupTheme,
) {
    match popup {
        PopupType::ContextMenu { .. } => {
            // Draw shadow
            let shadow_offset = theme.shadow_offset();
            let shadow_color = rgba_to_hex(theme.shadow_color());
            ctx.set_fill_color(&shadow_color);
            ctx.fill_rounded_rect(
                rect.x + shadow_offset.0,
                rect.y + shadow_offset.1,
                rect.width,
                rect.height,
                8.0,
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);

            // Draw border
            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);
        }

        PopupType::ColorPicker { .. } => {
            // Draw shadow
            let shadow_offset = theme.shadow_offset();
            let shadow_color = rgba_to_hex(theme.shadow_color());
            ctx.set_fill_color(&shadow_color);
            ctx.fill_rounded_rect(
                rect.x + shadow_offset.0,
                rect.y + shadow_offset.1,
                rect.width,
                rect.height,
                8.0,
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);

            // Draw border
            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);
        }

        PopupType::Custom { .. } => {
            // Draw shadow
            let shadow_offset = theme.shadow_offset();
            let shadow_color = rgba_to_hex(theme.shadow_color());
            ctx.set_fill_color(&shadow_color);
            ctx.fill_rounded_rect(
                rect.x + shadow_offset.0,
                rect.y + shadow_offset.1,
                rect.width,
                rect.height,
                8.0,
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);

            // Draw border
            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);
        }
    }
}
