//! ContextMenu widget — right-click / long-press context menu.
//!
//! Positioned at raw cursor coordinates with smart screen-edge clamping.
//! Z-layer 5 — above modals, below colour picker.
//!
//! # Render kinds
//!
//! | Kind    | Icons | Blur | Item height | Min width |
//! |---------|-------|------|-------------|-----------|
//! | Default | yes   | yes  | 32 px       | 180 px    |
//! | Minimal | no    | no   | 28 px       | 160 px    |
//! | Custom  | —     | —    | caller-owns | —         |

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
