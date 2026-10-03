//! Serialization module for saving/loading docking layouts.
//!
//! Serializes the tree structure (IDs, layout modes, proportions) but NOT
//! the panel content. The consumer provides a factory function to restore panels.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use super::{DockingTree, DockPanel, Leaf, Branch, PanelNode, WindowLayout, LeafId, BranchId, GridSpec};

/// Serialized tree layout (structure only, no panel content)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutSnapshot {
    pub version: String,  // "1.0"
    pub name: String,
    pub nodes: Vec<SerializedNode>,
    pub root_id: u64,
    pub active_leaf_id: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializedNode {
    pub id: u64,
    pub node_type: SerializedNodeType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SerializedNodeType {
    Leaf {
        panel_type_ids: Vec<String>,  // DockPanel::type_id() for each panel
        active_tab: usize,
        hidden: bool,
        color_tag: Option<u8>,
    },
    Branch {
        children: Vec<u64>,  // child node IDs
        layout: String,      // WindowLayout name (serialized)
        proportions: Vec<f64>,
        cross_ratio: Option<(f64, f64)>,
        /// Absent in snapshots written before the field existed; those load
        /// as `true`, the behaviour they were saved with.
        #[serde(default = "default_magnetic")]
        magnetic: bool,
        /// `rows × cols` grid shape and ratios. Absent (and not written) for
        /// branches that are not grids, so snapshots of preset-only trees
        /// are unchanged and older snapshots load with no grid.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grid: Option<SerializedGrid>,
    },
}

/// Wire form of a [`GridSpec`]: a branch laid out as `rows × cols` cells
/// (row-major children) with independent row and column weights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerializedGrid {
    pub rows: usize,
    pub cols: usize,
    pub row_ratios: Vec<f64>,
    pub col_ratios: Vec<f64>,
}

impl From<&GridSpec> for SerializedGrid {
    fn from(g: &GridSpec) -> Self {
        Self { rows: g.rows, cols: g.cols, row_ratios: g.row_ratios.clone(), col_ratios: g.col_ratios.clone() }
    }
}

impl SerializedGrid {
    /// Back to a [`GridSpec`]. Ratio blocks of the wrong length or with a
    /// non-positive weight are replaced by equal ratios; a zero dimension
    /// yields `None` (the branch loads as a plain preset).
    fn to_spec(&self) -> Option<GridSpec> {
        let mut spec = GridSpec::new(self.rows, self.cols)?;
        if self.row_ratios.len() == self.rows && GridSpec::valid_block(&self.row_ratios) {
            spec.row_ratios = self.row_ratios.clone();
        }
        if self.col_ratios.len() == self.cols && GridSpec::valid_block(&self.col_ratios) {
            spec.col_ratios = self.col_ratios.clone();
        }
        Some(spec)
    }
}

fn default_magnetic() -> bool {
    true
}

impl LayoutSnapshot {
    /// Create snapshot from a DockingTree
    pub fn from_tree<P: DockPanel>(tree: &DockingTree<P>, name: &str) -> Self {
        let mut nodes = Vec::new();
        let root_id = tree.root().id.0;

        // Walk the tree recursively
        Self::serialize_branch(tree.root(), &mut nodes);

        LayoutSnapshot {
            version: "1.0".to_string(),
            name: name.to_string(),
            nodes,
            root_id,
            active_leaf_id: tree.active_leaf_id().map(|id| id.0),
        }
    }

    fn serialize_branch<P: DockPanel>(branch: &Branch<P>, nodes: &mut Vec<SerializedNode>) {
        // Collect child IDs
        let child_ids: Vec<u64> = branch.children.iter().map(|c| c.raw_id()).collect();

        // Serialize branch node
        nodes.push(SerializedNode {
            id: branch.id.0,
            node_type: SerializedNodeType::Branch {
                children: child_ids,
                layout: Self::layout_to_string(branch.layout),
                proportions: branch.proportions.clone(),
                cross_ratio: branch.cross_ratio,
                magnetic: branch.magnetic,
                grid: branch.grid.as_ref().map(SerializedGrid::from),
            },
        });

        // Recurse into children
        for child in &branch.children {
            match child {
                PanelNode::Leaf(leaf) => Self::serialize_leaf(leaf, nodes),
                PanelNode::Branch(branch) => Self::serialize_branch(branch, nodes),
            }
        }
    }

    fn serialize_leaf<P: DockPanel>(leaf: &Leaf<P>, nodes: &mut Vec<SerializedNode>) {
        let panel_type_ids: Vec<String> = leaf.panels.iter()
            .map(|p| p.type_id().to_string())
            .collect();

        nodes.push(SerializedNode {
            id: leaf.id.0,
            node_type: SerializedNodeType::Leaf {
                panel_type_ids,
                active_tab: leaf.active_tab,
                hidden: leaf.hidden,
                color_tag: leaf.color_tag,
            },
        });
    }

