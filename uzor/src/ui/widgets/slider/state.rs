//! Slider state adapter - Contract/Connector for slider interaction state

use std::collections::HashMap;

/// Which handle of a slider (for state tracking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliderHandle {
    /// Single handle for SliderType::Single
    Single,
    /// Minimum/left handle for SliderType::Dual
    Min,
    /// Maximum/right handle for SliderType::Dual
    Max,
}

/// State adapter for slider interaction
pub trait SliderState {
    fn is_dragging(&self, slider_id: &str, handle: SliderHandle) -> bool;
    fn is_hovered(&self, slider_id: &str, handle: SliderHandle) -> bool;
    fn set_dragging(&mut self, slider_id: &str, handle: SliderHandle, dragging: bool);
    fn set_hovered(&mut self, slider_id: &str, handle: SliderHandle, hovered: bool);
}

/// Simple implementation of SliderState for prototyping
#[derive(Clone, Debug, Default)]
pub struct SimpleSliderState {
    dragging: HashMap<(String, SliderHandle), bool>,
    hovered: HashMap<(String, SliderHandle), bool>,
}

impl SimpleSliderState {
    pub fn new() -> Self {
        Self {
            dragging: HashMap::new(),
            hovered: HashMap::new(),
        }
    }
}

impl SliderState for SimpleSliderState {
    fn is_dragging(&self, slider_id: &str, handle: SliderHandle) -> bool {
        self.dragging
            .get(&(slider_id.to_string(), handle))
            .copied()
            .unwrap_or(false)
    }

    fn is_hovered(&self, slider_id: &str, handle: SliderHandle) -> bool {
        self.hovered
            .get(&(slider_id.to_string(), handle))
            .copied()
            .unwrap_or(false)
    }

    fn set_dragging(&mut self, slider_id: &str, handle: SliderHandle, dragging: bool) {
        if dragging {
            self.dragging.insert((slider_id.to_string(), handle), true);
        } else {
            self.dragging.remove(&(slider_id.to_string(), handle));
        }
    }

    fn set_hovered(&mut self, slider_id: &str, handle: SliderHandle, hovered: bool) {
        if hovered {
            self.hovered.insert((slider_id.to_string(), handle), true);
        } else {
            self.hovered.remove(&(slider_id.to_string(), handle));
        }
    }
}
