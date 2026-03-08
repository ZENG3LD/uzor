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
pub mod input;
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

// Re-export commonly used types
pub use animation::AnimationCoordinator;
pub use types::{IconId, Rect, WidgetId, WidgetState};
pub use input::{InputState, InputCoordinator, LayerId, ScopedRegion};
pub use widgets::{IconButtonConfig, IconButtonResponse};

// Re-export all 9 widget type enums at top level
pub use widgets::{
    ButtonType, ContainerType, PopupType,
    PanelType, ToolbarVariant, SidebarVariant, ModalVariant,
    OverlayType, TextInputType, DropdownType, SliderType, ToastType,
};
