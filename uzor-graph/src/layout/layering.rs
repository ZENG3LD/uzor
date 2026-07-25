//! Shared layer assignment + within-layer slot order, reused by both
//! [`super::hierarchical::HierarchicalLayout`] (cartesian: y = layer) and
//! [`super::radial::RadialLayout`] (polar: radius = layer — "same
//! layering but polar" per the design brief).
//!
//! **Thin adapter (2026-07-22 re-point) — this file no longer owns the
//! algorithm.** The Kahn longest-path layer assignment + one down-sweep/
//! one up-sweep barycenter within-layer ordering pass was lifted, logic
//! unchanged, into `uzor-figures::figure::dag::layering` for
//! `uzor-figures::figure::dag::DagFigure`'s own use (that crate must not
//! depend on this one). This file now converts `SimTopology`'s
//! `NodeIndex`-keyed edges to that module's plain `(usize, usize)`
//! pair-list API, calls it, and converts the result back — `uzor-graph`
//! already depends on `uzor-figures` (see this crate's `FocusSet`
//! re-point precedent), so the two implementations no longer need to be
//! kept in sync by hand. Behavior is bit-identical to the pre-re-point
//! algorithm (same Kahn longest-path layering, same one-down-sweep/
//! one-up-sweep barycenter ordering, same back-edge/self-loop/
//! out-of-range handling) — every test below is unchanged from before
//! this re-point and still passes against the SAME public
//! `compute_layering`/`Layering` shape.

use std::collections::VecDeque;

use crate::graph::{NodeIndex, SimTopology};

/// Which depth notion a radial layout ranks its rings/shells by — Wave G3
/// fix. The same underlying question ("how far is this node from a
/// root") has two individually-defensible but DIFFERENT answers for a
/// DAG whose nodes have more than one parent — traced concretely on this
/// module's own [`tests::multi_depth_parents_take_the_longest_path`]
/// fixture: node `D` has two parents at different depths (`C=1`, `G=2`).
/// [`DepthMetric::LongestPath`] (this file's own Kahn algorithm,
/// [`compute_layering`]) assigns `D` layer `3` (`1 + max(1, 2)`).
/// [`DepthMetric::ShortestPath`] (multi-source BFS, [`compute_bfs_layering`])
/// assigns `D` depth `2` (reached via `C` first). Before this wave,
/// `super::radial::RadialLayout` (2D) always used the former and
/// `super::radial_3d::RadialLayout3D` always used the latter — an
/// UNRECONCILED twin disagreement, not a deliberate choice either module
/// made about the OTHER's behavior. Both radial layouts now expose this
/// as a param, each keeping its own PRE-EXISTING default (doctrine: no
/// silent output change) — see `RadialParams::depth_metric`/
/// `RadialParams3D::depth_metric`'s own doc comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthMetric {
    /// Kahn topological layering (this file's own algorithm) — a node's
    /// depth is `1 + max(parent depths)`, guaranteeing every edge points
    /// from a strictly shallower layer to a strictly deeper one.
    LongestPath,
    /// Multi-source BFS — a node's depth is the fewest hops from ANY
    /// root, matching `super::radial_3d::RadialLayout3D`'s own
    /// pre-existing `compute_bfs_tree` algorithm/convention exactly (same
    /// root-detection rules, same "any still-unvisited node becomes its
    /// own secondary root" disconnected-component fallback).
    ShortestPath,
}

/// Layer assignment + within-layer slot order, computed once from a
/// [`SimTopology`]'s directed edges (`from` = parent, `to` = child).
#[derive(Debug, Clone, Default)]
pub struct Layering {
    /// `layer[i]` = depth of node `i` (`0` = root).
    pub layer: Vec<u32>,
    /// Node index -> its slot position within its own layer
    /// (`0..layer_len-1`, dense per layer), after the barycenter sweeps.
    pub slot: Vec<usize>,
    /// Node indices grouped by layer, already in slot order.
    pub layers: Vec<Vec<usize>>,
    /// Reduced (back-edge-free, self-loop-free) forward adjacency — the
    /// same graph layering/ordering used, exposed for
    /// [`super::radial`]'s leaf-count computation.
    pub children: Vec<Vec<usize>>,
}

