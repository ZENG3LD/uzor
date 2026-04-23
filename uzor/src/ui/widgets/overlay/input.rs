//! Overlay input adapter - Contract/Connector for overlay event handling

use std::time::Duration;
use crate::types::Rect;

/// Input handler adapter for overlay events
pub trait OverlayInputHandler {
    fn should_show(&self, hover_duration: Duration, delay_threshold_ms: u32) -> bool {
        hover_duration.as_millis() >= delay_threshold_ms as u128
    }

    fn calculate_fade_opacity(&self, elapsed_ms: u32, fade_duration_ms: u32) -> f64 {
        if elapsed_ms >= fade_duration_ms {
            1.0
        } else {
            (elapsed_ms as f64) / (fade_duration_ms as f64)
        }
    }

    fn adjust_position_to_screen(
        &self,
        pos: (f64, f64),
        size: (f64, f64),
        screen: (f64, f64)
    ) -> (f64, f64) {
        let (mut x, mut y) = pos;
        let (width, height) = size;
        let (screen_width, screen_height) = screen;

        if x + width > screen_width {
            x = screen_width - width;
        }
        if x < 0.0 {
            x = 0.0;
        }

        if y + height > screen_height {
            y = screen_height - height;
        }
        if y < 0.0 {
            y = 0.0;
        }

        (x, y)
    }

    fn calculate_tooltip_size(&self, text: &str, max_width: f64, font_size: f64) -> (f64, f64) {
        let char_width = font_size * 0.6;
        let chars_per_line = (max_width / char_width).max(1.0) as usize;
        let line_count = (text.len() + chars_per_line - 1) / chars_per_line.max(1);
        let width = max_width.min(text.len() as f64 * char_width);
        let height = line_count as f64 * font_size * 1.5;
        (width.max(0.0), height.max(font_size))
    }

    fn is_mouse_nearby(
        &self,
        mouse_pos: (f64, f64),
        overlay_rect: &Rect,
        threshold: f64
    ) -> bool {
        let (mx, my) = mouse_pos;
        mx >= overlay_rect.x - threshold
            && mx <= overlay_rect.x + overlay_rect.width + threshold
            && my >= overlay_rect.y - threshold
            && my <= overlay_rect.y + overlay_rect.height + threshold
    }
}

/// Default implementation of OverlayInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultOverlayInputHandler;

impl OverlayInputHandler for DefaultOverlayInputHandler {}
