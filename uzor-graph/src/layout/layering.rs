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

use crate::graph::{NodeIndex, SimTopology};

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
