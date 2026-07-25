//! `HierarchicalLayout` — layered top-down (Sugiyama-lite) layout: roots
//! (explicit or auto-detected no-incoming-edge nodes) at `y = 0`,
//! longest-path layer assignment + barycenter within-layer ordering
//! (both shared with [`super::radial::RadialLayout`] via
//! [`super::layering::compute_layering`] — "same layering but polar" per
//! the design brief). One-shot: [`Layout::tick`] computes positions once
//! then reports settled forever; [`Layout::reheat`] forces a recompute
//! (e.g. after changing [`HierarchicalParams`] or the graph's topology)
//! instead of resettling a physics simulation — this layout has none.
//!
//! **Wave G3 item 3 — `radius_aware_spacing`.** OFF by default (doctrine:
//! no silent output change) — see [`HierarchicalParams::radius_aware_spacing`]'s
//! own doc comment. Scoped to WITHIN-ROW (horizontal) spacing only —
//! vertical layer-to-layer spacing stays the plain `layer * layer_spacing`
//! formula regardless, matching the audit's own "guaranteed to overlap
//! its ring/LAYER NEIGHBOURS" framing (same-row siblings, the actual
//! reported overlap case).

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::layering::compute_layering;
use super::{Layout, LayoutTickResult};

#[derive(Debug, Clone)]
pub struct HierarchicalParams {
    /// World-unit spacing between adjacent slots within the same layer.
    pub slot_spacing: f32,
    /// World-unit spacing between adjacent layers (rows).
    pub layer_spacing: f32,
    /// Explicit root nodes; empty = auto-detect every zero-in-degree
    /// node as a root (see [`super::layering::compute_layering`]).
    pub roots: Vec<NodeIndex>,
    /// When `true`, adjacent slots within a row are spaced by
    /// `max(slot_spacing, radii[left] + radii[right])` (a real
    /// non-overlap guarantee, via [`SimTopology::radii`]) instead of the
    /// uniform `slot_spacing` every slot uses today, so a
    /// collapsed-cluster supernode (sqrt-scaled radius,
    /// `crate::cluster::supernode_radius`) doesn't overlap its row
    /// neighbors. Defaults to `false`: this CHANGES layout output for
    /// any graph with varying node radii, so today's uniform spacing is
    /// preserved unless a caller opts in (doctrine: no silent output
    /// change — recommended to the owner, not flipped here).
    pub radius_aware_spacing: bool,
}

impl Default for HierarchicalParams {
    fn default() -> Self {
        Self { slot_spacing: 70.0, layer_spacing: 110.0, roots: Vec::new(), radius_aware_spacing: false }
    }
}

/// Left-to-right x offset per slot in `row` (already barycenter-slot-
/// ordered), centered so the row's own midpoint sits at `x = 0` — the
/// [`HierarchicalParams::radius_aware_spacing`] alternative to the plain
/// `(pos - center_offset) * slot_spacing` uniform formula. Adjacent
/// slots are separated by `max(slot_spacing, radii[left] + radii[right])`
/// — a real non-overlap guarantee for two circles of those radii, while
/// never spacing TIGHTER than the ordinary uniform spacing would.
fn radius_aware_row_offsets(row: &[usize], radii: &[f32], slot_spacing: f32) -> Vec<f32> {
    if row.is_empty() {
        return Vec::new();
    }
    let own_radius = |node: usize| radii.get(node).copied().unwrap_or(1.0).max(0.0);
    let mut xs = vec![0.0f32; row.len()];
    for i in 1..row.len() {
        let gap = (own_radius(row[i - 1]) + own_radius(row[i])).max(slot_spacing);
        xs[i] = xs[i - 1] + gap;
    }
    let center = (xs[0] + xs[row.len() - 1]) * 0.5;
    for x in xs.iter_mut() {
        *x -= center;
    }
    xs
}

#[derive(Debug, Clone, Default)]
pub struct HierarchicalLayout {
    params: HierarchicalParams,
    computed: bool,
}

impl HierarchicalLayout {
    pub fn new(params: HierarchicalParams) -> Self {
        Self { params, computed: false }
    }

    pub fn params(&self) -> &HierarchicalParams {
        &self.params
    }

    /// Replace the params and force a recompute on the next `tick`.
    pub fn set_params(&mut self, params: HierarchicalParams) {
        self.params = params;
        self.computed = false;
    }
}

