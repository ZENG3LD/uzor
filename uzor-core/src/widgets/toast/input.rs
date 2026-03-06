//! Toast input adapter - Contract/Connector for toast interaction handling

use crate::types::Rect;

/// Fade animation phase for toast
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadePhase {
    /// Fading in (opacity 0.0 -> 1.0)
    FadeIn,
    /// Full display (opacity 1.0)
    Display,
    /// Fading out (opacity 1.0 -> 0.0)
    FadeOut,
}

/// Input handler adapter for toast interactions
pub trait ToastInputHandler {
    fn calculate_stack_offset(
        &self,
        toast_index: usize,
        toast_height: f64,
        spacing: f64,
    ) -> f64 {
        toast_index as f64 * (toast_height + spacing)
    }

    fn is_mouse_over(&self, mouse_pos: (f64, f64), toast_rect: &Rect) -> bool {
        let (mx, my) = mouse_pos;
        mx >= toast_rect.x
            && mx <= toast_rect.x + toast_rect.width
            && my >= toast_rect.y
            && my <= toast_rect.y + toast_rect.height
    }

    fn is_dismiss_click(&self, mouse_pos: (f64, f64), close_button_rect: &Rect) -> bool {
        self.is_mouse_over(mouse_pos, close_button_rect)
    }

    fn update_opacity_fade(
        &self,
        phase: FadePhase,
        elapsed_ms: u32,
        fade_duration_ms: u32,
    ) -> f64 {
        match phase {
            FadePhase::FadeIn => {
                if elapsed_ms >= fade_duration_ms {
                    1.0
                } else {
                    elapsed_ms as f64 / fade_duration_ms as f64
                }
            }
            FadePhase::Display => 1.0,
            FadePhase::FadeOut => {
                if elapsed_ms >= fade_duration_ms {
                    0.0
                } else {
                    1.0 - (elapsed_ms as f64 / fade_duration_ms as f64)
                }
            }
        }
    }

    fn calculate_position(
        &self,
        screen_size: (f64, f64),
        toast_size: (f64, f64),
        offset: (f64, f64),
        vertical_offset: f64,
    ) -> (f64, f64) {
        let (screen_width, _screen_height) = screen_size;
        let (toast_width, _toast_height) = toast_size;
        let (offset_x, offset_y) = offset;

        let x = screen_width - toast_width - offset_x;
        let y = offset_y + vertical_offset;

        (x, y)
    }
}

/// Default implementation of ToastInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultToastInputHandler;

impl ToastInputHandler for DefaultToastInputHandler {}
