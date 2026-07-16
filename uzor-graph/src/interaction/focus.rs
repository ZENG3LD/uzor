//! Shared dim/highlight set for hover-neighborhood, click-selection, and
//! (future) path-trace — one concept instead of three parallel,
//! drifting implementations (engine design doc §4.3).

use std::collections::HashSet;

use crate::graph::{EdgeIndex, Graph, NodeIndex};

#[derive(Debug, Clone, Default)]
pub struct FocusSet {
    pub nodes: HashSet<NodeIndex>,
    pub edges: HashSet<EdgeIndex>,
}

impl FocusSet {
    pub fn empty() -> Self {
        Self::default()
    }

    /// 1-hop neighborhood of `center`, built from `Graph`'s adjacency
    /// (already built once at load — not recomputed per frame from a
    /// predicate scan).
    pub fn neighborhood<N, E>(graph: &Graph<N, E>, center: NodeIndex) -> Self {
        let mut nodes = HashSet::new();
        let mut edges = HashSet::new();
        nodes.insert(center);
        for &eid in graph.incident_edges(center) {
            if let Some(edge) = graph.get_edge(eid) {
                edges.insert(eid);
                nodes.insert(edge.from);
                nodes.insert(edge.to);
            }
        }
        Self { nodes, edges }
    }

    pub fn is_active(&self) -> bool {
        !self.nodes.is_empty()
    }

    pub fn contains_node(&self, id: NodeIndex) -> bool {
        self.nodes.contains(&id)
    }

    pub fn contains_edge(&self, id: EdgeIndex) -> bool {
        self.edges.contains(&id)
    }
}
