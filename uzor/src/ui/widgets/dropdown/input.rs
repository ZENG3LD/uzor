//! Dropdown input adapter - Contract/Connector for dropdown event handling

use crate::types::Rect;

/// Input handler adapter for dropdown events
pub trait DropdownInputHandler {
    fn hit_test_dropdown(&self, dropdown_rect: &Rect, mouse_pos: (f64, f64)) -> bool {
        let (mouse_x, mouse_y) = mouse_pos;
        mouse_x >= dropdown_rect.x
            && mouse_x <= dropdown_rect.x + dropdown_rect.width
            && mouse_y >= dropdown_rect.y
            && mouse_y <= dropdown_rect.y + dropdown_rect.height
    }

    fn hit_test_item(&self, item_rect: &Rect, mouse_pos: (f64, f64)) -> bool {
        let (mouse_x, mouse_y) = mouse_pos;
        mouse_x >= item_rect.x
            && mouse_x <= item_rect.x + item_rect.width
            && mouse_y >= item_rect.y
            && mouse_y <= item_rect.y + item_rect.height
    }

    fn mouse_to_item_index(
        &self,
        mouse_y: f64,
        popup_y: f64,
        item_height: f64,
        item_count: usize,
    ) -> Option<usize> {
        if item_height <= 0.0 || item_count == 0 {
            return None;
        }

        let relative_y = mouse_y - popup_y;
        if relative_y < 0.0 {
            return None;
        }

        let index = (relative_y / item_height) as usize;
        if index < item_count {
            Some(index)
        } else {
            None
        }
    }

    fn is_close_click(&self, mouse_pos: (f64, f64), popup_rect: &Rect) -> bool {
        let (mouse_x, mouse_y) = mouse_pos;
        !(mouse_x >= popup_rect.x
            && mouse_x <= popup_rect.x + popup_rect.width
            && mouse_y >= popup_rect.y
            && mouse_y <= popup_rect.y + popup_rect.height)
    }

    fn mouse_to_grid_item_index(
        &self,
        mouse_pos: (f64, f64),
        popup_rect: &Rect,
        item_size: f64,
        columns: usize,
        item_count: usize,
    ) -> Option<usize> {
        if item_size <= 0.0 || columns == 0 || item_count == 0 {
            return None;
        }

        let (mouse_x, mouse_y) = mouse_pos;
        let relative_x = mouse_x - popup_rect.x;
        let relative_y = mouse_y - popup_rect.y;

        if relative_x < 0.0 || relative_y < 0.0 {
            return None;
        }

        let col = (relative_x / item_size) as usize;
        let row = (relative_y / item_size) as usize;

        if col >= columns {
            return None;
        }

        let index = row * columns + col;
        if index < item_count {
            Some(index)
        } else {
            None
        }
    }
}

/// Default implementation of DropdownInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultDropdownInputHandler;

impl DropdownInputHandler for DefaultDropdownInputHandler {}
