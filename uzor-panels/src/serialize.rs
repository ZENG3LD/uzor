//! Serialization module for saving/loading docking layouts.
//!
//! Serializes the tree structure (IDs, layout modes, proportions) but NOT
//! the panel content. The consumer provides a factory function to restore panels.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::{DockingTree, DockPanel, Leaf, Branch, PanelNode, WindowLayout, LeafId, BranchId};

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
    },
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

    /// Restore tree structure (consumer provides panel factory)
    pub fn restore_tree<P, F>(&self, mut create_panel: F) -> Result<DockingTree<P>, String>
    where
        P: DockPanel,
        F: FnMut(&str) -> Option<P>,
    {
        // Build node lookup map
        let node_map: HashMap<u64, &SerializedNode> = self.nodes.iter()
            .map(|n| (n.id, n))
            .collect();

        // Find root node
        let root_node = node_map.get(&self.root_id)
            .ok_or_else(|| format!("Root node {} not found", self.root_id))?;

        // Restore root branch
        let root_branch = Self::restore_branch(root_node, &node_map, &mut create_panel)?;

        // Create tree with restored structure
        let tree = DockingTree::from_restored_structure(
            root_branch,
            self.active_leaf_id.map(LeafId),
            self.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1,
        );

        Ok(tree)
    }

    fn restore_branch<P, F>(
        node: &SerializedNode,
        node_map: &HashMap<u64, &SerializedNode>,
        create_panel: &mut F,
    ) -> Result<Branch<P>, String>
    where
        P: DockPanel,
        F: FnMut(&str) -> Option<P>,
    {
        match &node.node_type {
            SerializedNodeType::Branch { children, layout, proportions, cross_ratio } => {
                let layout_enum = Self::string_to_layout(layout)?;

                // Restore children
                let mut child_nodes = Vec::new();
                for child_id in children {
                    let child_node = node_map.get(child_id)
                        .ok_or_else(|| format!("Child node {} not found", child_id))?;

                    let panel_node = match &child_node.node_type {
                        SerializedNodeType::Leaf { .. } => {
                            PanelNode::Leaf(Self::restore_leaf(child_node, create_panel)?)
                        }
                        SerializedNodeType::Branch { .. } => {
                            PanelNode::Branch(Self::restore_branch(child_node, node_map, create_panel)?)
                        }
                    };
                    child_nodes.push(panel_node);
                }

                Ok(Branch {
                    id: BranchId(node.id),
                    children: child_nodes,
                    layout: layout_enum,
                    custom_rects: Vec::new(),
                    proportions: proportions.clone(),
                    cross_ratio: *cross_ratio,
                })
            }
            _ => Err(format!("Expected branch node, got leaf for id {}", node.id)),
        }
    }

    fn restore_leaf<P, F>(
        node: &SerializedNode,
        create_panel: &mut F,
    ) -> Result<Leaf<P>, String>
    where
        P: DockPanel,
        F: FnMut(&str) -> Option<P>,
    {
        match &node.node_type {
            SerializedNodeType::Leaf { panel_type_ids, active_tab, hidden, color_tag } => {
                // Create panels using factory
                let mut panels = Vec::new();
                for type_id in panel_type_ids {
                    if let Some(panel) = create_panel(type_id) {
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
}
