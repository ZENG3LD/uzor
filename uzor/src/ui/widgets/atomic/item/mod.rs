//! Item widget — non-interactive label / icon / icon+text for lists and menus.
//!
//! Generalized from `button/render.rs` `draw_toolbar_label` / `LabelView`.
//!
//! Sense: NONE — item widget has no interaction behavior.
//!
//! `WidgetKind::Item` already existed in `widget_kind.rs` — this module
//! provides the full 8-file implementation.
//!
//! Self-contained:
//! - `types`    — `ItemRenderKind` (Label, Icon, TextIcon, Svg, Custom).
//! - `state`    — `ItemState` placeholder.
//! - `theme`    — `ItemTheme` trait.
//! - `tokens`   — `ItemTokens`: the token-contract implementation of
//!                `ItemTheme`, one pre-rendered value per required method,
//!                built from `SemanticRoles` + optional `component.item.*`
//!                overrides (H1 token contract design §3). `TokenTheme`
//!                (`crate::tokens::theme`) implements `ItemTheme` by reading
//!                these fields.
//! - `style`    — `ItemStyle` trait + `DefaultItemStyle` + `ToolbarItemStyle`.
//! - `settings` — `ItemSettings` bundle.
//! - `render`   — `draw_item` dispatcher + `ItemView`.
//! - `input`    — `register` helper (Sense::NONE).

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::ItemRenderKind;
pub use state::ItemState;
pub use theme::ItemTheme;
pub use style::{DefaultItemStyle, ItemStyle, ToolbarItemStyle};
pub use settings::ItemSettings;
pub use render::{ItemView, draw_item};
pub use input::{
    register,
    register_input_coordinator_item};
