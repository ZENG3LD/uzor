//! Kahn longest-path layering + one down-sweep/one up-sweep barycenter
//! within-layer ordering pass.
//!
//! **Canonical home (2026-07-22 re-point).** This is the ONE Sugiyama-lite
//! layering implementation this workspace runs. It was originally written
//! in `uzor-graph/src/layout/layering.rs` for that crate's own
//! `HierarchicalLayout`/`RadialLayout`, then lifted here (logic unchanged)
//! so [`super::DagFigure`] could use it without `uzor-figures` depending on
//! `uzor-graph` (this crate must not depend on `uzor-graph` — see this
//! crate's own `CLAUDE.md` Forbidden list). `uzor-graph/src/layout/
//! layering.rs` now RE-POINTS onto this module instead of keeping its own
//! copy — that file is a thin adapter converting `SimTopology`/`NodeIndex`
//! to this module's plain `(usize, usize)` pair-list API and back, its own
//! algorithm body deleted. This module is the canonical, owned-here
//! implementation; `uzor-graph`'s file is the (former duplicate, now
//! adapter) consumer.
//!
//! Deliberately NOT a full crossing-minimization Sugiyama pass: no
//! dummy-node chain insertion for edges spanning more than one layer, no
//! iterate-to-convergence median heuristic — one down-sweep plus one
//! up-sweep barycenter pass ("1-2 sweeps, no full crossing minimization").
//!
//! Public — both this crate's own [`super::DagFigure`] and `uzor-graph`'s
//! adapter consume it across a crate boundary, so this module and
//! [`Layering`]/[`compute_layering`] are `pub`, not `pub(super)`.

use std::collections::{HashSet, VecDeque};

/// Layer assignment + within-layer slot order, computed once from a plain
/// node/edge list (`edges[i] == (from, to)`, `from` = parent / upstream,
/// `to` = child / downstream).
#[derive(Debug, Clone, Default)]
pub struct Layering {
    /// `layer[i]` = depth of node `i` (`0` = root).
    pub layer: Vec<u32>,
    /// Node index -> its slot position within its own layer
    /// (`0..layer_len-1`, dense per layer), after the barycenter sweeps.
    pub slot: Vec<usize>,
    /// Node indices grouped by layer, already in barycenter-ordered slot
    /// order (`layers[l][k]` = the node sitting at slot `k` of layer
    /// `l`).
    pub layers: Vec<Vec<usize>>,
    /// Reduced (back-edge-free, self-loop-free) forward adjacency — kept
    /// alongside `layer`/`layers` for a caller that needs the same
    /// cycle-broken DAG structure the layering itself was computed over
    /// (e.g. `uzor-graph::layout::radial`'s own subtree leaf-count
    /// computation). [`super::DagFigure`] itself doesn't read this field.
    pub children: Vec<Vec<usize>>,
}

/// Compute node depths + within-layer slot order over `node_count` nodes
/// and `edges`.
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
/// looping forever or leaving every node stuck at layer 0. Back edges
/// are excluded ONLY from this depth/order computation — a caller
/// ([`super::layout_dag`]) still draws every ORIGINAL edge it was
/// given; this function never drops an edge from the figure's own
/// output, only from its own internal layering math.
///
/// Self-loops and out-of-range `(from, to)` indices are dropped
/// defensively — never trusted blindly, mirroring the source's own
/// defensive filter (a caller data bug, not a panic).
pub fn compute_layering(node_count: usize, edges: &[(usize, usize)], explicit_roots: &[usize]) -> Layering {
    let n = node_count;
    if n == 0 {
        return Layering::default();
    }

    // Forward adjacency (children) + in-degree over the FULL edge set.
    let mut children: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n]; // (edge_idx, child)
    let mut in_degree_all = vec![0u32; n];
    for (ei, &(a, b)) in edges.iter().enumerate() {
        if a >= n || b >= n || a == b {
            continue;
        }
        children[a].push((ei, b));
        in_degree_all[b] += 1;
    }

    let roots: Vec<usize> = {
        let explicit: Vec<usize> = explicit_roots.iter().copied().filter(|&r| r < n).collect();
        if !explicit.is_empty() {
            explicit
        } else {
            let auto: Vec<usize> = (0..n).filter(|&i| in_degree_all[i] == 0).collect();
            if auto.is_empty() {
                vec![0]
            } else {
                auto
            }
        }
    };

    let back_edges = find_back_edges(n, &children, &roots);

    // Reduced children/in-degree (back edges + self-loops excluded).
    let mut children_reduced: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree = vec![0u32; n];
    for (ei, &(a, b)) in edges.iter().enumerate() {
        if back_edges.contains(&ei) {
            continue;
        }
        if a >= n || b >= n || a == b {
            continue;
        }
        children_reduced[a].push(b);
        in_degree[b] += 1;
    }

    let layer = layer_by_longest_path(n, &children_reduced, &in_degree);
    let (slot, layers) = order_by_barycenter(n, &layer, &children_reduced);

    Layering { layer, slot, layers, children: children_reduced }
}

