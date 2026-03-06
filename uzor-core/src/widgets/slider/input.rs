//! Slider input adapter - Contract/Connector for slider event handling

use crate::types::Rect;

/// Input handler adapter for slider events
pub trait SliderInputHandler {
    fn hit_test(
        &self,
        handle_center: (f64, f64),
        handle_radius: f64,
        mouse_pos: (f64, f64),
    ) -> bool {
        let dx = mouse_pos.0 - handle_center.0;
        let dy = mouse_pos.1 - handle_center.1;
        (dx * dx + dy * dy) <= (handle_radius * handle_radius)
    }

    fn mouse_to_value(
        &self,
        mouse_x: f64,
        track_x: f64,
        track_width: f64,
        min: f64,
        max: f64,
    ) -> f64 {
        if track_width <= 0.0 {
            return min;
        }
        let t = ((mouse_x - track_x) / track_width).clamp(0.0, 1.0);
        min + t * (max - min)
    }

    fn scroll_to_delta(&self, scroll_delta: f64, min: f64, max: f64) -> f64 {
        let range = max - min;
        if range <= 0.0 {
            return 0.0;
        }
        scroll_delta.signum() * range * 0.1
    }

    fn is_on_track(&self, mouse_pos: (f64, f64), track_rect: &Rect) -> bool {
        mouse_pos.0 >= track_rect.x
            && mouse_pos.0 <= track_rect.x + track_rect.width
            && mouse_pos.1 >= track_rect.y
            && mouse_pos.1 <= track_rect.y + track_rect.height
    }
}

/// Default implementation of SliderInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultSliderInputHandler;

impl SliderInputHandler for DefaultSliderInputHandler {}
