//! Widget type catalog
//!
//! Platform-agnostic widget definitions (types, themes, state, input contracts).
//! Backends implement rendering; uzor defines the contract.

pub mod button;
pub mod container;
pub mod containers;
pub mod popup;
pub mod panel;
pub mod overlay;
pub mod text_input;
pub mod dropdown;
pub mod slider;
pub mod toast;
pub mod scrollbar;
pub mod radio;
pub mod separator;
pub mod tooltip;
pub mod tab;
pub mod chrome;

pub use button::ButtonType;
pub use container::ContainerType;
pub use popup::PopupType;
pub use panel::{PanelType, ToolbarVariant, SidebarVariant, ModalVariant};
pub use overlay::OverlayType;
pub use text_input::TextInputType;
pub use dropdown::DropdownType;
pub use slider::SliderType;
pub use toast::ToastType;
pub use scrollbar::{ScrollbarType, ScrollbarOrientation};
pub use radio::RadioType;
pub use separator::{SeparatorType, SeparatorOrientation};

pub use crate::types::ScrollState;

use crate::input::Sense;

/// Declares what interactions a widget type supports.
///
/// Every widget *Type enum implements this trait so InputCoordinator
/// and widget_state know what Sense flags to apply.
pub trait WidgetCapabilities {
    fn sense(&self) -> Sense;
}

