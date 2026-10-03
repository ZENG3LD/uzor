//! Level-1 register functions (the old `lm::raw` `build_*` list, minus the
//! blackbox panel — design §7.2 deletes that slot).
//!
//! These only declare a hit rect. The convenience functions in the parent
//! module register and draw together.

pub use uzor::ui::widgets::atomic::button::register_input_coordinator_button;
pub use uzor::ui::widgets::atomic::checkbox::register_input_coordinator_checkbox;
pub use uzor::ui::widgets::atomic::chevron::register_input_coordinator_chevron;
pub use uzor::ui::widgets::atomic::clock::register_input_coordinator_clock;
pub use uzor::ui::widgets::atomic::close_button::register_input_coordinator_close_button;
pub use uzor::ui::widgets::atomic::color_swatch::register_input_coordinator_color_swatch;
pub use uzor::ui::widgets::atomic::container::register_input_coordinator_container;
pub use uzor::ui::widgets::atomic::drag_handle::register_input_coordinator_drag_handle;
pub use uzor::ui::widgets::atomic::dropdown_trigger::register_input_coordinator_dropdown_trigger;
pub use uzor::ui::widgets::atomic::item::register_input_coordinator_item;
pub use uzor::ui::widgets::atomic::radio::register_input_coordinator_radio;
pub use uzor::ui::widgets::atomic::scroll_chevron::register_input_coordinator_scroll_chevron;
pub use uzor::ui::widgets::atomic::scrollbar::register_input_coordinator_scrollbar;
pub use uzor::ui::widgets::atomic::separator::register_input_coordinator_separator;
pub use uzor::ui::widgets::atomic::slider::register_input_coordinator_slider;
pub use uzor::ui::widgets::atomic::tab::register_input_coordinator_tab;
pub use uzor::ui::widgets::atomic::text::register_input_coordinator_text;
pub use uzor::ui::widgets::atomic::text_input::register_input_coordinator_text_input;
pub use uzor::ui::widgets::atomic::toast::register_input_coordinator_toast;
pub use uzor::ui::widgets::atomic::toggle::register_input_coordinator_toggle;
pub use uzor::ui::widgets::atomic::tooltip::register_input_coordinator_tooltip;

pub use uzor::ui::widgets::composite::chrome::register_input_coordinator_chrome;
pub use uzor::ui::widgets::composite::context_menu::register_input_coordinator_context_menu;
pub use uzor::ui::widgets::composite::dropdown::register_input_coordinator_dropdown;
pub use uzor::ui::widgets::composite::modal::register_input_coordinator_modal;
pub use uzor::ui::widgets::composite::panel::register_input_coordinator_panel;
pub use uzor::ui::widgets::composite::popup::register_input_coordinator_popup;
pub use uzor::ui::widgets::composite::sidebar::register_input_coordinator_sidebar;
pub use uzor::ui::widgets::composite::toolbar::register_input_coordinator_toolbar;
