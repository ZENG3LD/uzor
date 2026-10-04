//! Modal composite widget — full-screen overlay that blocks interaction behind it.
//!
//! Hit registration is `input::register_input_coordinator_modal`.
//! Click consumption is [`consume::consume_event`](consume::consume_event)
//! and [`consume::drag_outcome_modal`](consume::drag_outcome_modal)
//! (also re-exported from `input`).

pub mod consume;
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
