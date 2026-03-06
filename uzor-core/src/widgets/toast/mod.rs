//! Toast module - system notification architecture
//!
//! Provides toast notification system with 4 variants:
//! Info, Success, Warning, Error.

pub mod types;
pub mod theme;
pub mod state;
pub mod input;

// Re-export main types
pub use types::*;
