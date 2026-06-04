//! uzor - Platform-agnostic headless UI engine

pub mod core;
pub mod input;
pub mod layout;
pub mod platform;
/// Pure agnostic surface across composites — paint + measure +
/// hit-test + data types, no L1 / L2 / L3 wrappers.  Embedders that
/// drive their own input pipeline (custom L0 apps, parallel runtimes)
/// call into `l0::chrome::draw_chrome`, `l0::modal::draw_modal`, etc.
/// directly.
pub mod l0;
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
// `docking` was absorbed into `layout`. Keep the old paths as aliases for now.
pub use layout::docking as docking_panels;
pub use layout::docking as panels;
pub use layout::panel_api;
pub mod docking { pub use crate::layout::docking::*; pub use crate::layout::docking as panels; pub use crate::layout::panel_api; }
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

pub mod framework;
pub use framework::render_control::RenderControl;

/// Per-cluster text shaper (requires feature `shaper` / cosmic-text).
#[cfg(feature = "shaper")]
pub mod shaper;

// Re-export key types
pub use app_context::ContextManager;
pub use i18n::{Translate, current_lang_index, set_lang_index, t};
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

pub use platform::types::{
    RgbaIcon, RenderBackend, Scene2DBackend, UrxBackend, ResizeDirection, CornerStyle,
};

// Note: tier-organised registration shortcuts (`coord`, `ctx`, `lm`) live in
// `uzor-framework::widgets` — this core crate exposes only the long-form
// names (`register_layout_manager_*`, etc.) for tests and legacy callers
// that pin the old API.


