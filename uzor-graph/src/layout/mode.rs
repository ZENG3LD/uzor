//! `GraphLayoutMode` — a runtime-switchable [`super::Layout`] that
//! dispatches to one of the three concrete implementations (force-
//! directed default, hierarchical, radial). `GraphEngine<N, E, L>`'s own
//! design stays generic-over-`Layout` and untouched — this is one MORE
//! `Layout` impl, not a special case the engine knows about. A
//! `GraphEngine<N, E, GraphLayoutMode>` instantiation gets a runtime
//! `set_layout` agent action (see `agent.rs`) for free by downcasting;
//! any other concrete `L` simply doesn't expose that action.

use crate::graph::SimTopology;
use crate::particle::Particle;

use super::force_directed::{ForceDirectedLayout, ForceParams};
use super::hierarchical::{HierarchicalLayout, HierarchicalParams};
use super::radial::{RadialLayout, RadialParams};
use super::{Layout, LayoutTickResult};

/// Which concrete layout is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Force,
    Hierarchical,
    Radial,
}

pub struct GraphLayoutMode {
    kind: LayoutKind,
    force: ForceDirectedLayout,
    hierarchical: HierarchicalLayout,
    radial: RadialLayout,
}

impl Default for GraphLayoutMode {
    fn default() -> Self {
        Self {
            kind: LayoutKind::Force,
            force: ForceDirectedLayout::default(),
            hierarchical: HierarchicalLayout::default(),
            radial: RadialLayout::default(),
        }
    }
}

impl GraphLayoutMode {
    pub fn kind(&self) -> LayoutKind {
        self.kind
    }

    /// Switch the active layout. Hierarchical/radial always recompute
    /// fresh on the next tick (one-shot semantics — `reheat` just clears
    /// their `computed` flag); force-directed reheats to full alpha so
    /// it visibly resettles instead of sitting frozen wherever the
    /// previous layout left it.
    pub fn set_kind(&mut self, kind: LayoutKind) {
        self.kind = kind;
        match kind {
            LayoutKind::Force => self.force.reheat(1.0),
            LayoutKind::Hierarchical => self.hierarchical.reheat(1.0),
            LayoutKind::Radial => self.radial.reheat(1.0),
        }
    }

    pub fn force_params(&self) -> &ForceParams {
        self.force.params()
    }

    pub fn set_force_params(&mut self, params: ForceParams) {
        self.force.set_params(params);
    }

    pub fn hierarchical_params(&self) -> &HierarchicalParams {
        self.hierarchical.params()
    }

    pub fn set_hierarchical_params(&mut self, params: HierarchicalParams) {
        self.hierarchical.set_params(params);
    }

    pub fn radial_params(&self) -> &RadialParams {
        self.radial.params()
    }

    pub fn set_radial_params(&mut self, params: RadialParams) {
        self.radial.set_params(params);
    }
}

impl Layout for GraphLayoutMode {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult {
        match self.kind {
            LayoutKind::Force => self.force.tick(topo, particles, dt),
            LayoutKind::Hierarchical => self.hierarchical.tick(topo, particles, dt),
            LayoutKind::Radial => self.radial.tick(topo, particles, dt),
        }
    }

    fn reheat(&mut self, alpha: f32) {
        match self.kind {
            LayoutKind::Force => self.force.reheat(alpha),
            LayoutKind::Hierarchical => self.hierarchical.reheat(alpha),
            LayoutKind::Radial => self.radial.reheat(alpha),
        }
    }

    fn is_settled(&self) -> bool {
        match self.kind {
            LayoutKind::Force => self.force.is_settled(),
            LayoutKind::Hierarchical => self.hierarchical.is_settled(),
            LayoutKind::Radial => self.radial.is_settled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{NodeIndex, SimEdge};

    fn topo(node_count: usize, edges: &[SimEdge]) -> SimTopology<'_> {
        SimTopology { node_count, edges, degree: &[], radii: vec![1.0; node_count] }
    }

    #[test]
    fn set_kind_switches_which_algorithm_tick_dispatches_to() {
        let edges = [SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 1.0 }];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut mode = GraphLayoutMode::default();
        assert_eq!(mode.kind(), LayoutKind::Force);

        mode.set_kind(LayoutKind::Hierarchical);
        assert_eq!(mode.kind(), LayoutKind::Hierarchical);
        let r = mode.tick(&t, &mut particles, 1.0 / 60.0);
        // Hierarchical/radial are one-shot — settled immediately.
        assert!(r.settled);
        assert_eq!(particles[0].y, 0.0);
        assert!(particles[1].y > particles[0].y);
    }
}
