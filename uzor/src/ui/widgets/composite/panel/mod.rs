//! Panel composite widget — generic data-table / content card.
//!
//! # Templates (via `PanelRenderKind`)
//!
//! | Kind                      | header | col-header | body | footer | scrollbar |
//! |---------------------------|--------|------------|------|--------|-----------|
//! | `Plain`                   | ✗      | ✗          | ✓    | ✗      | opt       |
//! | `WithHeader`              | ✓      | ✗          | ✓    | ✗      | opt       |
//! | `WithHeaderColumns`       | ✓      | ✓          | ✓    | ✗      | opt       |
//! | `WithFooter`              | ✓      | ✗          | ✓    | ✓      | opt       |
//! | `WithHeaderColumnsFooter` | ✓      | ✓          | ✓    | ✓      | opt       |
//! | `Custom`                  | —      | —          | —    | —      | —         |
//!
//! # Entry points
//!
//! - `register_input_coordinator_panel` — hit-rect registration only
//! - `register_context_manager_panel`   — register + draw in one call

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