/// Iterative (non-recursive — safe on a large node count) DFS marking
/// back edges: white(0)/gray(1)/black(2) coloring, an edge into a gray
/// (on-stack) node is a back edge.
fn find_back_edges(n: usize, children: &[Vec<(usize, usize)>], roots: &[usize]) -> HashSet<usize> {
    let mut back_edges = HashSet::new();
    let mut state = vec![0u8; n];
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for &r in roots {
        dfs_mark(r, children, &mut state, &mut stack, &mut back_edges);
    }
    for i in 0..n {
        dfs_mark(i, children, &mut state, &mut stack, &mut back_edges);
    }
    back_edges
}

fn dfs_mark(
    start: usize,
    children: &[Vec<(usize, usize)>],
    state: &mut [u8],
    stack: &mut Vec<(usize, usize)>,
    back_edges: &mut HashSet<usize>,
) {
    if state[start] != 0 {
        return;
    }
    state[start] = 1;
    stack.clear();
    stack.push((start, 0));
    while let Some(&(node, child_pos)) = stack.last() {
        if child_pos >= children[node].len() {
            state[node] = 2;
            stack.pop();
            continue;
        }
        let (edge_idx, child) = children[node][child_pos];
        let last = stack.len() - 1;
        stack[last].1 += 1;
        match state[child] {
            0 => {
                state[child] = 1;
                stack.push((child, 0));
            }
            1 => {
                back_edges.insert(edge_idx);
            }
            _ => {}
        }
    }
}

/// Longest-path layering over the reduced (DAG) graph via Kahn's
/// algorithm: a node is only dequeued once every reduced parent has
/// already contributed its `layer + 1` candidate, so `layer[node]` is
/// final (`1 + max(parent layers)`) the moment it's processed.
fn layer_by_longest_path(n: usize, children: &[Vec<usize>], in_degree: &[u32]) -> Vec<u32> {
    let mut layer = vec![0u32; n];
    let mut remaining_in = in_degree.to_vec();
    let mut visited = vec![false; n];
    let mut queue: VecDeque<usize> = VecDeque::new();

    for i in 0..n {
        if in_degree[i] == 0 {
            visited[i] = true;
            queue.push_back(i);
        }
    }

    while let Some(node) = queue.pop_front() {
        for &child in &children[node] {
            let candidate = layer[node] + 1;
            if candidate > layer[child] {
                layer[child] = candidate;
            }
            remaining_in[child] = remaining_in[child].saturating_sub(1);
            if remaining_in[child] == 0 && !visited[child] {
                visited[child] = true;
                queue.push_back(child);
            }
        }
    }

    // Defensive fallback: a node the BFS never reached (shouldn't happen
    // once back edges are removed — the reduced graph is a DAG) becomes
    // its own layer-0 root instead of carrying a stale `0` that might
    // misleadingly look "reached".
    for i in 0..n {
        if !visited[i] {
            layer[i] = 0;
        }
    }
    layer
}

fn sync_slots(layers: &[Vec<usize>], slot_of: &mut [usize]) {
    for row in layers {
        for (pos, &node) in row.iter().enumerate() {
            slot_of[node] = pos;
        }
    }
}

