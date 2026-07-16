//! Generic node/edge graph model — no forensic/case/domain knowledge.
//!
//! Node and edge payloads (`N`, `E`) are opaque to the engine: the
//! simulation, camera, and renderer only ever touch `label`, `category`
//! (a plain string tag used for deterministic color + a future
//! collapse-predicate seam, see [`crate::cluster`]), and `radius`/
//! `weight`. Callers stash whatever domain data they want in `payload`.

/// Index of a node inside a [`Graph`]. Stable for the lifetime of the
/// graph (nodes are append-only this run — see [`Graph::push_node`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeIndex(pub u32);

/// Index of an edge inside a [`Graph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeIndex(pub u32);

impl NodeIndex {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl EdgeIndex {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// `uzor_figures::interact::FocusSet` is a flat `u64` key space (it has
/// no node-link identity type of its own — that stays here). A node and
/// an edge can share the same raw index (`NodeIndex(3)` / `EdgeIndex(3)`)
/// without colliding inside one `FocusSet` because the low bit is
/// tagged: node keys are even, edge keys are odd.
impl From<NodeIndex> for u64 {
    fn from(id: NodeIndex) -> u64 {
        (id.0 as u64) << 1
    }
}

impl From<EdgeIndex> for u64 {
    fn from(id: EdgeIndex) -> u64 {
        ((id.0 as u64) << 1) | 1
    }
}

/// One node's static (non-simulated) data.
pub struct GraphNode<N> {
    pub payload: N,
    pub label: String,
    /// Opaque category tag — used for deterministic color ([`crate::render::category_color`])
    /// and as the seam a future collapse predicate would key on
    /// ([`crate::cluster`]). Not interpreted structurally by the engine.
    pub category: String,
    /// Base visual/collision radius in world units, layout-independent.
    /// Caller sets this (e.g. from degree via [`Graph::degree`]) after
    /// edges are known — see [`Graph::set_radius`].
    pub radius: f32,
}

/// One edge's static (non-simulated) data.
pub struct GraphEdge<E> {
    pub from: NodeIndex,
    pub to: NodeIndex,
    /// Spring-strength / ideal-length input for `LinkForce`.
    pub weight: f32,
    pub payload: E,
}

/// Minimal, payload-free copy of an edge — what the simulation actually
/// needs. Kept in lockstep with `edges` at the same index so `Layout`
/// implementations never have to be generic over `E`.
#[derive(Clone, Copy, Debug)]
pub struct SimEdge {
    pub from: NodeIndex,
    pub to: NodeIndex,
    pub weight: f32,
}

/// Read-only topology view handed to [`crate::layout::Layout::tick`] —
/// structure only, no node/edge payload. Built fresh from [`Graph`] each
/// tick today; from a future clustering milestone onward this is what a
/// cluster-collapsed `ActiveView` projection would produce instead,
/// without changing the `Layout` trait contract (see [`crate::cluster`]).
pub struct SimTopology<'a> {
    pub node_count: usize,
    pub edges: &'a [SimEdge],
    pub degree: &'a [u32],
    /// Base radius per node, snapshotted fresh each call (radius can be
    /// changed after construction via [`Graph::set_radius`], so this is
    /// never cached — see that method's doc comment).
    pub radii: Vec<f32>,
}

/// The graph as the app declares it — static topology + payload.
///
/// Position/velocity live separately in a parallel `Vec<`[`crate::particle::Particle`]`>`
/// owned by [`crate::engine::GraphEngine`] (SoA split, design doc §2.3) —
/// `Graph` itself never touches simulated state.
pub struct Graph<N, E> {
    nodes: Vec<GraphNode<N>>,
    edges: Vec<GraphEdge<E>>,
    sim_edges: Vec<SimEdge>,
    adjacency: Vec<Vec<EdgeIndex>>,
    degree: Vec<u32>,
}

impl<N, E> Default for Graph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N, E> Graph<N, E> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            sim_edges: Vec::new(),
            adjacency: Vec::new(),
            degree: Vec::new(),
        }
    }

    /// Append a node. Returns its stable [`NodeIndex`].
    pub fn push_node(
        &mut self,
        payload: N,
        label: impl Into<String>,
        category: impl Into<String>,
        radius: f32,
    ) -> NodeIndex {
        let id = NodeIndex(self.nodes.len() as u32);
        self.nodes.push(GraphNode {
            payload,
            label: label.into(),
            category: category.into(),
            radius,
        });
        self.adjacency.push(Vec::new());
        self.degree.push(0);
        id
    }

    /// Append an edge between two already-pushed nodes.
    pub fn push_edge(&mut self, from: NodeIndex, to: NodeIndex, weight: f32, payload: E) -> EdgeIndex {
        let id = EdgeIndex(self.edges.len() as u32);
        self.edges.push(GraphEdge { from, to, weight, payload });
        self.sim_edges.push(SimEdge { from, to, weight });
        if let Some(adj) = self.adjacency.get_mut(from.index()) {
            adj.push(id);
        }
        if let Some(adj) = self.adjacency.get_mut(to.index()) {
            adj.push(id);
        }
        if let Some(d) = self.degree.get_mut(from.index()) {
            *d += 1;
        }
        if let Some(d) = self.degree.get_mut(to.index()) {
            *d += 1;
        }
        id
    }

    /// Overwrite a node's base radius (e.g. once degree is known, after
    /// all edges are pushed). Not cached anywhere else — see
    /// [`SimTopology::radii`].
    pub fn set_radius(&mut self, id: NodeIndex, radius: f32) {
        if let Some(node) = self.nodes.get_mut(id.index()) {
            node.radius = radius;
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Direct accessor — panics on an out-of-range index. Safe to use
    /// wherever the index provably came from this graph (iteration,
    /// [`Graph::push_node`] return values); untrusted indices (e.g. from
    /// an HTTP agent action) must go through [`Graph::get_node`] first.
    pub fn node(&self, id: NodeIndex) -> &GraphNode<N> {
        &self.nodes[id.index()]
    }

    pub fn get_node(&self, id: NodeIndex) -> Option<&GraphNode<N>> {
        self.nodes.get(id.index())
    }

    pub fn edge(&self, id: EdgeIndex) -> &GraphEdge<E> {
        &self.edges[id.index()]
    }

    pub fn get_edge(&self, id: EdgeIndex) -> Option<&GraphEdge<E>> {
        self.edges.get(id.index())
    }

    pub fn nodes(&self) -> impl Iterator<Item = (NodeIndex, &GraphNode<N>)> {
        self.nodes.iter().enumerate().map(|(i, n)| (NodeIndex(i as u32), n))
    }

    pub fn edges(&self) -> impl Iterator<Item = (EdgeIndex, &GraphEdge<E>)> {
        self.edges.iter().enumerate().map(|(i, e)| (EdgeIndex(i as u32), e))
    }

    pub fn degree(&self, id: NodeIndex) -> u32 {
        self.degree.get(id.index()).copied().unwrap_or(0)
    }

    pub fn incident_edges(&self, id: NodeIndex) -> &[EdgeIndex] {
        self.adjacency.get(id.index()).map(Vec::as_slice).unwrap_or(&[])
    }

    /// First node whose label matches exactly, if any. Linear scan —
    /// fine at demo scale; not a substitute for a real lookup index.
    pub fn find_by_label(&self, label: &str) -> Option<NodeIndex> {
        self.nodes()
            .find(|(_, n)| n.label == label)
            .map(|(id, _)| id)
    }

    /// Build the payload-free topology view the simulation needs.
    pub fn topology(&self) -> SimTopology<'_> {
        SimTopology {
            node_count: self.nodes.len(),
            edges: &self.sim_edges,
            degree: &self.degree,
            radii: self.nodes.iter().map(|n| n.radius).collect(),
        }
    }

    /// 1-hop neighborhood of `center` as `uzor_figures::interact::FocusSet`
    /// keys — `center` itself, every incident edge, and the far endpoint
    /// of each. Built from adjacency already cached at load (not a
    /// per-frame predicate scan). This is the behavior this crate's old
    /// `FocusSet::neighborhood` fork had that the generalized
    /// `uzor-figures` version can't express on its own (no graph type of
    /// its own) — [`crate::engine::GraphEngine::select`] hands the
    /// result straight to `FocusSet::select_many`.
    pub fn neighborhood_focus_keys(&self, center: NodeIndex) -> Vec<u64> {
        let mut keys = vec![u64::from(center)];
        for &eid in self.incident_edges(center) {
            if let Some(edge) = self.get_edge(eid) {
                keys.push(u64::from(eid));
                keys.push(u64::from(edge.from));
                keys.push(u64::from(edge.to));
            }
        }
        keys
    }
}
