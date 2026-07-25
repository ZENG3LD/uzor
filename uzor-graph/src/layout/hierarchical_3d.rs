//! `HierarchicalLayout3D` — the 3D sibling of
//! [`super::hierarchical::HierarchicalLayout`]: same layering
//! ([`super::layering::compute_layering`], the Kahn longest-path +
//! barycenter adapter both 2D layouts already share) but genuinely 3D —
//! each layer becomes a horizontal RING in the XZ plane at a fixed
//! world-Y height, instead of a flat row, so the result reads as a real
//! tiered "chandelier" of layers rather than a 2D layout pasted onto a
//! plane.
//!
//! `y = -(layer * layer_spacing)` — NEGATED relative to 2D hierarchical's
//! own `y = layer * layer_spacing`. This is not a sign error: 2D's screen
//! space is Y-DOWN (root at `y = 0`, children at increasing/more-positive
//! `y`, i.e. visually lower on screen), while [`crate::camera3d::Camera3D`]'s
//! world is Y-UP (`Camera3D::orbit_offset`'s own `distance * pitch.sin()`
//! term). Negating keeps the SAME visual convention — root on top,
//! descendants below it — under the opposite axis direction.
//!
//! Within a ring, nodes keep the same barycenter slot order 2D
//! hierarchical/radial already compute, spread at evenly-spaced angles;
//! the ring's own radius grows with how many nodes share it (`radius =
//! ring_spacing * count / TAU`, floored at
//! [`HierarchicalParams3D::min_ring_radius`]) so a crowded layer doesn't
//! overlap itself. One-shot: [`Layout::tick`]
//! computes positions once then reports settled forever;
//! [`Layout::reheat`] forces a recompute — same freeze semantics as
//! [`super::hierarchical::HierarchicalLayout`].
//!
//! **Wave G3 item 3 — [`HierarchicalParams3D::radius_aware_spacing`].**
//! ON by default (graph-strengthening arc, owner-approved flip,
//! 2026-07-26 — was OFF at Wave G3 introduction). When on, a
//! ring's own radius/angular allocation is derived from the SUM of each
//! member's own diameter (via [`SimTopology::radii`]) instead of a flat
//! per-member arc budget — see [`radius_aware_ring`]'s own doc comment.

use std::f32::consts::TAU;

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::layering::compute_layering;
use super::{Layout, LayoutTickResult};

/// Default for [`HierarchicalParams3D::min_ring_radius`] — floor on a
/// ring's own radius (see this module's own `r = ring_spacing * count /
/// TAU` formula) — keeps even a single-node layer visibly off the
/// central vertical axis instead of collapsing toward it at a small
/// `ring_spacing`.
const DEFAULT_MIN_RING_RADIUS: f32 = 20.0;

#[derive(Debug, Clone)]
pub struct HierarchicalParams3D {
    /// World-unit vertical spacing between adjacent layers (rings) —
    /// same name/role as 2D's `HierarchicalParams::layer_spacing`.
    pub layer_spacing: f32,
    /// World-unit circumferential spacing per ring member — a ring
    /// holding `count` nodes gets radius `ring_spacing * count / TAU`
    /// (clamped to [`HierarchicalParams3D::min_ring_radius`]), i.e.
    /// roughly `ring_spacing` world units of arc length per sibling.
    pub ring_spacing: f32,
    /// Explicit root nodes; empty = auto-detect every zero-in-degree
    /// node as a root (see [`super::layering::compute_layering`]).
    pub roots: Vec<NodeIndex>,
    /// Floor on a layer-ring's own radius — see
    /// [`DEFAULT_MIN_RING_RADIUS`]'s own doc comment (Wave G2b
    /// configurability — was the private `MIN_RING_RADIUS` constant, with
    /// no matching field before this).
    pub min_ring_radius: f32,
    /// When `true`, a ring's radius and per-member angular width are
    /// both derived from the SUM of each member's own diameter (via
    /// [`SimTopology::radii`]) instead of a flat `ring_spacing`-per-
    /// member arc budget — see [`radius_aware_ring`]'s own doc comment.
    /// Defaults to `true` (graph-strengthening arc, owner-approved flip,
    /// 2026-07-26 — was `false` at Wave G3 introduction): with it off, a
    /// collapsed-cluster supernode was guaranteed to overlap its ring
    /// neighbors, since the count-only ring formula had no knowledge
    /// that a particular node renders several times larger than a leaf.
    /// A caller can still pass `radius_aware_spacing: false` explicitly
    /// to recover the old count-only ring formula — the aware and
    /// non-aware formulas share the SAME slice-start angle convention
    /// (see [`radius_aware_ring`]'s own doc comment), so toggling this
    /// flag on a uniform-radius graph changes nothing at all, only a
    /// graph with real radius variance sees a different (wider-for-
    /// bigger-members) angular allocation.
    pub radius_aware_spacing: bool,
}