/// Mean slot (among `slot_of`) of `node`'s entries in `adjacency` — the
/// barycenter key. `node`'s own current slot if it has none (keeps its
/// current relative order instead of collapsing to `0.0`, which would
/// sort every parentless/childless node to the front).
fn barycenter_key(node: usize, adjacency: &[Vec<usize>], slot_of: &[usize]) -> f64 {
    let neighbors = &adjacency[node];
    if neighbors.is_empty() {
        slot_of[node] as f64
    } else {
        neighbors.iter().map(|&n| slot_of[n] as f64).sum::<f64>() / neighbors.len() as f64
    }
}

/// Re-sort one layer's row by `barycenter_key` (stable, ties broken by
/// node index for determinism) and refresh `slot_of` from the result.
fn resort_layer(layers: &mut [Vec<usize>], row_layer: usize, adjacency: &[Vec<usize>], slot_of: &mut [usize]) {
    let mut keyed: Vec<(f64, usize)> =
        layers[row_layer].iter().map(|&node| (barycenter_key(node, adjacency, slot_of), node)).collect();
    keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
    layers[row_layer] = keyed.into_iter().map(|(_, node)| node).collect();
    sync_slots(layers, slot_of);
}

/// One down-sweep (order each layer by the mean slot of its reduced
/// parents) then one up-sweep (order each layer by the mean slot of its
/// reduced children) — a minimal barycenter pass, not full crossing
/// minimization.
fn order_by_barycenter(n: usize, layer: &[u32], children: &[Vec<usize>]) -> (Vec<usize>, Vec<Vec<usize>>) {
    let max_layer = layer.iter().copied().max().unwrap_or(0) as usize;
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for i in 0..n {
        layers[layer[i] as usize].push(i); // deterministic: ascending node index
    }

    let mut slot_of = vec![0usize; n];
    sync_slots(&layers, &mut slot_of);

    let mut parents: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (node, kids) in children.iter().enumerate() {
        for &c in kids {
            parents[c].push(node);
        }
    }

    // Down-sweep: layer 1..=max, keyed by mean parent slot.
    for l in 1..=max_layer {
        resort_layer(&mut layers, l, &parents, &mut slot_of);
    }

    // Up-sweep: layer max-1 downto 0, keyed by mean child slot.
    if max_layer > 0 {
        for l in (0..max_layer).rev() {
            resort_layer(&mut layers, l, children, &mut slot_of);
        }
    }

    (slot_of, layers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_layers_increase_by_one_each_hop() {
        let edges = [(0, 1), (1, 2), (2, 3)];
        let layering = compute_layering(4, &edges, &[]);
        assert_eq!(layering.layer, vec![0, 1, 2, 3]);
    }

    #[test]
    fn diamond_layer_is_one_plus_max_of_parent_layers() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        let layering = compute_layering(4, &edges, &[]);
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
        let edges = [(0, 1), (2, 3), (3, 4), (1, 5), (4, 5)];
        let layering = compute_layering(6, &edges, &[]);
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
        let edges = [(0, 1), (1, 2), (2, 0)];
        let layering = compute_layering(3, &edges, &[]);
        assert_eq!(layering.layer, vec![0, 1, 2]);
        assert_eq!(layering.layers, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn layering_is_deterministic_across_repeated_calls() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3), (1, 4), (4, 3)];
        let first = compute_layering(5, &edges, &[]);
        let second = compute_layering(5, &edges, &[]);
        assert_eq!(first.layer, second.layer);
        assert_eq!(first.slot, second.slot);
        assert_eq!(first.layers, second.layers);
        assert_eq!(first.children, second.children);
    }

    #[test]
    fn explicit_roots_override_auto_detection() {
        // Without an explicit root, node 1 (in-degree 0) would be a root
        // too; forcing root = [0] alone still layers 1 relative to 0.
        let edges = [(0, 2), (1, 2)];
        let layering = compute_layering(3, &edges, &[0]);
        assert_eq!(layering.layer[0], 0);
        // node 1 has no incoming edge from the explicit root set, but is
        // still assigned a layer (own root) — never left unassigned/NaN.
        assert_eq!(layering.layer[1], 0);
        assert_eq!(layering.layer[2], 1);
    }

    #[test]
    fn empty_topology_produces_empty_layering() {
        let layering = compute_layering(0, &[], &[]);
        assert!(layering.layer.is_empty());
        assert!(layering.layers.is_empty());
    }

    #[test]
    fn self_loops_and_out_of_range_edges_are_dropped_without_panicking() {
        let edges = [(0, 0), (0, 99), (99, 1), (0, 1)];
        let layering = compute_layering(2, &edges, &[]);
        assert_eq!(layering.layer, vec![0, 1]);
    }

    /// `children` is the reduced (back-edge-free, self-loop-free) forward
    /// adjacency the layering itself was computed over — the field
    /// `uzor-graph::layout::radial`'s subtree leaf-count computation
    /// consumes via the `uzor-graph` adapter. Proven directly here (not
    /// just re-derived from `layer`/`layers`) since it's a distinct piece
    /// of this canonical module's own output contract.
    #[test]
    fn children_field_is_the_reduced_back_edge_free_adjacency() {
        // 0 -> 1 -> 2 -> 0 (cycle) plus a self-loop and an out-of-range
        // edge, both of which must never appear in `children` either.
        let edges = [(0, 1), (1, 2), (2, 0), (1, 1), (0, 99)];
        let layering = compute_layering(3, &edges, &[]);
        assert_eq!(layering.children[0], vec![1]);
        assert_eq!(layering.children[1], vec![2]);
        assert!(layering.children[2].is_empty(), "the 2->0 back edge must be excluded from the reduced adjacency");
    }

    /// Count bipartite edge crossings between two adjacent layers given
    /// each side's own slot order — the standard "count inversions among
    /// `(top_slot, bottom_slot)` pairs" definition: two edges cross iff
    /// one's top slot is smaller while its bottom slot is larger.
    fn count_crossings(top_order: &[usize], bottom_order: &[usize], edges: &[(usize, usize)]) -> usize {
        let top_slot: std::collections::HashMap<usize, usize> = top_order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let bottom_slot: std::collections::HashMap<usize, usize> = bottom_order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let pairs: Vec<(usize, usize)> = edges
            .iter()
            .filter_map(|&(a, b)| match (top_slot.get(&a), bottom_slot.get(&b)) {
                (Some(&ts), Some(&bs)) => Some((ts, bs)),
                _ => None,
            })
            .collect();
        let mut crossings = 0usize;
        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                let (t1, b1) = pairs[i];
                let (t2, b2) = pairs[j];
                if (t1 < t2 && b1 > b2) || (t1 > t2 && b1 < b2) {
                    crossings += 1;
                }
            }
        }
        crossings
    }

    #[test]
    fn barycenter_ordering_reduces_crossings_versus_naive_index_order() {
        // Top layer {0,1,2}, bottom layer {3,4,5} — a fully-reversed
        // bipartite mapping (0->5, 1->4, 2->3). Ascending node-index
        // order on BOTH sides (the "naive", pre-barycenter order this
        // algorithm's own initial `layers[layer[i]].push(i)` step
        // produces) crosses on every one of the 3 edge pairs — the
        // classic textbook worst case a barycenter sweep resolves.
        let edges = [(0, 5), (1, 4), (2, 3)];
        let layering = compute_layering(6, &edges, &[]);
        assert_eq!(layering.layers.len(), 2, "fixture must produce exactly 2 layers");

        let naive_top = vec![0usize, 1, 2];
        let naive_bottom = vec![3usize, 4, 5];
        let naive_crossings = count_crossings(&naive_top, &naive_bottom, &edges);
        assert_eq!(naive_crossings, 3, "the fully-reversed mapping must cross on every pair under naive ascending-index order");

        let final_top = &layering.layers[0];
        let final_bottom = &layering.layers[1];
        let final_crossings = count_crossings(final_top, final_bottom, &edges);
        assert!(
            final_crossings < naive_crossings,
            "barycenter ordering must reduce crossings below the naive baseline (naive={naive_crossings}, final={final_crossings})"
        );
        assert_eq!(final_crossings, 0, "this fixture's reversal is fully resolvable by one barycenter pass — must reach zero crossings");
    }
}
