//! uzor - Platform-agnostic headless UI engine

pub mod core;
pub mod docking;
pub mod input;
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
pub use app_context::layout;
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
pub use types::{IconId, Rect, WidgetId, WidgetState};
pub use input::{InputState, InputCoordinator, LayerId, ScopedRegion};
pub use input::{TextFieldStore, TextFieldConfig, TextAction, InputCapability, KeyPress};

pub use widgets::{
    ButtonType, ContainerType, PopupRenderKind,
    PanelType, ToolbarVariant, SidebarVariant, ModalVariant,
    TextInputType, DropdownKind, SliderType, ToastType,
};

pub use ui::assets::cursors::CursorIcon;
