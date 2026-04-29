//! BlackboxPanel composite widget — canvas-like panel with externally managed input.
//!
//! Unlike every other composite widget, BlackboxPanel registers ONE rect with the
//! `InputCoordinator` and rejects children.  `InputCoordinator::is_over_ui()` returns
//! `false` when the cursor is over a BlackboxPanel rect — the area behaves as a canvas,
//! not as UI chrome.
//!
//! # Kinds
//!
//! | Kind               | header | border | body |
//! |--------------------|--------|--------|------|
//! | `Default`          | ✗      | ✗      | ✓    |
//! | `WithHeader`       | ✓      | ✗      | ✓    |
//! | `WithBorder`       | ✗      | ✓      | ✓    |
//! | `WithHeaderBorder` | ✓      | ✓      | ✓    |
//! | `Custom`           | —      | —      | —    |
//!
//! # Entry points
//!
//! - `register_input_coordinator_blackbox_panel` — hit-rect registration only
//! - `register_context_manager_blackbox_panel`   — register + draw in one call
//! - `dispatch_blackbox_event`                   — forward events to the caller handler

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
