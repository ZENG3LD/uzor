//! ShapeSelector widget — shape/icon selector, theme preset selector, UI style selector.
//!
//! Covers sections 34, 39, 40 of button-full.md.
//!
//! Self-contained:
//! - `types`    — `ShapeSelectorView`, `ThemePresetView`, `UIStyleView`, `ShapeSelectorRenderKind`.
//! - `state`    — `ShapeSelectorState` placeholder.
//! - `theme`    — `ShapeSelectorTheme` trait + `DefaultShapeSelectorTheme`.
//! - `style`    — `SelectorButtonStyle` trait + `ShapeSelectorStyle`
//!                / `ThemePresetButtonStyle` / `UIStyleSelectorStyle`.
//! - `settings` — `ShapeSelectorSettings` bundle.
//! - `render`   — `draw_shape_selector` dispatcher + `draw_shape_selector_button`
//!                + `draw_theme_preset_button` + `draw_ui_style_button`.
//! - `input`    — `register_shape_selector` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{ShapeSelectorRenderKind, ShapeSelectorView, ThemePresetView, UIStyleView};
pub use state::ShapeSelectorState;
pub use theme::{DefaultShapeSelectorTheme, ShapeSelectorTheme};
pub use style::{
    SelectorButtonStyle, ShapeSelectorStyle,
    ThemePresetButtonStyle, UIStyleSelectorStyle,
};
pub use settings::ShapeSelectorSettings;
pub use render::{
    draw_shape_selector, draw_shape_selector_button,
    draw_theme_preset_button, draw_ui_style_button,
};
pub use input::register_shape_selector;
