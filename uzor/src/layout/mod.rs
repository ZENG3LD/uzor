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
mod edge_panels;
mod manager;
mod overlay_stack;
mod solve;
mod tree;
mod types;
mod z_layers;

pub use chrome_slot::ChromeSlot;
pub use edge_panels::{EdgePanels, EdgeSlot};
pub use manager::LayoutManager;
pub use overlay_stack::{OverlayEntry, OverlayStack};
pub use solve::solve_layout;
pub use tree::{LayoutNode, LayoutNodeId, LayoutTree, LayoutTreeEntry};
pub use types::{EdgeRects, EdgeSide, LayoutSolved, OverlayKind, OverlayRect, SlotId};
pub use z_layers::ZLayerTable;
