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
//! | `Plain` | frame + shadow | body closure |
//! | `ColorPickerGrid` | frame + shadow + blur | swatch grid + opacity |
//! | `ColorPickerHsv` | frame + shadow + blur | SV + hue + hex + opacity + actions |
//! | `SwatchGrid` | frame + shadow | preset grid + custom + remove |
//! | `ItemList` | frame + shadow | vertical item list |
//! | `IndicatorStrip` | alpha fill only | per-indicator action rows |
//! | `Custom` | none | caller-provided draw closure |
//!
//! ## Usage
//!
//! ```ignore
//! use uzor::ui::widgets::composite::popup::{
//!     register_context_manager_popup,
//!     PopupView, PopupViewKind, PopupState, PopupSettings,
//!     PopupRenderKind, BackdropKind, ColorPickerLevel,
//!     DefaultPopupTheme, DefaultPopupStyle, BackgroundFill,
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

pub use input::{handle_popup_dismiss, register_input_coordinator_popup};
pub use render::register_context_manager_popup;
pub use settings::PopupSettings;
pub use state::PopupState;
pub use style::{BackgroundFill, DefaultPopupStyle, PopupStyle};
pub use theme::{DefaultPopupTheme, PopupTheme};
pub use types::{
    BackdropKind, ColorPickerLevel, DropdownItem, HsvColor, IndicatorRowInfo,
    PopupRenderKind, PopupView, PopupViewKind,
};
