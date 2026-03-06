//! Persistent widget state registry
//!
//! Stores behavioral state (scroll offsets, focus, etc.) that must persist across frames.

use std::any::Any;
use std::collections::HashMap;
use crate::types::state::WidgetId;

/// Central registry for persistent widget data
#[derive(Default)]
pub struct StateRegistry {
    /// Generic data storage by widget ID
    /// Stores Any to allow different widgets to have different state structures
    states: HashMap<WidgetId, Box<dyn Any + Send + Sync>>,
}

impl StateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get reference to state of type T for a widget
    pub fn get<T: 'static>(&self, id: &WidgetId) -> Option<&T> {
        self.states.get(id).and_then(|any| any.downcast_ref::<T>())
    }

    /// Get mutable reference to state or create default if missing
    pub fn get_or_insert_with<T: 'static + Send + Sync, F: FnOnce() -> T>(&mut self, id: WidgetId, default: F) -> &mut T {
        let entry = self.states.entry(id).or_insert_with(|| Box::new(default()));
        entry.downcast_mut::<T>().expect("State type mismatch for WidgetId")
    }

    /// Insert or update state
    pub fn insert<T: 'static + Send + Sync>(&mut self, id: WidgetId, state: T) {
        self.states.insert(id, Box::new(state));
    }

    /// Remove state for a widget
    pub fn remove(&mut self, id: &WidgetId) {
        self.states.remove(id);
    }

    /// Clear all states
    pub fn clear(&mut self) {
        self.states.clear();
    }
}
