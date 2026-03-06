//! Text input factory rendering for desktop

use super::{RenderContext, TextAlign, TextBaseline, rgba_to_hex};
use uzor_core::types::{Rect, WidgetState};
use uzor_core::widgets::text_input::types::TextInputType;
use uzor_core::widgets::text_input::theme::TextInputTheme;

/// Render text input with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    input: &TextInputType,
    rect: Rect,
    state: WidgetState,
    theme: &dyn TextInputTheme,
) {
    let border_color = match state {
        WidgetState::Pressed | WidgetState::Active => rgba_to_hex(theme.focused_border_color()),
        WidgetState::Hovered => rgba_to_hex(theme.border_color()), // No hover-specific color
        WidgetState::Disabled => rgba_to_hex(theme.border_color()),
        WidgetState::Normal | WidgetState::Toggled => rgba_to_hex(theme.border_color()),
    };

    match input {
        TextInputType::Text { value, placeholder, .. } => {
            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw border
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw text or placeholder
            let display_text = if value.is_empty() {
                placeholder.as_str()
            } else {
                value.as_str()
            };

            let text_color = if value.is_empty() {
                rgba_to_hex(theme.placeholder_color())
            } else {
                rgba_to_hex(theme.text_color())
            };

            ctx.set_fill_color(&text_color);
            ctx.set_font("13px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(display_text, rect.x + theme.padding(), rect.center_y());
        }

        TextInputType::Number { value, .. } => {
            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw border
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw value
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("13px sans-serif");
            ctx.set_text_align(TextAlign::Right);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(value, rect.x + rect.width - theme.padding(), rect.center_y());
        }

        TextInputType::Search { value, .. } => {
            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 16.0); // More rounded for search

            // Draw border
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 16.0);

            // Draw search icon
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("14px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text("🔍", rect.x + 8.0, rect.center_y());

            // Draw query text
            ctx.set_font("13px sans-serif");
            ctx.fill_text(value, rect.x + 32.0, rect.center_y());

            // Draw clear button if query is not empty
            if !value.is_empty() {
                ctx.set_text_align(TextAlign::Right);
                ctx.fill_text("×", rect.x + rect.width - 12.0, rect.center_y());
            }
        }

        TextInputType::Password { value, .. } => {
            // Draw background
            let bg_color = rgba_to_hex(theme.background_color());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw border
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(1.0);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw masked text (dots)
            let masked = "•".repeat(value.len());
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("13px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(&masked, rect.x + theme.padding(), rect.center_y());
        }
    }
}
