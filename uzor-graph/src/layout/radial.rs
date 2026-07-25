//! `RadialLayout` — same layering as [`super::hierarchical::HierarchicalLayout`]
//! ([`super::layering::compute_layering`]) but polar: depth -> ring
//! radius, slot -> angle within the ring. Per-ring angular width blends
//! a uniform floor with a share proportional to each node's own
//! subtree leaf-count (in the reduced, back-edge-free graph) — see
//! [`assign_ring_angles`]'s doc comment for the exact blend and why it
//! guarantees a minimum angular gap between siblings without any
//! post-hoc renormalization. One-shot, same freeze semantics as
//! [`super::hierarchical::HierarchicalLayout`].

use std::f32::consts::TAU;

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::layering::{compute_layering, Layering};
use super::{Layout, LayoutTickResult};

/// Default for [`RadialParams::leaf_weight_blend`] — how much of a ring's
/// angular budget is distributed by subtree leaf-count vs. split
/// uniformly. `1.0` would let a single giant subtree squeeze a lone
/// sibling down to (near) zero width; `0.0` would ignore leaf-count
/// entirely. `0.7` keeps a real minimum (`(1 - blend) / ring_len` of the
/// full circle) for every node while still favoring bigger subtrees.
const DEFAULT_LEAF_WEIGHT_BLEND: f32 = 0.7;

#[derive(Debug, Clone)]
pub struct RadialParams {
    /// World-unit radius of the innermost ring (depth 0 — the roots).
    /// Kept non-zero even for depth 0 so multiple roots land on
    /// distinguishable points instead of all collapsing onto the exact
    /// center (where angle becomes meaningless at radius 0).
    pub base_radius: f32,
    /// World-unit radius added per depth level.
    pub ring_spacing: f32,
    /// Explicit root nodes; empty = auto-detect (see
    /// [`super::layering::compute_layering`]).
    pub roots: Vec<NodeIndex>,
    /// How much of a ring's angular budget [`assign_ring_angles`]
    /// distributes by subtree leaf-count vs. split uniformly — see
    /// [`DEFAULT_LEAF_WEIGHT_BLEND`]'s own doc comment for the exact
    /// blend semantics (Wave G2b configurability — was the private
    /// `LEAF_WEIGHT_BLEND` constant, with no matching field before this).
    pub leaf_weight_blend: f32,
}

impl Default for RadialParams {
    fn default() -> Self {
        Self { base_radius: 60.0, ring_spacing: 90.0, roots: Vec::new(), leaf_weight_blend: DEFAULT_LEAF_WEIGHT_BLEND }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadialLayout {
    params: RadialParams,
    computed: bool,
}

impl RadialLayout {
    pub fn new(params: RadialParams) -> Self {
        Self { params, computed: false }
    }

    pub fn params(&self) -> &RadialParams {
        &self.params
    }

    /// Replace the params and force a recompute on the next `tick`.
    pub fn set_params(&mut self, params: RadialParams) {
        self.params = params;
        self.computed = false;
    }
}

/// Leaf-descendant count per node over the reduced (back-edge-free)
/// graph, computed bottom-up from the deepest layer up (valid even for
/// a non-tree DAG since every reduced child sits at a layer no shallower
/// than its parent's, by construction — a node reachable from multiple
/// parents has its descendants double-counted across those parents;
/// accepted simplification for a general graph, not a literal tree).
fn leaf_counts(layering: &Layering) -> Vec<u32> {
    let n = layering.layer.len();
    let mut counts = vec![0u32; n];
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(layering.layer[i]));
    for node in order {
        counts[node] = if layering.children[node].is_empty() {
            1
        } else {
            layering.children[node].iter().map(|&c| counts[c]).sum::<u32>().max(1)
        };
    }
    counts
}

/// Allocate each node in `ring` (already slot-ordered) a contiguous
/// angular span blending a uniform floor (`(1 - leaf_weight_blend) /
/// ring.len()`) with a leaf-count-proportional share
/// (`leaf_weight_blend * leaf[node] / total`), then returns each node's
/// angle at the MIDPOINT of its span.
///
/// Because the two blended terms individually sum to `1.0` over the
/// ring (`sum(1/ring_len) = 1`, `sum(leaf/total) = 1`), their weighted
/// sum is exactly `1.0` too — spans sum to exactly `TAU`, no
/// post-hoc rescale needed, and the uniform term alone guarantees a
/// hard, provable minimum span per node regardless of leaf-count skew
/// (an all-siblings-are-leaves ring still gets non-degenerate spacing;
/// a `ring.len() == 1` ring degenerates to the whole circle, which is
/// correct — a single node has no sibling to collide with).
fn assign_ring_angles(ring: &[usize], leaf: &[u32], leaf_weight_blend: f32) -> Vec<f32> {
    let len = ring.len();
    if len == 0 {
        return Vec::new();
    }
    let total: f32 = ring.iter().map(|&n| leaf[n] as f32).sum::<f32>().max(1.0);
    let uniform_share = (1.0 - leaf_weight_blend) / len as f32;

    let mut angles = Vec::with_capacity(len);
    let mut offset = 0.0f32;
    for &node in ring {
        let weighted_share = leaf_weight_blend * (leaf[node] as f32) / total;
        let width = TAU * (uniform_share + weighted_share);
        angles.push(offset + width * 0.5);
        offset += width;
    }
    angles
}

impl Layout for RadialLayout {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        if self.computed {
            return LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true };
        }
        let layering = compute_layering(topo, &self.params.roots);
        let leaf = leaf_counts(&layering);

