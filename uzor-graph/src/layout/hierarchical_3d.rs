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
//! ring_spacing * count / TAU`, floored at [`MIN_RING_RADIUS`]) so a
//! crowded layer doesn't overlap itself. One-shot: [`Layout::tick`]
//! computes positions once then reports settled forever;
//! [`Layout::reheat`] forces a recompute — same freeze semantics as
//! [`super::hierarchical::HierarchicalLayout`].

use std::f32::consts::TAU;

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::layering::compute_layering;
use super::{Layout, LayoutTickResult};

/// Floor on a ring's own radius (see this module's own `r = ring_spacing
/// * count / TAU` formula) — keeps even a single-node layer visibly off
/// the central vertical axis instead of collapsing toward it at a small
/// `ring_spacing`.
const MIN_RING_RADIUS: f32 = 20.0;

#[derive(Debug, Clone)]
pub struct HierarchicalParams3D {
    /// World-unit vertical spacing between adjacent layers (rings) —
    /// same name/role as 2D's `HierarchicalParams::layer_spacing`.
    pub layer_spacing: f32,
    /// World-unit circumferential spacing per ring member — a ring
    /// holding `count` nodes gets radius `ring_spacing * count / TAU`
    /// (clamped to [`MIN_RING_RADIUS`]), i.e. roughly `ring_spacing`
    /// world units of arc length per sibling.
    pub ring_spacing: f32,
    /// Explicit root nodes; empty = auto-detect every zero-in-degree
    /// node as a root (see [`super::layering::compute_layering`]).
    pub roots: Vec<NodeIndex>,
}

impl Default for HierarchicalParams3D {
    fn default() -> Self {
        Self { layer_spacing: 220.0, ring_spacing: 70.0, roots: Vec::new() }
    }
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
            let radius = (self.params.ring_spacing * count / TAU).max(MIN_RING_RADIUS);
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
                let angle = slot as f32 * TAU / row.len().max(1) as f32;
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
}