/// Compute node depths + within-layer slot order.
///
/// Roots: `explicit_roots` if non-empty (filtered to valid indices),
/// otherwise every node with zero in-degree; if the graph is fully
/// cyclic (no zero-in-degree node exists), node `0` is used as a
/// fallback single root — a degenerate case (an all-cycle graph has no
/// natural "top") but still deterministic and NaN-free.
///
/// Cycles: broken by ignoring "back edges" found via one DFS pass from
/// the roots (then any still-unvisited node, for disconnected
/// components) — an edge landing on a node currently on the DFS
/// recursion stack is a back edge, excluded from BOTH layer assignment
/// and slot ordering. This is real cycle-breaking, not a no-op: e.g. an
/// intra-cluster mesh with a triangle still layers cleanly instead of
/// looping forever or leaving every node stuck at layer 0.
///
/// See `uzor_figures::figure::dag::layering::compute_layering` for the
/// full algorithm this delegates to — this wrapper only adapts
/// `SimTopology`/`NodeIndex` at the boundary.
pub fn compute_layering(topo: &SimTopology<'_>, explicit_roots: &[NodeIndex]) -> Layering {
    let pairs: Vec<(usize, usize)> = topo.edges.iter().map(|e| (e.from.index(), e.to.index())).collect();
    let roots: Vec<usize> = explicit_roots.iter().map(|r| r.index()).collect();

    let inner = uzor_figures::figure::dag::layering::compute_layering(topo.node_count, &pairs, &roots);

    Layering { layer: inner.layer, slot: inner.slot, layers: inner.layers, children: inner.children }
}

/// Multi-source BFS layering — the [`DepthMetric::ShortestPath`]
/// alternative to [`compute_layering`]'s own Kahn longest-path algorithm.
/// Same root-detection convention (explicit roots; else every
/// zero-in-degree node; else node `0` as a last-resort fallback for a
/// fully-cyclic graph; any node still unvisited after that becomes its
/// own secondary root — a disconnected component's own root), literally
/// mirroring `super::radial_3d::RadialLayout3D`'s own (private)
/// `compute_bfs_tree` — this is the shared home so `RadialLayout` (2D)
/// can select the identical metric without depending on the 3D module.
///
/// `slot`/`layers` order is BFS discovery order (deterministic, since
/// [`SimTopology::edges`]'s own order is fixed), NOT [`compute_layering`]'s
/// barycenter crossing-reduction sweep — a documented simplification:
/// this metric exists to make a caller's depth NUMBER match a
/// shortest-path sibling layout, not to reproduce Kahn's own
/// slot-ordering heuristic. `children` is this BFS run's own
/// single-parent spanning-tree structure — unlike [`compute_layering`]'s
/// `children` (which can list more than one parent pointing at the same
/// node for a general DAG), this one is a genuine tree by construction.
pub fn compute_bfs_layering(topo: &SimTopology<'_>, explicit_roots: &[NodeIndex]) -> Layering {
    let n = topo.node_count;
    let mut depth = vec![0u32; n];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut visited = vec![false; n];

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in topo.edges {
        let a = e.from.index();
        let b = e.to.index();
        if a < n && b < n && a != b {
            adjacency[a].push(b);
        }
    }

    let mut roots: Vec<usize> = explicit_roots.iter().map(|r| r.index()).filter(|&i| i < n).collect();
    if roots.is_empty() {
        let mut in_degree = vec![0u32; n];
        for e in topo.edges {
            let a = e.from.index();
            let b = e.to.index();
            if a < n && b < n && a != b {
                in_degree[b] += 1;
            }
        }
        roots = (0..n).filter(|&i| in_degree[i] == 0).collect();
    }
    if roots.is_empty() && n > 0 {
        roots = vec![0];
    }

    let mut queue: VecDeque<usize> = VecDeque::new();
    for &r in &roots {
        if !visited[r] {
            visited[r] = true;
            depth[r] = 0;
            queue.push_back(r);
        }
    }
    bfs_layering_fill(&adjacency, &mut visited, &mut depth, &mut children, &mut queue);

    // Disconnected components: any still-unvisited node becomes its own
    // secondary root (ascending index order — deterministic), same
    // convention `compute_layering`'s own algorithm uses.
    for i in 0..n {
        if !visited[i] {
            visited[i] = true;
            depth[i] = 0;
            queue.push_back(i);
            bfs_layering_fill(&adjacency, &mut visited, &mut depth, &mut children, &mut queue);
        }
    }

    let max_layer = depth.iter().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); if n == 0 { 0 } else { max_layer as usize + 1 }];
    for i in 0..n {
        layers[depth[i] as usize].push(i);
    }
    let mut slot = vec![0usize; n];
    for layer in &layers {
        for (s, &node) in layer.iter().enumerate() {
            slot[node] = s;
        }
    }

    Layering { layer: depth, slot, layers, children }
}

