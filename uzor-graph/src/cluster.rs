//! Cluster collapse — replace a caller-declared group of member nodes
//! with one super-node when collapsed: position = centroid of the
//! members at the moment of collapse, size ∝ member count (sub-linear,
//! sqrt-scaled — see [`supernode_radius`]), edge weights to every
//! outside neighbor aggregated (summed) across the whole cluster.
//! Expand restores the exact prior member positions (round-trip
//! identity — see `ClusterRegistry::expand`'s doc comment).
//!
//! Wiring: [`crate::engine::GraphEngine::define_cluster`]/
//! `collapse_cluster`/`expand_cluster`/`is_collapsed` drive this;
//! `render::{draw_cluster_edges, draw_cluster_supernodes}` paint the
//! collapsed state; a single click on a collapsed super-node expands it
//! (`GraphEngine::on_event`'s click-select path) — see that module's
//! doc comment for why this isn't a double-click gesture.
//!
//! Implementation choice: no literal particle/node array resize. One
//! member (`ClusterState::representative`, always `members[0]`) stands
//! in as the super-node — its own `Particle`/`GraphNode` entries are
//! reused (radius bumped, position moved to the centroid); every OTHER
//! member is pinned exactly onto the representative's position (so a
//! still-ticking force-directed layout can't drift them apart — the
//! stack of coincident pinned particles is physically equivalent to one
//! heavier point mass to every other node's repulsion/Barnes-Hut pass)
//! and excluded from the visible/pick/edge-draw set for as long as the
//! cluster stays collapsed. Cross-cluster edges are pre-aggregated per
//! outside neighbor (summed weight) at collapse time and drawn as one
//! synthetic line per neighbor instead of N raw overlapping lines (which
//! `render::draw_edges` excludes via `DrawContext::hidden`).

use std::collections::{HashMap, HashSet};

use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;

const SUPERNODE_BASE_RADIUS: f32 = 8.0;
const SUPERNODE_RADIUS_PER_SQRT_MEMBER: f32 = 2.2;

/// Sub-linear (sqrt) size scaling by member count — matches
/// `force_graph_demo`'s own degree-based radius convention
/// (`3.0 + degree.sqrt() * 1.6`), so a huge cluster doesn't become a
/// visually absurd disc.
fn supernode_radius(member_count: usize) -> f32 {
    SUPERNODE_BASE_RADIUS + (member_count as f32).sqrt() * SUPERNODE_RADIUS_PER_SQRT_MEMBER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub u32);

/// One cross-cluster neighbor once the cluster is collapsed — an
/// outside node plus the SUM of every raw edge weight connecting it to
/// any member.
#[derive(Debug, Clone, Copy)]
pub struct AggregatedEdge {
    pub outside: NodeIndex,
    pub weight: f32,
}

/// A caller-declared cluster and its runtime collapse state.
pub struct ClusterState {
    pub members: Vec<NodeIndex>,
    /// `members[0]` — stands in for the whole cluster once collapsed.
    pub representative: NodeIndex,
    collapsed: bool,
    /// Snapshot of every member's `(x, y)` at the moment of collapse —
    /// restored verbatim on expand (round-trip identity, not a
    /// re-simulated approximation).
    saved_positions: Vec<(f32, f32)>,
    /// The representative's own radius before collapse — restored on
    /// expand (collapse bumps it to reflect member count).
    saved_representative_radius: f32,
    /// Cross-cluster neighbors, summed by outside node. Recomputed once
    /// at collapse time from `Graph`'s current edges.
    aggregated: Vec<AggregatedEdge>,
}

impl ClusterState {
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    pub fn aggregated_edges(&self) -> &[AggregatedEdge] {
        &self.aggregated
    }
}

