//! Pure layout mechanics for uzor: docking, solve, dispatch, overlays.
//!
//! The old single-writer layout façade and its agent / sync / host doors
//! were removed in brief C1. Window composition now goes through
//! `uzor-framework` engines; this module keeps the pure trees, docking
//! math, click dispatch vocabulary, and overlay helpers those engines
//! call.

mod chrome_slot;
mod dispatcher;
pub mod dock_state;
pub mod docking;
pub mod panel_api;
mod edge_panels;
mod handles;
pub mod window;
mod overlay_stack;
mod solve;
mod tree;
mod types;
mod z_layers;

pub use chrome_slot::ChromeSlot;
pub use dispatcher::{
    ChevronStepDirection, ChromeWindowControl, ClickDispatcher, DispatchEvent, EventBuilder,
    ResizeEdge,
};
pub use edge_panels::{EdgePanels, EdgePlacement, EdgeSlot};
pub use overlay_stack::{OverlayEntry, OverlayStack};
pub use solve::solve_layout;
pub use handles::{
    ContextMenuHandle, DropdownHandle, ModalHandle, PopupHandle, SidebarHandle, ToolbarHandle,
};
pub use tree::{LayoutNode, LayoutNodeId, LayoutTree, LayoutTreeEntry, SystemNodeKind, WidgetNode};
pub use types::{DragOutcome, EdgeRects, EdgeSide, LayoutSolved, OverlayKind, OverlayRect, SlotId};
pub use z_layers::ZLayerTable;
pub use dock_state::DockState;
