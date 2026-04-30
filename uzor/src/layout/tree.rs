use crate::core::types::Rect;
use crate::input::{Sense, WidgetKind};
use crate::types::WidgetId;
use super::types::{EdgeSide, SlotId};

/// Stable numeric identifier for a node in `LayoutTree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutNodeId(pub u64);

/// Synthetic root id — parent of all top-level system nodes.
/// Value 0 is never used by real nodes (system nodes start at 1).
impl LayoutNodeId {
    pub const ROOT: LayoutNodeId = LayoutNodeId(0);
}

/// Kind of a macro layout system node owned by `LayoutTree`.
#[derive(Debug, Clone)]
pub enum SystemNodeKind {
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

/// A widget node inserted into the `LayoutTree` by L3 registration functions.
#[derive(Debug, Clone)]
pub struct WidgetNode {
    /// Stable identity for this widget in the tree.
    pub id: WidgetId,
    /// Widget kind — drives composite vs atomic logic in flush_input.
    pub kind: WidgetKind,
    /// Bounding rect (screen-space, already computed by app or LayoutManager::solve).
    pub rect: Rect,
    /// Sense flags for this widget.
    pub sense: Sense,
}

/// Discriminated union for a tree node — either a fixed system node or a
/// per-frame widget node inserted by L3 registration helpers.
#[derive(Debug, Clone)]
pub enum LayoutNode {
    /// Fixed infrastructure node (chrome, edge, dock, overlay).
    System(SystemNodeKind),
    /// Widget node inserted each frame by L3 register calls.
    Widget(WidgetNode),
}

/// One entry in the flat `LayoutTree` node list.
#[derive(Debug, Clone)]
pub struct LayoutTreeEntry {
    /// Stable id for this node.
    pub id: LayoutNodeId,
    /// Node kind.
    pub node: LayoutNode,
    /// Computed rect — populated by `solve_layout` for system nodes,
    /// or the widget's own rect for widget nodes.
    pub rect: Rect,
    /// Parent node — `None` for top-level system nodes.
    pub parent: Option<LayoutNodeId>,
    /// Child node ids (populated when widgets are added under this node).
    pub children: Vec<LayoutNodeId>,
}

/// Macro layout tree.
///
/// Owns the eight system-level layout nodes and per-frame widget nodes.
/// System node rects are computed by `solve_layout`. Widget nodes are
/// inserted via [`add_widget`] and cleared via [`clear_widgets`] each frame.
///
/// [`add_widget`]: LayoutTree::add_widget
/// [`clear_widgets`]: LayoutTree::clear_widgets
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
    /// Counter for allocating widget node ids (starts at 9, above system ids 1–8).
    next_widget_id: u64,
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

