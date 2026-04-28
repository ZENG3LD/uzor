//! Modal composite widget — full-screen overlay that blocks interaction behind it.
//!
//! ## API convention
//!
//! All composite widgets expose three functions:
//!
//! - `register_modal`  — registers the composite + child hit-rects.  No drawing.
//! - `draw_modal`      — pure rendering.  No registration.
//! - `modal`           — convenience wrapper that does both.
//!
//! ## Usage
//!
//! ```ignore
//! use uzor::ui::widgets::composite::modal::{
//!     modal, draw_modal, register_modal,
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

pub use input::handle_modal_drag;
pub use render::{draw_modal, modal, register_modal};
pub use settings::ModalSettings;
pub use state::ModalState;
pub use style::{BackgroundFill, DefaultModalStyle, ModalStyle};
pub use theme::{DefaultModalTheme, ModalTheme};
pub use types::{
    BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView, WizardPageInfo,
};
