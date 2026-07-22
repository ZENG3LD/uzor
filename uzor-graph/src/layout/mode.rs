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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Freezes [`Layout::tick`] in place regardless of `kind` (HUD
    /// pause/resume — see [`GraphLayoutMode::set_paused`]'s own doc
    /// comment for the mode-wrapper-level-not-trait-level design choice,
    /// which [`super::mode3d::GraphLayoutMode3D`] mirrors for 3D).
    paused: bool,
}

impl Default for GraphLayoutMode {
    fn default() -> Self {
        Self {
            kind: LayoutKind::Force,
            force: ForceDirectedLayout::default(),
            hierarchical: HierarchicalLayout::default(),
            radial: RadialLayout::default(),
            paused: false,
        }
    }
}

impl GraphLayoutMode {
    pub fn kind(&self) -> LayoutKind {
        self.kind
    }

    /// Switch the active layout — ALSO clears [`GraphLayoutMode::paused`]
    /// unconditionally, so a freshly-selected kind always actually runs
    /// instead of silently inheriting a stale freeze from whatever was
    /// active before (see [`Layout::tick`]'s own pause short-circuit
    /// below). Hierarchical/radial always recompute fresh on the next
    /// tick (one-shot semantics — `reheat` just clears their `computed`
    /// flag); force-directed reheats to full alpha so it visibly
    /// resettles instead of sitting frozen wherever the previous layout
    /// left it.
    pub fn set_kind(&mut self, kind: LayoutKind) {
        self.kind = kind;
        self.paused = false;
        match kind {
            LayoutKind::Force => self.force.reheat(1.0),
            LayoutKind::Hierarchical => self.hierarchical.reheat(1.0),
            LayoutKind::Radial => self.radial.reheat(1.0),
        }
    }

    /// Whether the active kind is currently frozen — HUD "re-click the
    /// active FORCE button pauses/resumes" semantics
    /// (`force_graph_demo`'s own `apply_hud_control`). Deliberately kept
    /// at THIS wrapper level rather than added to the [`Layout`] trait
    /// itself: pausing is a "freeze the sim in place" concept that only a
    /// continuously-running layout (Force) genuinely has anything to
    /// freeze — Hierarchical/Radial are already one-shot, with no ongoing
    /// motion of their own — so putting it on every `Layout` impl would
    /// force unrelated layouts to carry plumbing they have no use for.
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Freeze (`true`) or resume (`false`) the active kind in place.
    /// Does NOT touch `reheat`/`set_alpha_target` state — pausing is a
    /// pure integration freeze, not a physics reset, so resuming
    /// continues exactly where the sim left off.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
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
        // Paused: skip integration entirely, regardless of `kind` — a
        // paused Force sim's particles/velocities/alpha stay exactly
        // where they were; a paused one-shot Hierarchical/Radial simply
        // never (re)computes. `settled: true` so `GraphEngine::is_hot`'s
        // redraw-gating never spins on a paused layout (see this
        // struct's own `paused` doc comment).
        if self.paused {
            return LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true };
        }
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
        if self.paused {
            return true;
        }
        match self.kind {
            LayoutKind::Force => self.force.is_settled(),
            LayoutKind::Hierarchical => self.hierarchical.is_settled(),
            LayoutKind::Radial => self.radial.is_settled(),
        }
    }

    fn set_alpha_target(&mut self, target: f32) {
        match self.kind {
            LayoutKind::Force => self.force.set_alpha_target(target),
            LayoutKind::Hierarchical => self.hierarchical.set_alpha_target(target),
            LayoutKind::Radial => self.radial.set_alpha_target(target),
        }
    }

    fn alpha_target(&self) -> f32 {
        match self.kind {
            LayoutKind::Force => self.force.alpha_target(),
            LayoutKind::Hierarchical => self.hierarchical.alpha_target(),
            LayoutKind::Radial => self.radial.alpha_target(),
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

    #[test]
    fn set_kind_clears_a_previous_pause() {
        let mut mode = GraphLayoutMode::default();
        mode.set_paused(true);
        assert!(mode.paused());
        mode.set_kind(LayoutKind::Hierarchical);
        assert!(!mode.paused(), "switching kind must clear a stale pause from the previous kind");
    }

    #[test]
    fn paused_layout_does_not_advance_particles_and_reports_settled() {
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges);
        let mut particles = vec![Particle::at(-10.0, 0.0), Particle::at(10.0, 0.0), Particle::at(0.0, 15.0)];
        let mut mode = GraphLayoutMode::default();
        // A couple of unpaused ticks so the force sim genuinely has
        // motion to freeze, not a trivially-already-static start.
        mode.tick(&t, &mut particles, 1.0 / 60.0);
        mode.tick(&t, &mut particles, 1.0 / 60.0);
        let before = particles.clone();

        mode.set_paused(true);
        for _ in 0..10 {
            let r = mode.tick(&t, &mut particles, 1.0 / 60.0);
            assert!(r.settled, "a paused layout must report settled for redraw purposes");
            assert_eq!(r.max_displacement, 0.0);
        }
        assert_eq!(particles, before, "a paused layout must not move any particle");
        assert!(mode.is_settled());

        mode.set_paused(false);
        mode.tick(&t, &mut particles, 1.0 / 60.0);
        assert_ne!(particles, before, "resuming must let the sim move particles again");
    }
}
