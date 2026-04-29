//! Sidebar composite widget — collapsible side panel.
//!
//! # Templates (via `SidebarRenderKind`)
//!
//! | Kind               | Description                                      |
//! |--------------------|--------------------------------------------------|
//! | `Right`            | Collapsible right panel; resize edge on the left |
//! | `Left`             | Mirror of Right; resize edge on the right        |
//! | `WithTypeSelector` | Adds a top tab strip to switch panel types       |
//! | `Embedded`         | Minimalist — no resize, no collapse              |
//! | `Custom`           | Caller drives every draw call                    |
//!
//! # Entry points
//!
//! - `register_input_coordinator_sidebar` — hit-rect registration only
//! - `register_context_manager_sidebar`   — register + draw in one call

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