impl Layout for HierarchicalLayout {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        if self.computed {
            return LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true };
        }
        let layering = compute_layering(topo, &self.params.roots);
        for row in &layering.layers {
            let center_offset = (row.len() as f32 - 1.0) / 2.0;
            let radius_aware_xs =
                self.params.radius_aware_spacing.then(|| radius_aware_row_offsets(row, &topo.radii, self.params.slot_spacing));
            for (pos, &node) in row.iter().enumerate() {
                let Some(p) = particles.get_mut(node) else { continue };
                // Every `Layout::tick` must honor a set fx/fy (see the
                // trait doc) — a pinned particle (e.g. a collapsed
                // cluster's frozen member) stays exactly where it is
                // instead of being overwritten by the layering math.
                if let (Some(fx), Some(fy)) = (p.fx, p.fy) {
                    p.x = fx;
                    p.y = fy;
                    p.vx = 0.0;
                    p.vy = 0.0;
                    continue;
                }
                p.x = match &radius_aware_xs {
                    Some(xs) => xs[pos],
                    None => (pos as f32 - center_offset) * self.params.slot_spacing,
                };
                p.y = layering.layer[node] as f32 * self.params.layer_spacing;
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

    #[test]
    fn one_shot_tick_places_every_node_with_no_nan_and_layer_ordered_y() {
        let edges = [e(0, 1), e(0, 2), e(1, 3), e(2, 3)];
        let t = topo(4, &edges);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = HierarchicalLayout::default();

        let r1 = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(r1.settled);
        for p in &particles {
            assert!(p.x.is_finite() && !p.x.is_nan());
            assert!(p.y.is_finite() && !p.y.is_nan());
        }
        // Root strictly above its two children, which sit strictly
        // above the shared bottom node.
        assert!(particles[0].y < particles[1].y);
        assert!(particles[0].y < particles[2].y);
        assert!(particles[1].y < particles[3].y);
        assert!(particles[2].y < particles[3].y);
    }

    #[test]
    fn one_shot_layout_freezes_after_first_tick() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = HierarchicalLayout::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let after_first = particles.clone();

        // Nudge a position by hand — a second tick must NOT touch it
        // (one-shot: compute once, then freeze).
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
        let mut layout = HierarchicalLayout::default();
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
        particles[1].pin(42.0, -7.0);
        let mut layout = HierarchicalLayout::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert_eq!((particles[1].x, particles[1].y), (42.0, -7.0));
    }

    // ── Wave G3 item 3 — `radius_aware_spacing` ─────────────────────────

    #[test]
    fn hierarchical_params_radius_aware_spacing_defaults_to_false() {
        assert!(!HierarchicalParams::default().radius_aware_spacing, "must default OFF — doctrine: no silent output change");
    }

    #[test]
    fn radius_aware_row_offsets_with_small_radii_matches_the_uniform_baseline_exactly() {
        // Every radius well under slot_spacing/2 — the `.max(slot_spacing)`
        // floor must dominate everywhere, recovering an evenly-spaced row.
        let row = vec![0usize, 1, 2, 3, 4];
        let radii = vec![1.0f32; 5];
        let xs = radius_aware_row_offsets(&row, &radii, 70.0);
        for pair in xs.windows(2) {
            assert!((pair[1] - pair[0] - 70.0).abs() < 1e-4, "gap must equal slot_spacing when radii are small: {xs:?}");
        }
        let center_offset = (row.len() as f32 - 1.0) / 2.0;
        for (pos, &x) in xs.iter().enumerate() {
            let expected = (pos as f32 - center_offset) * 70.0;
            assert!((x - expected).abs() < 1e-4, "pos {pos}: {x} vs uniform-formula {expected}");
        }
    }

    /// Wave G3 item 3 gate: a member whose own radius exceeds
    /// `slot_spacing` must widen the gaps on EITHER side of it, not just
    /// silently ignore the larger value.
    #[test]
    fn radius_aware_row_offsets_widens_the_gap_around_a_large_radius_member() {
        let row = vec![0usize, 1, 2, 3, 4];
        let mut radii = vec![1.0f32; 5];
        radii[2] = 100.0; // slot 2 (index 2 in `row`) is a big supernode
        let xs = radius_aware_row_offsets(&row, &radii, 70.0);

        let gap_01 = xs[1] - xs[0];
        let gap_12 = xs[2] - xs[1];
        let gap_23 = xs[3] - xs[2];
        let gap_34 = xs[4] - xs[3];
        assert!((gap_01 - 70.0).abs() < 1e-4, "an ordinary gap must stay at slot_spacing: {gap_01}");
        assert!((gap_34 - 70.0).abs() < 1e-4, "an ordinary gap must stay at slot_spacing: {gap_34}");
        assert!(gap_12 > gap_01, "the gap approaching the big member must widen: {gap_12} vs {gap_01}");
        assert!(gap_23 > gap_01, "the gap leaving the big member must widen too: {gap_23} vs {gap_01}");
        assert!((gap_12 - 101.0).abs() < 1e-2, "gap must equal radii[1] + radii[2] = 1 + 100: {gap_12}");
    }

    /// Integration gate: `HierarchicalParams::radius_aware_spacing` must
    /// actually reach `tick()` and change output for a real graph with
    /// varying radii, while leaving Y (layer height) untouched — this
    /// item's own documented scope is within-row spacing only.
    #[test]
    fn hierarchical_layout_radius_aware_spacing_actually_changes_output_when_node_radii_vary() {
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5)];
        // Node 1 is the FIRST child in slot order (position 0, not the
        // row's own center) — a big radius there must genuinely shift
        // its own x, unlike a perfectly-centered row member (whose x
        // stays 0 either way purely from symmetry, an accidental
        // non-proof this test avoids).
        let mut radii = vec![1.0f32; 6];
        radii[1] = 100.0;
        let t = SimTopology { node_count: 6, edges: &edges, degree: &[], radii };

        let mut off_particles = vec![Particle::default(); 6];
        let mut off_layout = HierarchicalLayout::default();
        off_layout.tick(&t, &mut off_particles, 1.0 / 60.0);

        let mut on_particles = vec![Particle::default(); 6];
        let mut on_layout = HierarchicalLayout::new(HierarchicalParams { radius_aware_spacing: true, ..HierarchicalParams::default() });
        on_layout.tick(&t, &mut on_particles, 1.0 / 60.0);

        assert_ne!(off_particles[1].x, on_particles[1].x, "turning radius_aware_spacing on must actually move the big-radius member");
        for i in 0..6 {
            assert_eq!(off_particles[i].y, on_particles[i].y, "layer height (Y) must stay untouched by this item — within-row spacing only");
        }
    }
}
