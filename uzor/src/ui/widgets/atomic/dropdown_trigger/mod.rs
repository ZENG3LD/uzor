//! DropdownTrigger widget — the clickable trigger button for dropdown menus.
//!
//! NOTE: This is the **trigger only** (atomic leaf). The composite `Dropdown`
//! widget (trigger + open menu + rows) is a separate composite widget.
//! `draw_dropdown_menu_row` / row styles / row theme slots belong in
//! the composite Dropdown widget.
//!
//! Self-contained:
//! - `types`    — `SplitDropdownView`, `DropdownFieldView`, `DropdownTriggerRenderKind`.
//! - `state`    — `DropdownTriggerState` placeholder.
//! - `theme`    — `DropdownTriggerTheme` trait + `DefaultDropdownTriggerTheme`.
//! - `style`    — `SplitDropdownStyle` / `DefaultSplitDropdownStyle`
//!                + `DropdownFieldStyle` / `DefaultDropdownFieldStyle`.
//! - `settings` — `DropdownTriggerSettings` bundle.
//! - `render`   — `draw_dropdown_trigger` dispatcher + `draw_split_dropdown` + `draw_dropdown_field`.
//! - `input`    — `register_dropdown_trigger` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{DropdownFieldView, DropdownTriggerRenderKind, SplitDropdownView};
pub use state::DropdownTriggerState;
pub use theme::{DefaultDropdownTriggerTheme, DropdownTriggerTheme};
pub use style::{
    DefaultDropdownFieldStyle, DefaultSplitDropdownStyle,
    DropdownFieldStyle, SplitDropdownStyle,
};
pub use settings::DropdownTriggerSettings;
pub use render::{draw_dropdown_field, draw_dropdown_trigger, draw_split_dropdown};
pub use input::register_dropdown_trigger;
