//! Per-frame accessibility tree.

use crate::types::WidgetId;
use super::A11yNode;

/// Per-frame accessibility tree, rebuilt fresh every frame (immediate-mode,
/// mirrors `InputCoordinator::begin_frame`/`end_frame`'s own per-frame
/// widget list) rather than retained/diffed. Retained diffing against
/// AccessKit's `TreeUpdate` is an H7 concern once the adapter exists.
#[derive(Default, Debug, Clone)]
pub struct A11yTree {
    nodes: Vec<A11yNode>,
}

impl A11yTree {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove every node — call at the start of a frame before pushing this
    /// frame's nodes.
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// Add a node for this frame.
    pub fn push(&mut self, node: A11yNode) {
        self.nodes.push(node);
    }

    /// All nodes currently in the tree.
    pub fn nodes(&self) -> &[A11yNode] {
        &self.nodes
    }

    /// Find a node by its widget id.
    pub fn get(&self, id: &WidgetId) -> Option<&A11yNode> {
        self.nodes.iter().find(|n| &n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::A11yRole;
    use crate::types::Rect;

    fn node(id: &str) -> A11yNode {
        A11yNode::new(id, A11yRole::Button, id, Rect::new(0.0, 0.0, 10.0, 10.0))
    }

    #[test]
    fn new_tree_is_empty() {
        let tree = A11yTree::new();
        assert!(tree.nodes().is_empty());
    }

    #[test]
    fn push_adds_nodes_in_order() {
        let mut tree = A11yTree::new();
        tree.push(node("a"));
        tree.push(node("b"));
        assert_eq!(tree.nodes().len(), 2);
        assert_eq!(tree.nodes()[0].id, WidgetId::from("a"));
        assert_eq!(tree.nodes()[1].id, WidgetId::from("b"));
    }

    #[test]
    fn get_finds_node_by_id() {
        let mut tree = A11yTree::new();
        tree.push(node("a"));
        tree.push(node("b"));

        let found = tree.get(&WidgetId::from("b"));
        assert!(found.is_some());
        assert_eq!(found.map(|n| n.name.as_str()), Some("b"));

        assert!(tree.get(&WidgetId::from("missing")).is_none());
    }

    #[test]
    fn clear_empties_the_tree() {
        let mut tree = A11yTree::new();
        tree.push(node("a"));
        assert!(!tree.nodes().is_empty());

        tree.clear();
        assert!(tree.nodes().is_empty());
        assert!(tree.get(&WidgetId::from("a")).is_none());
    }
}
