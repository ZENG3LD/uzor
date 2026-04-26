//! Dropdown factory rendering for desktop

use super::{RenderContext, TextAlign, TextBaseline, rgba_to_hex};
use uzor::types::{Rect, WidgetState};
use uzor::widgets::dropdown::types::DropdownType;
use uzor::widgets::dropdown::theme::DropdownTheme;

/// Render dropdown with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    dropdown: &DropdownType,
    rect: Rect,
    state: WidgetState,
    theme: &dyn DropdownTheme,
) {
    match dropdown {
        DropdownType::Standard { placeholder, .. } => {
            let border_color = match state {
                WidgetState::Pressed | WidgetState::Active => rgba_to_hex(theme.border_focus_color()),
                WidgetState::Hovered => rgba_to_hex(theme.border_hover_color()),
                _ => rgba_to_hex(theme.border_color()),
            };

            // Draw button background
            let bg_color = if matches!(state, WidgetState::Hovered) {
                rgba_to_hex(theme.background_hover_color())
            } else {
                rgba_to_hex(theme.background_color())
            };
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, theme.dropdown_height(), 4.0);

            // Draw border
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, theme.dropdown_height(), 4.0);

            // Draw placeholder/selected value
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("13px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(placeholder, rect.x + 12.0, rect.y + theme.dropdown_height() / 2.0);

            // Draw chevron
            let arrow_color = if matches!(state, WidgetState::Hovered) {
                rgba_to_hex(theme.arrow_hover_color())
            } else {
                rgba_to_hex(theme.arrow_color())
            };
            ctx.set_fill_color(&arrow_color);
            ctx.set_font("12px sans-serif");
            ctx.set_text_align(TextAlign::Right);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text("▼", rect.x + rect.width - 12.0, rect.y + theme.dropdown_height() / 2.0);
        }

        DropdownType::Grid { .. } => {
            let border_color = match state {
                WidgetState::Hovered => rgba_to_hex(theme.border_hover_color()),
                _ => rgba_to_hex(theme.border_color()),
            };

            let bg_color = if matches!(state, WidgetState::Hovered) {
                rgba_to_hex(theme.background_hover_color())
            } else {
                rgba_to_hex(theme.background_color())
            };
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, theme.dropdown_height(), 4.0);

            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, theme.dropdown_height(), 4.0);
        }

        DropdownType::Layout { .. } => {
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, theme.dropdown_height(), 4.0);

            let border_color = rgba_to_hex(theme.border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, theme.dropdown_height(), 4.0);
        }
    }
}