/// Registry of caller-declared clusters, keyed by [`GroupId`]. Owned by
/// [`crate::engine::GraphEngine`]; the pure aggregation/centroid math
/// lives here, `Particle`/`Graph` mutation happens only through
/// `collapse`/`expand`.
#[derive(Default)]
pub struct ClusterRegistry {
    clusters: HashMap<GroupId, ClusterState>,
    /// Node -> its cluster, for O(1) "is this node hidden right now"
    /// lookups during render/pick. A node belongs to at most one
    /// cluster — `define` doesn't validate overlap (this crate's usual
    /// "trust the caller" convention for topology/membership data).
    node_cluster: HashMap<NodeIndex, GroupId>,
    next_id: u32,
}

impl ClusterRegistry {
    /// Declare a cluster over `members` (first member becomes the
    /// collapse representative). `None` if `members` is empty or its
    /// first entry isn't a node in `graph`.
    pub fn define<N, E>(&mut self, graph: &Graph<N, E>, members: Vec<NodeIndex>) -> Option<GroupId> {
        let representative = *members.first()?;
        let saved_representative_radius = graph.get_node(representative)?.radius;
        let id = GroupId(self.next_id);
        self.next_id += 1;
        for &m in &members {
            self.node_cluster.insert(m, id);
        }
        self.clusters.insert(
            id,
            ClusterState {
                members,
                representative,
                collapsed: false,
                saved_positions: Vec::new(),
                saved_representative_radius,
                aggregated: Vec::new(),
            },
        );
        Some(id)
    }

    pub fn get(&self, id: GroupId) -> Option<&ClusterState> {
        self.clusters.get(&id)
    }

    pub fn cluster_of(&self, node: NodeIndex) -> Option<GroupId> {
        self.node_cluster.get(&node).copied()
    }

    pub fn is_collapsed(&self, id: GroupId) -> bool {
        self.clusters.get(&id).is_some_and(ClusterState::is_collapsed)
    }

    pub fn any_collapsed(&self) -> bool {
        self.clusters.values().any(|c| c.collapsed)
    }

    /// Every node currently hidden by some collapsed cluster — every
    /// member except that cluster's representative.
    pub fn hidden_nodes(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.clusters.values().filter(|c| c.collapsed).flat_map(|c| c.members.iter().skip(1).copied())
    }

    pub fn collapsed_clusters(&self) -> impl Iterator<Item = &ClusterState> {
        self.clusters.values().filter(|c| c.collapsed)
    }

    /// Sorted ids of every currently-collapsed cluster (deterministic
    /// order for agent-state JSON).
    pub fn collapsed_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.clusters.iter().filter(|(_, c)| c.collapsed).map(|(id, _)| id.0).collect();
        ids.sort_unstable();
        ids
    }

    pub fn iter(&self) -> impl Iterator<Item = (GroupId, &ClusterState)> {
        self.clusters.iter().map(|(&id, c)| (id, c))
    }

    /// Collapse `id`: snapshot every member's current position, sum
    /// cross-cluster edge weights per outside neighbor, move the
    /// representative to the member centroid, pin every other member
    /// onto that same point, and bump the representative's radius.
    /// Returns `false` (no-op) if `id` is unknown or already collapsed.
    pub fn collapse<N, E>(&mut self, id: GroupId, graph: &mut Graph<N, E>, particles: &mut [Particle]) -> bool {
        let Some(cluster) = self.clusters.get_mut(&id) else { return false };
        if cluster.collapsed {
            return false;
        }

        let saved_positions: Vec<(f32, f32)> = cluster
            .members
            .iter()
            .map(|&m| particles.get(m.index()).map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0)))
            .collect();

        let (sum_x, sum_y) = saved_positions.iter().fold((0.0f32, 0.0f32), |acc, &(x, y)| (acc.0 + x, acc.1 + y));
        let count = saved_positions.len().max(1) as f32;
        let centroid = (sum_x / count, sum_y / count);

        for &m in cluster.members.iter().skip(1) {
            if let Some(p) = particles.get_mut(m.index()) {
                p.pin(centroid.0, centroid.1);
            }
        }
        if let Some(p) = particles.get_mut(cluster.representative.index()) {
            p.x = centroid.0;
            p.y = centroid.1;
            p.vx = 0.0;
            p.vy = 0.0;
        }

        let aggregated = aggregate_cross_edges(graph, &cluster.members);
        graph.set_radius(cluster.representative, supernode_radius(cluster.members.len()));

        cluster.saved_positions = saved_positions;
        cluster.aggregated = aggregated;
        cluster.collapsed = true;
        true
    }

    /// Expand `id`: restore every member's EXACT pre-collapse position
    /// (the snapshot `collapse` took, not a re-simulated approximation
    /// — `collapse` then `expand` with no intervening `tick` is an
    /// identity round-trip), unpin them, and restore the representative's
    /// original radius. Returns `false` (no-op) if `id` is unknown or
    /// not currently collapsed.
    pub fn expand<N, E>(&mut self, id: GroupId, graph: &mut Graph<N, E>, particles: &mut [Particle]) -> bool {
        let Some(cluster) = self.clusters.get_mut(&id) else { return false };
        if !cluster.collapsed {
            return false;
        }
        for (&m, &(x, y)) in cluster.members.iter().zip(cluster.saved_positions.iter()) {
            if let Some(p) = particles.get_mut(m.index()) {
                p.x = x;
                p.y = y;
                p.vx = 0.0;
                p.vy = 0.0;
                p.unpin();
            }
        }
        graph.set_radius(cluster.representative, cluster.saved_representative_radius);
        cluster.collapsed = false;
        cluster.aggregated.clear();
        true
    }
}

