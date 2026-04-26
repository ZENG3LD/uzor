//! Chrome widget — window decoration container with titlebar, tabs, and
//! system buttons (min / max / close).
//!
//! # Design
//!
//! Chrome is a composite widget. Its titlebar buttons are registered as
//! atomic `Button` children via [`register_chrome_button`]. Tabs are
//! registered as independent `Tab` composites (top-level) whose rects fall
//! inside the chrome rect — the coordinator's last-registered-wins rule
//! ensures tabs receive events ahead of the chrome background.
//!
//! This avoids the need for composite-in-composite nesting in the coordinator
//! while still keeping the visual hierarchy correct.

pub mod types;
pub mod state;
pub mod input;

pub use types::*;
pub use state::*;
pub use input::*;