        for row in &layering.layers {
            let angles = assign_ring_angles(row, &leaf, self.params.leaf_weight_blend);
            for (&node, &angle) in row.iter().zip(angles.iter()) {
                let Some(p) = particles.get_mut(node) else { continue };
                if let (Some(fx), Some(fy)) = (p.fx, p.fy) {
                    p.x = fx;
                    p.y = fy;
                    p.vx = 0.0;
                    p.vy = 0.0;
                    continue;
                }
                let radius = self.params.base_radius + layering.layer[node] as f32 * self.params.ring_spacing;
                p.x = radius * angle.cos();
                p.y = radius * angle.sin();
                p.vx = 0.0;
                p.vy = 0.0;
            }
        }
        self.computed = true;
        LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true }
    }

    fn reheat(&mut self, _alpha: f32) {
        self.computed = false;
    }

    fn is_settled(&self) -> bool {
        true
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

    fn radius_of(p: &Particle) -> f32 {
        (p.x * p.x + p.y * p.y).sqrt()
    }

    /// Angle in `[0, TAU)`, matching `assign_ring_angles`'s own range.
    fn angle_of(p: &Particle) -> f32 {
        let a = p.y.atan2(p.x);
        if a < 0.0 {
            a + TAU
        } else {
            a
        }
    }

    #[test]
    fn ring_radius_strictly_increases_with_depth() {
        let edges = [e(0, 1), e(1, 2)];
        let t = topo(3, &edges);
        let mut particles = vec![Particle::default(); 3];
        let mut layout = RadialLayout::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let r0 = radius_of(&particles[0]);
        let r1 = radius_of(&particles[1]);
        let r2 = radius_of(&particles[2]);
        assert!(r0 < r1, "depth 0 ring must sit inside depth 1's: {r0} vs {r1}");
        assert!(r1 < r2, "depth 1 ring must sit inside depth 2's: {r1} vs {r2}");
    }

    #[test]
    fn every_angle_is_within_0_to_tau_and_finite() {
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4)];
        let t = topo(5, &edges);
        let mut particles = vec![Particle::default(); 5];
        let mut layout = RadialLayout::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        for p in &particles {
            assert!(p.x.is_finite() && !p.x.is_nan());
            assert!(p.y.is_finite() && !p.y.is_nan());
            let a = angle_of(p);
            assert!((0.0..TAU).contains(&a), "angle {a} out of [0, TAU)");
        }
    }

    #[test]
    fn siblings_in_the_same_ring_never_collide() {
        // 5 siblings, same layer (all children of root 0).
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5)];
        let t = topo(6, &edges);
        let mut particles = vec![Particle::default(); 6];
        let mut layout = RadialLayout::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let mut angles: Vec<f32> = (1..=5).map(|i| angle_of(&particles[i])).collect();
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Guaranteed hard floor per this ring (5 siblings, no leaf-count
        // skew among them since they're all leaves): (1 - blend) / len.
        let min_expected_gap = (1.0 - DEFAULT_LEAF_WEIGHT_BLEND) / 5.0 * TAU;
        for pair in angles.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(gap >= min_expected_gap - 1e-4, "adjacent siblings too close: gap {gap} < floor {min_expected_gap}");
        }
    }

    /// Wave G2b configurability gate: a caller-tuned `leaf_weight_blend`
    /// must actually change the ring's angular allocation, not just exist
    /// as an unread field. `0.0` (pure uniform split) must produce a
    /// LARGER minimum gap than the default `0.7` blend does, for a ring
    /// with real leaf-count skew (`0` disables the uniform-floor's own
    /// leaf-count-proportional erosion entirely).
    #[test]
    fn leaf_weight_blend_of_zero_widens_the_uniform_floor_for_a_skewed_ring() {
        // Node 1 has a large subtree (5 leaves via nodes 6..=10), nodes
        // 2..=5 are bare leaves — real leaf-count skew within one ring.
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5), e(1, 6), e(1, 7), e(1, 8), e(1, 9), e(1, 10)];
        let t = topo(11, &edges);

        let mut default_particles = vec![Particle::default(); 11];
        let mut default_layout = RadialLayout::default();
        default_layout.tick(&t, &mut default_particles, 1.0 / 60.0);
        let mut default_angles: Vec<f32> = (1..=5).map(|i| angle_of(&default_particles[i])).collect();
        default_angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let default_min_gap = default_angles.windows(2).map(|p| p[1] - p[0]).fold(f32::MAX, f32::min);

        let mut uniform_particles = vec![Particle::default(); 11];
        let mut uniform_layout = RadialLayout::new(RadialParams { leaf_weight_blend: 0.0, ..RadialParams::default() });
        uniform_layout.tick(&t, &mut uniform_particles, 1.0 / 60.0);
        let mut uniform_angles: Vec<f32> = (1..=5).map(|i| angle_of(&uniform_particles[i])).collect();
        uniform_angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let uniform_min_gap = uniform_angles.windows(2).map(|p| p[1] - p[0]).fold(f32::MAX, f32::min);

        assert!(
            uniform_min_gap > default_min_gap + 1e-4,
            "leaf_weight_blend=0.0 must widen the minimum sibling gap vs. the default 0.7 blend on a leaf-skewed ring: uniform={uniform_min_gap} default={default_min_gap}"
        );
    }

    #[test]
    fn radial_layout_is_deterministic_across_repeated_ticks_after_reheat() {
        let edges = [e(0, 1), e(0, 2), e(1, 3)];
        let t = topo(4, &edges);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = RadialLayout::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let first: Vec<(f32, f32)> = particles.iter().map(|p| (p.x, p.y)).collect();

        layout.reheat(1.0);
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let second: Vec<(f32, f32)> = particles.iter().map(|p| (p.x, p.y)).collect();
        assert_eq!(first, second);
    }
}
