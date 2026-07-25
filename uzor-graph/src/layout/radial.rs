//! `RadialLayout` — same layering as [`super::hierarchical::HierarchicalLayout`]
//! ([`super::layering::compute_layering`]) but polar: depth -> ring
//! radius, slot -> angle within the ring. Per-ring angular width blends
//! a uniform floor with a share proportional to each node's own
//! subtree leaf-count (in the reduced, back-edge-free graph) — see
//! [`assign_ring_angles`]'s doc comment for the exact blend and why it
//! guarantees a minimum angular gap between siblings without any
//! post-hoc renormalization. One-shot, same freeze semantics as
//! [`super::hierarchical::HierarchicalLayout`].
//!
//! **Wave G3 — `depth_metric`**: depth for the RING assignment is
//! [`super::layering::DepthMetric::LongestPath`] by default (this
//! module's own pre-existing behavior, unchanged) — [`super::radial_3d::
//! RadialLayout3D`] defaults to the OPPOSITE metric
//! (`ShortestPath`), an unreconciled divergence the layout audit traced
//! concretely (see [`super::layering::DepthMetric`]'s own doc comment).
//! [`RadialParams::depth_metric`] lets a caller align the two.
//!
//! **Wave G3 — `radius_aware_spacing`**: OFF by default (doctrine: no
//! silent output change) — see [`RadialParams::radius_aware_spacing`]'s
//! own doc comment.

use std::f32::consts::TAU;

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::layering::{compute_bfs_layering, compute_layering, DepthMetric, Layering};
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
    /// Which depth notion assigns a node to a ring — see
    /// [`super::layering::DepthMetric`]'s own doc comment (Wave G3 fix).
    /// Defaults to [`DepthMetric::LongestPath`] — this module's own
    /// PRE-EXISTING behavior via [`compute_layering`], unchanged.
    pub depth_metric: DepthMetric,
    /// When `true`, a ring's angular-width allocation ALSO scales by each
    /// member's own [`SimTopology::radii`] (relative to that ring's own
    /// average radius) on top of the existing leaf-count blend, so a
    /// collapsed-cluster supernode (sqrt-scaled radius,
    /// `crate::cluster::supernode_radius`) gets proportionally more
    /// angular room than a same-ring leaf — see [`assign_ring_angles`]'s
    /// own doc comment for the exact renormalization. Defaults to
    /// `false`: this CHANGES layout output for any graph with varying
    /// node radii, so today's leaf-only allocation is preserved unless a
    /// caller opts in (doctrine: no silent output change; Wave G3 item 3
    /// — recommended to the owner, not flipped here).
    pub radius_aware_spacing: bool,
}

