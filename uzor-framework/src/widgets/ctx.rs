//! L2 — `ContextManager` paint+register.  Registers the widget with the
//! embedded coordinator AND draws it in one call.
//!
//! `draw_X(ctx, render, ...)` is the L2 convenience wrapper — useful inside
//! blackbox handler bodies that want widgets registered on the manager
//! provided by the host but paint within their own owned rect.

// Atomics
pub use uzor::ui::widgets::atomic::button::input::register_context_manager_button as draw_button;
pub use uzor::ui::widgets::atomic::checkbox::input::register_context_manager_checkbox as draw_checkbox;
pub use uzor::ui::widgets::atomic::chevron::input::register_context_manager_chevron as draw_chevron;
pub use uzor::ui::widgets::atomic::clock::input::register_context_manager_clock as draw_clock;
pub use uzor::ui::widgets::atomic::close_button::input::register_context_manager_close_button as draw_close_button;
pub use uzor::ui::widgets::atomic::color_swatch::input::register_context_manager_color_swatch as draw_color_swatch;
pub use uzor::ui::widgets::atomic::container::input::register_context_manager_container as draw_container;
pub use uzor::ui::widgets::atomic::drag_handle::input::register_context_manager_drag_handle as draw_drag_handle;
pub use uzor::ui::widgets::atomic::dropdown_trigger::input::register_context_manager_dropdown_trigger as draw_dropdown_trigger;
pub use uzor::ui::widgets::atomic::item::input::register_context_manager_item as draw_item;
pub use uzor::ui::widgets::atomic::radio::input::register_context_manager_radio as draw_radio;
pub use uzor::ui::widgets::atomic::scroll_chevron::input::register_context_manager_scroll_chevron as draw_scroll_chevron;
pub use uzor::ui::widgets::atomic::scrollbar::input::register_context_manager_scrollbar as draw_scrollbar;
pub use uzor::ui::widgets::atomic::separator::input::register_context_manager_separator as draw_separator;
pub use uzor::ui::widgets::atomic::shape_selector::input::register_context_manager_shape_selector as draw_shape_selector;
pub use uzor::ui::widgets::atomic::slider::input::register_context_manager_slider as draw_slider;
pub use uzor::ui::widgets::atomic::tab::input::register_context_manager_tab as draw_tab;
pub use uzor::ui::widgets::atomic::text::input::register_context_manager_text as draw_text;
pub use uzor::ui::widgets::atomic::text_input::input::register_context_manager_text_input as draw_text_input;
pub use uzor::ui::widgets::atomic::toast::input::register_context_manager_toast as draw_toast;
pub use uzor::ui::widgets::atomic::toggle::input::register_context_manager_toggle as draw_toggle;
pub use uzor::ui::widgets::atomic::tooltip::input::register_context_manager_tooltip as draw_tooltip;

// Composites
pub use uzor::ui::widgets::composite::blackbox_panel::render::register_context_manager_blackbox_panel as draw_blackbox_panel;
pub use uzor::ui::widgets::composite::chrome::render::register_context_manager_chrome as draw_chrome;
pub use uzor::ui::widgets::composite::context_menu::render::register_context_manager_context_menu as draw_context_menu;
pub use uzor::ui::widgets::composite::dropdown::render::register_context_manager_dropdown as draw_dropdown;
pub use uzor::ui::widgets::composite::modal::render::register_context_manager_modal as draw_modal;
pub use uzor::ui::widgets::composite::panel::render::register_context_manager_panel as draw_panel;
pub use uzor::ui::widgets::composite::popup::render::register_context_manager_popup as draw_popup;
pub use uzor::ui::widgets::composite::sidebar::render::register_context_manager_sidebar as draw_sidebar;
pub use uzor::ui::widgets::composite::toolbar::render::register_context_manager_toolbar as draw_toolbar;
