//! Clock widget — toolbar time display with hover background.
//!
//! Extracted from `button/render.rs` section 9 (`draw_toolbar_clock`).
//!
//! The clock widget does NOT read or manage system time. The caller
//! supplies a pre-formatted `time_text: &str` via `ClockView` each frame.
//!
//! Sense: HOVER only (mlc has hover-only behavior; no click action).
//!
//! Self-contained:
//! - `types`    — `ClockRenderKind`.
//! - `state`    — `ClockState` placeholder.
//! - `theme`    — `ClockTheme` trait + `DefaultClockTheme`.
//! - `style`    — `ClockStyle` trait + `DefaultClockStyle`.
//! - `settings` — `ClockSettings` bundle.
//! - `render`   — `draw_clock` dispatcher + `ClockView`.
//! - `input`    — `register` helper (HOVER sense).

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::ClockRenderKind;
pub use state::ClockState;
pub use theme::{ClockTheme, DefaultClockTheme};
pub use style::{ClockStyle, DefaultClockStyle};
pub use settings::ClockSettings;
pub use render::{ClockView, draw_clock};
pub use input::{
    register,
    register_input_coordinator_clock,
    register_context_manager_clock,
};
