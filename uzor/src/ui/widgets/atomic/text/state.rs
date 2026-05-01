//! Persistent text widget state.

/// Text widget has no persistent state beyond per-frame hover (caller-supplied).
#[derive(Default, Debug, Clone)]
pub struct TextState;
