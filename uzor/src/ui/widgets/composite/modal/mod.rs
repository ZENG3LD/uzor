//! Modal composite widget — full-screen overlay that blocks interaction behind it.
//!
//! ## API convention
//!
//! - `register_input_coordinator_modal` — registers the composite + child
//!   hit-rects with an `InputCoordinator`.  No drawing.  Use when you need
//!   explicit z-order control (register multiple composites, draw in order).
//! - `register_context_manager_modal`   — convenience wrapper that takes a
//!   `ContextManager`, registers, and draws in one call.
//!
//! ## Usage
//!
//! ```ignore
//! use uzor::ui::widgets::composite::modal::{
//!     register_input_coordinator_modal, register_context_manager_modal,
//!     ModalView, ModalState, ModalSettings, ModalRenderKind,
//!     BackdropKind, FooterBtn, FooterBtnStyle, WizardPageInfo,
//!     DefaultModalTheme, DefaultModalStyle, BackgroundFill,
//! };
//! ```

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod types;

// --- Re-exports ---------------------------------------------------------------

pub use input::{handle_modal_drag, register_input_coordinator_modal};
pub use render::register_context_manager_modal;
pub use settings::ModalSettings;
pub use state::ModalState;
pub use style::{BackgroundFill, DefaultModalStyle, ModalStyle};
pub use theme::{DefaultModalTheme, ModalTheme};
pub use types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView, WizardPageInfo,
};
