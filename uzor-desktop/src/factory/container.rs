//! Container factory rendering for desktop

use super::{RenderContext, rgba_to_hex};
use uzor::types::{Rect, WidgetState};
use uzor::widgets::container::types::ContainerType;
use uzor::widgets::container::theme::ContainerTheme;

/// Render container with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    container: &ContainerType,
    rect: Rect,
    _state: WidgetState,
    theme: &dyn ContainerTheme,
) {
    match container {
        ContainerType::Plain { .. } => {
            // Note: Plain container has no background/border in uzor-core types
            // Just render as transparent for now
        }

        ContainerType::Scrollable { scroll_offset, content_height, viewport_height, .. } => {
            // Draw scrollbar if content is taller than viewport
            if *content_height > *viewport_height {
                let scrollbar_width = theme.scrollbar_width();
                let scrollbar_x = rect.x + rect.width - scrollbar_width - 2.0;
                let scrollbar_y = rect.y + 2.0;
                let scrollbar_height = rect.height - 4.0;

                // Scrollbar track
                let track_color = theme.scrollbar_track_color();
                ctx.set_fill_color(&rgba_to_hex(track_color));
                ctx.fill_rounded_rect(scrollbar_x, scrollbar_y, scrollbar_width, scrollbar_height, 6.0);

                // Calculate thumb position and size
                let thumb_ratio = viewport_height / content_height;
                let thumb_height = (scrollbar_height * thumb_ratio).max(theme.min_thumb_height());
                let max_scroll = content_height - viewport_height;
                let thumb_position = if max_scroll > 0.0 {
                    (scroll_offset / max_scroll) * (scrollbar_height - thumb_height)
                } else {
                    0.0
                };

                // Scrollbar thumb
                let thumb_color = theme.scrollbar_thumb_color();
                ctx.set_fill_color(&rgba_to_hex(thumb_color));
                ctx.fill_rounded_rect(
                    scrollbar_x + 2.0,
                    scrollbar_y + thumb_position,
                    scrollbar_width - 4.0,
                    thumb_height,
                    4.0,
                );
            }
        }
    }
}
