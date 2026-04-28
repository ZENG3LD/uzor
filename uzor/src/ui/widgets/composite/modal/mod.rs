//! Modal composite widget — full-screen overlay that blocks interaction behind it.
//!
//! ## Usage
//!
//! ```ignore
//! use uzor::ui::widgets::composite::modal::{
//!     draw_modal, register_modal,
//!     ModalView, ModalState, ModalSettings, ModalRenderKind,
//!     BackdropKind, FooterBtn, FooterBtnStyle, WizardPageInfo,
//!     DefaultModalTheme, DefaultModalStyle,
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

pub use input::{handle_modal_drag, register_modal};
pub use render::draw_modal;
pub use settings::ModalSettings;
pub use state::ModalState;
pub use style::{DefaultModalStyle, ModalStyle};
pub use theme::{DefaultModalTheme, ModalTheme};
pub use types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView, WizardPageInfo,
};
