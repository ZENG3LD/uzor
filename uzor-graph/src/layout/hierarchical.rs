//! `HierarchicalLayout` — layered top-down (Sugiyama-lite) layout: roots
//! (explicit or auto-detected no-incoming-edge nodes) at `y = 0`,
//! longest-path layer assignment + barycenter within-layer ordering
//! (both shared with [`super::radial::RadialLayout`] via
//! [`super::layering::compute_layering`] — "same layering but polar" per
//! the design brief). One-shot: [`Layout::tick`] computes positions once
//! then reports settled forever; [`Layout::reheat`] forces a recompute
//! (e.g. after changing [`HierarchicalParams`] or the graph's topology)
//! instead of resettling a physics simulation — this layout has none.

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
}

impl Default for HierarchicalParams {
    fn default() -> Self {
        Self { slot_spacing: 70.0, layer_spacing: 110.0, roots: Vec::new() }
    }
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
                p.x = (pos as f32 - center_offset) * self.params.slot_spacing;
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
}
