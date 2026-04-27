//! Radio persistent state.

/// Persistent state for a radio group widget.
#[derive(Debug, Default, Clone)]
pub struct RadioState {
    /// Index of the currently selected option.
    pub selected_idx: usize,
}

impl RadioState {
    /// Select a specific index.
    pub fn select(&mut self, idx: usize) {
        self.selected_idx = idx;
    }
}
