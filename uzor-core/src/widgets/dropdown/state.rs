//! Dropdown state adapter - Contract/Connector for dropdown interaction state

use std::collections::HashMap;

/// State adapter for dropdown interaction
pub trait DropdownState {
    fn is_open(&self, dropdown_id: &str) -> bool;
    fn is_item_hovered(&self, dropdown_id: &str, item_index: usize) -> bool;
    fn set_open(&mut self, dropdown_id: &str, open: bool);
    fn set_item_hovered(&mut self, dropdown_id: &str, item_index: Option<usize>);
}

/// Simple implementation of DropdownState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimpleDropdownState {
    pub open_state: HashMap<String, bool>,
    pub hovered_items: HashMap<String, Option<usize>>,
}

impl SimpleDropdownState {
    pub fn new() -> Self {
        Self {
            open_state: HashMap::new(),
            hovered_items: HashMap::new(),
        }
    }

    pub fn close_all(&mut self) {
        for (_, open) in self.open_state.iter_mut() {
            *open = false;
        }
    }

    pub fn clear_all_hovers(&mut self) {
        for (_, hover) in self.hovered_items.iter_mut() {
            *hover = None;
        }
    }
}

impl DropdownState for SimpleDropdownState {
    fn is_open(&self, dropdown_id: &str) -> bool {
        self.open_state.get(dropdown_id).copied().unwrap_or(false)
    }

    fn is_item_hovered(&self, dropdown_id: &str, item_index: usize) -> bool {
        self.hovered_items
            .get(dropdown_id)
            .map(|idx| *idx == Some(item_index))
            .unwrap_or(false)
    }

    fn set_open(&mut self, dropdown_id: &str, open: bool) {
        self.open_state.insert(dropdown_id.to_string(), open);
    }

    fn set_item_hovered(&mut self, dropdown_id: &str, item_index: Option<usize>) {
        self.hovered_items.insert(dropdown_id.to_string(), item_index);
    }
}
