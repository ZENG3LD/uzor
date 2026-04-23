//! Popup module - unified popup architecture
//!
//! This module provides a complete popup system covering context menus,
//! color pickers, and custom popup dialogs.
//!
//! # Five-Level Architecture
//!
//! | Level | Module | Purpose |
//! |-------|--------|---------|
//! | L1 | `types` | Semantic enum catalog (PopupType) |
//! | L2 | `theme` | Color/dimension contract trait (PopupTheme) |
//! | L3 | `state` | Interaction state trait (PopupState) |
//! | L4 | `input` | Event handling trait (PopupInputHandler) |
//! | L5 | `defaults` | Default sizes (placeholder) |

pub mod types;
pub mod theme;
pub mod state;
pub mod input;
pub mod defaults;

// Re-export main types (used in production terminal code)
pub use types::*;
