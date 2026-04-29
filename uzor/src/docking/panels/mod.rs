//! uzor-panels — Generic docking panel layout engine
//!
//! Provides an N-ary panel tree with tabs, splits, grids, drag-and-drop,
//! separator resize, floating windows, and layout presets.
//!
//! This module is rendering-agnostic: it only handles geometry, hit-testing,
//! and layout computation. Rendering is left to the consumer.

pub mod id;
pub mod rect;
pub mod tree;
pub mod drop_zone;
pub mod drag;
pub mod separator;
pub mod snap_back;
pub mod tabs;
pub mod floating;
pub mod hit_test;
pub mod presets;
pub mod grid;
pub mod layout;
pub mod manager;
pub mod serialize;

// Re-exports
pub use id::{LeafId, BranchId, NodeId};
pub use rect::PanelRect;
pub use tree::{PanelTree, PanelStore, Tile, Container, Tabs, Linear, Grid, Shares, LinearDirection, GridLayout};
pub use drop_zone::{DropZone, DropZoneDetector, CompassZone};
pub use drag::{DragDropState, LockState, DragSource, HoverTarget, PanelDragState};
pub use separator::{Separator, SeparatorOrientation, SeparatorState, SeparatorLevel, SeparatorController};
pub use snap_back::SnapBackAnimation;
pub use tabs::{TabBar, TabInfo, TabHit, TabDragController, TabDragState, TabReorderState, TabBarInfo, TabItem};
pub use floating::{FloatingWindow, FloatingWindowId, FloatingDragState};
pub use hit_test::{HitResult, CornerHandle};
pub use presets::{WindowLayout, SplitKind, PANEL_GAP};
pub use grid::{DockingTree, Leaf, Branch, PanelNode};
pub use serialize::{LayoutSnapshot, SerializedNode, SerializedNodeType};
pub use manager::PanelDockingManager;

/// Trait for panel types stored in the docking tree.
///
/// Consumers implement this for their domain-specific panel types.
/// The docking engine uses it for tab titles, serialization routing,
/// and minimum size enforcement.
pub trait DockPanel: Clone + Send + Sync {
    /// Title displayed in tab bar
    fn title(&self) -> &str;

    /// Type identifier for serialization routing
    fn type_id(&self) -> &'static str;

    /// Minimum size (width, height) in pixels
    fn min_size(&self) -> (f32, f32) {
        (200.0, 200.0)
    }

    /// Whether this panel can be closed by the user
    fn closable(&self) -> bool {
        true
    }
}
