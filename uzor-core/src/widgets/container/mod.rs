//! Container module - unified scrollable container architecture
//!
//! This module provides a complete container system covering scrollable panels,
//! chat areas, order lists, and other content regions.
//!
//! # Five-Level Architecture
//!
//! | Level | Module | Purpose |
//! |-------|--------|---------|
//! | L1 | `types` | Semantic enum catalog (ContainerType) |
//! | L2 | `theme` | Color/dimension contract trait (ContainerTheme) |
//! | L3 | `state` | Scroll state trait (ContainerState) |
//! | L4 | `input` | Scroll calculation trait (ContainerInputHandler) |
//! | L5 | `defaults` | Default sizes (placeholder) |

pub mod types;
pub mod theme;
pub mod state;
pub mod input;
pub mod defaults;

// Re-export main types (used in production terminal code)
pub use types::*;
