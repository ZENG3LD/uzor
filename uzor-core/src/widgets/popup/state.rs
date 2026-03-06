//! Popup state adapter - Contract/Connector for popup interaction state
//!
//! # Architecture Role
//!
//! **PopupState is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/mod.rs`)
//! - External state management systems (app state, Redux, ECS, etc.)
//!
//! # How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ 1. External State Manager (e.g., AppState, UIState)     │
//! │    - Stores popup interaction state                     │
//! │    - Implements PopupState trait (mapping)              │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 2. PopupState trait (THIS MODULE)                       │
//! │    - Defines contract (which state popups need)         │
//! │    - Acts as connector interface                        │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 3. Factory render functions (factory/mod.rs)            │
//! │    - Accept &PopupState or &mut PopupState              │
//! │    - Call trait methods to get/update state             │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # What is Popup State?
//!
//! Popup state tracks **temporal interaction state** - ephemeral data that changes during user interaction:
//! - **Open state** - Is popup visible?
//! - **Position** - Where is the popup displayed?
//! - **Selected item** - Which menu item is highlighted? (ContextMenu)
//! - **Selected color** - Which color is selected? (ColorPicker)
//! - **Custom mode** - Is hex input active? (ColorPicker)
//!
//! # Implementation Example
//!
//! ```rust,ignore
//! // In ui/state.rs (or your state module)
//! pub struct UIState {
//!     pub open_popups: HashMap<String, bool>,
//!     pub popup_positions: HashMap<String, (f64, f64)>,
//!     pub selected_items: HashMap<String, Option<usize>>,
//!     // ... other UI state
//! }
//!
//! impl PopupState for UIState {
//!     fn is_open(&self, popup_id: &str) -> bool {
//!         self.open_popups.get(popup_id).copied().unwrap_or(false)
//!     }
//!
//!     fn set_open(&mut self, popup_id: &str, open: bool) {
//!         self.open_popups.insert(popup_id.to_string(), open);
//!     }
//!     // ... other methods
//! }
//! ```
//!
//! # Notes
//!
//! - **State lives in app** - PopupState connects to app's state management system
//! - **Factory reads state** - Uses `is_open()` to determine visibility
//! - **Factory writes state** - Uses `set_open()` when popup is dismissed

use std::collections::HashMap;

/// State adapter for popup interaction
///
/// This trait defines the contract for tracking popup interaction state.
/// External projects implement this trait to integrate with their state management systems.
///
/// # Responsibilities
///
/// - Track open/closed state (visibility)
/// - Track position (where popup is displayed)
/// - Track selected item (for ContextMenu)
/// - Track selected color (for ColorPicker)
/// - Track custom mode (for ColorPicker hex input)
///
/// # Popup Identity
///
/// All methods take `popup_id: &str` to identify which popup's state to check/update.
/// This enables a single state manager to track multiple popups.
pub trait PopupState {
    // =========================================================================
    // Read State (Immutable)
    // =========================================================================

    /// Check if popup is currently open
    ///
    /// # Parameters
    /// - `popup_id` - Unique identifier for this popup (e.g., "chart_context_menu")
    ///
    /// # Returns
    /// `true` if popup is visible, `false` otherwise
    fn is_open(&self, popup_id: &str) -> bool;

    /// Get popup position
    ///
    /// # Parameters
    /// - `popup_id` - Unique identifier for this popup
    ///
    /// # Returns
    /// (x, y) position of popup top-left corner
    fn position(&self, popup_id: &str) -> (f64, f64);

    /// Get selected menu item index (ContextMenu)
    ///
    /// # Parameters
    /// - `popup_id` - Unique identifier for this popup
    ///
    /// # Returns
    /// Index of currently highlighted/selected item, or None
    fn selected_item(&self, popup_id: &str) -> Option<usize>;

    /// Get selected color (ColorPicker)
    ///
    /// # Parameters
    /// - `popup_id` - Unique identifier for this popup
    ///
    /// # Returns
    /// Currently selected color as hex string (e.g., "#FF0000"), or None
    fn selected_color(&self, popup_id: &str) -> Option<String>;

    /// Check if custom color mode is active (ColorPicker)
    ///
    /// # Parameters
    /// - `popup_id` - Unique identifier for this popup
    ///
    /// # Returns
    /// `true` if hex input is active, `false` otherwise
    fn is_custom_mode(&self, popup_id: &str) -> bool;

    // =========================================================================
    // Write State (Mutable)
    // =========================================================================

    /// Set popup open/closed state
    ///
    /// # Parameters
    /// - `popup_id` - Which popup to set open state for
    /// - `open` - `true` to show popup, `false` to hide
    fn set_open(&mut self, popup_id: &str, open: bool);

    /// Set popup position
    ///
    /// # Parameters
    /// - `popup_id` - Which popup to set position for
    /// - `pos` - (x, y) position of popup top-left corner
    fn set_position(&mut self, popup_id: &str, pos: (f64, f64));

    /// Set selected menu item (ContextMenu)
    ///
    /// # Parameters
    /// - `popup_id` - Which popup to set selected item for
    /// - `index` - Index of highlighted item, or None to clear selection
    fn set_selected_item(&mut self, popup_id: &str, index: Option<usize>);

    /// Set selected color (ColorPicker)
    ///
    /// # Parameters
    /// - `popup_id` - Which popup to set selected color for
    /// - `color` - Hex color string (e.g., "#FF0000"), or None to clear
    fn set_selected_color(&mut self, popup_id: &str, color: Option<String>);

    /// Set custom color mode (ColorPicker)
    ///
    /// # Parameters
    /// - `popup_id` - Which popup to set custom mode for
    /// - `custom` - `true` to enable hex input, `false` to disable
    fn set_custom_mode(&mut self, popup_id: &str, custom: bool);
}

// =============================================================================
// Default State Implementation
// =============================================================================

/// Simple implementation of PopupState for prototyping
///
/// This struct provides a minimal state implementation for external projects
/// that don't need complex state management integration.
///
/// Tracks state for multiple popups using HashMap<String, T>.
///
/// # Usage
///
/// ```rust,ignore
/// use popup::state::{PopupState, SimplePopupState};
///
/// let mut state = SimplePopupState::new();
///
/// // Open popup at position
/// state.set_open("context_menu", true);
/// state.set_position("context_menu", (100.0, 200.0));
///
/// assert!(state.is_open("context_menu"));
/// assert_eq!(state.position("context_menu"), (100.0, 200.0));
///
/// // Close popup
/// state.set_open("context_menu", false);
/// assert!(!state.is_open("context_menu"));
/// ```
///
/// For production, implement PopupState for your app's state manager instead.
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
