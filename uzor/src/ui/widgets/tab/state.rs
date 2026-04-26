//! Tab persistent state.

/// Transient interaction state for one tab.
#[derive(Debug, Clone, Default)]
pub struct TabState {
    /// Pointer is over the tab body.
    pub hovered: bool,
    /// Tab body is pressed (button-down, not yet released).
    pub pressed: bool,
    /// Pointer is specifically over the close button.
    pub close_btn_hovered: bool,
}

impl TabState {
    pub fn new() -> Self {
        Self::default()
    }
}
