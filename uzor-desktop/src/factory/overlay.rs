//! Overlay factory rendering for desktop

use super::{RenderContext, TextAlign, TextBaseline};
use uzor::types::{Rect, WidgetState};
use uzor::widgets::overlay::types::OverlayType;
use uzor::widgets::overlay::theme::OverlayTheme;

/// Render overlay with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    overlay: &OverlayType,
    rect: Rect,
    _state: WidgetState,
    theme: &dyn OverlayTheme,
) {
    match overlay {
        OverlayType::Tooltip { text, .. } => {
            let shadow_offset = theme.shadow_offset();
            // Draw shadow
            ctx.set_fill_color(theme.shadow_color());
            ctx.fill_rounded_rect(
                rect.x + shadow_offset.0,
                rect.y + shadow_offset.1,
                rect.width,
                rect.height,
                4.0,
            );

            // Draw background
            ctx.set_fill_color(theme.background_color());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw border
            ctx.set_stroke_color(theme.border_color());
            ctx.set_stroke_width(theme.border_width());
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

            // Draw text
            ctx.set_fill_color(theme.text_color());
            ctx.set_font("12px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(text, rect.x + theme.tooltip_padding(), rect.center_y());
        }

        OverlayType::InfoOverlay { text, .. } => {
            let shadow_offset = theme.shadow_offset();
            // Draw shadow
            ctx.set_fill_color(theme.shadow_color());
            ctx.fill_rounded_rect(
                rect.x + shadow_offset.0,
                rect.y + shadow_offset.1,
                rect.width,
                rect.height,
                6.0,
            );

            // Draw background
            ctx.set_fill_color(theme.background_color());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 6.0);

            // Draw border
            ctx.set_stroke_color(theme.border_color());
            ctx.set_stroke_width(theme.border_width());
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, 6.0);

            // Draw content
            ctx.set_fill_color(theme.text_color());
            ctx.set_font("12px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            ctx.fill_text(text, rect.x + 12.0, rect.y + 12.0);
        }
    }
}
