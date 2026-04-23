//! Panel input adapter - Contract/Connector for panel event handling

use crate::types::Rect;

/// Input handler adapter for panel events
pub trait PanelInputHandler {
    fn hit_test(&self, rect: Rect, mouse_pos: (f64, f64)) -> bool {
        let (mx, my) = mouse_pos;
        mx >= rect.x
            && mx <= rect.x + rect.width
            && my >= rect.y
            && my <= rect.y + rect.height
    }

    fn hit_test_resize_handle(
        &self,
        handle_rect: Rect,
        mouse_pos: (f64, f64),
    ) -> bool {
        self.hit_test(handle_rect, mouse_pos)
    }

    fn hit_test_chevron(
        &self,
        chevron_rect: Rect,
        mouse_pos: (f64, f64),
    ) -> bool {
        self.hit_test(chevron_rect, mouse_pos)
    }

    fn mouse_to_size(
        &self,
        mouse_pos: f64,
        min_size: f64,
        max_size: f64,
    ) -> f64 {
        mouse_pos.clamp(min_size, max_size)
    }

    fn calculate_resize_handle(
        &self,
        panel_rect: Rect,
        handle_width: f64,
        is_left: bool,
    ) -> Rect {
        if is_left {
            Rect {
                x: panel_rect.x + panel_rect.width - handle_width / 2.0,
                y: panel_rect.y,
                width: handle_width,
                height: panel_rect.height,
            }
        } else {
            Rect {
                x: panel_rect.x - handle_width / 2.0,
                y: panel_rect.y,
                width: handle_width,
                height: panel_rect.height,
            }
        }
    }

    fn is_outside_click(
        &self,
        mouse_pos: (f64, f64),
        modal_rect: Rect,
    ) -> bool {
        !self.hit_test(modal_rect, mouse_pos)
    }

    fn calculate_floating_position(
        &self,
        anchor_rect: Rect,
        offset: (f64, f64),
    ) -> (f64, f64) {
        (anchor_rect.x + offset.0, anchor_rect.y + offset.1)
    }

    fn calculate_chevron_rect(
        &self,
        panel_x: f64,
        panel_y: f64,
        button_height: f64,
        panel_width: f64,
    ) -> Rect {
        Rect {
            x: panel_x,
            y: panel_y,
            width: panel_width,
            height: button_height,
        }
    }
}

/// Default implementation of PanelInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultPanelInputHandler;

impl PanelInputHandler for DefaultPanelInputHandler {}
