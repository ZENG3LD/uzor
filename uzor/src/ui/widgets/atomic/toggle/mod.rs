//! Toggle widget — iOS-style switch, wide switch, and icon-swap variants.
//!
//! Self-contained:
//! - `types`    — `ToggleView`, `ToggleConfig`, `ToggleRenderKind`.
//! - `state`    — `ToggleState` (toggled flag).
//! - `theme`    — `ToggleTheme` trait + `DefaultToggleTheme`.
//! - `style`    — `ToggleSwitchStyle` trait + `IndicatorToggleStyle` / `SignalsToggleStyle`
//!                + `ToggleIconStyle` trait + `DefaultToggleIconStyle`.
//! - `settings` — `ToggleSettings` bundle.
//! - `render`   — `draw_toggle` dispatcher.
//! - `input`    — `register_toggle` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{ToggleConfig, ToggleRenderKind, ToggleView};
pub use state::ToggleState;
pub use theme::{DefaultToggleTheme, ToggleTheme};
pub use style::{
    DefaultToggleIconStyle, IndicatorToggleStyle, SignalsToggleStyle,
    ToggleIconStyle, ToggleSwitchStyle,
};
pub use settings::ToggleSettings;
pub use render::draw_toggle;
pub use input::register_toggle;
