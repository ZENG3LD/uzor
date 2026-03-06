//! Cursor icon system for uzor
//!
//! Provides cursor icon types and state management based on CSS cursor specification.
//! Supports priority-based cursor requests to handle overlapping widgets.

/// Priority constants for cursor requests
pub const PRIORITY_DEFAULT: u8 = 0;
pub const PRIORITY_WIDGET: u8 = 100;
pub const PRIORITY_DRAG: u8 = 150;
pub const PRIORITY_MODAL: u8 = 200;
pub const PRIORITY_SYSTEM: u8 = 255;

/// Cursor icon types (based on CSS cursor specification)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CursorIcon {
    /// Default cursor (usually an arrow)
    #[default]
    Default,
    /// No cursor displayed
    None,

    // Links and help
    /// Context menu available
    ContextMenu,
    /// Help information available
    Help,
    /// Pointing hand (typically for links)
    PointingHand,

    // Progress
    /// Progress indicator (work in background)
    Progress,
    /// Wait/busy indicator
    Wait,

    // Selection
    /// Cell or table selection
    Cell,
    /// Crosshair for precise selection
    Crosshair,
    /// Text selection cursor (I-beam)
    Text,
    /// Vertical text selection
    VerticalText,

    // Drag and drop
    /// Alias/shortcut will be created
    Alias,
    /// Copy operation
    Copy,
    /// Move operation
    Move,
    /// Drop not allowed
    NoDrop,
    /// Action not allowed
    NotAllowed,
    /// Grabbable item
    Grab,
    /// Currently grabbing
    Grabbing,

    // Resizing
    /// Omni-directional resize
    AllScroll,
    /// Horizontal resize (↔)
    ResizeHorizontal,
    /// Vertical resize (↕)
    ResizeVertical,
    /// Diagonal resize (↗↙)
    ResizeNeSw,
    /// Diagonal resize (↖↘)
    ResizeNwSe,
    /// Resize east (right)
    ResizeEast,
    /// Resize west (left)
    ResizeWest,
    /// Resize north (up)
    ResizeNorth,
    /// Resize south (down)
    ResizeSouth,
    /// Resize north-east
    ResizeNorthEast,
    /// Resize north-west
    ResizeNorthWest,
    /// Resize south-east
    ResizeSouthEast,
    /// Resize south-west
    ResizeSouthWest,
    /// Column resize
    ResizeColumn,
    /// Row resize
    ResizeRow,

    // Zoom
    /// Zoom in cursor
    ZoomIn,
    /// Zoom out cursor
    ZoomOut,
}

impl CursorIcon {
    /// Returns true if this cursor is a resize variant
    pub fn is_resize(&self) -> bool {
        matches!(
            self,
            CursorIcon::AllScroll
                | CursorIcon::ResizeHorizontal
                | CursorIcon::ResizeVertical
                | CursorIcon::ResizeNeSw
                | CursorIcon::ResizeNwSe
                | CursorIcon::ResizeEast
                | CursorIcon::ResizeWest
                | CursorIcon::ResizeNorth
                | CursorIcon::ResizeSouth
                | CursorIcon::ResizeNorthEast
                | CursorIcon::ResizeNorthWest
                | CursorIcon::ResizeSouthEast
                | CursorIcon::ResizeSouthWest
                | CursorIcon::ResizeColumn
                | CursorIcon::ResizeRow
        )
    }

    /// Returns true if this cursor represents a drag operation
    pub fn is_drag(&self) -> bool {
        matches!(self, CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move)
    }