impl Default for HierarchicalParams3D {
    fn default() -> Self {
        Self {
            layer_spacing: 220.0,
            ring_spacing: 70.0,
            roots: Vec::new(),
            min_ring_radius: DEFAULT_MIN_RING_RADIUS,
            radius_aware_spacing: true,
        }
    }
}

/// [`HierarchicalParams3D::radius_aware_spacing`]'s own ring
/// radius/angle formula — each member's own per-ring "arc budget" is
/// `max(ring_spacing, 2 * own_radius)` (its own diameter, floored at the
/// ordinary `ring_spacing`), the ring's radius is `sum(budgets) / TAU`
/// (floored at `min_ring_radius`), and each member's angle sits at the
/// START of its own budget's cumulative span — the SAME convention the
/// non-aware `slot * TAU / count` formula uses (every non-aware slot IS
/// the start of an equal-width TAU/count span). When every member's
/// radius is small enough that `2 * own_radius <= ring_spacing`, every
/// budget floors to the SAME `ring_spacing` value, so the per-slot width
/// is identically `TAU / count` too and this recovers the non-aware
/// formula EXACTLY — both radius AND every single angle, not radius
/// only.
///
/// Graph-strengthening arc, owner-caught defect (2026-07-26): this
/// function previously placed each angle at its span's MIDPOINT instead
/// of its start — a second, unrelated behavior riding along on the same
/// flag: turning `radius_aware_spacing` on rotated the WHOLE ring by
/// half a slot's own width even when every radius was perfectly uniform
/// (measured 20-35 world-unit position shifts on a plain uniform-radius
/// fixture, nothing to do with radius awareness at all). A flag named
/// "radius-aware spacing" should change nothing when radii don't vary —
/// fixed to the start convention above, which is both what "spacing"
/// (not "rotation") means and what the non-aware sibling formula already
/// does, so the two now degenerate to literally the same output at
/// uniform radii, not merely the same RADIUS.
fn radius_aware_ring(row: &[usize], radii: &[f32], ring_spacing: f32, min_ring_radius: f32) -> (f32, Vec<f32>) {
    if row.is_empty() {
        return (min_ring_radius, Vec::new());
    }
    let own_radius = |node: usize| radii.get(node).copied().unwrap_or(1.0).max(0.0);
    let budgets: Vec<f32> = row.iter().map(|&n| (2.0 * own_radius(n)).max(ring_spacing)).collect();
    let total: f32 = budgets.iter().sum();
    let radius = (total / TAU).max(min_ring_radius);

    let mut angles = Vec::with_capacity(row.len());
    let mut offset = 0.0f32;
    for &budget in &budgets {
        angles.push(offset);
        let width = TAU * budget / total;
        offset += width;
    }
    (radius, angles)
}

#[derive(Debug, Clone, Default)]
pub struct HierarchicalLayout3D {
    params: HierarchicalParams3D,
    computed: bool,
}

impl HierarchicalLayout3D {
    pub fn new(params: HierarchicalParams3D) -> Self {
        Self { params, computed: false }
    }

    pub fn params(&self) -> &HierarchicalParams3D {
        &self.params
    }

    /// Replace the params and force a recompute on the next `tick`.
    pub fn set_params(&mut self, params: HierarchicalParams3D) {
        self.params = params;
        self.computed = false;
    }
}

