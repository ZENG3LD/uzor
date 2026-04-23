//! Popup input adapter - Contract/Connector for popup event handling
//!
//! **PopupInputHandler is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory event handling logic (`factory/mod.rs`)
//! - External input systems (event loops, input managers, etc.)

use crate::types::Rect;

/// Input handler adapter for popup events
///
/// This trait defines the contract for converting raw input events into popup actions.
/// External projects implement this trait to customize input behavior (rare).
pub trait PopupInputHandler {
    // =========================================================================
    // Hit Testing
    // =========================================================================

    /// Test if mouse position is inside popup rect
    fn hit_test(&self, popup_rect: &Rect, mouse_pos: (f64, f64)) -> bool {
        let (mouse_x, mouse_y) = mouse_pos;
        mouse_x >= popup_rect.x
            && mouse_x <= popup_rect.x + popup_rect.width
            && mouse_y >= popup_rect.y
            && mouse_y <= popup_rect.y + popup_rect.height
    }

    /// Test if click was outside popup (for auto-dismiss)
    fn is_outside_click(&self, mouse_pos: (f64, f64), popup_rect: &Rect) -> bool {
        !self.hit_test(popup_rect, mouse_pos)
    }

    // =========================================================================
    // ContextMenu - Item Selection
    // =========================================================================

    /// Convert mouse Y coordinate to menu item index
    fn mouse_to_item_index(
        &self,
        mouse_y: f64,
        popup_y: f64,
        item_height: f64,
        item_count: usize,
        padding_vertical: f64,
    ) -> Option<usize> {
        let relative_y = mouse_y - popup_y - padding_vertical;
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

    // =========================================================================
    // ColorPicker - Color Selection
    // =========================================================================

    /// Convert mouse position to color grid index
    fn mouse_to_color_index(
        &self,
        mouse_pos: (f64, f64),
        popup_rect: &Rect,
        grid_cols: usize,
        swatch_size: f64,
        grid_spacing: f64,
        padding: f64,
    ) -> Option<usize> {
        let (mouse_x, mouse_y) = mouse_pos;

        // Calculate position relative to grid start
        let grid_x = mouse_x - popup_rect.x - padding;
        let grid_y = mouse_y - popup_rect.y - padding;

        if grid_x < 0.0 || grid_y < 0.0 {
            return None;
        }

        // Calculate cell size (swatch + spacing)
        let cell_size = swatch_size + grid_spacing;

        // Calculate column and row
        let col = (grid_x / cell_size) as usize;
        let row = (grid_y / cell_size) as usize;

        // Check if within actual swatch (not in spacing)
        let local_x = grid_x - (col as f64 * cell_size);
        let local_y = grid_y - (row as f64 * cell_size);

        if local_x > swatch_size || local_y > swatch_size {
            return None; // In spacing between swatches
        }

        if col >= grid_cols {
            return None;
        }

        // Calculate linear index
        Some(row * grid_cols + col)
    }

    // =========================================================================
    // Position Adjustment
    // =========================================================================

    /// Adjust popup position to keep it on screen
    fn adjust_position_to_screen(
        &self,
        pos: (f64, f64),
        size: (f64, f64),
        screen: (f64, f64),
    ) -> (f64, f64) {
        let (mut x, mut y) = pos;
        let (width, height) = size;
        let (screen_width, screen_height) = screen;

        // Adjust X - keep on screen
        if x + width > screen_width {
            x = (x - width).max(0.0);
        }
        if x < 0.0 {
            x = 0.0;
        }

        // Adjust Y - keep on screen
        if y + height > screen_height {
            y = (y - height).max(0.0);
        }
        if y < 0.0 {
            y = 0.0;
        }

        (x, y)
    }
}

// =============================================================================
// Default Input Handler Implementation
// =============================================================================

/// Default implementation of PopupInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultPopupInputHandler;

impl PopupInputHandler for DefaultPopupInputHandler {
    // All methods use trait defaults (no overrides needed)
}
