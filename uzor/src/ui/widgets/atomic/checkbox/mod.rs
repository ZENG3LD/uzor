//! Checkbox widget — Standard, Visibility, LevelVisibility, Notification, Cross, CircleCheck.
//!
//! Self-contained:
//! - `types`    — `CheckboxView`, `CheckboxConfig`, `CheckboxRenderKind`.
//! - `state`    — `CheckboxState` (checked flag).
//! - `theme`    — `CheckboxTheme` trait.
//! - `tokens`   — `CheckboxTokens`: the token-contract implementation of
//!                `CheckboxTheme`, one pre-rendered value per method, built
//!                from `SemanticRoles` + optional `component.checkbox.*`
//!                overrides (H1 token contract design §3). `TokenTheme`
//!                (`crate::tokens::theme`) implements `CheckboxTheme` by
//!                reading these fields.
//! - `style`    — `CheckboxStyle` trait + `Standard/Visibility/LevelVisibility/NotificationCheckboxStyle`.
//! - `settings` — `CheckboxSettings` bundle.
//! - `render`   — `draw_checkbox` dispatcher.
//! - `input`    — `register_checkbox` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{CheckboxConfig, CheckboxRenderKind, CheckboxView};
pub use state::CheckboxState;
pub use theme::CheckboxTheme;
pub use style::{
    CheckboxStyle,
    LevelVisibilityCheckboxStyle, NotificationCheckboxStyle,
    StandardCheckboxStyle, VisibilityCheckboxStyle};
pub use settings::CheckboxSettings;
pub use render::draw_checkbox;
pub use input::{
    register_checkbox,
    register_input_coordinator_checkbox};
