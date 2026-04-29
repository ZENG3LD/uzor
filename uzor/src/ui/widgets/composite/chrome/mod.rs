//! Chrome widget — window decoration container with titlebar, tabs, and
//! system buttons (min / max / close).
//!
//! # Design
//!
//! Chrome is a composite widget. Its titlebar buttons are registered as atomic
//! `Button` children via the coordinator. Tabs are registered as `Button`
//! children as well; the coordinator's last-registered-wins rule ensures tabs
//! receive events ahead of the chrome background.
//!
//! # Render kinds
//!
//! | Kind                | tabs | drag | menu | win-controls |
//! |---------------------|------|------|------|--------------|
//! | `Default`           | ✓    | ✓    | ✓    | ✓            |
//! | `Minimal`           | ✓    | ✓    | ✓    | ✗            |
//! | `WindowControlsOnly`| ✗    | ✓    | ✗    | ✓            |
//! | `Custom`            | caller manages everything            |

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
