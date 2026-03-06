//! Button input adapter - Contract/Connector for button event handling
//!
//! **ButtonInputHandler is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory event handling logic (`factory/events.rs` - future)
//! - External input systems (event loops, input managers, etc.)

use crate::types::Rect;

/// Input handler adapter for button events
///
/// This trait defines the contract for converting raw input events into button actions.
/// External projects implement this trait to customize input behavior (rare).
pub trait ButtonInputHandler {
    // =========================================================================
    // Hit Testing
    // =========================================================================

    /// Test if mouse position is inside button rect
    fn hit_test(&self, mouse_x: f64, mouse_y: f64, rect: &Rect) -> bool {
        mouse_x >= rect.x
            && mouse_x <= rect.x + rect.width
            && mouse_y >= rect.y
            && mouse_y <= rect.y + rect.height
    }

    // =========================================================================
    // Click Detection
    // =========================================================================

    /// Detect if button was clicked
    fn is_click(
        &self,
        press_x: f64,
        press_y: f64,
        release_x: f64,
        release_y: f64,
        rect: &Rect
    ) -> bool {
        self.hit_test(press_x, press_y, rect)
            && self.hit_test(release_x, release_y, rect)
    }

    // =========================================================================
    // Keyboard Navigation
    // =========================================================================

    /// Get next focus target when Tab is pressed
    fn next_focus(&self, current_id: &str, all_ids: &[String]) -> String {
        if all_ids.is_empty() {
            return String::new();
        }

        if let Some(idx) = all_ids.iter().position(|id| id == current_id) {
            let next_idx = (idx + 1) % all_ids.len();
            all_ids[next_idx].clone()
        } else {
            // Current not found, return first
            all_ids[0].clone()
        }
    }

    /// Get previous focus target when Shift+Tab is pressed
    fn prev_focus(&self, current_id: &str, all_ids: &[String]) -> String {
        if all_ids.is_empty() {
            return String::new();
        }

        if let Some(idx) = all_ids.iter().position(|id| id == current_id) {
            let prev_idx = if idx == 0 {
                all_ids.len() - 1
            } else {
                idx - 1
            };
            all_ids[prev_idx].clone()
        } else {
            // Current not found, return last
            all_ids[all_ids.len() - 1].clone()
        }
    }

    // =========================================================================
    // Activation Keys
    // =========================================================================

    /// Check if key activates button (like click)
    fn is_activation_key(&self, key: &str) -> bool {
        key == "Enter" || key == "Space" || key == " "
    }
}

// =============================================================================
// Default Input Handler Implementation
// =============================================================================

/// Default implementation of ButtonInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultButtonInputHandler;

impl ButtonInputHandler for DefaultButtonInputHandler {
    // All methods use trait defaults (no overrides needed)
}
