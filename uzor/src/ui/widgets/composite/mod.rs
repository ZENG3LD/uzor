//! Composite widgets — parents that own children.
//!
//! See `WidgetKind::is_composite()` for the full list. Each module owns
//! the widget's data, theme, style, state, render math, and input
//! registration.
//!
//! ## Register / Draw convention
//!
//! Every composite widget exposes three entry points:
//!
//! - `register_<widget>` — registers the composite + all child hit-rects with
//!   the `InputCoordinator`.  **No drawing.**
//! - `draw_<widget>`     — pure rendering.  **No registration.**  The caller
//!   must have called `register_<widget>` beforehand.
//! - `<widget>`          — convenience wrapper that calls both in order.
//!
//! Callers who need to interleave registration and rendering across multiple
//! composites (e.g. for explicit z-order control) should use the split forms.
//! For the common case use the convenience wrapper.

pub mod blackbox_panel;
pub mod chrome;
pub mod chrome_tab;
pub mod context_menu;
pub mod dropdown;
pub mod modal;
pub mod panel;
pub mod popup;
pub mod sidebar;
pub mod toolbar;
