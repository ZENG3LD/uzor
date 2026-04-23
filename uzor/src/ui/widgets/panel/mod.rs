//! Panel module - unified panel architecture
//!
//! Provides panel system with 4 types and 12 variants:
//! - Toolbar (4): Top, Bottom, Left, Right
//! - Sidebar (3): Left, Right, Bottom
//! - Modal (4): Search, Settings, Simple, Primitive
//! - Hideable (1): Floating collapsible panel

pub mod types;
pub mod theme;
pub mod state;
pub mod input;

// Re-export main types
pub use types::*;
