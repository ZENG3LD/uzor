//! Button state adapter - Contract/Connector for button interaction state
//!
//! **ButtonState is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/render.rs`)
//! - External state management systems (app state, Redux, ECS, etc.)

/// State adapter for button interaction
///
/// This trait defines the contract for tracking button interaction state.
/// External projects implement this trait to integrate with their state management systems.
pub trait ButtonState {
    // =========================================================================
    // Read State (Immutable)
    // =========================================================================

    /// Check if button is currently hovered
    fn is_hovered(&self, button_id: &str) -> bool;

    /// Check if button is currently pressed
    fn is_pressed(&self, button_id: &str) -> bool;

    /// Check if button is currently focused
    fn is_focused(&self, button_id: &str) -> bool;

    // =========================================================================
    // Write State (Mutable)
    // =========================================================================

    /// Set button hover state
    fn set_hovered(&mut self, button_id: Option<&str>);

    /// Set button pressed state
    fn set_pressed(&mut self, button_id: Option<&str>);

    /// Set button focus state
    fn set_focused(&mut self, button_id: Option<&str>);
}

// =============================================================================
// Default State Implementation
// =============================================================================

/// Simple implementation of ButtonState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimpleButtonState {
    /// Currently hovered button ID
    pub hovered: Option<String>,

    /// Currently pressed button ID
    pub pressed: Option<String>,

    /// Currently focused button ID
    pub focused: Option<String>,
}

impl SimpleButtonState {
    /// Create new button state
    pub fn new() -> Self {
        Self {
            hovered: None,
            pressed: None,
            focused: None,
        }
    }
}

impl ButtonState for SimpleButtonState {
    fn is_hovered(&self, button_id: &str) -> bool {
        self.hovered.as_deref() == Some(button_id)
    }

    fn is_pressed(&self, button_id: &str) -> bool {
        self.pressed.as_deref() == Some(button_id)
    }

    fn is_focused(&self, button_id: &str) -> bool {
        self.focused.as_deref() == Some(button_id)
    }

    fn set_hovered(&mut self, button_id: Option<&str>) {
        self.hovered = button_id.map(|s| s.to_string());
    }

    fn set_pressed(&mut self, button_id: Option<&str>) {
        self.pressed = button_id.map(|s| s.to_string());
    }

    fn set_focused(&mut self, button_id: Option<&str>) {
        self.focused = button_id.map(|s| s.to_string());
    }
}
