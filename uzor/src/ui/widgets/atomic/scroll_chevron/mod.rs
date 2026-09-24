//! Scroll chevron widget — directional triangle for toolbar overflow navigation.
//!
//! Extracted from `button/render.rs` section 42.
//!
//! Direction (Up/Down/Left/Right) lives on `ScrollChevronView` as per-instance
//! data, not as a `ScrollChevronRenderKind`. `ChevronDirection` is re-exported
//! from `button::types` where the canonical definition lives.
//!
//! Self-contained:
//! - `types`    — `ScrollChevronRenderKind`.
//! - `state`    — `ScrollChevronState` placeholder.
//! - `theme`    — `ScrollChevronTheme` trait.
//! - `tokens`   — `ScrollChevronTokens`: the token-contract implementation of
//!                `ScrollChevronTheme`, one pre-rendered value per method,
//!                built from `SemanticRoles` + optional
//!                `component.scroll_chevron.*` overrides (H1 token contract
//!                design §3). `TokenTheme` (`crate::tokens::theme`)
//!                implements `ScrollChevronTheme` by reading these fields.
//! - `style`    — `ScrollChevronStyle` trait + `DefaultScrollChevronStyle`.
//! - `settings` — `ScrollChevronSettings` bundle.
//! - `render`   — `draw_scroll_chevron` dispatcher + `ScrollChevronView` + `ScrollChevronResult`.
//!                `ChevronDirection` re-exported from `button::types`.
//! - `input`    — `register` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::ScrollChevronRenderKind;
pub use state::ScrollChevronState;
pub use theme::ScrollChevronTheme;
pub use style::{DefaultScrollChevronStyle, ScrollChevronStyle};
pub use settings::ScrollChevronSettings;
pub use render::{ChevronDirection, ScrollChevronResult, ScrollChevronView, draw_scroll_chevron};
pub use input::{
    register,
    register_input_coordinator_scroll_chevron,
    register_context_manager_scroll_chevron,
    register_layout_manager_scroll_chevron,
};
