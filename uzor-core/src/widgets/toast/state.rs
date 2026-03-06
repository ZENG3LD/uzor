//! Toast state adapter - Contract/Connector for toast notification state

use std::collections::HashMap;

/// State adapter for toast notifications
pub trait ToastState {
    fn is_visible(&self, toast_id: &str) -> bool;
    fn opacity(&self, toast_id: &str) -> f64;
    fn remaining_duration_ms(&self, toast_id: &str) -> u32;
    fn vertical_offset(&self, toast_id: &str) -> f64;
    fn is_hovered(&self, toast_id: &str) -> bool;
    fn set_visible(&mut self, toast_id: &str, visible: bool);
    fn set_opacity(&mut self, toast_id: &str, opacity: f64);
    fn set_remaining_duration_ms(&mut self, toast_id: &str, ms: u32);
    fn set_vertical_offset(&mut self, toast_id: &str, offset: f64);
    fn set_hovered(&mut self, toast_id: &str, hovered: bool);
}

/// Simple implementation of ToastState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimpleToastState {
    pub visibility: HashMap<String, bool>,
    pub opacity: HashMap<String, f64>,
    pub remaining_duration_ms: HashMap<String, u32>,
    pub vertical_offset: HashMap<String, f64>,
    pub hovered: HashMap<String, bool>,
}

impl SimpleToastState {
    pub fn new() -> Self {
        Self {
            visibility: HashMap::new(),
            opacity: HashMap::new(),
            remaining_duration_ms: HashMap::new(),
            vertical_offset: HashMap::new(),
            hovered: HashMap::new(),
        }
    }

    pub fn remove_toast(&mut self, toast_id: &str) {
        self.visibility.remove(toast_id);
        self.opacity.remove(toast_id);
        self.remaining_duration_ms.remove(toast_id);
        self.vertical_offset.remove(toast_id);
        self.hovered.remove(toast_id);
    }
}

impl ToastState for SimpleToastState {
    fn is_visible(&self, toast_id: &str) -> bool {
        self.visibility.get(toast_id).copied().unwrap_or(false)
    }

    fn opacity(&self, toast_id: &str) -> f64 {
        self.opacity.get(toast_id).copied().unwrap_or(0.0)
    }

    fn remaining_duration_ms(&self, toast_id: &str) -> u32 {
        self.remaining_duration_ms.get(toast_id).copied().unwrap_or(0)
    }

    fn vertical_offset(&self, toast_id: &str) -> f64 {
        self.vertical_offset.get(toast_id).copied().unwrap_or(0.0)
    }

    fn is_hovered(&self, toast_id: &str) -> bool {
        self.hovered.get(toast_id).copied().unwrap_or(false)
    }

    fn set_visible(&mut self, toast_id: &str, visible: bool) {
        self.visibility.insert(toast_id.to_string(), visible);
    }

    fn set_opacity(&mut self, toast_id: &str, opacity: f64) {
        self.opacity.insert(toast_id.to_string(), opacity);
    }

    fn set_remaining_duration_ms(&mut self, toast_id: &str, ms: u32) {
        self.remaining_duration_ms.insert(toast_id.to_string(), ms);
    }

    fn set_vertical_offset(&mut self, toast_id: &str, offset: f64) {
        self.vertical_offset.insert(toast_id.to_string(), offset);
    }

    fn set_hovered(&mut self, toast_id: &str, hovered: bool) {
        self.hovered.insert(toast_id.to_string(), hovered);
    }
}
