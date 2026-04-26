//! Separator drag state (for resize-handle variant).

use crate::types::WidgetId;

#[derive(Debug, Default, Clone)]
pub struct SeparatorDragState {
    pub field_id: Option<WidgetId>,
    pub start_pos: f64,
    pub start_value: f64,
}

impl SeparatorDragState {
    pub fn is_active(&self) -> bool {
        self.field_id.is_some()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
