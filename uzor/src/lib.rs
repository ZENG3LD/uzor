//! uzor - Platform-agnostic headless UI engine
//!
//! This crate provides a headless UI framework for:
//! - Geometry calculation and layout
//! - Input handling and interaction detection
//! - Widget state management
//! - Platform abstraction
//!
//! Rendering is delegated to platform-specific implementations.

pub mod animation;
pub mod containers;
pub mod context;
pub mod fonts;
pub mod i18n;
pub mod input;
pub mod input_coordinator;
pub mod layout;
pub mod layout_helpers;
pub mod panel_api;
pub mod panels;
pub mod macos;
pub mod interactive;
pub mod text_fx;
pub mod cursor;
pub mod numbers;
pub mod scroll_fx;
pub mod platform;
pub mod render;
pub mod state;
pub mod types;
pub mod widgets;

pub use context::{Context, ButtonResponse};
pub use i18n::{Language, current_language, set_language, Translatable, TextKey, MonthKey, TooltipKey, month_names_short, t_tooltip};

// Re-export commonly used types
pub use animation::AnimationCoordinator;
pub use types::{IconId, Rect, WidgetId, WidgetState};
pub use input::{InputState, InputCoordinator, LayerId, ScopedRegion};
pub use input_coordinator::InputCoordinator as InputCoordinator2;
pub use widgets::{IconButtonConfig, IconButtonResponse};

// Re-export all 9 widget type enums at top level
pub use widgets::{
    ButtonType, ContainerType, PopupType,
    PanelType, ToolbarVariant, SidebarVariant, ModalVariant,
    OverlayType, TextInputType, DropdownType, SliderType, ToastType,
};

// Re-export unified tooltip system
pub use input::{
    TooltipState, TooltipConfig, TooltipRequest,
    TooltipTheme, DefaultTooltipTheme,
    calculate_tooltip_position, estimate_tooltip_size,
};
