//! LayoutManager — top-level macro layout for uzor.
//!
//! Single source of truth for all widget rects: chrome, edges (toolbars/sidebars),
//! dock panels, floating panels, and z-ordered overlays.
//!
//! ## User-vs-system access split
//!
//! - System slots (chrome, edges, overlays): managed by uzor internals.
//! - User slots (dock panels, floating panels): app accesses via `panels_mut()`.
//!
//! ## See also
//!
//! `docs/uzor-widget-research/layout-manager-deep.md` for the full design doc.

mod chrome_slot;
mod consume_chain;
mod dispatcher;
mod edge_panels;
mod handles;
pub mod host;
mod manager;
mod overlay_stack;
mod solve;
mod tree;
mod types;
mod z_layers;

pub use chrome_slot::ChromeSlot;
pub use host::WindowHost;
pub use consume_chain::consume_event_chain;
pub use dispatcher::{ChevronStepDirection, ChromeWindowControl, ClickDispatcher, DispatchEvent, EventBuilder, ResizeEdge};
pub use edge_panels::{EdgePanels, EdgePlacement, EdgeSlot};
pub use manager::{ClickOutcome, CompositeKind, CompositeRegistration, DismissFrame, LayoutManager, PointerUpOutcome};
pub use overlay_stack::{OverlayEntry, OverlayStack};
pub use solve::solve_layout;
pub use handles::{
    BlackboxPanelNode, ChromeNode, ContextMenuNode, DropdownNode, ModalNode,
    PanelNode, PopupNode, SidebarNode, ToolbarNode,
    // State handles — opaque composite keys returned by LayoutManager::add_*
    ContextMenuHandle, DropdownHandle, ModalHandle, OverlayHandle, PopupHandle,
    SidebarHandle, ToolbarHandle,
};
pub use tree::{LayoutNode, LayoutNodeId, LayoutTree, LayoutTreeEntry, SystemNodeKind, WidgetNode};
pub use types::{DragOutcome, EdgeRects, EdgeSide, LayoutSolved, OverlayKind, OverlayRect, SlotId};
pub use z_layers::ZLayerTable;
