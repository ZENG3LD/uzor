//! Popup state adapter - Contract/Connector for popup interaction state
//!
//! **PopupState is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/mod.rs`)
//! - External state management systems (app state, Redux, ECS, etc.)
//!
//! Popup state tracks open/closed state, position, selected item (ContextMenu),
//! selected color and custom mode (ColorPicker).

use std::collections::HashMap;

/// State adapter for popup interaction
///
/// This trait defines the contract for tracking popup interaction state.
/// External projects implement this trait to integrate with their state management systems.
pub trait PopupState {
    // =========================================================================
    // Read State (Immutable)
    // =========================================================================

    /// Check if popup is currently open
    fn is_open(&self, popup_id: &str) -> bool;

    /// Get popup position as (x, y)
    fn position(&self, popup_id: &str) -> (f64, f64);

    /// Get selected menu item index (ContextMenu)
    fn selected_item(&self, popup_id: &str) -> Option<usize>;

    /// Get selected color (ColorPicker), hex string or None
    fn selected_color(&self, popup_id: &str) -> Option<String>;

    /// Check if custom color mode is active (ColorPicker)
    fn is_custom_mode(&self, popup_id: &str) -> bool;

    // =========================================================================
    // Write State (Mutable)
    // =========================================================================

    /// Set popup open/closed state
    fn set_open(&mut self, popup_id: &str, open: bool);

    /// Set popup position
    fn set_position(&mut self, popup_id: &str, pos: (f64, f64));

    /// Set selected menu item (ContextMenu)
    fn set_selected_item(&mut self, popup_id: &str, index: Option<usize>);

    /// Set selected color (ColorPicker)
    fn set_selected_color(&mut self, popup_id: &str, color: Option<String>);

    /// Set custom color mode (ColorPicker)
    fn set_custom_mode(&mut self, popup_id: &str, custom: bool);
}

// =============================================================================
// Default State Implementation
// =============================================================================

/// Simple implementation of PopupState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimplePopupState {
    /// Open state for each popup
    open: HashMap<String, bool>,

    /// Position for each popup
    positions: HashMap<String, (f64, f64)>,

    /// Selected item index for each popup (ContextMenu)
    selected_items: HashMap<String, Option<usize>>,

    /// Selected color for each popup (ColorPicker)
    selected_colors: HashMap<String, Option<String>>,

    /// Custom mode state for each popup (ColorPicker)
    custom_modes: HashMap<String, bool>,
}

impl SimplePopupState {
    /// Create new popup state
    pub fn new() -> Self {
        Self {
            open: HashMap::new(),
            positions: HashMap::new(),
            selected_items: HashMap::new(),
            selected_colors: HashMap::new(),
            custom_modes: HashMap::new(),
        }
    }

    /// Clear all state for a popup
    pub fn clear(&mut self, popup_id: &str) {
        self.open.remove(popup_id);
        self.positions.remove(popup_id);
        self.selected_items.remove(popup_id);
        self.selected_colors.remove(popup_id);
        self.custom_modes.remove(popup_id);
    }

    /// Clear all popup state
    pub fn clear_all(&mut self) {
        self.open.clear();
        self.positions.clear();
        self.selected_items.clear();
        self.selected_colors.clear();
        self.custom_modes.clear();
    }
}

impl PopupState for SimplePopupState {
    fn is_open(&self, popup_id: &str) -> bool {
        self.open.get(popup_id).copied().unwrap_or(false)
    }

    fn position(&self, popup_id: &str) -> (f64, f64) {
        self.positions.get(popup_id).copied().unwrap_or((0.0, 0.0))
    }

    fn selected_item(&self, popup_id: &str) -> Option<usize> {
        self.selected_items.get(popup_id).cloned().flatten()
    }

    fn selected_color(&self, popup_id: &str) -> Option<String> {
        self.selected_colors.get(popup_id).cloned().flatten()
    }

    fn is_custom_mode(&self, popup_id: &str) -> bool {
        self.custom_modes.get(popup_id).copied().unwrap_or(false)
    }

    fn set_open(&mut self, popup_id: &str, open: bool) {
        self.open.insert(popup_id.to_string(), open);
    }

    fn set_position(&mut self, popup_id: &str, pos: (f64, f64)) {
        self.positions.insert(popup_id.to_string(), pos);
    }

    fn set_selected_item(&mut self, popup_id: &str, index: Option<usize>) {
        self.selected_items.insert(popup_id.to_string(), index);
    }

    fn set_selected_color(&mut self, popup_id: &str, color: Option<String>) {
        self.selected_colors.insert(popup_id.to_string(), color);
    }

    fn set_custom_mode(&mut self, popup_id: &str, custom: bool) {
        self.custom_modes.insert(popup_id.to_string(), custom);
    }
}