/// Drain `queue` via plain BFS over `adjacency`, filling `visited`/
/// `depth`/`children` as it goes — shared by [`compute_bfs_layering`]'s
/// initial root-seeded pass and its disconnected-component fallback pass,
/// literally mirroring `super::radial_3d`'s own private `bfs_fill`.
fn bfs_layering_fill(adjacency: &[Vec<usize>], visited: &mut [bool], depth: &mut [u32], children: &mut [Vec<usize>], queue: &mut VecDeque<usize>) {
    while let Some(u) = queue.pop_front() {
        for &v in &adjacency[u] {
            if !visited[v] {
                visited[v] = true;
                depth[v] = depth[u] + 1;
                children[u].push(v);
                queue.push_back(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SimEdge;

    fn topo(node_count: usize, edges: &[SimEdge]) -> SimTopology<'_> {
        SimTopology { node_count, edges, degree: &[], radii: vec![1.0; node_count] }
    }

    fn e(from: u32, to: u32) -> SimEdge {
        SimEdge { from: NodeIndex(from), to: NodeIndex(to), weight: 1.0 }
    }

    #[test]
    fn chain_layers_increase_by_one_each_hop() {
        let edges = [e(0, 1), e(1, 2), e(2, 3)];
        let t = topo(4, &edges);
        let layering = compute_layering(&t, &[]);
        assert_eq!(layering.layer, vec![0, 1, 2, 3]);
    }

    #[test]
    fn diamond_layer_is_one_plus_max_of_parent_layers() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let edges = [e(0, 1), e(0, 2), e(1, 3), e(2, 3)];
        let t = topo(4, &edges);
        let layering = compute_layering(&t, &[]);
        assert_eq!(layering.layer[0], 0);
        assert_eq!(layering.layer[1], 1);
        assert_eq!(layering.layer[2], 1);
        assert_eq!(layering.layer[3], 1 + layering.layer[1].max(layering.layer[2]));
    }

    #[test]
    fn multi_depth_parents_take_the_longest_path() {
        // A(0) -> C(1); E(2) -> F(3) -> G(4); C(1) -> D(5); G(4) -> D(5).
        // D's two parents sit at different depths (C=1, G=2) — D must
        // take the LONGER path: 1 + max(1, 2) = 3, not 1 + 1 = 2.
        let edges = [e(0, 1), e(2, 3), e(3, 4), e(1, 5), e(4, 5)];
        let t = topo(6, &edges);
        let layering = compute_layering(&t, &[]);
        assert_eq!(layering.layer[0], 0); // A
        assert_eq!(layering.layer[2], 0); // E
        assert_eq!(layering.layer[1], 1); // C
        assert_eq!(layering.layer[3], 1); // F
        assert_eq!(layering.layer[4], 2); // G
        assert_eq!(layering.layer[5], 1 + layering.layer[1].max(layering.layer[4])); // D
        assert_eq!(layering.layer[5], 3);
    }

    /// Wave G3 fix — the SAME fixture as
    /// `multi_depth_parents_take_the_longest_path`, but via
    /// [`compute_bfs_layering`]: node D is reached via C (BFS depth 1)
    /// before G's own `G -> D` edge is ever traversed, so D lands at
    /// SHORTEST-PATH depth `2`, not Kahn's longest-path `3` — this is the
    /// exact twin-disagreement the layout audit traced between
    /// `RadialLayout`/`RadialLayout3D`, now reproducible from ONE shared
    /// module instead of two unreconciled algorithms.
    #[test]
    fn bfs_layering_takes_the_shortest_path_on_the_same_multi_parent_fixture() {
        let edges = [e(0, 1), e(2, 3), e(3, 4), e(1, 5), e(4, 5)];
        let t = topo(6, &edges);
        let bfs = compute_bfs_layering(&t, &[]);
        assert_eq!(bfs.layer[0], 0); // A
        assert_eq!(bfs.layer[2], 0); // E
        assert_eq!(bfs.layer[1], 1); // C
        assert_eq!(bfs.layer[3], 1); // F
        assert_eq!(bfs.layer[4], 2); // G
        assert_eq!(bfs.layer[5], 2, "D must take the SHORTEST path (via C, depth 1) — 2, not Kahn's longest-path 3");

        let kahn = compute_layering(&t, &[]);
        assert_ne!(bfs.layer[5], kahn.layer[5], "the two metrics must genuinely disagree on this fixture — that's the whole point of DepthMetric existing");
    }

    #[test]
    fn bfs_layering_is_deterministic_and_disconnected_components_each_get_their_own_root() {
        // Two disconnected components: 0->1, and lone node 2.
        let edges = [e(0, 1)];
        let t = topo(3, &edges);
        let first = compute_bfs_layering(&t, &[]);
        let second = compute_bfs_layering(&t, &[]);
        assert_eq!(first.layer, second.layer);
        assert_eq!(first.layer[0], 0);
        assert_eq!(first.layer[1], 1);
        assert_eq!(first.layer[2], 0, "node 2 has no incoming edge and no path from node 0 — it must become its own secondary root, not stay unassigned");
    }

    #[test]
    fn cycle_is_broken_by_a_back_edge_and_layers_cleanly() {
        // 0 -> 1 -> 2 -> 0 (a pure 3-cycle, no zero-in-degree node) —
        // node 0 is the fallback root, edge 2->0 is the back edge.
        let edges = [e(0, 1), e(1, 2), e(2, 0)];
        let t = topo(3, &edges);
        let layering = compute_layering(&t, &[]);
        assert_eq!(layering.layer, vec![0, 1, 2]);
        assert_eq!(layering.layers, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn layering_is_deterministic_across_repeated_calls() {
        let edges = [e(0, 1), e(0, 2), e(1, 3), e(2, 3), e(1, 4), e(4, 3)];
        let t = topo(5, &edges);
        let first = compute_layering(&t, &[]);
        let second = compute_layering(&t, &[]);
        assert_eq!(first.layer, second.layer);
        assert_eq!(first.slot, second.slot);
        assert_eq!(first.layers, second.layers);
    }

    #[test]
    fn explicit_roots_override_auto_detection() {
        // Without an explicit root, node 1 (in-degree 0) would be a root
        // too; forcing root = [0] alone still layers 1 relative to 0.
        let edges = [e(0, 2), e(1, 2)];
        let t = topo(3, &edges);
        let layering = compute_layering(&t, &[NodeIndex(0)]);
        assert_eq!(layering.layer[0], 0);
        // node 1 has no incoming edge from the explicit root set, but is
        // still assigned a layer (own root) — never left unassigned/NaN.
        assert_eq!(layering.layer[1], 0);
        assert_eq!(layering.layer[2], 1);
    }

    #[test]
    fn empty_topology_produces_empty_layering() {
        let t = topo(0, &[]);
        let layering = compute_layering(&t, &[]);
        assert!(layering.layer.is_empty());
        assert!(layering.layers.is_empty());
    }
}