impl Layout for HierarchicalLayout3D {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        if self.computed {
            return LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true };
        }
        let layering = compute_layering(topo, &self.params.roots);
        for (layer_idx, row) in layering.layers.iter().enumerate() {
            let count = row.len().max(1) as f32;
            let (radius, radius_aware_angles) = if self.params.radius_aware_spacing {
                let (r, angles) = radius_aware_ring(row, &topo.radii, self.params.ring_spacing, self.params.min_ring_radius);
                (r, Some(angles))
            } else {
                ((self.params.ring_spacing * count / TAU).max(self.params.min_ring_radius), None)
            };
            let y = -(layer_idx as f32 * self.params.layer_spacing);
            for (slot, &node) in row.iter().enumerate() {
                let Some(p) = particles.get_mut(node) else { continue };
                // Every `Layout::tick` must honor a set fx/fy/fz (see the
                // trait doc) — a pinned particle stays exactly where it
                // is instead of being overwritten by the layering math,
                // the same 3-axis extension of 2D hierarchical's own
                // `(Some(fx), Some(fy))` guard.
                if let (Some(fx), Some(fy), Some(fz)) = (p.fx, p.fy, p.fz) {
                    p.x = fx;
                    p.y = fy;
                    p.z = fz;
                    p.vx = 0.0;
                    p.vy = 0.0;
                    p.vz = 0.0;
                    continue;
                }
                let angle = match &radius_aware_angles {
                    Some(angles) => angles[slot],
                    None => slot as f32 * TAU / row.len().max(1) as f32,
                };
                p.x = radius * angle.cos();
                p.z = radius * angle.sin();
                p.y = y;
                p.vx = 0.0;
                p.vy = 0.0;
                p.vz = 0.0;
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

    #[test]
    fn one_shot_tick_places_every_node_with_no_nan_and_root_strictly_above_children() {
        let edges = [e(0, 1), e(0, 2), e(1, 3), e(2, 3)];
        let t = topo(4, &edges);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = HierarchicalLayout3D::default();

        let r1 = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(r1.settled);
        for p in &particles {
            assert!(p.x.is_finite() && !p.x.is_nan());
            assert!(p.y.is_finite() && !p.y.is_nan());
            assert!(p.z.is_finite() && !p.z.is_nan());
        }
        // Y-UP world: root strictly ABOVE (numerically greater y than)
        // its children, which sit strictly above the shared bottom node.
        assert!(particles[0].y > particles[1].y, "root must sit above its child in Y-up world");
        assert!(particles[0].y > particles[2].y);
        assert!(particles[1].y > particles[3].y);
        assert!(particles[2].y > particles[3].y);
    }

    #[test]
    fn same_layer_nodes_share_y_and_spread_around_a_nondegenerate_ring() {
        // 5 siblings, same layer (all children of root 0).
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5)];
        let t = topo(6, &edges);
        let mut particles = vec![Particle::default(); 6];
        let mut layout = HierarchicalLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let sibling_y = particles[1].y;
        for i in 2..=5 {
            assert_eq!(particles[i].y, sibling_y, "every node in the same layer must share the same y height");
        }
        // Non-degenerate ring — every sibling gets a DISTINCT (x, z),
        // not a single collapsed point.
        let mut positions: Vec<(f32, f32)> = (1..=5).map(|i| (particles[i].x, particles[i].z)).collect();
        positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in positions.windows(2) {
            assert_ne!(pair[0], pair[1], "adjacent siblings on the same ring must not collapse onto one point");
        }
        // Every sibling sits genuinely off the central vertical axis.
        for i in 1..=5 {
            let r2 = particles[i].x * particles[i].x + particles[i].z * particles[i].z;
            assert!(r2 > 1.0, "ring members must sit at a non-trivial radius, got r^2={r2}");
        }
    }

    #[test]
    fn one_shot_layout_freezes_after_first_tick() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = HierarchicalLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let after_first = particles.clone();

        particles[0].x = 999.0;
        let r2 = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(r2.settled);
        assert_eq!(particles[0].x, 999.0);
        assert_eq!(particles[1], after_first[1]);
    }

    #[test]
    fn reheat_forces_a_fresh_recompute() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = HierarchicalLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        particles[0].x = 999.0;
        layout.reheat(1.0);
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert_ne!(particles[0].x, 999.0, "reheat must force a fresh compute, overwriting the manual nudge");
    }

    #[test]
    fn pinned_particle_is_never_overwritten_by_the_layering_math() {
        let edges = [e(0, 1), e(0, 2)];
        let t = topo(3, &edges);
        let mut particles = vec![Particle::default(); 3];
        particles[1].pin3(42.0, -7.0, 13.0);
        let mut layout = HierarchicalLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert_eq!((particles[1].x, particles[1].y, particles[1].z), (42.0, -7.0, 13.0));
    }

    /// Wave G2b configurability gate: [`HierarchicalParams3D::default`]
    /// must equal the prior hardcoded constant exactly, and a
    /// caller-tuned `min_ring_radius` must actually raise a crowded-ring
    /// radius above the default floor when `ring_spacing` alone would
    /// have landed below it.
    #[test]
    fn min_ring_radius_defaults_to_the_prior_constant_and_a_larger_override_raises_a_small_rings_radius() {
        assert_eq!(HierarchicalParams3D::default().min_ring_radius, 20.0);

        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        // A tiny ring_spacing means `ring_spacing * count / TAU` sits well
        // under both the default and the raised floor — the raised floor
        // must dominate `.max(...)` either way, and a bigger floor must
        // produce a bigger actual radius.
        let mut default_layout =
            HierarchicalLayout3D::new(HierarchicalParams3D { ring_spacing: 0.01, ..HierarchicalParams3D::default() });
        default_layout.tick(&t, &mut particles, 1.0 / 60.0);
        let default_r2 = particles[0].x * particles[0].x + particles[0].z * particles[0].z;

        let mut particles2 = vec![Particle::default(); 2];
        let mut raised_layout =
            HierarchicalLayout3D::new(HierarchicalParams3D { ring_spacing: 0.01, min_ring_radius: 200.0, ..HierarchicalParams3D::default() });
        raised_layout.tick(&t, &mut particles2, 1.0 / 60.0);
        let raised_r2 = particles2[0].x * particles2[0].x + particles2[0].z * particles2[0].z;

        assert!((default_r2 - 20.0f32 * 20.0).abs() < 1e-2, "the default floor (20.0) must govern when ring_spacing alone is tiny");
        assert!(
            raised_r2 > default_r2,
            "a larger min_ring_radius override must raise the actual ring radius above the default floor: default_r2={default_r2} raised_r2={raised_r2}"
        );
    }

    #[test]
    fn hierarchical_3d_layout_is_deterministic_across_repeated_ticks_after_reheat() {
        let edges = [e(0, 1), e(0, 2), e(1, 3)];
        let t = topo(4, &edges);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = HierarchicalLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let first: Vec<(f32, f32, f32)> = particles.iter().map(|p| (p.x, p.y, p.z)).collect();

        layout.reheat(1.0);
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let second: Vec<(f32, f32, f32)> = particles.iter().map(|p| (p.x, p.y, p.z)).collect();
        assert_eq!(first, second);
    }

    // ── Wave G3 item 3 — `radius_aware_spacing` ─────────────────────────

    /// Graph-strengthening arc, owner-approved flip (2026-07-26): was
    /// `hierarchical_params_3d_radius_aware_spacing_defaults_to_false`,
    /// asserting the opposite.
    #[test]
    fn hierarchical_params_3d_radius_aware_spacing_defaults_to_true() {
        assert!(HierarchicalParams3D::default().radius_aware_spacing, "must default ON — graph-strengthening arc owner-approved flip");
    }

    /// When every member's own radius is small (`2 * r <= ring_spacing`),
    /// `radius_aware_ring` must recover the non-aware formula's own
    /// RADIUS *and* every ANGLE exactly — graph-strengthening arc fix:
    /// the two formulas now share one slice-start convention, so a
    /// uniform-radius ring is byte-identical either way, not just
    /// same-radius-different-rotation.
    #[test]
    fn radius_aware_ring_with_small_radii_recovers_the_non_aware_radius_and_angles_exactly() {
        let row: Vec<usize> = (0..5).collect();
        let radii = vec![1.0f32; 5];
        let (radius, angles) = radius_aware_ring(&row, &radii, 70.0, 20.0);
        let expected_radius = (70.0 * 5.0 / TAU).max(20.0);
        assert!((radius - expected_radius).abs() < 1e-3, "radius must match the non-aware formula: {radius} vs {expected_radius}");
        assert_eq!(angles.len(), 5);
        for (slot, &a) in angles.iter().enumerate() {
            assert!(a.is_finite());
            let expected_angle = slot as f32 * TAU / 5.0;
            assert!((a - expected_angle).abs() < 1e-4, "angle {slot}: {a} vs non-aware slice-start {expected_angle}");
        }
    }

    /// Wave G3 item 3 gate: a member whose own diameter exceeds
    /// `ring_spacing` must widen the RING RADIUS to fit the larger
    /// circumference its budget demands.
    #[test]
    fn radius_aware_ring_widens_the_radius_for_a_large_radius_member() {
        let row: Vec<usize> = (0..5).collect();
        let mut radii = vec![1.0f32; 5];
        radii[2] = 200.0; // one big supernode-like member
        let (radius, _) = radius_aware_ring(&row, &radii, 70.0, 20.0);
        let baseline = (70.0 * 5.0 / TAU).max(20.0);
        assert!(radius > baseline, "a large member must widen the ring radius: {radius} vs baseline {baseline}");
    }

    /// Graph-strengthening arc, owner-requested degeneracy gate: on a
    /// UNIFORM-radius graph (every node the same nonzero radius, a real
    /// `SimTopology::radii` value, not the `1.0` default-fallback), a
    /// full `tick()` through the engine-level API must produce IDENTICAL
    /// positions whether `radius_aware_spacing` is on or off — proves the
    /// angle-convention defect (fixed above, in `radius_aware_ring`'s own
    /// doc comment) is actually closed end-to-end, not just at the
    /// isolated helper-function level the two tests above already cover.
    #[test]
    fn radius_aware_spacing_is_a_true_no_op_on_a_uniform_radius_graph_through_a_full_tick() {
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5), e(1, 6), e(1, 7)];
        let radii = vec![7.5f32; 8];
        let t = SimTopology { node_count: 8, edges: &edges, degree: &[], radii };

        let mut off_particles = vec![Particle::default(); 8];
        let mut off_layout = HierarchicalLayout3D::new(HierarchicalParams3D { radius_aware_spacing: false, ..HierarchicalParams3D::default() });
        off_layout.tick(&t, &mut off_particles, 1.0 / 60.0);

        let mut on_particles = vec![Particle::default(); 8];
        let mut on_layout = HierarchicalLayout3D::new(HierarchicalParams3D { radius_aware_spacing: true, ..HierarchicalParams3D::default() });
        on_layout.tick(&t, &mut on_particles, 1.0 / 60.0);

        assert_eq!(off_particles, on_particles, "a uniform-radius graph must produce byte-identical positions regardless of radius_aware_spacing");
    }

    /// Integration gate: `HierarchicalParams3D::radius_aware_spacing`
    /// must actually reach `tick()`, not just exist as an unread field.
    #[test]
    fn hierarchical_3d_layout_radius_aware_spacing_actually_changes_output_when_node_radii_vary() {
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5)];
        let mut radii = vec![1.0f32; 6];
        radii[3] = 100.0;
        let t = SimTopology { node_count: 6, edges: &edges, degree: &[], radii };

        // `radius_aware_spacing` now defaults to `true` (graph-
        // strengthening arc, owner-approved flip, 2026-07-26) — this test
        // still isolates the ON/OFF comparison explicitly on BOTH sides
        // rather than relying on `HierarchicalLayout3D::default()` for
        // "off", since that would no longer BE off.
        let mut off_particles = vec![Particle::default(); 6];
        let mut off_layout =
            HierarchicalLayout3D::new(HierarchicalParams3D { radius_aware_spacing: false, ..HierarchicalParams3D::default() });
        off_layout.tick(&t, &mut off_particles, 1.0 / 60.0);

        let mut on_particles = vec![Particle::default(); 6];
        let mut on_layout =
            HierarchicalLayout3D::new(HierarchicalParams3D { radius_aware_spacing: true, ..HierarchicalParams3D::default() });
        on_layout.tick(&t, &mut on_particles, 1.0 / 60.0);

        assert_ne!(
            (off_particles[3].x, off_particles[3].z),
            (on_particles[3].x, on_particles[3].z),
            "turning radius_aware_spacing on must actually move the big-radius member"
        );
        for i in 0..6 {
            assert_eq!(off_particles[i].y, on_particles[i].y, "layer height (Y) must stay untouched by this item");
        }
    }
}

