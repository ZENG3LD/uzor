//! Dropdown composite widget — anchor-relative menu panel attached to a trigger.
//!
//! Five render templates:
//! - `Flat`    — single-level item list anchored below trigger
//! - `Inline`  — split-button trigger used inside settings modals
//! - `Grid`    — icon-only N×M cell grid, no labels
//! - `Grouped` — sections with header rows + checkbox list
//! - `Custom`  — caller-supplied draw closure; composite provides frame

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
