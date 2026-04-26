//! Widget catalog — atomic + composite split.
//!
//! Layout mirrors `WidgetKind`:
//! - `atomic/`    — leaf widgets (Button, Slider, Tooltip, …).
//! - `composite/` — parents that own children (Modal, Dropdown, Chrome, …).
//!
//! Each widget folder owns `types`, `state`, `theme`, `style`, `render`, `input`.
//! Some files are placeholders for now and will be filled in subsequent passes.

pub mod atomic;
pub mod composite;

// Backward-compat module aliases — to be removed once consumers migrate to
// `widgets::atomic::*` / `widgets::composite::*` paths directly.
pub use atomic::button;
pub use atomic::container;
pub use atomic::scrollbar;
pub use atomic::separator;
pub use atomic::slider;
pub use atomic::tab;
pub use atomic::text_input;
pub use atomic::toast;
pub use atomic::tooltip;
pub use composite::chrome;
pub use composite::dropdown;
pub use composite::panel;
pub use composite::popup;

// ─── Atomic re-exports ──────────────────────────────────────────────────────
pub use atomic::button::ButtonType;
pub use atomic::container::ContainerType;
pub use atomic::scrollbar::{ScrollbarType, ScrollbarOrientation};
pub use atomic::separator::{SeparatorType, SeparatorOrientation};
pub use atomic::slider::SliderType;
pub use atomic::text_input::TextInputType;
pub use atomic::toast::ToastType;

// ─── Composite re-exports ───────────────────────────────────────────────────
pub use composite::dropdown::DropdownType;
pub use composite::panel::{PanelType, ToolbarVariant, SidebarVariant, ModalVariant};
pub use composite::popup::PopupType;

pub use crate::types::ScrollState;

use crate::input::Sense;

/// Declares what interactions a widget type supports.
///
/// Every widget *Type enum implements this trait so InputCoordinator
/// and widget_state know what Sense flags to apply.
pub trait WidgetCapabilities {
    fn sense(&self) -> Sense;
}
