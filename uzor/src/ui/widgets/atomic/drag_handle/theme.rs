//! Drag handle theme trait and default implementation.

/// Colour slots for the drag handle grip-dots visual.
pub trait DragHandleTheme {
    /// Fill colour of each grip dot.  Default: `#4a4e5a`.
    fn grip_dots_color(&self) -> &str;
}

/// Default drag handle theme.
pub struct DefaultDragHandleTheme;

impl Default for DefaultDragHandleTheme {
    fn default() -> Self {
        Self
    }
}

impl DragHandleTheme for DefaultDragHandleTheme {
    fn grip_dots_color(&self) -> &str {
        "#4a4e5a"
    }
}
