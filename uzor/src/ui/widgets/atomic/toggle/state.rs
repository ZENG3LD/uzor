//! Toggle persistent state.

/// Persistent state for a toggle widget.
///
/// The `toggled` flag is the only persistent bit; everything else
/// is per-frame in `ToggleView`.
#[derive(Debug, Default, Clone)]
pub struct ToggleState {
    /// Whether the toggle is currently ON.
    pub toggled: bool,
}

impl ToggleState {
    /// Flip the toggle value and return the new state.
    pub fn flip(&mut self) -> bool {
        self.toggled = !self.toggled;
        self.toggled
    }
}
