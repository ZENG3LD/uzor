//! Modal composite widget — full-screen overlay that blocks interaction behind it.
//!
//! ## API convention
//!
//! - `register_input_coordinator_modal` — registers the composite + child
//!   hit-rects with an `InputCoordinator`.  No drawing.  Use when you need
//!   explicit z-order control (register multiple composites, draw in order).
//! - `the old L2 register helper`   — convenience wrapper that takes a
//!   `app context`, registers, and draws in one call.
//!
//! ## Usage
//!
//! ```ignore
//! use uzor::ui::widgets::composite::modal::{
//!     register_input_coordinator_modal, the old L2 register helper,
//!     ModalView, ModalState, ModalSettings, ModalRenderKind,
//!     BackdropKind, FooterBtn, FooterBtnStyle, WizardPageInfo,
//!     ModalTheme, DefaultModalStyle, BackgroundFill,
//! };
//! ```

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod tokens;
pub mod types;

#[cfg(test)]
mod tests;

// --- Re-exports ---------------------------------------------------------------

pub use input::{handle_modal_drag, register_input_coordinator_modal};
pub use settings::ModalSettings;
pub use state::ModalState;
pub use style::{BackgroundFill, DefaultModalStyle, ModalStyle};
pub use theme::ModalTheme;
pub use types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView, WizardPageInfo};
