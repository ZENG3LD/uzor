//! Checkbox persistent state.

/// Persistent state for a checkbox widget.
#[derive(Debug, Default, Clone)]
pub struct CheckboxState {
    /// Whether the checkbox is currently checked.
    pub checked: bool,
}

impl CheckboxState {
    /// Flip the checked value and return the new state.
    pub fn toggle(&mut self) -> bool {
        self.checked = !self.checked;
        self.checked
    }
}
