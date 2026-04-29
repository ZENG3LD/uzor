//! Composite widgets — parents that own children.
//!
//! See `WidgetKind::is_composite()` for the full list. Each module owns
//! the widget's data, theme, style, state, render math, and input
//! registration.
//!
//! ## Register / Draw convention
//!
//! Every composite widget exposes two entry points:
//!
//! - `register_input_coordinator_<widget>` — registers the composite + all
//!   child hit-rects with an `InputCoordinator`.  **No drawing.**  Use when
//!   you need explicit z-order control (register multiple composites, then
//!   draw them in order).
//! - `register_context_manager_<widget>`   — convenience wrapper that takes a
//!   `ContextManager`, registers, and draws in one call (passes `coord` to
//!   the body closure so inner widgets can self-register).
//!
//! For the common case use the `register_context_manager_*` form.

pub mod blackbox_panel;
pub mod chrome;
pub mod context_menu;
pub mod dropdown;
pub mod modal;
pub mod panel;
pub mod popup;
pub mod sidebar;
pub mod toolbar;
