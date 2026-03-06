//! Panel Tree System - Core Data Structures
//!
//! N-ary tree architecture with HashMap arena allocation for panel management.
//! This module provides the foundational data structures for a flexible panel
//! container system supporting tabs, splits, and grids.
//!
//! # Architecture
//!
//! - **PanelTree**: Root structure containing the tree hierarchy
//! - **PanelStore**: HashMap arena for efficient panel storage
//! - **Tile**: Either a Panel (leaf) or Container (branch)
//! - **Container**: Tabs, Linear splits, or Grid layouts
//!
//! # Usage
//!
//! ```rust,ignore
//! use uzor_panels::{PanelTree, Tile, DockPanel};
//!
//! // Create tree with single panel
//! let tree = PanelTree::with_panel(my_panel);
//!
//! // Create tree with tabs
//! let tree = PanelTree::with_tabs(vec![panel1, panel2]);
//! ```

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::DockPanel;
use crate::id::NodeId;

/// Arena-based storage for all panels and containers
/// Uses std::HashMap for compatibility
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanelStore<P: DockPanel> {
    tiles: HashMap<NodeId, Tile<P>>,
    next_id: u64,
}

impl<P: DockPanel> PanelStore<P> {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
            next_id: 1,
        }
    }

    /// Generate next sequential ID with collision check
    pub fn next_id(&mut self) -> NodeId {
        let mut id = NodeId(self.next_id);
        self.next_id += 1;

        // Collision check (if ID already exists, keep incrementing)
        while self.tiles.contains_key(&id) {
            id = NodeId(self.next_id);
            self.next_id += 1;
        }

        id
    }

    /// Insert tile and return its ID
    pub fn insert(&mut self, tile: Tile<P>) -> NodeId {
        let id = self.next_id();
        self.tiles.insert(id, tile);
        id
    }

    /// Get tile by ID (immutable)
    pub fn get(&self, id: NodeId) -> Option<&Tile<P>> {
        self.tiles.get(&id)
    }

    /// Get tile by ID (mutable)
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Tile<P>> {
        self.tiles.get_mut(&id)
    }

    /// Remove tile and return it
    pub fn remove(&mut self, id: NodeId) -> Option<Tile<P>> {
        self.tiles.remove(&id)
    }

    /// Iterator over all tiles
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Tile<P>)> {
        self.tiles.iter().map(|(&id, tile)| (id, tile))
    }

    /// Mutable iterator over all tiles
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut Tile<P>)> {
        self.tiles.iter_mut().map(|(&id, tile)| (id, tile))
    }

    /// Get number of tiles in store
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Insert tile with specific ID (for deserialization)
    ///
    /// This is used when loading layouts from JSON. Normally you should use
    /// `insert()` which auto-generates IDs.
    pub fn insert_with_id(&mut self, id: NodeId, tile: Tile<P>) {
        self.tiles.insert(id, tile);
        // Update next_id to ensure no collision with future inserts
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
    }
}

impl<P: DockPanel> Default for PanelStore<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Panel tree structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanelTree<P: DockPanel> {
    /// Root tile ID (None = empty tree)
    pub root: Option<NodeId>,
    /// Arena with all panels and containers
    pub tiles: PanelStore<P>,
}

impl<P: DockPanel> PanelTree<P> {
    /// Create empty tree
    pub fn new() -> Self {
        Self {
            root: None,
            tiles: PanelStore::new(),
        }
    }

    /// Create tree with single panel
    pub fn with_panel(panel: P) -> Self {
        let mut tree = Self::new();
        let id = tree.tiles.insert(Tile::Panel(panel));
        tree.root = Some(id);
        tree
    }

    /// Create tree with tabs container
    pub fn with_tabs(panels: Vec<P>) -> Self {
        let mut tree = Self::new();
        let panel_ids: Vec<NodeId> = panels
            .into_iter()
            .map(|p| tree.tiles.insert(Tile::Panel(p)))
            .collect();

        let tabs = Tabs {
            children: panel_ids.clone(),
            active: panel_ids.first().copied(),
        };

        let id = tree.tiles.insert(Tile::Container(Container::Tabs(tabs)));
        tree.root = Some(id);
        tree
    }
}

impl<P: DockPanel> Default for PanelTree<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Tile: either Panel (leaf) or Container (branch)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Tile<P: DockPanel> {
    Panel(P),
    Container(Container),
}

/// Container: Tabs, Linear splits, or Grid
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Container {
    Tabs(Tabs),
    Linear(Linear),
    Grid(Grid),
}