    /// Returns the CSS cursor name for this icon
    pub fn css_name(&self) -> &'static str {
        match self {
            CursorIcon::Default => "default",
            CursorIcon::None => "none",
            CursorIcon::ContextMenu => "context-menu",
            CursorIcon::Help => "help",
            CursorIcon::PointingHand => "pointer",
            CursorIcon::Progress => "progress",
            CursorIcon::Wait => "wait",
            CursorIcon::Cell => "cell",
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Text => "text",
            CursorIcon::VerticalText => "vertical-text",
            CursorIcon::Alias => "alias",
            CursorIcon::Copy => "copy",
            CursorIcon::Move => "move",
            CursorIcon::NoDrop => "no-drop",
            CursorIcon::NotAllowed => "not-allowed",
            CursorIcon::Grab => "grab",
            CursorIcon::Grabbing => "grabbing",
            CursorIcon::AllScroll => "all-scroll",
            CursorIcon::ResizeHorizontal => "ew-resize",
            CursorIcon::ResizeVertical => "ns-resize",
            CursorIcon::ResizeNeSw => "nesw-resize",
            CursorIcon::ResizeNwSe => "nwse-resize",
            CursorIcon::ResizeEast => "e-resize",
            CursorIcon::ResizeWest => "w-resize",
            CursorIcon::ResizeNorth => "n-resize",
            CursorIcon::ResizeSouth => "s-resize",
            CursorIcon::ResizeNorthEast => "ne-resize",
            CursorIcon::ResizeNorthWest => "nw-resize",
            CursorIcon::ResizeSouthEast => "se-resize",
            CursorIcon::ResizeSouthWest => "sw-resize",
            CursorIcon::ResizeColumn => "col-resize",
            CursorIcon::ResizeRow => "row-resize",
            CursorIcon::ZoomIn => "zoom-in",
            CursorIcon::ZoomOut => "zoom-out",
        }
    }
}

/// Cursor state for the current frame
///
/// Manages cursor icon requests with priority system. Higher priority requests
/// override lower priority ones. Reset at the start of each frame.
#[derive(Clone, Debug)]
pub struct CursorState {
    /// Currently requested cursor icon
    requested: CursorIcon,
    /// Priority level (higher overrides lower)
    priority: u8,
}

impl Default for CursorState {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorState {
    /// Create a new cursor state with default cursor
    pub fn new() -> Self {
        Self {
            requested: CursorIcon::Default,
            priority: PRIORITY_DEFAULT,
        }
    }

    /// Set cursor icon with default widget priority
    ///
    /// Uses `PRIORITY_WIDGET` (100) as the default priority level.
    pub fn set(&mut self, icon: CursorIcon) {
        self.set_with_priority(icon, PRIORITY_WIDGET);
    }

    /// Set cursor icon with explicit priority
    ///
    /// Only updates the cursor if the new priority is greater than or equal
    /// to the current priority. This allows higher priority requests to
    /// override lower priority ones.
    pub fn set_with_priority(&mut self, icon: CursorIcon, priority: u8) {
        if priority >= self.priority {
            self.requested = icon;
            self.priority = priority;
        }
    }

    /// Get the current cursor icon
    pub fn get(&self) -> CursorIcon {
        self.requested
    }

    /// Reset cursor state to default (call at frame start)
    ///
    /// Resets both the cursor icon and priority to default values.
    pub fn reset(&mut self) {
        self.requested = CursorIcon::Default;
        self.priority = PRIORITY_DEFAULT;
    }

