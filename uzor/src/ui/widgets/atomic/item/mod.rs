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
//! - `theme`    — `ItemTheme` trait + `DefaultItemTheme` + `ToolbarItemTheme`.
//! - `style`    — `ItemStyle` trait + `DefaultItemStyle` + `ToolbarItemStyle`.
//! - `settings` — `ItemSettings` bundle.
//! - `render`   — `draw_item` dispatcher + `ItemView`.
//! - `input`    — `register` helper (Sense::NONE).

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::ItemRenderKind;
pub use state::ItemState;
pub use theme::{DefaultItemTheme, ItemTheme, ToolbarItemTheme};
pub use style::{DefaultItemStyle, ItemStyle, ToolbarItemStyle};
pub use settings::ItemSettings;
pub use render::{ItemView, draw_item};
pub use input::register;
