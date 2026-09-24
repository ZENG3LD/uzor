//! Close button widget — X glyph for modal/panel dismissal.
//!
//! Extracted from `button/render.rs` section 41.
//!
//! Self-contained:
//! - `types`    — `CloseButtonRenderKind`.
//! - `state`    — `CloseButtonState` placeholder.
//! - `theme`    — `CloseButtonTheme` trait.
//! - `tokens`   — `CloseButtonTokens`: the token-contract implementation of
//!                `CloseButtonTheme`, one pre-rendered value per method,
//!                built from `SemanticRoles` + optional
//!                `component.close_button.*` overrides (H1 token contract
//!                design §3). `TokenTheme` (`crate::tokens::theme`)
//!                implements `CloseButtonTheme` by reading these fields.
//! - `style`    — `CloseButtonStyle` trait + `DefaultCloseButtonStyle` / `LargeCloseButtonStyle`.
//! - `settings` — `CloseButtonSettings` bundle.
//! - `render`   — `draw_close_button` dispatcher + `CloseButtonView` + `CloseButtonResult`.
//! - `input`    — `register` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::CloseButtonRenderKind;
pub use state::CloseButtonState;
pub use theme::CloseButtonTheme;
pub use style::{CloseButtonStyle, DefaultCloseButtonStyle, LargeCloseButtonStyle};
pub use settings::CloseButtonSettings;
pub use render::{CloseButtonResult, CloseButtonView, draw_close_button};
pub use input::{
    register,
    register_input_coordinator_close_button,
    register_context_manager_close_button,
    register_layout_manager_close_button,
};
