//! uzor - Platform-agnostic headless UI engine

pub mod core;
pub mod docking;
pub mod input;
pub mod layout;
pub mod platform;
pub use input as input_coordinator;
pub mod ui;
pub mod app_context;

// Compat shims — core internals at crate root
pub use ui::animation;
pub use self::core::render;
pub use self::core::types;
pub use self::core::window;

// Compat shims — old names
pub use app_context as context;
pub use docking::panels;
pub use docking::panel_api;
/// CSS-flex micro-layout engine (widget subtrees). Macro layout lives in `crate::layout`.
pub use app_context::layout as app_layout;
pub use app_context::state;

// Compat shims — ui internals at crate root
pub use ui::widgets;
pub use ui::themes;
pub use ui::assets;
pub use ui::i18n;
pub use themes::macos as macos;
pub use assets::fonts as fonts;
pub use assets::icons as icons;

// Compat shim — old `engine` path
pub use self::core as engine;

// Re-export key types
pub use app_context::ContextManager;
pub use i18n::{Language, current_language, set_language, Translatable, TextKey, MonthKey, TooltipKey, month_names_short, t_tooltip};
pub use ui::animation::AnimationCoordinator;
pub use types::{IconId, Rect, WidgetId, WidgetState, CompositeId, AtomicId, unsafe_widget_id};
pub use input::{InputState, InputCoordinator, LayerId, ScopedRegion};
pub use input::{TextFieldStore, TextFieldConfig, TextAction, InputCapability, KeyPress};

pub use widgets::{
    ButtonType, ContainerType, PopupRenderKind,
    PanelType, ToolbarVariant, SidebarVariant, ModalVariant,
    TextInputType, DropdownKind, SliderType, ToastType,
};

pub use ui::assets::cursors::CursorIcon;

// =============================================================================
// API tier modules — coord (L1) / ctx (L2) / lm (L3)
// =============================================================================
//
// Three short namespaces re-exporting widget registration functions per access
// level.  The verb in the function name signals what it does, the module signals
// which manager owns the registration.
//
// - `coord::register_X(coord, ...)`        — L1, raw InputCoordinator entry
// - `ctx::draw_X(ctx, render, ...)`         — L2, ContextManager paint+register
// - `lm::build_X(layout, render, ...)`      — L3, LayoutManager declarative API
//
// L3 apps should only need `lm::*`.  L1/L2 are for the lib internals and
// blackbox handler bodies that paint their own subtree.

/// L1 — direct InputCoordinator registration. No drawing.
pub mod coord {
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
}

/// L2 — ContextManager paint+register. Registers the widget with the embedded
/// coordinator AND draws it in one call.
pub mod ctx {
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
}

/// L3 — LayoutManager declarative API. Resolves rect from layout slot, manages
/// state via typed handles, draws + registers in one call.
pub mod lm {
    // Atomics
    pub use crate::ui::widgets::atomic::button::input::register_layout_manager_button as build_button;
    pub use crate::ui::widgets::atomic::checkbox::input::register_layout_manager_checkbox as build_checkbox;
    pub use crate::ui::widgets::atomic::chevron::input::register_layout_manager_chevron as build_chevron;
    pub use crate::ui::widgets::atomic::clock::input::register_layout_manager_clock as build_clock;
    pub use crate::ui::widgets::atomic::close_button::input::register_layout_manager_close_button as build_close_button;
    pub use crate::ui::widgets::atomic::color_swatch::input::register_layout_manager_color_swatch as build_color_swatch;
    pub use crate::ui::widgets::atomic::container::input::register_layout_manager_container as build_container;
    pub use crate::ui::widgets::atomic::drag_handle::input::register_layout_manager_drag_handle as build_drag_handle;
    pub use crate::ui::widgets::atomic::dropdown_trigger::input::register_layout_manager_dropdown_trigger as build_dropdown_trigger;
    pub use crate::ui::widgets::atomic::item::input::register_layout_manager_item as build_item;
    pub use crate::ui::widgets::atomic::radio::input::register_layout_manager_radio as build_radio;
    pub use crate::ui::widgets::atomic::scroll_chevron::input::register_layout_manager_scroll_chevron as build_scroll_chevron;
    pub use crate::ui::widgets::atomic::scrollbar::input::register_layout_manager_scrollbar as build_scrollbar;
    pub use crate::ui::widgets::atomic::separator::input::register_layout_manager_separator as build_separator;
    pub use crate::ui::widgets::atomic::shape_selector::input::register_layout_manager_shape_selector as build_shape_selector;
    pub use crate::ui::widgets::atomic::slider::input::register_layout_manager_slider as build_slider;
    pub use crate::ui::widgets::atomic::tab::input::register_layout_manager_tab as build_tab;
    pub use crate::ui::widgets::atomic::text::input::register_layout_manager_text as build_text;
    pub use crate::ui::widgets::atomic::text_input::input::register_layout_manager_text_input as build_text_input;
    pub use crate::ui::widgets::atomic::toast::input::register_layout_manager_toast as build_toast;
    pub use crate::ui::widgets::atomic::toggle::input::register_layout_manager_toggle as build_toggle;
    pub use crate::ui::widgets::atomic::tooltip::input::register_layout_manager_tooltip as build_tooltip;
    // Composites
    pub use crate::ui::widgets::composite::blackbox_panel::input::register_layout_manager_blackbox_panel as build_blackbox_panel;
    pub use crate::ui::widgets::composite::blackbox_panel::input::register_layout_manager_stub_panel as build_stub_panel;
    pub use crate::ui::widgets::composite::chrome::input::register_layout_manager_chrome as build_chrome;
    pub use crate::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu as build_context_menu;
    pub use crate::ui::widgets::composite::dropdown::input::register_layout_manager_dropdown as build_dropdown;
    pub use crate::ui::widgets::composite::modal::input::register_layout_manager_modal as build_modal;
    pub use crate::ui::widgets::composite::panel::input::register_layout_manager_panel as build_panel;
    pub use crate::ui::widgets::composite::popup::input::register_layout_manager_popup as build_popup;
    pub use crate::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar as build_sidebar;
    pub use crate::ui::widgets::composite::toolbar::input::register_layout_manager_toolbar as build_toolbar;
}
