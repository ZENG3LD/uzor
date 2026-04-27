//! Chrome widget — window decoration container with titlebar, tabs, and
//! system buttons (min / max / close).
//!
//! # Design
//!
//! Chrome is a composite widget. Its titlebar buttons are registered as
//! atomic `Button` children via [`register_chrome_button`]. Tabs are
//! registered as independent `ChromeTab` composites (top-level) whose rects
//! fall inside the chrome rect — the coordinator's last-registered-wins rule
//! ensures tabs receive events ahead of the chrome background.
//!
//! This avoids the need for composite-in-composite nesting in the coordinator
//! while still keeping the visual hierarchy correct.

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod types;

pub use input::*;
pub use render::*;
pub use settings::*;
pub use state::*;
pub use style::*;
pub use theme::*;
pub use types::*;