        entries.push(LayoutTreeEntry { id: chrome_id,     node: LayoutNode::System(SystemNodeKind::Chrome),                                                         rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: edge_top_id,   node: LayoutNode::System(SystemNodeKind::EdgeSide { side: EdgeSide::Top,    slot_ids: vec![] }), rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: edge_bot_id,   node: LayoutNode::System(SystemNodeKind::EdgeSide { side: EdgeSide::Bottom, slot_ids: vec![] }), rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: edge_left_id,  node: LayoutNode::System(SystemNodeKind::EdgeSide { side: EdgeSide::Left,   slot_ids: vec![] }), rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: edge_right_id, node: LayoutNode::System(SystemNodeKind::EdgeSide { side: EdgeSide::Right,  slot_ids: vec![] }), rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: dock_root_id,  node: LayoutNode::System(SystemNodeKind::DockRoot),                                                       rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: floating_id,   node: LayoutNode::System(SystemNodeKind::FloatingLayer),                                                   rect: ZERO_RECT, parent: None, children: vec![] });
        entries.push(LayoutTreeEntry { id: overlay_id,    node: LayoutNode::System(SystemNodeKind::OverlayStack),                                                    rect: ZERO_RECT, parent: None, children: vec![] });

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
            next_widget_id: next_id, // starts at 9
        }
    }

    // ------------------------------------------------------------------
    // System node accessors (unchanged public API)
    // ------------------------------------------------------------------

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

    /// Synthetic root id (value 0). Use when there is no meaningful system parent.
    pub fn root_id(&self) -> LayoutNodeId {
        LayoutNodeId::ROOT
    }

    /// All entries in tree order (system nodes first, then widget nodes).
    pub fn entries(&self) -> &[LayoutTreeEntry] {
        &self.entries
    }

    // ------------------------------------------------------------------
    // Widget node insertion / removal
    // ------------------------------------------------------------------

    /// Insert a widget node as a child of `parent`. Returns its `LayoutNodeId`.
    ///
    /// If `parent` is `LayoutNodeId::ROOT` or an unknown id, the node is added
    /// without a recorded parent (root-level widget).
    pub fn add_widget(&mut self, parent: LayoutNodeId, node: WidgetNode) -> LayoutNodeId {
        let id = LayoutNodeId(self.next_widget_id);
        self.next_widget_id += 1;

        let parent_opt = if parent == LayoutNodeId::ROOT {
            None
        } else if self.entries.iter().any(|e| e.id == parent) {
            Some(parent)
        } else {
            None
        };

        // Register as child of parent entry.
        if let Some(pid) = parent_opt {
            if let Some(pe) = self.entries.iter_mut().find(|e| e.id == pid) {
                pe.children.push(id);
            }
        }

        let rect = node.rect;
        self.entries.push(LayoutTreeEntry {
            id,
            node: LayoutNode::Widget(node),
            rect,
            parent: parent_opt,
            children: vec![],
        });
        id
    }

    /// Remove a widget node and all its widget-node descendants.
    /// System nodes (ids 1–8) are never removed.
    pub fn remove_widget(&mut self, id: LayoutNodeId) {
        // Collect all descendant widget ids to remove.
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let current = to_remove[i];
            if let Some(entry) = self.entries.iter().find(|e| e.id == current) {
                for &child in &entry.children {
                    to_remove.push(child);
                }
            }
            i += 1;
        }

        // Remove from parent's children list.
        if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
            if let Some(pid) = entry.parent {
                if let Some(pe) = self.entries.iter_mut().find(|e| e.id == pid) {
                    pe.children.retain(|&c| c != id);
                }
            }
        }

        self.entries.retain(|e| !to_remove.contains(&e.id));
    }

    /// Clear all widget nodes (kind = `LayoutNode::Widget`). System nodes (ids 1–8) are preserved.
    ///
    /// Call at frame start before any L3 widget registration.
    pub fn clear_widgets(&mut self) {
        // Clear children lists on system nodes.
        for entry in self.entries.iter_mut() {
            if matches!(entry.node, LayoutNode::System(_)) {
                entry.children.clear();
            }
        }
        self.entries.retain(|e| matches!(e.node, LayoutNode::System(_)));
        self.next_widget_id = 9; // reset counter — system nodes occupy 1–8
    }

    // ------------------------------------------------------------------
    // Query helpers
    // ------------------------------------------------------------------

    /// Look up an entry by id.
    pub fn entry(&self, id: LayoutNodeId) -> Option<&LayoutTreeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Parent id of a node, or `None` if it is a root-level node.
    pub fn parent_of(&self, id: LayoutNodeId) -> Option<LayoutNodeId> {
        self.entries.iter().find(|e| e.id == id).and_then(|e| e.parent)
    }

    /// Children of a node.
    pub fn children_of(&self, id: LayoutNodeId) -> &[LayoutNodeId] {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.children.as_slice())
            .unwrap_or(&[])
    }

    /// Walk all entries top-down (system nodes first, then widget subtrees
    /// in insertion order).
    pub fn walk_topdown<F: FnMut(LayoutNodeId, &LayoutTreeEntry)>(&self, mut f: F) {
        for entry in &self.entries {
            f(entry.id, entry);
        }
    }

    /// Return the parent chain for a node from the outermost ancestor down to
    /// (but not including) the node itself.
    pub fn parent_chain(&self, id: LayoutNodeId) -> Vec<LayoutNodeId> {
        let mut chain = Vec::new();
        let mut current = id;
        while let Some(pid) = self.parent_of(current) {
            chain.push(pid);
            current = pid;
        }
        chain.reverse();
        chain
    }

    /// Find the nearest ancestor that is a composite `WidgetNode`, walking from
    /// `id` upward. Returns `(ancestor_id, kind)` or `None`.
    pub fn composite_ancestor(&self, id: LayoutNodeId) -> Option<(LayoutNodeId, WidgetKind)> {
        let mut current = id;
        loop {
            let parent = self.parent_of(current)?;
            if let Some(entry) = self.entry(parent) {
                if let LayoutNode::Widget(ref w) = entry.node {
                    if w.kind.is_composite() {
                        return Some((parent, w.kind));
                    }
                }
            }
            current = parent;
        }
    }

    /// Find the nearest ancestor `WidgetNode` (composite or atomic).
    /// Returns `None` when no widget parent exists (node is parented to system node or root).
    pub fn widget_parent(&self, id: LayoutNodeId) -> Option<WidgetId> {
        let parent_id = self.parent_of(id)?;
        let entry = self.entry(parent_id)?;
        if let LayoutNode::Widget(ref w) = entry.node {
            Some(w.id.clone())
        } else {
            None
        }
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}
