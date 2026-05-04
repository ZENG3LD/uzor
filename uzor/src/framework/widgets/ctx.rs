//! L2 — `ContextManager` paint+register.  Registers the widget with the
//! embedded coordinator AND draws it in one call.
//!
//! `draw_X(ctx, render, ...)` is the L2 convenience wrapper — useful inside
//! blackbox handler bodies that want widgets registered on the manager
//! provided by the host but paint within their own owned rect.

// Atomics
pub use crate::ui::widgets::atomic::button::input::register_context_manager_button as draw_button;
pub use crate::ui::widgets::atomic::checkbox::input::register_context_manager_checkbox as draw_checkbox;
pub use crate::ui::widgets::atomic::chevron::input::register_context_manager_chevron as draw_chevron;
pub use crate::ui::widgets::atomic::clock::input::register_context_manager_clock as draw_clock;
pub use crate::ui::widgets::atomic::close_button::input::register_context_manager_close_button as draw_close_button;
pub use crate::ui::widgets::atomic::color_swatch::input::register_context_manager_color_swatch as draw_color_swatch;
pub use crate::ui::widgets::atomic::container::input::register_context_manager_container as draw_container;
pub use crate::ui::widgets::atomic::drag_handle::input::register_context_manager_drag_handle as draw_drag_handle;
pub use crate::ui::widgets::atomic::dropdown_trigger::input::register_context_manager_dropdown_trigger as draw_dropdown_trigger;
pub use crate::ui::widgets::atomic::item::input::register_context_manager_item as draw_item;
pub use crate::ui::widgets::atomic::radio::input::register_context_manager_radio as draw_radio;
pub use crate::ui::widgets::atomic::scroll_chevron::input::register_context_manager_scroll_chevron as draw_scroll_chevron;
pub use crate::ui::widgets::atomic::scrollbar::input::register_context_manager_scrollbar as draw_scrollbar;
pub use crate::ui::widgets::atomic::separator::input::register_context_manager_separator as draw_separator;
pub use crate::ui::widgets::atomic::shape_selector::input::register_context_manager_shape_selector as draw_shape_selector;
pub use crate::ui::widgets::atomic::slider::input::register_context_manager_slider as draw_slider;
pub use crate::ui::widgets::atomic::tab::input::register_context_manager_tab as draw_tab;
pub use crate::ui::widgets::atomic::text::input::register_context_manager_text as draw_text;
pub use crate::ui::widgets::atomic::text_input::input::register_context_manager_text_input as draw_text_input;
pub use crate::ui::widgets::atomic::toast::input::register_context_manager_toast as draw_toast;
pub use crate::ui::widgets::atomic::toggle::input::register_context_manager_toggle as draw_toggle;
pub use crate::ui::widgets::atomic::tooltip::input::register_context_manager_tooltip as draw_tooltip;

// Composites
pub use crate::ui::widgets::composite::blackbox_panel::render::register_context_manager_blackbox_panel as draw_blackbox_panel;
pub use crate::ui::widgets::composite::chrome::render::register_context_manager_chrome as draw_chrome;
pub use crate::ui::widgets::composite::context_menu::render::register_context_manager_context_menu as draw_context_menu;
pub use crate::ui::widgets::composite::dropdown::render::register_context_manager_dropdown as draw_dropdown;
pub use crate::ui::widgets::composite::modal::render::register_context_manager_modal as draw_modal;
pub use crate::ui::widgets::composite::panel::render::register_context_manager_panel as draw_panel;
pub use crate::ui::widgets::composite::popup::render::register_context_manager_popup as draw_popup;
pub use crate::ui::widgets::composite::sidebar::render::register_context_manager_sidebar as draw_sidebar;
pub use crate::ui::widgets::composite::toolbar::render::register_context_manager_toolbar as draw_toolbar;
