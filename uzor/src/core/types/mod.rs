//! Core types for widgets
//!
//! This module contains fundamental types used throughout the UI system:
//! - Rectangles and layout primitives
//! - Widget state tracking (focus, hover, drag)

pub mod icon;
pub mod rect;
pub mod state;

// Re-export all types at the module level
pub use icon::IconId;
pub use rect::*;
pub use state::*;
