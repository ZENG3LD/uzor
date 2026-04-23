//! Overlay state adapter - Contract/Connector for overlay interaction state

use std::collections::HashMap;
use std::time::Instant;

/// State adapter for overlay interaction
pub trait OverlayState {
    fn is_visible(&self, overlay_id: &str) -> bool;
    fn position(&self, overlay_id: &str) -> (f64, f64);
    fn opacity(&self, overlay_id: &str) -> f64;
    fn hover_start_time(&self, overlay_id: &str) -> Option<Instant>;
    fn set_visible(&mut self, overlay_id: &str, visible: bool);
    fn set_position(&mut self, overlay_id: &str, pos: (f64, f64));
    fn set_opacity(&mut self, overlay_id: &str, opacity: f64);
    fn set_hover_start_time(&mut self, overlay_id: &str, time: Option<Instant>);
}

/// Simple implementation of OverlayState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimpleOverlayState {
    pub visible: HashMap<String, bool>,
    pub position: HashMap<String, (f64, f64)>,
    pub opacity: HashMap<String, f64>,
    pub hover_start: HashMap<String, Option<Instant>>,
}

impl SimpleOverlayState {
    pub fn new() -> Self {
        Self {
            visible: HashMap::new(),
            position: HashMap::new(),
            opacity: HashMap::new(),
            hover_start: HashMap::new(),
        }
    }
}

impl OverlayState for SimpleOverlayState {
    fn is_visible(&self, overlay_id: &str) -> bool {
        *self.visible.get(overlay_id).unwrap_or(&false)
    }

    fn position(&self, overlay_id: &str) -> (f64, f64) {
        *self.position.get(overlay_id).unwrap_or(&(0.0, 0.0))
    }

    fn opacity(&self, overlay_id: &str) -> f64 {
        *self.opacity.get(overlay_id).unwrap_or(&0.0)
    }

    fn hover_start_time(&self, overlay_id: &str) -> Option<Instant> {
        self.hover_start.get(overlay_id).copied().flatten()
    }

    fn set_visible(&mut self, overlay_id: &str, visible: bool) {
        self.visible.insert(overlay_id.to_string(), visible);
    }

    fn set_position(&mut self, overlay_id: &str, pos: (f64, f64)) {
        self.position.insert(overlay_id.to_string(), pos);
    }

    fn set_opacity(&mut self, overlay_id: &str, opacity: f64) {
        self.opacity.insert(overlay_id.to_string(), opacity);
    }

    fn set_hover_start_time(&mut self, overlay_id: &str, time: Option<Instant>) {
        self.hover_start.insert(overlay_id.to_string(), time);
    }
}
