//! Button widget — Action / Toggle / Checkbox / Tab / ColorSwatch / Dropdown.
//!
//! Self-contained:
//! - `types`     — variant catalog (`ButtonType`, ActionVariant, …) + capabilities.
//! - `defaults`  — per-variant prototype defaults (kept from earlier work).
//! - `state`     — placeholder for future persistent state.
//! - `theme`     — `ButtonTheme` colour trait + `DefaultButtonTheme`.
//! - `style`     — `ButtonStyle` geometry trait + Default/Compact/Flat presets.
//! - `settings`  — `ButtonSettings` bundle (theme + style).
//! - `render`    — `draw_button` math (bg, active border, icon, text).
//! - `input`     — `register` helper for `InputCoordinator`.

pub mod types;
pub mod defaults;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{
    ButtonType, ActionVariant, ToggleVariant, CheckboxVariant, TabVariant,
    ColorSwatchVariant, DropdownVariant, ButtonStyle as ButtonStyleEnum,
    ButtonContent, ChevronDirection,
};
pub use state::ButtonState;
pub use theme::{ButtonTheme, DefaultButtonTheme};
pub use style::{ButtonStyle, DefaultButtonStyle, CompactButtonStyle, FlatButtonStyle};
pub use settings::ButtonSettings;
pub use render::{draw_button, ButtonResult, ButtonView};
pub use input::register;
