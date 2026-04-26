//! Text input widget — Text / Number / Search / Password.
//!
//! Self-contained:
//! - `types`     — visual variant (`InputType`) + capability declaration.
//! - `behavior`  — validation enum + per-instance config + key/action enums.
//! - `state`     — `TextFieldStore` / `TextFieldState` (text/cursor/selection).
//! - `theme`     — colour palette trait.
//! - `style`     — geometry trait (radius / padding / font size / cursor blink).
//! - `settings`  — bundle of theme + style + behavior config.
//! - `render`    — math: `draw_input`, `draw_input_cursor`,
//!                 `cursor_from_char_positions`.
//! - `input`     — `register` helper for `InputCoordinator`.

pub mod types;
pub mod behavior;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{InputType, TextInputType};
pub use behavior::{
    ConfirmedValue, TextFieldBehavior, TextFieldConfig as TextFieldBehaviorConfig,
    TextInputAction, TextInputKey,
};
pub use state::{
    InputCapability, TextAction, TextFieldConfig, TextFieldState, TextFieldStore,
};
pub use theme::{DefaultTextInputTheme, TextInputTheme};
pub use style::{DefaultTextInputStyle, TextInputStyle};
pub use settings::TextInputSettings;
pub use render::{
    cursor_from_char_positions, draw_input, draw_input_cursor, InputResult, InputView,
};
pub use input::register;
