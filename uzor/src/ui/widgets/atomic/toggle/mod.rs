//! Toggle widget — iOS-style switch, wide switch, and icon-swap variants.
//!
//! Self-contained:
//! - `types`    — `ToggleView`, `ToggleConfig`, `ToggleRenderKind`.
//! - `state`    — `ToggleState` (toggled flag).
//! - `theme`    — `ToggleTheme` trait.
//! - `tokens`   — `ToggleTokens`: the token-contract implementation of
//!                `ToggleTheme`, built from `SemanticRoles` + optional
//!                `component.toggle.*` overrides (H1 token contract design §3).
//! - `style`    — `ToggleSwitchStyle` trait + `IndicatorToggleStyle` / `SignalsToggleStyle`
//!                + `ToggleIconStyle` trait + `DefaultToggleIconStyle`.
//! - `settings` — `ToggleSettings` bundle.
//! - `render`   — `draw_toggle` dispatcher.
//! - `input`    — `register_toggle` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{ToggleConfig, ToggleRenderKind, ToggleView};
pub use state::ToggleState;
pub use theme::ToggleTheme;
pub use style::{
    DefaultToggleIconStyle, IndicatorToggleStyle, SignalsToggleStyle,
    ToggleIconStyle, ToggleSwitchStyle,
};
pub use settings::ToggleSettings;
pub use render::draw_toggle;
pub use input::{
    register_toggle,
    register_input_coordinator_toggle,
    register_context_manager_toggle,
    register_layout_manager_toggle,
};
