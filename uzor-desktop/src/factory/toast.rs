//! Toast factory rendering for desktop

use super::{RenderContext, TextAlign, TextBaseline, rgba_to_hex};
use uzor::types::{Rect, WidgetState};
use uzor::widgets::toast::types::ToastType;
use uzor::widgets::toast::theme::ToastTheme;

/// Render toast with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    toast: &ToastType,
    rect: Rect,
    _state: WidgetState,
    theme: &dyn ToastTheme,
) {
    match toast {
        ToastType::Info { message, .. } => {
            // Draw shadow
            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(
                rect.x + 2.0,
                rect.y + 2.0,
                rect.width,
                rect.height,
                theme.border_radius(),
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.info_background());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, theme.border_radius());

            // Draw icon
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("16px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(theme.info_icon(), rect.x + theme.toast_padding(), rect.center_y());

            // Draw message
            ctx.set_font("13px sans-serif");
            ctx.fill_text(message, rect.x + theme.toast_padding() + theme.icon_size() + 8.0, rect.center_y());
        }

        ToastType::Success { message, .. } => {
            // Draw shadow
            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(
                rect.x + 2.0,
                rect.y + 2.0,
                rect.width,
                rect.height,
                theme.border_radius(),
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.success_background());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, theme.border_radius());

            // Draw icon
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("16px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(theme.success_icon(), rect.x + theme.toast_padding(), rect.center_y());

            // Draw message
            ctx.set_font("13px sans-serif");
            ctx.fill_text(message, rect.x + theme.toast_padding() + theme.icon_size() + 8.0, rect.center_y());
        }

        ToastType::Warning { message, .. } => {
            // Draw shadow
            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(
                rect.x + 2.0,
                rect.y + 2.0,
                rect.width,
                rect.height,
                theme.border_radius(),
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.warning_background());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, theme.border_radius());

            // Draw icon
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("16px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(theme.warning_icon(), rect.x + theme.toast_padding(), rect.center_y());

            // Draw message
            ctx.set_font("13px sans-serif");
            ctx.fill_text(message, rect.x + theme.toast_padding() + theme.icon_size() + 8.0, rect.center_y());
        }

        ToastType::Error { message, .. } => {
            // Draw shadow
            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(
                rect.x + 2.0,
                rect.y + 2.0,
                rect.width,
                rect.height,
                theme.border_radius(),
            );

            // Draw background
            let bg_color = rgba_to_hex(theme.error_background());
            ctx.set_fill_color(&bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, theme.border_radius());

            // Draw icon
            let text_color = rgba_to_hex(theme.text_color());
            ctx.set_fill_color(&text_color);
            ctx.set_font("16px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(theme.error_icon(), rect.x + theme.toast_padding(), rect.center_y());

            // Draw message
            ctx.set_font("13px sans-serif");
            ctx.fill_text(message, rect.x + theme.toast_padding() + theme.icon_size() + 8.0, rect.center_y());
        }
    }
}
