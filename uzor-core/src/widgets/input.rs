//! Text input widget configuration
//!
//! Provides input configuration and response types for headless architecture.

use crate::types::{WidgetState, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(Default)]
pub struct InputConfig {
    pub value: String,
    pub placeholder: String,
    pub disabled: bool,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputResponse {
    pub changed: bool,
    pub new_value: String,
    pub focused: bool,
    pub hovered: bool,
    pub state: WidgetState,
    pub rect: Rect,
}

// TODO: Implement headless text input interaction detection