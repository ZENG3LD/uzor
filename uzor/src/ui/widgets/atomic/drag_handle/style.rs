//! Drag handle geometry trait and default implementation.

/// Geometry parameters for the `GripDots` drag handle variant.
pub trait DragHandleStyle {
    /// Diameter of each grip dot in pixels.  Default: `3.0`.
    fn grip_dot_size(&self) -> f64;

    /// Gap between adjacent dots in the grid.  Default: `4.0`.
    fn grip_spacing(&self) -> f64;

    /// Number of dots in the grip pattern (rendered as a 2×N grid where
    /// `N = grip_count / 2`, clamped to an even number).  Default: `6`.
    fn grip_count(&self) -> usize;
}

/// Default drag handle style — 6-dot 2×3 grip.
pub struct DefaultDragHandleStyle;

impl Default for DefaultDragHandleStyle {
    fn default() -> Self {
        Self
    }
}

impl DragHandleStyle for DefaultDragHandleStyle {
    fn grip_dot_size(&self) -> f64   { 3.0 }
    fn grip_spacing(&self)  -> f64   { 4.0 }
    fn grip_count(&self)    -> usize { 6 }
}
