//! Popup composite widget — transient floating panel anchored to a trigger.
//!
//! ## API convention
//!
//! - `register_input_coordinator_popup` — registers the composite + child
//!   hit-rects with an `InputCoordinator`. No drawing. Use for explicit
//!   z-order control.
//! - `register_context_manager_popup`   — convenience wrapper: takes a
//!   `ContextManager`, registers, and draws in one call.
//!
//! ## Templates (`PopupRenderKind`)
//!
//! | Kind | Chrome | Body |
//! |------|--------|------|
//! | `Plain` | frame + shadow | caller-drawn into `body_rect()` |
//! | `Custom` | none | caller drives every paint call |
//!
//! Anything more elaborate (color pickers, swatch grids, indicator strips,
//! item lists) is composed by the caller inside a `Plain` popup using
//! atomic widgets. The composite is intentionally minimal.

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod types;

// --- Re-exports ---------------------------------------------------------------

pub use input::{handle_popup_dismiss, register_input_coordinator_popup, register_layout_manager_popup};
pub use render::register_context_manager_popup;
pub use settings::PopupSettings;
pub use state::PopupState;
pub use style::{BackgroundFill, DefaultPopupStyle, PopupStyle};
pub use theme::{DefaultPopupTheme, PopupTheme};
pub use types::{BackdropKind, PopupRenderKind, PopupView, PopupViewKind};
