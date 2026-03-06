//! Layout calculation helpers for UZOR
//!
//! This crate provides Level 2 utilities: layout calculation without rendering.
//! Use these helpers to quickly position widgets, then render them yourself.
//!
//! # Examples
//!
//! ```
//! use uzor_layout::helpers;
//! use uzor_core::types::rect::Rect;
//!
//! let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
//! let button = helpers::center_rect(screen, 200.0, 50.0);
//! // Now render button at calculated position
//! ```

pub mod helpers;

// Re-export commonly used functions
pub use helpers::{
    center_rect, align_left, align_right, align_top, align_bottom,
    stack_vertical, stack_horizontal, grid_layout, distribute_space,
    aspect_ratio, fit_in_bounds, modal_rect,
};