/// Sum every raw edge weight between a cluster member and a node
/// outside the cluster, grouped by that outside node (edges between two
/// members are internal — they vanish, not aggregated, when collapsed).
/// Deterministic output order: first-seen outside node.
fn aggregate_cross_edges<N, E>(graph: &Graph<N, E>, members: &[NodeIndex]) -> Vec<AggregatedEdge> {
    let member_set: HashSet<NodeIndex> = members.iter().copied().collect();
    let mut sums: HashMap<NodeIndex, f32> = HashMap::new();
    let mut order: Vec<NodeIndex> = Vec::new();

    for &m in members {
        for &eid in graph.incident_edges(m) {
            let Some(edge) = graph.get_edge(eid) else { continue };
            let other = if edge.from == m { edge.to } else { edge.from };
            if member_set.contains(&other) {
                continue;
            }
            sums
                .entry(other)
                .and_modify(|w| *w += edge.weight)
                .or_insert_with(|| {
                    order.push(other);
                    edge.weight
                });
        }
    }

    order.into_iter().map(|outside| AggregatedEdge { outside, weight: sums[&outside] }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    type G = Graph<(), ()>;

    /// 4-member ring cluster plus one outside node connected to two
    /// different members (weights 2.0 and 3.0 — sums to 5.0).
    fn small_graph_with_cluster() -> (G, Vec<NodeIndex>, NodeIndex) {
        let mut graph = G::new();
        let outside = graph.push_node((), "outside", "x", 4.0);
        let mut members = Vec::new();
        for i in 0..4 {
            members.push(graph.push_node((), format!("m{i}"), "cluster", 4.0));
        }
        for i in 0..members.len() {
            graph.push_edge(members[i], members[(i + 1) % members.len()], 1.0, ());
        }
        graph.push_edge(members[0], outside, 2.0, ());
        graph.push_edge(outside, members[2], 3.0, ());
        (graph, members, outside)
    }

    #[test]
    fn collapse_aggregates_cross_cluster_edge_weights_and_centers_at_the_centroid() {
        let (mut graph, members, outside) = small_graph_with_cluster();
        let mut particles = vec![Particle::at(0.0, 0.0); graph.node_count()];
        particles[members[0].index()] = Particle::at(-10.0, 0.0);
        particles[members[1].index()] = Particle::at(0.0, -10.0);
        particles[members[2].index()] = Particle::at(10.0, 0.0);
        particles[members[3].index()] = Particle::at(0.0, 10.0);

        let mut registry = ClusterRegistry::default();
        let id = registry.define(&graph, members.clone()).expect("non-empty cluster");
        assert!(registry.collapse(id, &mut graph, &mut particles));
        assert!(!registry.collapse(id, &mut graph, &mut particles), "collapsing an already-collapsed cluster is a no-op");

        let cluster = registry.get(id).expect("cluster exists");
        assert!(cluster.is_collapsed());
        assert_eq!(cluster.member_count(), 4);

        let aggregated = cluster.aggregated_edges();
        assert_eq!(aggregated.len(), 1, "both cross-cluster edges land on the same outside node");
        assert_eq!(aggregated[0].outside, outside);
        assert_eq!(aggregated[0].weight, 5.0, "2.0 + 3.0 summed");

        // Representative sits at the centroid of the 4 pre-collapse positions.
        let rep = particles[cluster.representative.index()];
        assert!((rep.x - 0.0).abs() < 1e-5);
        assert!((rep.y - 0.0).abs() < 1e-5);

        // Every non-representative member is pinned exactly onto that centroid.
        for &m in &members[1..] {
            let p = particles[m.index()];
            assert!(p.is_pinned());
            assert!((p.x - rep.x).abs() < 1e-5);
            assert!((p.y - rep.y).abs() < 1e-5);
        }

        // Representative's radius grew to reflect the member count.
        assert!(graph.get_node(cluster.representative).unwrap().radius > 4.0);
    }

    #[test]
    fn expand_restores_exact_prior_member_positions_round_trip() {
        let (mut graph, members, _outside) = small_graph_with_cluster();
        let mut particles = vec![Particle::at(0.0, 0.0); graph.node_count()];
        let original: Vec<(f32, f32)> = vec![(1.0, 2.0), (3.0, -4.0), (-5.0, 6.0), (7.0, 8.0)];
        for (&m, &(x, y)) in members.iter().zip(&original) {
            particles[m.index()] = Particle::at(x, y);
        }
        let original_radius = graph.get_node(members[0]).unwrap().radius;

        let mut registry = ClusterRegistry::default();
        let id = registry.define(&graph, members.clone()).expect("non-empty cluster");

        assert!(registry.collapse(id, &mut graph, &mut particles));
        assert!(registry.expand(id, &mut graph, &mut particles));
        assert!(!registry.expand(id, &mut graph, &mut particles), "expanding an already-expanded cluster is a no-op");

        let cluster = registry.get(id).expect("cluster exists");
        assert!(!cluster.is_collapsed());
        for (&m, &(x, y)) in members.iter().zip(&original) {
            let p = particles[m.index()];
            assert_eq!((p.x, p.y), (x, y), "expand must restore the EXACT pre-collapse position");
            assert!(!p.is_pinned());
        }
        assert_eq!(graph.get_node(members[0]).unwrap().radius, original_radius);
    }

    #[test]
    fn cluster_of_and_hidden_nodes_reflect_collapse_state() {
        let (mut graph, members, outside) = small_graph_with_cluster();
        let mut particles = vec![Particle::at(0.0, 0.0); graph.node_count()];

        let mut registry = ClusterRegistry::default();
        let id = registry.define(&graph, members.clone()).expect("non-empty cluster");
        assert_eq!(registry.cluster_of(members[0]), Some(id));
        assert_eq!(registry.cluster_of(outside), None);
        assert!(!registry.any_collapsed());
        assert_eq!(registry.hidden_nodes().count(), 0);

        registry.collapse(id, &mut graph, &mut particles);
        assert!(registry.any_collapsed());
        let hidden: HashSet<NodeIndex> = registry.hidden_nodes().collect();
        assert_eq!(hidden.len(), members.len() - 1, "every member except the representative is hidden");
        assert!(!hidden.contains(&members[0]), "the representative itself stays visible");
    }
}
