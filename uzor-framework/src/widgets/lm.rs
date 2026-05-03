//! L3 — `LayoutManager` declarative API.
//!
//! `build_X(layout, render, ...)` resolves the widget rect from the layout's
//! slot map (overlay / edge / dock leaf), pulls state out of the typed handle,
//! draws + registers in one call.  The framework-app callsite sees only this
//! tier — no `WidgetId::new`, no manual `push_overlay`, no atomic-level
//! plumbing.

// Atomics
pub use uzor::ui::widgets::atomic::button::input::register_layout_manager_button as build_button;
pub use uzor::ui::widgets::atomic::checkbox::input::register_layout_manager_checkbox as build_checkbox;
pub use uzor::ui::widgets::atomic::chevron::input::register_layout_manager_chevron as build_chevron;
pub use uzor::ui::widgets::atomic::clock::input::register_layout_manager_clock as build_clock;
pub use uzor::ui::widgets::atomic::close_button::input::register_layout_manager_close_button as build_close_button;
pub use uzor::ui::widgets::atomic::color_swatch::input::register_layout_manager_color_swatch as build_color_swatch;
pub use uzor::ui::widgets::atomic::container::input::register_layout_manager_container as build_container;
pub use uzor::ui::widgets::atomic::drag_handle::input::register_layout_manager_drag_handle as build_drag_handle;
pub use uzor::ui::widgets::atomic::dropdown_trigger::input::register_layout_manager_dropdown_trigger as build_dropdown_trigger;
pub use uzor::ui::widgets::atomic::item::input::register_layout_manager_item as build_item;
pub use uzor::ui::widgets::atomic::radio::input::register_layout_manager_radio as build_radio;
pub use uzor::ui::widgets::atomic::scroll_chevron::input::register_layout_manager_scroll_chevron as build_scroll_chevron;
pub use uzor::ui::widgets::atomic::scrollbar::input::register_layout_manager_scrollbar as build_scrollbar;
pub use uzor::ui::widgets::atomic::separator::input::register_layout_manager_separator as build_separator;
pub use uzor::ui::widgets::atomic::shape_selector::input::register_layout_manager_shape_selector as build_shape_selector;
pub use uzor::ui::widgets::atomic::slider::input::register_layout_manager_slider as build_slider;
pub use uzor::ui::widgets::atomic::tab::input::register_layout_manager_tab as build_tab;
pub use uzor::ui::widgets::atomic::text::input::register_layout_manager_text as build_text;
pub use uzor::ui::widgets::atomic::text_input::input::register_layout_manager_text_input as build_text_input;
pub use uzor::ui::widgets::atomic::toast::input::register_layout_manager_toast as build_toast;
pub use uzor::ui::widgets::atomic::toggle::input::register_layout_manager_toggle as build_toggle;
pub use uzor::ui::widgets::atomic::tooltip::input::register_layout_manager_tooltip as build_tooltip;

// Composites
pub use uzor::ui::widgets::composite::blackbox_panel::input::register_layout_manager_blackbox_panel as build_blackbox_panel;
pub use uzor::ui::widgets::composite::blackbox_panel::input::register_layout_manager_stub_panel as build_stub_panel;
pub use uzor::ui::widgets::composite::chrome::input::register_layout_manager_chrome as build_chrome;
pub use uzor::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu as build_context_menu;
pub use uzor::ui::widgets::composite::dropdown::input::register_layout_manager_dropdown as build_dropdown;
pub use uzor::ui::widgets::composite::modal::input::register_layout_manager_modal as build_modal;
pub use uzor::ui::widgets::composite::panel::input::register_layout_manager_panel as build_panel;
pub use uzor::ui::widgets::composite::popup::input::register_layout_manager_popup as build_popup;
pub use uzor::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar as build_sidebar;
pub use uzor::ui::widgets::composite::toolbar::input::register_layout_manager_toolbar as build_toolbar;
