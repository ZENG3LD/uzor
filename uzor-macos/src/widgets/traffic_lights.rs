//! macOS traffic lights widget renderer

use uzor_render::{RenderContext, draw_svg_icon};
use crate::colors::WidgetState;
use crate::themes::traffic_lights::{
    TrafficLightTheme, TrafficLightButton, TrafficLightGroupState, MaximizeMode,
};
use crate::icons::paths;
use std::f64::consts::PI;

/// Render all 3 traffic light buttons. Returns total (width, height).
pub fn render_traffic_lights(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    group_state: TrafficLightGroupState,
    hovered_button: Option<TrafficLightButton>,
    pressed_button: Option<TrafficLightButton>,
    maximize_mode: MaximizeMode,
    theme: &TrafficLightTheme,
) -> (f64, f64) {
    let positions = theme.button_positions(x, y);
    let radius = theme.button_radius();
    let diameter = theme.button_diameter();
    let show_icons = theme.show_icon(group_state);
    let border_color = theme.border_color();
    let has_border = border_color != "#00000000";

    let buttons = [
        (TrafficLightButton::Close, paths::TRAFFIC_LIGHT_CLOSE),
        (TrafficLightButton::Minimize, paths::TRAFFIC_LIGHT_MINIMIZE),
        (
            TrafficLightButton::Maximize,
            match maximize_mode {
                MaximizeMode::Zoom => paths::TRAFFIC_LIGHT_MAXIMIZE,
                MaximizeMode::Fullscreen => paths::TRAFFIC_LIGHT_FULLSCREEN,
            },
        ),
    ];

    for (i, &(button_type, icon_svg)) in buttons.iter().enumerate() {
        let (cx, cy) = positions[i];

        // Determine button state
        let widget_state = if Some(button_type) == pressed_button {
            WidgetState::Pressed
        } else if Some(button_type) == hovered_button {
            WidgetState::Hovered
        } else {
            WidgetState::Normal
        };

        // Get button color
        let button_color = theme.button_color(button_type, group_state, widget_state);

        // Draw filled circle
        ctx.save();
        ctx.set_fill_color(button_color);
        ctx.begin_path();
        ctx.arc(cx, cy, radius, 0.0, 2.0 * PI);
        ctx.fill();
        ctx.restore();

        // Draw border in light mode
        if has_border {
            ctx.save();
            ctx.set_stroke_color(border_color);
            ctx.set_stroke_width(0.5);
            ctx.begin_path();
            ctx.arc(cx, cy, radius, 0.0, 2.0 * PI);
            ctx.stroke();
            ctx.restore();
        }

        // Draw icon if group is hovered
        if show_icons {
            let icon_size = match button_type {
                TrafficLightButton::Close => 6.0,
                TrafficLightButton::Minimize => 8.0,
                TrafficLightButton::Maximize => 8.0,
            };

            let icon_x = cx - icon_size / 2.0;
            let icon_y = cy - icon_size / 2.0;
            let icon_color = theme.icon_color(button_type);

            draw_svg_icon(
                ctx,
                icon_svg,
                icon_x,
                icon_y,
                icon_size,
                icon_size,
                icon_color,
            );
        }
    }

    (theme.total_width(), diameter)
}

/// Hit test: which button was clicked?
pub fn hit_test(
    x: f64,
    y: f64,
    mouse_x: f64,
    mouse_y: f64,
    theme: &TrafficLightTheme,
) -> Option<TrafficLightButton> {
    theme.hit_test(x, y, mouse_x, mouse_y)
}

/// Check if mouse is over the entire traffic light group
pub fn is_group_hovered(
    x: f64,
    y: f64,
    mouse_x: f64,
    mouse_y: f64,
    theme: &TrafficLightTheme,
) -> bool {
    theme.is_group_hovered(x, y, mouse_x, mouse_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::AppearanceMode;

    #[test]
    fn test_hit_test() {
        let theme = TrafficLightTheme::new(AppearanceMode::Dark);

        // Test close button hit (center at 18.0, 20.0)
        let result = hit_test(0.0, 20.0, 18.0, 20.0, &theme);
        assert_eq!(result, Some(TrafficLightButton::Close));

        // Test minimize button hit
        let result = hit_test(0.0, 20.0, 38.0, 20.0, &theme);
        assert_eq!(result, Some(TrafficLightButton::Minimize));

        // Test maximize button hit
        let result = hit_test(0.0, 20.0, 58.0, 20.0, &theme);
        assert_eq!(result, Some(TrafficLightButton::Maximize));

        // Test miss
        let result = hit_test(0.0, 20.0, 100.0, 100.0, &theme);
        assert_eq!(result, None);
    }

    #[test]
    fn test_group_hover() {
        let theme = TrafficLightTheme::new(AppearanceMode::Light);

        // Inside group
        assert!(is_group_hovered(0.0, 20.0, 30.0, 20.0, &theme));

        // Outside group
        assert!(!is_group_hovered(0.0, 20.0, 100.0, 20.0, &theme));
    }
}
