use crate::core::types::Rect;
use super::types::{EdgeSide, SlotId};

/// Stable numeric identifier for a node in `LayoutTree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutNodeId(pub u64);

/// Kind of a macro layout node owned by `LayoutTree`.
#[derive(Debug, Clone)]
pub enum LayoutNode {
    /// System titlebar / menubar strip.
    Chrome,
    /// One edge side (toolbar strip or sidebar column).
    EdgeSide {
        /// Which side of the viewport this node represents.
        side: EdgeSide,
        /// Ordered list of slot ids contributing to this edge.
        slot_ids: Vec<SlotId>,
    },
    /// Root of the dock panel subtree (panels live in `PanelDockingManager`).
    DockRoot,
    /// Floating window layer (floating windows live in `PanelDockingManager`).
    FloatingLayer,
    /// Overlay composite stack (entries live in `OverlayStack`).
    OverlayStack,
}

/// One entry in the flat `LayoutTree` node list.
#[derive(Debug, Clone)]
pub struct LayoutTreeEntry {
    /// Stable id for this node.
    pub id: LayoutNodeId,
    /// Node kind.
    pub node: LayoutNode,
    /// Computed rect — populated by `solve_layout`; zero-sized before first solve.
    pub rect: Rect,
}

/// Macro layout tree.
///
/// Owns the nine system-level layout nodes and their computed rects.
/// The dock subtree and floating windows live inside `PanelDockingManager`;
/// overlay entries live inside `OverlayStack`. This tree only tracks the
/// macro-level structure and per-node rects.
///
/// Implemented as a flat list with pre-allocated named-slot accessors;
/// the N-ary dock hierarchy is handled separately by `PanelDockingManager`.
#[derive(Debug, Clone)]
pub struct LayoutTree {
    entries: Vec<LayoutTreeEntry>,
    chrome_id: LayoutNodeId,
    edge_top_id: LayoutNodeId,
    edge_bot_id: LayoutNodeId,
    edge_left_id: LayoutNodeId,
    edge_right_id: LayoutNodeId,
    dock_root_id: LayoutNodeId,
    floating_id: LayoutNodeId,
    overlay_id: LayoutNodeId,
}

const ZERO_RECT: Rect = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

impl LayoutTree {
    /// Construct a fresh tree pre-populated with the standard eight system nodes.
    pub fn new() -> Self {
        let mut next_id = 1u64;
        let mut entries = Vec::with_capacity(8);

        let chrome_id       = LayoutNodeId(next_id); next_id += 1;
        let edge_top_id     = LayoutNodeId(next_id); next_id += 1;
        let edge_bot_id     = LayoutNodeId(next_id); next_id += 1;
        let edge_left_id    = LayoutNodeId(next_id); next_id += 1;
        let edge_right_id   = LayoutNodeId(next_id); next_id += 1;
        let dock_root_id    = LayoutNodeId(next_id); next_id += 1;
        let floating_id     = LayoutNodeId(next_id); next_id += 1;
        let overlay_id      = LayoutNodeId(next_id); next_id += 1;

        entries.push(LayoutTreeEntry { id: chrome_id,     node: LayoutNode::Chrome,                                                rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: edge_top_id,   node: LayoutNode::EdgeSide { side: EdgeSide::Top,    slot_ids: vec![] }, rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: edge_bot_id,   node: LayoutNode::EdgeSide { side: EdgeSide::Bottom, slot_ids: vec![] }, rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: edge_left_id,  node: LayoutNode::EdgeSide { side: EdgeSide::Left,   slot_ids: vec![] }, rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: edge_right_id, node: LayoutNode::EdgeSide { side: EdgeSide::Right,  slot_ids: vec![] }, rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: dock_root_id,  node: LayoutNode::DockRoot,                                             rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: floating_id,   node: LayoutNode::FloatingLayer,                                        rect: ZERO_RECT });
        entries.push(LayoutTreeEntry { id: overlay_id,    node: LayoutNode::OverlayStack,                                         rect: ZERO_RECT });

        let _ = next_id; // all IDs pre-allocated; field removed

        Self {
            entries,
            chrome_id,
            edge_top_id,
            edge_bot_id,
            edge_left_id,
            edge_right_id,
            dock_root_id,
            floating_id,
            overlay_id,
        }
    }

    /// Return the computed rect for a node, or `None` if the id is unknown.
    pub fn rect_of(&self, id: LayoutNodeId) -> Option<Rect> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.rect)
    }

    /// Write a computed rect back for a node (no-op for unknown ids).
    pub fn set_rect(&mut self, id: LayoutNodeId, rect: Rect) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.rect = rect;
        }
    }

    /// Id of the chrome strip node.
    pub fn chrome_id(&self) -> LayoutNodeId {
        self.chrome_id
    }

    /// Id of the edge node for a given side.
    pub fn edge_id(&self, side: EdgeSide) -> LayoutNodeId {
        match side {
            EdgeSide::Top    => self.edge_top_id,
            EdgeSide::Bottom => self.edge_bot_id,
            EdgeSide::Left   => self.edge_left_id,
            EdgeSide::Right  => self.edge_right_id,
        }
    }

    /// Id of the dock root node.
    pub fn dock_root_id(&self) -> LayoutNodeId {
        self.dock_root_id
    }

    /// Id of the floating layer node.
    pub fn floating_id(&self) -> LayoutNodeId {
        self.floating_id
    }

    /// Id of the overlay stack node.
    pub fn overlay_id(&self) -> LayoutNodeId {
        self.overlay_id
    }

    /// All entries in tree order.
    pub fn entries(&self) -> &[LayoutTreeEntry] {
        &self.entries
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}
