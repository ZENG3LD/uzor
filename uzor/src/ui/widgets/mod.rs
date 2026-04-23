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

pub use button::ButtonType;
pub use container::ContainerType;
pub use popup::PopupType;
pub use panel::{PanelType, ToolbarVariant, SidebarVariant, ModalVariant};
pub use overlay::OverlayType;
pub use text_input::TextInputType;
pub use dropdown::DropdownType;
pub use slider::SliderType;
pub use toast::ToastType;

pub use crate::types::ScrollState;

