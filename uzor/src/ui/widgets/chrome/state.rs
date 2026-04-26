//! Chrome persistent state.

use super::types::ChromeButton;
use super::super::tooltip::TooltipState;

/// Transient interaction state for the Chrome titlebar.
#[derive(Debug, Clone, Default)]
pub struct ChromeState {
    /// Which titlebar button the pointer is currently over.
    pub hovered_button: Option<ChromeButton>,
    /// Which titlebar button is currently pressed (button-down).
    pub pressed_button: Option<ChromeButton>,
    /// ID of the tab the pointer is currently over.
    pub hovered_tab: Option<String>,
    /// Tooltip state for button labels (e.g. "Minimize", "Close").
    pub tooltip: TooltipState,
}

impl ChromeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all transient hover/press state (call at end of frame if needed).
    pub fn clear_hover(&mut self) {
        self.hovered_button = None;
        self.hovered_tab = None;
    }
}