impl Default for RadialParams {
    fn default() -> Self {
        Self {
            base_radius: 60.0,
            ring_spacing: 90.0,
            roots: Vec::new(),
            leaf_weight_blend: DEFAULT_LEAF_WEIGHT_BLEND,
            depth_metric: DepthMetric::LongestPath,
            radius_aware_spacing: false,
        }
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
///
/// `radii` (Wave G3 item 3, `RadialParams::radius_aware_spacing`) —
/// `None` reproduces the leaf-only blend above byte-for-byte identically
/// (the default). `Some(radii)` multiplies each node's leaf-blended base
/// share by `own_radius / ring_average_radius` BEFORE the final
/// renormalization (`share_i = raw_i / sum(raw)`), so the spans still
/// sum to exactly `TAU` — a node with an above-average radius (e.g. a
/// collapsed-cluster supernode) gets proportionally MORE angular room, a
/// below-average one gets less, while the leaf-count skew from the base
/// blend is still respected relative to same-radius siblings.
fn assign_ring_angles(ring: &[usize], leaf: &[u32], leaf_weight_blend: f32, radii: Option<&[f32]>) -> Vec<f32> {
    let len = ring.len();
    if len == 0 {
        return Vec::new();
    }
    let total: f32 = ring.iter().map(|&n| leaf[n] as f32).sum::<f32>().max(1.0);
    let uniform_share = (1.0 - leaf_weight_blend) / len as f32;
    let base_shares: Vec<f32> = ring.iter().map(|&node| uniform_share + leaf_weight_blend * (leaf[node] as f32) / total).collect();

    let shares: Vec<f32> = match radii {
        None => base_shares,
        Some(radii) => {
            let own_radius = |node: usize| radii.get(node).copied().unwrap_or(1.0).max(1e-3);
            let avg_radius = (ring.iter().map(|&n| own_radius(n)).sum::<f32>() / len as f32).max(1e-3);
            let raw: Vec<f32> = ring.iter().zip(base_shares.iter()).map(|(&node, &base)| base * (own_radius(node) / avg_radius)).collect();
            let raw_total: f32 = raw.iter().sum::<f32>().max(1e-6);
            raw.into_iter().map(|v| v / raw_total).collect()
        }
    };

    let mut angles = Vec::with_capacity(len);
    let mut offset = 0.0f32;
    for &share in &shares {
        let width = TAU * share;
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
        let layering = match self.params.depth_metric {
            DepthMetric::LongestPath => compute_layering(topo, &self.params.roots),
            DepthMetric::ShortestPath => compute_bfs_layering(topo, &self.params.roots),
        };
        let leaf = leaf_counts(&layering);
        let radii_for_angles = self.params.radius_aware_spacing.then(|| topo.radii.as_slice());

        for row in &layering.layers {
            let angles = assign_ring_angles(row, &leaf, self.params.leaf_weight_blend, radii_for_angles);
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

    // ── Wave G3 item 2 — `depth_metric` ─────────────────────────────────

    #[test]
    fn radial_params_default_depth_metric_is_longest_path_and_radius_aware_spacing_is_off() {
        let p = RadialParams::default();
        assert_eq!(p.depth_metric, DepthMetric::LongestPath, "RadialLayout's pre-existing behavior — must not change silently");
        assert!(!p.radius_aware_spacing, "must default OFF — doctrine: no silent output change");
    }

    /// Wave G3 item 2 gate: the SAME fixture
    /// `layering::tests::multi_depth_parents_take_the_longest_path` uses —
    /// node D has two parents at different depths (C=1, G=2). Under the
    /// default `LongestPath` metric D lands on ring depth 3; switching to
    /// `ShortestPath` must genuinely move it to ring depth 2, not just
    /// accept the field silently.
    #[test]
    fn depth_metric_shortest_path_places_a_multi_parent_node_on_a_different_ring_than_longest_path() {
        let edges = [e(0, 1), e(2, 3), e(3, 4), e(1, 5), e(4, 5)];
        let t = topo(6, &edges);

        let mut longest_particles = vec![Particle::default(); 6];
        let mut longest_layout = RadialLayout::default(); // LongestPath default
        longest_layout.tick(&t, &mut longest_particles, 1.0 / 60.0);

        let mut shortest_particles = vec![Particle::default(); 6];
        let mut shortest_layout = RadialLayout::new(RadialParams { depth_metric: DepthMetric::ShortestPath, ..RadialParams::default() });
        shortest_layout.tick(&t, &mut shortest_particles, 1.0 / 60.0);

        let r_longest = radius_of(&longest_particles[5]); // D
        let r_shortest = radius_of(&shortest_particles[5]);
        assert!(r_longest > r_shortest, "D's LongestPath layer (3) must sit on a strictly larger ring than its ShortestPath depth (2): {r_longest} vs {r_shortest}");

        let default_params = RadialParams::default();
        let expected_longest = default_params.base_radius + 3.0 * default_params.ring_spacing;
        let expected_shortest = default_params.base_radius + 2.0 * default_params.ring_spacing;
        assert!((r_longest - expected_longest).abs() < 1e-2, "got {r_longest}, expected {expected_longest}");
        assert!((r_shortest - expected_shortest).abs() < 1e-2, "got {r_shortest}, expected {expected_shortest}");
    }

    /// Cross-layout consistency gate: with a MATCHED `DepthMetric`,
    /// `RadialLayout` (2D) and `RadialLayout3D` must assign node D the
    /// SAME depth number on the identical fixture — the actual defect
    /// this item fixes (previously unreconciled, always disagreeing).
    /// `base_radius`/`ring_spacing`(2D) and `shell_spacing`(3D) are tuned
    /// to `0.0`/`1.0` so each layout's own non-root radius formula
    /// recovers the depth number exactly (D is never a root in this
    /// fixture, so the Wave G3 item 1 root-ring special case never
    /// applies to it).
    #[test]
    fn matched_depth_metric_gives_2d_and_3d_radial_layouts_the_same_depth_for_the_layering_fixture() {
        use super::super::radial_3d::{RadialLayout3D, RadialParams3D};

        let edges = [e(0, 1), e(2, 3), e(3, 4), e(1, 5), e(4, 5)];
        let t = topo(6, &edges);

        for (metric, expected_depth) in [(DepthMetric::LongestPath, 3.0f32), (DepthMetric::ShortestPath, 2.0f32)] {
            let mut particles_2d = vec![Particle::default(); 6];
            let mut layout_2d = RadialLayout::new(RadialParams { base_radius: 0.0, ring_spacing: 1.0, depth_metric: metric, ..RadialParams::default() });
            layout_2d.tick(&t, &mut particles_2d, 1.0 / 60.0);
            let r2d = radius_of(&particles_2d[5]);
            assert!((r2d - expected_depth).abs() < 1e-3, "2D radial depth mismatch for {metric:?}: got {r2d}, expected {expected_depth}");

            let mut particles_3d = vec![Particle::default(); 6];
            let mut layout_3d = RadialLayout3D::new(RadialParams3D { shell_spacing: 1.0, depth_metric: metric, ..RadialParams3D::default() });
            layout_3d.tick(&t, &mut particles_3d, 1.0 / 60.0);
            let r3d = (particles_3d[5].x.powi(2) + particles_3d[5].y.powi(2) + particles_3d[5].z.powi(2)).sqrt();
            assert!((r3d - expected_depth).abs() < 1e-3, "3D radial depth mismatch for {metric:?}: got {r3d}, expected {expected_depth}");
        }
    }

    // ── Wave G3 item 3 — `radius_aware_spacing` ─────────────────────────

    #[test]
    fn assign_ring_angles_with_radii_none_matches_the_leaf_only_baseline_exactly() {
        let leaf = vec![1u32, 1, 1, 1, 1];
        let ring: Vec<usize> = (0..5).collect();
        let angles = assign_ring_angles(&ring, &leaf, DEFAULT_LEAF_WEIGHT_BLEND, None);
        let step = TAU / ring.len() as f32;
        for (i, &a) in angles.iter().enumerate() {
            let expected = step * i as f32 + step * 0.5;
            assert!((a - expected).abs() < 1e-4, "angle {i}: {a} vs {expected}");
        }
    }

    /// Wave G3 item 3 gate: a member whose own radius is well above its
    /// ring's average must claim a visibly WIDER angular span than an
    /// ordinary same-size neighbor — proven via the gap either side of it
    /// (the gap between two ordinary same-size neighbors must NOT widen
    /// the same way, and must actually shrink slightly since every span
    /// still sums to exactly `TAU`).
    #[test]
    fn assign_ring_angles_radius_aware_widens_the_span_around_a_larger_member() {
        let leaf = vec![1u32; 5];
        let ring: Vec<usize> = (0..5).collect();
        let radii = vec![1.0f32, 4.0, 1.0, 1.0, 1.0]; // ring slot 1 is 4x bigger

        let off = assign_ring_angles(&ring, &leaf, DEFAULT_LEAF_WEIGHT_BLEND, None);
        let gap01_off = off[1] - off[0];
        let gap23_off = off[3] - off[2];
        assert!((gap01_off - gap23_off).abs() < 1e-4, "without radius-awareness every gap must be identical (uniform leaf counts)");

        let on = assign_ring_angles(&ring, &leaf, DEFAULT_LEAF_WEIGHT_BLEND, Some(&radii));
        let gap01_on = on[1] - on[0];
        let gap12_on = on[2] - on[1];
        let gap23_on = on[3] - on[2];
        assert!(gap01_on > gap01_off, "the gap approaching the larger member must widen: {gap01_on} vs baseline {gap01_off}");
        assert!(gap12_on > gap23_off, "the gap leaving the larger member must widen too: {gap12_on} vs an ordinary gap {gap23_off}");
        assert!(gap23_on < gap23_off, "an ordinary same-size gap must shrink slightly — every span still sums to exactly TAU: {gap23_on} vs baseline {gap23_off}");
    }

    /// Integration gate: `RadialParams::radius_aware_spacing` must
    /// actually reach `tick()`, not just exist as an unread field — and
    /// must leave the RING RADIUS itself untouched (this item's own
    /// documented scope is within-ring angular allocation only).
    #[test]
    fn radial_layout_radius_aware_spacing_actually_changes_output_when_node_radii_vary() {
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5)];
        let mut radii = vec![1.0f32; 6];
        radii[1] = 6.0; // node 1 is a big supernode-like member
        let t = SimTopology { node_count: 6, edges: &edges, degree: &[], radii };

        let mut off_particles = vec![Particle::default(); 6];
        let mut off_layout = RadialLayout::default();
        off_layout.tick(&t, &mut off_particles, 1.0 / 60.0);

        let mut on_particles = vec![Particle::default(); 6];
        let mut on_layout = RadialLayout::new(RadialParams { radius_aware_spacing: true, ..RadialParams::default() });
        on_layout.tick(&t, &mut on_particles, 1.0 / 60.0);

        assert_ne!(
            (off_particles[1].x, off_particles[1].y),
            (on_particles[1].x, on_particles[1].y),
            "turning radius_aware_spacing on must actually move the big-radius member, not silently do nothing"
        );
        assert!(
            (radius_of(&off_particles[1]) - radius_of(&on_particles[1])).abs() < 1e-3,
            "ring RADIUS itself must stay untouched by this item — only the angular allocation changes"
        );
    }
}
