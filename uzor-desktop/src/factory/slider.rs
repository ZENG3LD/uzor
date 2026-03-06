//! Slider factory rendering for desktop

use super::{RenderContext, rgba_to_hex};
use uzor_core::types::{Rect, WidgetState};
use uzor_core::widgets::slider::types::SliderType;
use uzor_core::widgets::slider::theme::SliderTheme;

/// Render slider with default settings
pub fn render_default(
    ctx: &mut dyn RenderContext,
    slider: &SliderType,
    rect: Rect,
    state: WidgetState,
    theme: &dyn SliderTheme,
) {
    match slider {
        SliderType::Single { value, min, max, .. } => {
            let track_height = theme.track_height();
            let handle_radius = theme.handle_radius();
            let track_y = rect.center_y() - track_height / 2.0;

            // Draw track background
            let track_color = rgba_to_hex(theme.track_color());
            ctx.set_fill_color(&track_color);
            ctx.fill_rounded_rect(rect.x, track_y, rect.width, track_height, track_height / 2.0);

            // Calculate fill width
            let range = max - min;
            let normalized = (value - min) / range;
            let fill_width = rect.width * normalized;

            // Draw filled track (same color as track for now)
            ctx.set_fill_color(&track_color);
            ctx.fill_rounded_rect(rect.x, track_y, fill_width, track_height, track_height / 2.0);

            // Calculate thumb position
            let thumb_size = handle_radius * 2.0;
            let thumb_x = rect.x + fill_width - handle_radius;
            let thumb_y = rect.center_y() - handle_radius;

            // Draw thumb shadow
            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(thumb_x + 1.0, thumb_y + 1.0, thumb_size, thumb_size, handle_radius);

            // Draw thumb
            let thumb_color = match state {
                WidgetState::Pressed | WidgetState::Active => rgba_to_hex(theme.active_handle_color()),
                WidgetState::Hovered => rgba_to_hex(theme.hover_handle_color()),
                _ => rgba_to_hex(theme.handle_color()),
            };
            ctx.set_fill_color(&thumb_color);
            ctx.fill_rounded_rect(thumb_x, thumb_y, thumb_size, thumb_size, handle_radius);

            // Draw thumb border
            let border_color = rgba_to_hex(theme.handle_border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(theme.handle_border_width());
            ctx.stroke_rounded_rect(thumb_x, thumb_y, thumb_size, thumb_size, handle_radius);
        }

        SliderType::Dual { min_value, max_value, min, max, .. } => {
            let track_height = theme.track_height();
            let handle_radius = theme.handle_radius();
            let track_y = rect.center_y() - track_height / 2.0;

            // Draw track background
            let track_color = rgba_to_hex(theme.track_color());
            ctx.set_fill_color(&track_color);
            ctx.fill_rounded_rect(rect.x, track_y, rect.width, track_height, track_height / 2.0);

            // Calculate positions
            let range = max - min;
            let normalized_start = (min_value - min) / range;
            let normalized_end = (max_value - min) / range;
            let fill_x = rect.x + rect.width * normalized_start;
            let fill_width = rect.width * (normalized_end - normalized_start);

            // Draw filled range
            ctx.set_fill_color(&track_color);
            ctx.fill_rounded_rect(fill_x, track_y, fill_width, track_height, track_height / 2.0);

            // Draw start thumb
            let thumb_size = handle_radius * 2.0;
            let thumb1_x = fill_x - handle_radius;
            let thumb_y = rect.center_y() - handle_radius;

            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(thumb1_x + 1.0, thumb_y + 1.0, thumb_size, thumb_size, handle_radius);

            let handle_color = rgba_to_hex(theme.handle_color());
            ctx.set_fill_color(&handle_color);
            ctx.fill_rounded_rect(thumb1_x, thumb_y, thumb_size, thumb_size, handle_radius);

            let border_color = rgba_to_hex(theme.handle_border_color());
            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(theme.handle_border_width());
            ctx.stroke_rounded_rect(thumb1_x, thumb_y, thumb_size, thumb_size, handle_radius);

            // Draw end thumb
            let thumb2_x = fill_x + fill_width - handle_radius;

            ctx.set_fill_color("#00000030");
            ctx.fill_rounded_rect(thumb2_x + 1.0, thumb_y + 1.0, thumb_size, thumb_size, handle_radius);

            ctx.set_fill_color(&handle_color);
            ctx.fill_rounded_rect(thumb2_x, thumb_y, thumb_size, thumb_size, handle_radius);

            ctx.set_stroke_color(&border_color);
            ctx.set_stroke_width(theme.handle_border_width());
            ctx.stroke_rounded_rect(thumb2_x, thumb_y, thumb_size, thumb_size, handle_radius);
        }
    }
}
