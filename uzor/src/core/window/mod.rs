//! Window management abstractions
//!
//! Platform-agnostic traits and types for window control:
//! chrome, borders, drag, resize, minimize, maximize, fullscreen.
//! Backends (desktop, web, mobile) implement these traits.

pub mod types;
pub mod traits;

pub use types::*;
pub use traits::*;
