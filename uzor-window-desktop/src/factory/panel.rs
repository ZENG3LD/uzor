//! Panel factory rendering for desktop

use super::{RenderContext, rgba_to_hex};
use uzor::types::{Rect, WidgetState};
use uzor::widgets::panel::types::PanelType;
use uzor::widgets::panel::theme::PanelTheme;

/// Render panel with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    panel: &PanelType,
    rect: Rect,
    _state: WidgetState,
    theme: &dyn PanelTheme,
) {
    match panel {
        PanelType::Toolbar { .. } => {
            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

            // Draw bottom border
            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.begin_path();
            ctx.move_to(rect.x, rect.y + rect.height);
            ctx.line_to(rect.x + rect.width, rect.y + rect.height);
            ctx.stroke();
        }

        PanelType::Sidebar { .. } => {
            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

            // Draw right border
            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.begin_path();
            ctx.move_to(rect.x + rect.width, rect.y);
            ctx.line_to(rect.x + rect.width, rect.y + rect.height);
            ctx.stroke();
        }

        PanelType::Modal { .. } => {
            // Dim background
            let backdrop_color = rgba_to_hex(theme.modal_backdrop_color());
            ctx.set_fill_color(&backdrop_color);
            ctx.fill_rect(0.0, 0.0, 9999.0, 9999.0);

            // Draw shadow
            ctx.set_fill_color("#00000040");
            ctx.fill_rounded_rect(rect.x + 4.0, rect.y + 4.0, rect.width, rect.height, 8.0);

            // Draw modal background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);

            // Draw border
            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(2.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 8.0);
        }

        PanelType::Hideable { is_hidden, .. } => {
            if *is_hidden {
                // Just show collapsed state
                let bg_color = rgba_to_hex(theme.background_color());
                ctx.set_fill_color(&bg_color);
                ctx.fill_rect(rect.x, rect.y, 32.0, rect.height);
            } else {
                // Full panel
                let bg_color = rgba_to_hex(theme.background_color());
                ctx.set_fill_color(&bg_color);
                ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

                let border_color = rgba_to_hex(theme.border_color());
                ctx.set_stroke_color(&border_color);
                ctx.set_stroke_width(1.0);
                ctx.stroke_rect(rect.x, rect.y, rect.width, rect.height);
            }
        }
    }
}
