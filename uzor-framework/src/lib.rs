//! uzor-framework — end-to-end app runner crate.
//!
//! Builds on top of `uzor` (headless input/widgets) by providing a
//! complete window/event/render runtime so consumer apps only need to
//! implement business logic.
//!
//! Status: under construction. Currently provides only platform helpers
//! lifted from mylittlechart's production code.

pub mod platform;
pub mod single_instance;
pub mod screenshot;
pub mod window;

pub use single_instance::{single_instance, SingleInstanceGuard};

/// Render backend hub (re-export from uzor-render-hub).
pub use uzor_render_hub as render_hub;
