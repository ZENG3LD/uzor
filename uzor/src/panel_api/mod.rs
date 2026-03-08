//! Panel Application API
//!
//! Defines the contract between the terminal orchestrator and autonomous panel crates.
//! Each panel crate (chart, map, trading-panels, etc.) implements `PanelApp` to become
//! a self-contained application that renders its own content, toolbar, and handles its own input.
//!
//! The terminal orchestrator manages layout, focus, grouping, and system chrome.
//! Panel crates own their rendering, toolbar, and actions.

mod toolbar;
mod types;
mod traits;

pub use toolbar::*;
pub use types::*;
pub use traits::*;