    fn layout_to_string(layout: WindowLayout) -> String {
        match layout {
            WindowLayout::Single => "Single".to_string(),
            WindowLayout::SplitHorizontal => "SplitHorizontal".to_string(),
            WindowLayout::SplitVertical => "SplitVertical".to_string(),
            WindowLayout::Grid2x2 => "Grid2x2".to_string(),
            WindowLayout::TwoLeftOneRight => "TwoLeftOneRight".to_string(),
            WindowLayout::OneLeftTwoRight => "OneLeftTwoRight".to_string(),
            WindowLayout::TwoTopOneBottom => "TwoTopOneBottom".to_string(),
            WindowLayout::OneTopTwoBottom => "OneTopTwoBottom".to_string(),
            WindowLayout::ThreeColumns => "ThreeColumns".to_string(),
            WindowLayout::ThreeRows => "ThreeRows".to_string(),
            WindowLayout::Custom => "Custom".to_string(),
        }
    }

    fn string_to_layout(s: &str) -> Result<WindowLayout, String> {
        match s {
            "Single" => Ok(WindowLayout::Single),
            "SplitHorizontal" => Ok(WindowLayout::SplitHorizontal),
            "SplitVertical" => Ok(WindowLayout::SplitVertical),
            "Grid2x2" => Ok(WindowLayout::Grid2x2),
            "TwoLeftOneRight" => Ok(WindowLayout::TwoLeftOneRight),
            "OneLeftTwoRight" => Ok(WindowLayout::OneLeftTwoRight),
            "TwoTopOneBottom" => Ok(WindowLayout::TwoTopOneBottom),
            "OneTopTwoBottom" => Ok(WindowLayout::OneTopTwoBottom),
            "ThreeColumns" => Ok(WindowLayout::ThreeColumns),
            "ThreeRows" => Ok(WindowLayout::ThreeRows),
            "Custom" => Ok(WindowLayout::Custom),
            _ => Err(format!("Unknown layout: {}", s)),
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize layout: {}", e))
    }

    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json)
            .map_err(|e| format!("Failed to deserialize layout: {}", e))
    }

    /// Restore tree structure (consumer provides panel factory).
    ///
    /// The factory closure receives `type_id` (the value returned by
    /// [`DockPanel::type_id`] during serialization) and must return the
    /// reconstructed panel, or `None` to signal a failure.
    pub fn restore_tree<P, F>(&self, mut create_panel: F) -> Result<DockingTree<P>, String>
    where
        P: DockPanel,
        F: FnMut(&str) -> Option<P>,
    {
        // Wrap into the id-aware variant; ignore leaf_id.
        self.restore_tree_with_id(|_leaf_id, type_id| create_panel(type_id))
    }

    /// Restore tree structure with leaf-id aware panel factory.
    ///
    /// Like [`restore_tree`] but the factory closure also receives the
    /// `leaf_id` of the leaf node being reconstructed.  This allows callers
    /// to look up per-leaf persisted data (e.g. panel state) without encoding
    /// any runtime identity into [`DockPanel::type_id`], which must remain a
    /// zero-allocation `&'static str`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let by_leaf: HashMap<u64, &PersistedLeaf> = …;
    /// snap.restore_tree_with_id::<MyPanel, _>(|leaf_id, _type_id| {
    ///     let leaf = by_leaf.get(&leaf_id)?;
    ///     Some(MyPanel::from_persisted(leaf))
    /// })
    /// ```
    pub fn restore_tree_with_id<P, F>(&self, mut create_panel: F) -> Result<DockingTree<P>, String>
    where
        P: DockPanel,
        F: FnMut(u64, &str) -> Option<P>,
    {
        // Build node lookup map
        let node_map: HashMap<u64, &SerializedNode> = self.nodes.iter()
            .map(|n| (n.id, n))
            .collect();

        // Find root node
        let root_node = node_map.get(&self.root_id)
            .ok_or_else(|| format!("Root node {} not found", self.root_id))?;

        // Restore root branch
        let root_branch = Self::restore_branch_with_id(root_node, &node_map, &mut create_panel)?;

        // Create tree with restored structure
        let tree = DockingTree::from_restored_structure(
            root_branch,
            self.active_leaf_id.map(LeafId),
            self.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1,
        );

        Ok(tree)
    }

    fn restore_branch_with_id<P, F>(
        node: &SerializedNode,
        node_map: &HashMap<u64, &SerializedNode>,
        create_panel: &mut F,
    ) -> Result<Branch<P>, String>
    where
        P: DockPanel,
        F: FnMut(u64, &str) -> Option<P>,
    {
        match &node.node_type {
            SerializedNodeType::Branch { children, layout, proportions, cross_ratio, magnetic, grid } => {
                let layout_enum = Self::string_to_layout(layout)?;

                // Restore children
                let mut child_nodes = Vec::new();
                for child_id in children {
                    let child_node = node_map.get(child_id)
                        .ok_or_else(|| format!("Child node {} not found", child_id))?;

                    let panel_node = match &child_node.node_type {
                        SerializedNodeType::Leaf { .. } => {
                            PanelNode::Leaf(Self::restore_leaf_with_id(child_node, create_panel)?)
                        }
                        SerializedNodeType::Branch { .. } => {
                            PanelNode::Branch(Self::restore_branch_with_id(child_node, node_map, create_panel)?)
                        }
                    };
                    child_nodes.push(panel_node);
                }

                // Kept only while it still fits the restored children.
                let restored_grid = grid.as_ref()
                    .and_then(SerializedGrid::to_spec)
                    .filter(|g| g.cell_count() == child_nodes.len());

                Ok(Branch {
                    id: BranchId(node.id),
                    children: child_nodes,
                    layout: layout_enum,
                    custom_rects: Vec::new(),
                    proportions: proportions.clone(),
                    cross_ratio: *cross_ratio,
                    // Not part of the serialized format yet — consumer should
                    // re-mark preserve_if_empty after restore via the (future)
                    // tree-walk hook.  Defaulting to false matches the
                    // pre-existing aggressive-collapse behavior.
                    preserve_if_empty: false,
                    magnetic: *magnetic,
                    grid: restored_grid,
                })
            }
            _ => Err(format!("Expected branch node, got leaf for id {}", node.id)),
        }
    }

    fn restore_leaf_with_id<P, F>(
        node: &SerializedNode,
        create_panel: &mut F,
    ) -> Result<Leaf<P>, String>
    where
        P: DockPanel,
        F: FnMut(u64, &str) -> Option<P>,
    {
        match &node.node_type {
            SerializedNodeType::Leaf { panel_type_ids, active_tab, hidden, color_tag } => {
                // Create panels using factory; pass the leaf node id so callers
                // can match against persisted-leaf descriptors without encoding
                // panel identity into type_id.
                let mut panels = Vec::new();
                for type_id in panel_type_ids {
                    if let Some(panel) = create_panel(node.id, type_id) {
                        panels.push(panel);
                    } else {
                        return Err(format!("Failed to create panel with type_id: {}", type_id));
                    }
                }

                if panels.is_empty() {
                    return Err(format!("No panels restored for leaf {}", node.id));
                }

                // Clamp active_tab
                let active_tab = (*active_tab).min(panels.len() - 1);

                Ok(Leaf {
                    id: LeafId(node.id),
                    panels,
                    active_tab,
                    hidden: *hidden,
                    color_tag: *color_tag,
                })
            }
            _ => Err(format!("Expected leaf node, got branch for id {}", node.id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestPanel {
        title: String,
        type_id: &'static str,
    }

    impl DockPanel for TestPanel {
        fn title(&self) -> &str {
            &self.title
        }

        fn type_id(&self) -> &'static str {
            self.type_id
        }
    }

    #[test]
    fn test_serialize_deserialize_single_leaf() {
        let panel = TestPanel { title: "Test".to_string(), type_id: "test" };
        let tree = DockingTree::with_single_leaf(panel);

        let snapshot = LayoutSnapshot::from_tree(&tree, "test_layout");
        let json = snapshot.to_json().unwrap();

        let restored_snapshot = LayoutSnapshot::from_json(&json).unwrap();
        assert_eq!(restored_snapshot.name, "test_layout");
        assert_eq!(restored_snapshot.version, "1.0");
    }

    #[test]
    fn test_restore_tree() {
        let panel = TestPanel { title: "Test".to_string(), type_id: "test" };
        let tree = DockingTree::with_single_leaf(panel);

        let snapshot = LayoutSnapshot::from_tree(&tree, "test_layout");

        let restored_tree = snapshot.restore_tree(|type_id| {
            if type_id == "test" {
                Some(TestPanel { title: "Test".to_string(), type_id: "test" })
            } else {
                None
            }
        }).unwrap();

        assert_eq!(restored_tree.leaf_count(), 1);
        assert_eq!(restored_tree.layout(), WindowLayout::Single);
    }

    #[test]
    fn magnetic_round_trips_and_old_snapshots_load_as_magnetic() {
        let panel = TestPanel { title: "T".to_string(), type_id: "test" };
        let mut tree = DockingTree::with_single_leaf(panel.clone());
        tree.add_leaf(panel.clone());
        let root_id = tree.root().id;
        tree.find_branch_mut(root_id).unwrap().magnetic = false;

        let json = LayoutSnapshot::from_tree(&tree, "m").to_json().unwrap();
        let restored = LayoutSnapshot::from_json(&json)
            .unwrap()
            .restore_tree(|_| Some(panel.clone()))
            .unwrap();
        assert!(!restored.root().magnetic);

        // A snapshot written before the field existed: drop every "magnetic" key.
        fn strip(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::Object(map) => {
                    map.remove("magnetic");
                    map.values_mut().for_each(strip);
                }
                serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
                _ => {}
            }
        }
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(json.contains("magnetic"), "the field is written: {json}");
        strip(&mut value);
        let old = value.to_string();
        assert!(!old.contains("magnetic"));
        let restored = LayoutSnapshot::from_json(&old)
            .unwrap()
            .restore_tree(|_| Some(panel.clone()))
            .unwrap();
        assert!(restored.root().magnetic);
    }
}


#[cfg(test)]
mod grid_tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct T;
    impl DockPanel for T {
        fn title(&self) -> &str { "t" }
        fn type_id(&self) -> &'static str { "t" }
    }

    fn strip_key(v: &mut serde_json::Value, key: &str) {
        match v {
            serde_json::Value::Object(map) => {
                map.remove(key);
                map.values_mut().for_each(|x| strip_key(x, key));
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|x| strip_key(x, key)),
            _ => {}
        }
    }

    #[test]
    fn rxc_branch_round_trips_rows_cols_and_ratios() {
        // Root split: leaf | 2×3 grid with uneven rows and columns.
        let mut tree = DockingTree::with_single_leaf(T);
        let second = tree.add_leaf(T);
        let (grid_id, cells) = tree.wrap_leaf_in_grid(second, 2, 3, vec![T; 5]).unwrap();
        assert!(tree.set_grid_row_ratios(grid_id, vec![0.25, 0.75]));
        assert!(tree.set_grid_col_ratios(grid_id, vec![0.2, 0.5, 0.3]));

        let json = LayoutSnapshot::from_tree(&tree, "g").to_json().unwrap();
        let restored = LayoutSnapshot::from_json(&json).unwrap()
            .restore_tree(|_| Some(T)).unwrap();

        let g = restored.branch_grid(grid_id).expect("grid restored");
        assert_eq!((g.rows, g.cols), (2, 3));
        assert_eq!(g.row_ratios, vec![0.25, 0.75]);
        assert_eq!(g.col_ratios, vec![0.2, 0.5, 0.3]);
        assert_eq!(restored.find_branch(grid_id).unwrap().layout, WindowLayout::Custom);
        for id in &cells {
            assert_eq!(
                restored.rect_for_leaf(*id, 1000.0, 800.0),
                tree.rect_for_leaf(*id, 1000.0, 800.0),
            );
        }
    }

    #[test]
    fn preset_only_snapshots_carry_no_grid_key_and_old_snapshots_load_without_a_grid() {
        let mut tree = DockingTree::with_single_leaf(T);
        tree.add_leaf(T);
        tree.add_leaf(T);
        let json = LayoutSnapshot::from_tree(&tree, "p").to_json().unwrap();
        assert!(!json.contains("grid"), "non-grid branches write nothing new: {json}");

        // A snapshot written before the field existed (grid key removed from
        // a grid tree's snapshot) loads as today's preset branch.
        let mut grid_tree = DockingTree::with_grid(1, 3, vec![T; 3]).unwrap();
        let root = grid_tree.root().id;
        assert!(grid_tree.set_grid_col_ratios(root, vec![1.0, 2.0, 1.0]));
        let mut value: serde_json::Value = serde_json::from_str(
            &LayoutSnapshot::from_tree(&grid_tree, "g").to_json().unwrap(),
        ).unwrap();
        strip_key(&mut value, "grid");
        let old = value.to_string();
        assert!(!old.contains("grid"));
        let restored = LayoutSnapshot::from_json(&old).unwrap()
            .restore_tree(|_| Some(T)).unwrap();
        assert!(restored.root().grid.is_none());
        assert_eq!(restored.layout(), WindowLayout::Custom);
        assert!(restored.root().magnetic);
    }

    #[test]
    fn a_grid_that_no_longer_fits_its_children_is_dropped_on_restore() {
        let tree = DockingTree::with_grid(2, 2, vec![T; 4]).unwrap();
        let mut snap = LayoutSnapshot::from_tree(&tree, "g");
        for node in &mut snap.nodes {
            if let SerializedNodeType::Branch { grid: Some(g), .. } = &mut node.node_type {
                g.cols = 3; // 2 × 3 = 6 ≠ 4 children
            }
        }
        let restored = snap.restore_tree(|_| Some(T)).unwrap();
        assert!(restored.root().grid.is_none());
    }
}
