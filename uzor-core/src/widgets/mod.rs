//! Core widgets module
//!
//! This module contains platform-agnostic widget types:
//! - Button: Unified button architecture (6 types, 19 variants)
//! - Container: Scrollable and plain containers
//! - Popup: Context menus, color pickers, custom popups
//! - Panel: Large container panels (toolbars, sidebars, modals, hideable)
//! - Overlay: Tooltips and info overlays
//! - TextInput: Text, Number, Search, Password inputs
//! - Dropdown: Standard, Grid, Layout dropdowns
//! - Slider: Single and dual-point sliders (new 5-level architecture)
//! - Toast: Info, Success, Warning, Error notifications
//! - IconButton: Icon-only buttons for toolbars
//! - Checkbox: Checkbox with label
//! - RadioGroup: Radio button group (single selection)
//! - Input: Text input fields with cursor and selection
//! - Scrollbar: Custom scrollbar with drag support
//! - Scrollable: Scrollable container with automatic scrollbar
//! - Toolbar: Toolbar containers
//! - ContextMenu: Right-click context menus

// New 5-level widget modules (migrated from terminal)
pub mod button;
pub mod container;
pub mod popup;
pub mod panel;
pub mod overlay;
pub mod text_input;
pub mod dropdown;
pub mod slider;
pub mod toast;

// Existing widget modules
pub mod checkbox;
pub mod radio_group;
pub mod context_menu;
pub mod icon_button;
pub mod input;
pub mod scrollable;
pub mod scrollbar;
pub mod slider_system;
pub mod toolbar;

// Re-export new widget main types
pub use button::ButtonType;
pub use container::ContainerType;
pub use popup::PopupType;
pub use panel::{PanelType, ToolbarVariant, SidebarVariant, ModalVariant};
pub use overlay::OverlayType;
pub use text_input::TextInputType;
pub use dropdown::DropdownType;
pub use slider::SliderType;
pub use toast::ToastType;

// Re-export all existing widget types and functions
pub use checkbox::*;
pub use radio_group::*;
pub use context_menu::*;
pub use icon_button::*;
pub use input::*;
pub use scrollable::*;
pub use scrollbar::*;
pub use toolbar::*;

// Common widget state types used by render helpers
pub use crate::types::ScrollState;
