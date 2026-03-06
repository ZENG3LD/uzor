//! Panel state adapter - Contract/Connector for panel interaction state

use std::collections::HashMap;

/// State adapter for panel interaction
pub trait PanelState {
    fn is_open(&self, panel_id: &str) -> bool;
    fn is_hidden(&self, panel_id: &str) -> bool;
    fn is_dragging(&self, panel_id: &str) -> bool;
    fn size(&self, panel_id: &str) -> f64;
    fn set_open(&mut self, panel_id: &str, open: bool);
    fn set_hidden(&mut self, panel_id: &str, hidden: bool);
    fn set_dragging(&mut self, panel_id: &str, dragging: bool);
    fn set_size(&mut self, panel_id: &str, size: f64);
}

/// Simple implementation of PanelState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimplePanelState {
    pub open: HashMap<String, bool>,
    pub hidden: HashMap<String, bool>,
    pub dragging: Option<String>,
    pub sizes: HashMap<String, f64>,
}

impl SimplePanelState {
    pub fn new() -> Self {
        Self {
            open: HashMap::new(),
            hidden: HashMap::new(),
            dragging: None,
            sizes: HashMap::new(),
        }
    }

    pub fn with_open(panel_ids: &[&str]) -> Self {
        let mut state = Self::new();
        for id in panel_ids {
            state.open.insert((*id).to_string(), true);
        }
        state
    }
}

impl PanelState for SimplePanelState {
    fn is_open(&self, panel_id: &str) -> bool {
        *self.open.get(panel_id).unwrap_or(&false)
    }

    fn is_hidden(&self, panel_id: &str) -> bool {
        *self.hidden.get(panel_id).unwrap_or(&false)
    }

    fn is_dragging(&self, panel_id: &str) -> bool {
        self.dragging.as_deref() == Some(panel_id)
    }

    fn size(&self, panel_id: &str) -> f64 {
        *self.sizes.get(panel_id).unwrap_or(&250.0)
    }

    fn set_open(&mut self, panel_id: &str, open: bool) {
        self.open.insert(panel_id.to_string(), open);
    }

    fn set_hidden(&mut self, panel_id: &str, hidden: bool) {
        self.hidden.insert(panel_id.to_string(), hidden);
    }

    fn set_dragging(&mut self, panel_id: &str, dragging: bool) {
        if dragging {
            self.dragging = Some(panel_id.to_string());
        } else if self.dragging.as_deref() == Some(panel_id) {
            self.dragging = None;
        }
    }

    fn set_size(&mut self, panel_id: &str, size: f64) {
        self.sizes.insert(panel_id.to_string(), size);
    }
}