/// Tabs container: N children, one active
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tabs {
    pub children: Vec<NodeId>,
    pub active: Option<NodeId>,
}

/// Linear container: horizontal or vertical splits
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Linear {
    pub children: Vec<NodeId>,
    pub direction: LinearDirection,
    pub shares: Shares,
}

/// Direction for linear splits
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinearDirection {
    Horizontal, // Left to right
    Vertical,   // Top to bottom
}

/// Grid container: 2D grid layout
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub children: Vec<NodeId>, // Row-major order
    pub layout: GridLayout,
    pub col_shares: Vec<f32>, // Width per column
    pub row_shares: Vec<f32>, // Height per row
}

/// Grid layout mode
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GridLayout {
    Columns(usize), // Fixed column count
    Auto,           // Automatic (uses heuristic)
}

/// Proportional space allocation
///
/// Example: shares=[1.0, 2.0, 1.0] results in 25%, 50%, 25% distribution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shares {
    shares: HashMap<NodeId, f32>,
}

impl Shares {
    /// Create new empty shares map
    pub fn new() -> Self {
        Self {
            shares: HashMap::new(),
        }
    }

    /// Set share value for panel
    pub fn set(&mut self, id: NodeId, share: f32) {
        self.shares.insert(id, share);
    }

    /// Get share value for panel (default = 1.0)
    pub fn get(&self, id: NodeId) -> f32 {
        self.shares.get(&id).copied().unwrap_or(1.0)
    }

    /// Split available space according to shares
    /// Returns pixel sizes for each child
    pub fn split(&self, children: &[NodeId], total_size: f32) -> Vec<f32> {
        if children.is_empty() {
            return vec![];
        }

        let total_shares: f32 = children.iter().map(|&id| self.get(id)).sum();

        if total_shares <= 0.0 {
            // Equal distribution if no shares defined
            let size = total_size / children.len() as f32;
            return vec![size; children.len()];
        }

        children
            .iter()
            .map(|&id| (self.get(id) / total_shares) * total_size)
            .collect()
    }
}

impl Default for Shares {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestPanel {
        title: String,
    }

    impl DockPanel for TestPanel {
        fn title(&self) -> &str {
            &self.title
        }

        fn type_id(&self) -> &'static str {
            "test"
        }
    }

    #[test]
    fn test_panel_store_sequential_ids() {
        let mut store: PanelStore<TestPanel> = PanelStore::new();
        let id1 = store.next_id();
        let id2 = store.next_id();

        assert_eq!(id1.0, 1);
        assert_eq!(id2.0, 2);
    }

    #[test]
    fn test_panel_store_insert_get() {
        let mut store: PanelStore<TestPanel> = PanelStore::new();
        let panel = TestPanel { title: "Test".to_string() };
        let id = store.insert(Tile::Panel(panel));

        assert!(store.get(id).is_some());
    }

    #[test]
    fn test_panel_tree_single() {
        let panel = TestPanel { title: "Chart".to_string() };
        let tree = PanelTree::with_panel(panel);

        assert!(tree.root.is_some());
        assert_eq!(tree.tiles.len(), 1);
    }

    #[test]
    fn test_panel_tree_tabs() {
        let panels = vec![
            TestPanel { title: "Tab 1".to_string() },
            TestPanel { title: "Tab 2".to_string() },
        ];
        let tree = PanelTree::with_tabs(panels);

        assert!(tree.root.is_some());
        assert_eq!(tree.tiles.len(), 3); // 2 panels + 1 tabs container
    }

    #[test]
    fn test_shares_split_equal() {
        let shares = Shares::new();
        let ids = vec![NodeId(1), NodeId(2), NodeId(3)];
        let sizes = shares.split(&ids, 300.0);

        assert_eq!(sizes.len(), 3);
        for size in sizes {
            assert!((size - 100.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_shares_split_proportional() {
        let mut shares = Shares::new();
        let ids = vec![NodeId(1), NodeId(2), NodeId(3)];
        shares.set(NodeId(1), 1.0);
        shares.set(NodeId(2), 2.0);
        shares.set(NodeId(3), 1.0);

        let sizes = shares.split(&ids, 400.0);

        assert_eq!(sizes.len(), 3);
        assert!((sizes[0] - 100.0).abs() < 0.01); // 25%
        assert!((sizes[1] - 200.0).abs() < 0.01); // 50%
        assert!((sizes[2] - 100.0).abs() < 0.01); // 25%
    }
}
