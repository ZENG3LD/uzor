//! Atomic widgets — leaves of the widget hierarchy.
//!
//! See `WidgetKind::is_atomic()` for the full list. Each module owns the
//! widget's data, theme, style, state, render math, and input registration.

pub mod button;
pub mod container;
pub mod scrollbar;
pub mod separator;
pub mod slider;
pub mod tab;
pub mod text_input;
pub mod toast;
pub mod tooltip;
