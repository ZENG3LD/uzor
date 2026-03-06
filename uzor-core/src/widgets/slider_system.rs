//! Advanced slider system widget configuration
//!
//! Provides slider system configuration for headless architecture.

use crate::types::{WidgetState, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SliderSystemConfig {
    pub value: f64,
    pub min: f64,
    pub max: f64,
}

impl Default for SliderSystemConfig {
    fn default() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SliderSystemResponse {
    pub changed: bool,
    pub value: f64,
    pub dragging: bool,
    pub state: WidgetState,
    pub rect: Rect,
}

// TODO: Implement headless slider system interaction detection