//! ColorSwatch widget — color fill square, indicator swatch, appearance swatch,
//! primitive swatch, and fill-toggle variants (sections 27-31).
//!
//! Self-contained:
//! - `types`    — `ColorSwatchView`, `FillToggleView`, `ColorSwatchRenderKind`.
//! - `state`    — `ColorSwatchState` placeholder.
//! - `theme`    — `ColorSwatchTheme` trait.
//! - `tokens`   — `ColorSwatchTokens`: the token-contract implementation of
//!                `ColorSwatchTheme`, one pre-rendered value per method,
//!                built from `SemanticRoles` + optional
//!                `component.color_swatch.*` overrides (H1 token contract
//!                design §3). `TokenTheme` (`crate::tokens::theme`)
//!                implements `ColorSwatchTheme` by reading these fields.
//! - `style`    — `ColorSwatchStyle` trait + `SimpleSwatchStyle` / `IndicatorSwatchStyle`
//!                / `AppearanceSwatchStyle` / `PrimitiveSwatchStyle`
//!                + `FillToggleStyle` trait + `PrimitiveFillToggleStyle`.
//! - `settings` — `ColorSwatchSettings` bundle.
//! - `render`   — `draw_color_swatch` dispatcher + `draw_fill_toggle`.
//! - `input`    — `register_color_swatch` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{ColorSwatchRenderKind, ColorSwatchView, FillToggleView};
pub use state::ColorSwatchState;
pub use theme::ColorSwatchTheme;
pub use style::{
    AppearanceSwatchStyle, ColorSwatchStyle,
    FillToggleStyle, IndicatorSwatchStyle,
    PrimitiveFillToggleStyle, PrimitiveSwatchStyle,
    SimpleSwatchStyle};
pub use settings::ColorSwatchSettings;
pub use render::{draw_color_swatch, draw_fill_toggle};
pub use input::{
    register_color_swatch,
    register_input_coordinator_color_swatch};
