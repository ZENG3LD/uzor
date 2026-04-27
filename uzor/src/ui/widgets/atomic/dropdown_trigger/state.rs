//! DropdownTrigger persistent state.

/// Persistent state for a dropdown trigger widget.
///
/// Interaction state (hovered zone, open) is passed frame-by-frame in the
/// view structs.  This placeholder is reserved for future state such as
/// animation timers or focus tracking.
#[derive(Debug, Default, Clone)]
pub struct DropdownTriggerState;
