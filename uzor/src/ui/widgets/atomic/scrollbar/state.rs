//! Scrollbar drag state.

use crate::types::WidgetId;

#[derive(Debug, Default, Clone)]
pub struct ScrollbarDragState {
    pub field_id: Option<WidgetId>,
    /// Pixel offset from the thumb top to the cursor at drag start.
    pub thumb_grab_offset: f64,
    /// Last opacity value, for overlay fade-in/out animation.
    pub overlay_opacity: f64,
}

impl ScrollbarDragState {
    pub fn is_active(&self) -> bool {
        self.field_id.is_some()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
