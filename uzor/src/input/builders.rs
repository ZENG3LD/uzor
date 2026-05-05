//! L1 — direct `InputCoordinator` registration shortcuts (legacy).
//!
//! `register_X(coord, ...)` registers a widget hit-zone with the
//! coordinator and returns a `WidgetId` (or a composite / atomic
//! typed id).  The caller paints the widget separately through its
//! `*::render` module.
//!
//! ## Status: legacy / low-level
//!
//! L4 framework apps should drive the UI through
//! [`crate::framework::widgets::lm`] (the L3 builder surface).
//! These shortcuts exist for the L1 example
//! (`uzor-examples/src/l1/coord.rs`) and for low-level tooling that
//! talks to `InputCoordinator` directly.  They live under `input::`
//! rather than `framework::widgets::` so app authors aren't tempted
//! to reach for them.

// Atomics
pub use crate::ui::widgets::atomic::button::input::register_input_coordinator_button as register_button;
pub use crate::ui::widgets::atomic::checkbox::input::register_input_coordinator_checkbox as register_checkbox;
pub use crate::ui::widgets::atomic::chevron::input::register_input_coordinator_chevron as register_chevron;
pub use crate::ui::widgets::atomic::clock::input::register_input_coordinator_clock as register_clock;
pub use crate::ui::widgets::atomic::close_button::input::register_input_coordinator_close_button as register_close_button;
pub use crate::ui::widgets::atomic::color_swatch::input::register_input_coordinator_color_swatch as register_color_swatch;
pub use crate::ui::widgets::atomic::container::input::register_input_coordinator_container as register_container;
pub use crate::ui::widgets::atomic::drag_handle::input::register_input_coordinator_drag_handle as register_drag_handle;
pub use crate::ui::widgets::atomic::dropdown_trigger::input::register_input_coordinator_dropdown_trigger as register_dropdown_trigger;
pub use crate::ui::widgets::atomic::item::input::register_input_coordinator_item as register_item;
pub use crate::ui::widgets::atomic::radio::input::register_input_coordinator_radio as register_radio;
pub use crate::ui::widgets::atomic::scroll_chevron::input::register_input_coordinator_scroll_chevron as register_scroll_chevron;
pub use crate::ui::widgets::atomic::scrollbar::input::register_input_coordinator_scrollbar as register_scrollbar;
pub use crate::ui::widgets::atomic::separator::input::register_input_coordinator_separator as register_separator;
pub use crate::ui::widgets::atomic::shape_selector::input::register_input_coordinator_shape_selector as register_shape_selector;
pub use crate::ui::widgets::atomic::slider::input::register_input_coordinator_slider as register_slider;
pub use crate::ui::widgets::atomic::tab::input::register_input_coordinator_tab as register_tab;
pub use crate::ui::widgets::atomic::text::input::register_input_coordinator_text as register_text;
pub use crate::ui::widgets::atomic::text_input::input::register_input_coordinator_text_input as register_text_input;
pub use crate::ui::widgets::atomic::toast::input::register_input_coordinator_toast as register_toast;
pub use crate::ui::widgets::atomic::toggle::input::register_input_coordinator_toggle as register_toggle;
pub use crate::ui::widgets::atomic::tooltip::input::register_input_coordinator_tooltip as register_tooltip;

// Composites
pub use crate::ui::widgets::composite::blackbox_panel::render::register_input_coordinator_blackbox_panel as register_blackbox_panel;
pub use crate::ui::widgets::composite::chrome::render::register_input_coordinator_chrome as register_chrome;
pub use crate::ui::widgets::composite::context_menu::render::register_input_coordinator_context_menu as register_context_menu;
pub use crate::ui::widgets::composite::dropdown::render::register_input_coordinator_dropdown as register_dropdown;
pub use crate::ui::widgets::composite::modal::render::register_input_coordinator_modal as register_modal;
pub use crate::ui::widgets::composite::panel::render::register_input_coordinator_panel as register_panel;
pub use crate::ui::widgets::composite::popup::render::register_input_coordinator_popup as register_popup;
pub use crate::ui::widgets::composite::sidebar::render::register_input_coordinator_sidebar as register_sidebar;
pub use crate::ui::widgets::composite::toolbar::render::register_input_coordinator_toolbar as register_toolbar;