    /// Check if cursor is currently set to default
    pub fn is_default(&self) -> bool {
        self.requested == CursorIcon::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_state_new() {
        let state = CursorState::new();
        assert_eq!(state.get(), CursorIcon::Default);
        assert!(state.is_default());
    }

    #[test]
    fn test_cursor_state_set() {
        let mut state = CursorState::new();
        state.set(CursorIcon::PointingHand);
        assert_eq!(state.get(), CursorIcon::PointingHand);
        assert!(!state.is_default());
    }

    #[test]
    fn test_cursor_state_reset() {
        let mut state = CursorState::new();
        state.set(CursorIcon::Grab);
        assert!(!state.is_default());

        state.reset();
        assert_eq!(state.get(), CursorIcon::Default);
        assert!(state.is_default());
    }

    #[test]
    fn test_cursor_priority_override() {
        let mut state = CursorState::new();

        // Set with widget priority
        state.set_with_priority(CursorIcon::PointingHand, PRIORITY_WIDGET);
        assert_eq!(state.get(), CursorIcon::PointingHand);

        // Higher priority overrides
        state.set_with_priority(CursorIcon::Grab, PRIORITY_DRAG);
        assert_eq!(state.get(), CursorIcon::Grab);

        // Lower priority does not override
        state.set_with_priority(CursorIcon::Text, PRIORITY_WIDGET);
        assert_eq!(state.get(), CursorIcon::Grab);

        // Equal priority overrides
        state.set_with_priority(CursorIcon::Grabbing, PRIORITY_DRAG);
        assert_eq!(state.get(), CursorIcon::Grabbing);

        // System priority always wins
        state.set_with_priority(CursorIcon::Wait, PRIORITY_SYSTEM);
        assert_eq!(state.get(), CursorIcon::Wait);
    }

    #[test]
    fn test_cursor_icon_is_resize() {
        assert!(CursorIcon::ResizeHorizontal.is_resize());
        assert!(CursorIcon::ResizeVertical.is_resize());
        assert!(CursorIcon::ResizeNeSw.is_resize());
        assert!(CursorIcon::ResizeNwSe.is_resize());
        assert!(CursorIcon::ResizeEast.is_resize());
        assert!(CursorIcon::ResizeWest.is_resize());
        assert!(CursorIcon::ResizeNorth.is_resize());
        assert!(CursorIcon::ResizeSouth.is_resize());
        assert!(CursorIcon::ResizeNorthEast.is_resize());
        assert!(CursorIcon::ResizeNorthWest.is_resize());
        assert!(CursorIcon::ResizeSouthEast.is_resize());
        assert!(CursorIcon::ResizeSouthWest.is_resize());
        assert!(CursorIcon::ResizeColumn.is_resize());
        assert!(CursorIcon::ResizeRow.is_resize());
        assert!(CursorIcon::AllScroll.is_resize());

        assert!(!CursorIcon::Default.is_resize());
        assert!(!CursorIcon::PointingHand.is_resize());
        assert!(!CursorIcon::Grab.is_resize());
        assert!(!CursorIcon::Text.is_resize());
    }

    #[test]
    fn test_cursor_icon_is_drag() {
        assert!(CursorIcon::Grab.is_drag());
        assert!(CursorIcon::Grabbing.is_drag());
        assert!(CursorIcon::Move.is_drag());

        assert!(!CursorIcon::Default.is_drag());
        assert!(!CursorIcon::PointingHand.is_drag());
        assert!(!CursorIcon::ResizeHorizontal.is_drag());
        assert!(!CursorIcon::Copy.is_drag());
    }

    #[test]
    fn test_cursor_icon_css_name() {
        assert_eq!(CursorIcon::Default.css_name(), "default");
        assert_eq!(CursorIcon::None.css_name(), "none");
        assert_eq!(CursorIcon::PointingHand.css_name(), "pointer");
        assert_eq!(CursorIcon::Grab.css_name(), "grab");
        assert_eq!(CursorIcon::Grabbing.css_name(), "grabbing");
        assert_eq!(CursorIcon::Move.css_name(), "move");
        assert_eq!(CursorIcon::Text.css_name(), "text");
        assert_eq!(CursorIcon::ResizeHorizontal.css_name(), "ew-resize");
        assert_eq!(CursorIcon::ResizeVertical.css_name(), "ns-resize");
        assert_eq!(CursorIcon::ResizeNeSw.css_name(), "nesw-resize");
        assert_eq!(CursorIcon::ResizeNwSe.css_name(), "nwse-resize");
        assert_eq!(CursorIcon::Wait.css_name(), "wait");
        assert_eq!(CursorIcon::Progress.css_name(), "progress");
        assert_eq!(CursorIcon::NotAllowed.css_name(), "not-allowed");
        assert_eq!(CursorIcon::ZoomIn.css_name(), "zoom-in");
        assert_eq!(CursorIcon::ZoomOut.css_name(), "zoom-out");
    }

    #[test]
    fn test_default_priority_constant() {
        let mut state = CursorState::new();
        state.set(CursorIcon::PointingHand);
        assert_eq!(state.priority, PRIORITY_WIDGET);
    }

    #[test]
    fn test_cursor_state_default_trait() {
        let state = CursorState::default();
        assert_eq!(state.get(), CursorIcon::Default);
        assert!(state.is_default());
    }

    #[test]
    fn test_cursor_icon_default_trait() {
        let icon = CursorIcon::default();
        assert_eq!(icon, CursorIcon::Default);
    }
}
