//! L1 — direct `InputCoordinator` registration. No drawing.
//!
//! `register_X(coord, ...)` registers the widget hit-zone with the coordinator
//! and returns a `WidgetId` (or composite/atomic typed id).  The caller paints
//! the widget separately.

// Atomics
pub use uzor::ui::widgets::atomic::button::input::register_input_coordinator_button as register_button;
pub use uzor::ui::widgets::atomic::checkbox::input::register_input_coordinator_checkbox as register_checkbox;
pub use uzor::ui::widgets::atomic::chevron::input::register_input_coordinator_chevron as register_chevron;
pub use uzor::ui::widgets::atomic::clock::input::register_input_coordinator_clock as register_clock;
pub use uzor::ui::widgets::atomic::close_button::input::register_input_coordinator_close_button as register_close_button;
pub use uzor::ui::widgets::atomic::color_swatch::input::register_input_coordinator_color_swatch as register_color_swatch;
pub use uzor::ui::widgets::atomic::container::input::register_input_coordinator_container as register_container;
pub use uzor::ui::widgets::atomic::drag_handle::input::register_input_coordinator_drag_handle as register_drag_handle;
pub use uzor::ui::widgets::atomic::dropdown_trigger::input::register_input_coordinator_dropdown_trigger as register_dropdown_trigger;
pub use uzor::ui::widgets::atomic::item::input::register_input_coordinator_item as register_item;
pub use uzor::ui::widgets::atomic::radio::input::register_input_coordinator_radio as register_radio;
pub use uzor::ui::widgets::atomic::scroll_chevron::input::register_input_coordinator_scroll_chevron as register_scroll_chevron;
pub use uzor::ui::widgets::atomic::scrollbar::input::register_input_coordinator_scrollbar as register_scrollbar;
pub use uzor::ui::widgets::atomic::separator::input::register_input_coordinator_separator as register_separator;
pub use uzor::ui::widgets::atomic::shape_selector::input::register_input_coordinator_shape_selector as register_shape_selector;
pub use uzor::ui::widgets::atomic::slider::input::register_input_coordinator_slider as register_slider;
pub use uzor::ui::widgets::atomic::tab::input::register_input_coordinator_tab as register_tab;
pub use uzor::ui::widgets::atomic::text::input::register_input_coordinator_text as register_text;
pub use uzor::ui::widgets::atomic::text_input::input::register_input_coordinator_text_input as register_text_input;
pub use uzor::ui::widgets::atomic::toast::input::register_input_coordinator_toast as register_toast;
pub use uzor::ui::widgets::atomic::toggle::input::register_input_coordinator_toggle as register_toggle;
pub use uzor::ui::widgets::atomic::tooltip::input::register_input_coordinator_tooltip as register_tooltip;

// Composites
pub use uzor::ui::widgets::composite::blackbox_panel::render::register_input_coordinator_blackbox_panel as register_blackbox_panel;
pub use uzor::ui::widgets::composite::chrome::render::register_input_coordinator_chrome as register_chrome;
pub use uzor::ui::widgets::composite::context_menu::render::register_input_coordinator_context_menu as register_context_menu;
pub use uzor::ui::widgets::composite::dropdown::render::register_input_coordinator_dropdown as register_dropdown;
pub use uzor::ui::widgets::composite::modal::render::register_input_coordinator_modal as register_modal;
pub use uzor::ui::widgets::composite::panel::render::register_input_coordinator_panel as register_panel;
pub use uzor::ui::widgets::composite::popup::render::register_input_coordinator_popup as register_popup;
pub use uzor::ui::widgets::composite::sidebar::render::register_input_coordinator_sidebar as register_sidebar;
pub use uzor::ui::widgets::composite::toolbar::render::register_input_coordinator_toolbar as register_toolbar;
