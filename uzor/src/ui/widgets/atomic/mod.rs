//! Atomic widgets — leaves of the widget hierarchy.
//!
//! See `WidgetKind::is_atomic()` for the full list. Each module owns the
//! widget's data, theme, style, state, render math, and input registration.

pub mod button;
pub mod checkbox;
pub mod clock;
pub mod close_button;
pub mod drag_handle;
pub mod color_swatch;
pub mod container;
pub mod dropdown_trigger;
pub mod item;
pub mod radio;
pub mod scrollbar;
pub mod scroll_chevron;
pub mod separator;
pub mod shape_selector;
pub mod slider;
pub mod tab;
pub mod text_input;
pub mod toast;
pub mod toggle;
pub mod tooltip;
