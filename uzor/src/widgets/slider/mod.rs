//! Slider widget - Single and dual-point sliders
//!
//! This module contains the new 5-level slider architecture with
//! types, theme, state, and input layers.
//!
//! Note: The old `Slider` layout builder and `SliderConfig`/`SliderResponse`
//! types are in `slider_system.rs` (legacy headless architecture).

pub mod types;
pub mod theme;
pub mod state;
pub mod input;

// Re-export types publicly (used in production)
pub use types::*;
