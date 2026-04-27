//! Composite widgets — parents that own children.
//!
//! See `WidgetKind::is_composite()` for the full list. Each module owns
//! the widget's data, theme, style, state, render math, and input
//! registration.

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
