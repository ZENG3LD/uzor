use serde::{Serialize, Deserialize};

/// Unique identifier for a leaf node (panel container with tabs)
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct LeafId(pub u64);

/// Unique identifier for a branch node (container with children)
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BranchId(pub u64);

/// Unique identifier for nodes in PanelTree arena
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl std::fmt::Display for LeafId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Leaf({})", self.0)
    }
}

impl std::fmt::Display for BranchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Branch({})", self.0)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Node({})", self.0)
    }
}
