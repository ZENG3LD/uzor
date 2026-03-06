//! Button module - unified button architecture
//!
//! This module provides a complete button system covering 141 buttons
//! across 6 types and 19 variants.
//!
//! # Five-Level Architecture
//!
//! | Level | Module | Purpose |
//! |-------|--------|---------|
//! | L1 | `types` | Semantic enum catalog (ButtonType, variants) |
//! | L2 | `theme` | Color contract trait (ButtonTheme) |
//! | L3 | `state` | Interaction state trait (ButtonState) |
//! | L4 | `input` | Event handling trait (ButtonInputHandler) |
//! | L5 | `defaults` | Default sizes and prototype colors |

pub mod types;
pub mod theme;
pub mod state;
pub mod input;
pub mod defaults;

pub use types::*;
pub use theme::*;
pub use state::*;
pub use input::*;
