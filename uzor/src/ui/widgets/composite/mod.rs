//! Composite widgets — parents that own children.
//!
//! See `WidgetKind::is_composite()` for the full list. Each module owns
//! the widget's data, theme, style, state, render math, and input
//! registration.
//!
//! ## Register convention
//!
//! - `register_input_coordinator_<widget>` — registers the composite and its
//!   child hit-rects with an `InputCoordinator`. No drawing.
//! - `consume::consume_event` / `consume::drag_outcome_*` — click consumption
//!   for modal, popup, dropdown, toolbar, and sidebar. Level-1 registration
//!   stays in `input`; those modules re-export the consume items.

pub mod blackbox_panel;
pub mod chrome;
pub mod context_menu;
pub mod dropdown;
pub mod modal;
pub mod overflow;
pub mod panel;
pub mod popup;
pub mod resize_drag;
pub mod sidebar;
pub mod toolbar;

pub use resize_drag::ResizeDrag;
