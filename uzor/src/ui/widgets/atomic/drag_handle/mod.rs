//! Drag handle widget — invisible or grip-dots hit zone for composite drag regions.
//!
//! Composite widgets (e.g. Modal) register one of these over their header so
//! the input coordinator can track drag gestures independently of click events.
//!
//! Self-contained:
//! - `types`    — `DragHandleView`, `DragHandleRenderKind`.
//! - `state`    — `DragHandleState` with `start`, `update`, `end`, `is_active`.
//! - `theme`    — `DragHandleTheme` trait.
//! - `tokens`   — `DragHandleTokens`: the token-contract implementation of
//!                `DragHandleTheme`, one pre-rendered value per method,
//!                built from `SemanticRoles` + optional
//!                `component.drag_handle.*` overrides (H1 token contract
//!                design §3). `TokenTheme` (`crate::tokens::theme`)
//!                implements `DragHandleTheme` by reading these fields.
//! - `style`    — `DragHandleStyle` trait + `DefaultDragHandleStyle`.
//! - `settings` — `DragHandleSettings` bundle.
//! - `render`   — `draw_drag_handle` dispatcher.
//! - `input`    — `register_drag_handle` helper.

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod tokens;
pub mod types;

pub use input::{
    register_drag_handle,
    register_input_coordinator_drag_handle};
pub use render::draw_drag_handle;
pub use settings::DragHandleSettings;
pub use state::DragHandleState;
pub use style::{DefaultDragHandleStyle, DragHandleStyle};
pub use theme::DragHandleTheme;
pub use types::{DragHandleRenderKind, DragHandleView};
