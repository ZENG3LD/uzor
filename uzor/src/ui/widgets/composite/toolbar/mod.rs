//! Toolbar composite widget — horizontal strip, vertical column,
//! ChromeStrip titlebar, or inline embedded bar.
//!
//! ## API convention
//!
//! - `register_input_coordinator_toolbar` — registers the composite + child
//!   hit-rects with an `InputCoordinator`.  **No drawing.**  Use when you
//!   need explicit z-order control.
//! - Click consumption is [`consume::consume_event`](consume::consume_event)
//!   and [`consume::drag_outcome_toolbar`](consume::drag_outcome_toolbar).

pub mod consume;
pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod tokens;
pub mod types;

// --- Re-exports ---------------------------------------------------------------

pub use input::{
    handle_toolbar_keyboard, handle_toolbar_overflow_scroll,
    register_input_coordinator_toolbar};
pub use settings::ToolbarSettings;
pub use state::ToolbarState;
pub use style::{
    BackgroundFill, ChromeStripStyle, DefaultToolbarStyle, HorizontalToolbarStyle,
    InlineToolbarStyle, ToolbarStyle, VerticalToolbarStyle};
pub use theme::ToolbarTheme;
pub use types::{
    ChromeStripView, SplitButtonHoverZone, TabConfig, ToolbarItem, ToolbarRenderKind,
    ToolbarSection, ToolbarView};
