//! BlackboxPanel persistent state.
//!
//! `BlackboxState` is informational — uzor does not mutate it.
//! Callers update fields in their `handle_event` closure or after calling
//! `dispatch_blackbox_event`.

// ---------------------------------------------------------------------------
// BlackboxState
// ---------------------------------------------------------------------------

/// Suggested per-instance state for callers that want consistent field naming.
///
/// uzor does not read or write these fields; the caller owns the data.
#[derive(Debug, Clone)]
pub struct BlackboxState {
    /// Pointer is inside the panel rect this frame.
    pub hovered: bool,

    /// A drag gesture is currently active.
    pub dragging: bool,

    /// Panel has keyboard focus.
    pub has_focus: bool,

    /// Last known pointer position in panel-local coordinates (px).
    pub last_pointer_pos: (f64, f64),

    /// Pointer is hovering over the close-X button in the header strip.
    ///
    /// Close-X hit detection is handled by the caller (coordinate comparison).
    pub hovered_close: bool,
}

impl Default for BlackboxState {
    fn default() -> Self {
        Self {
            hovered:          false,
            dragging:         false,
            has_focus:        false,
            last_pointer_pos: (0.0, 0.0),
            hovered_close:    false,
        }
    }
}
