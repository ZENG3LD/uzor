//! `GraphLayoutMode3D` — the 3D sibling of [`super::mode::GraphLayoutMode`]:
//! a runtime-switchable [`super::Layout`] dispatching to one of the three
//! concrete 3D implementations (force-directed default, hierarchical,
//! radial). Reuses [`LayoutKind`] byte-for-byte (`Force`/`Hierarchical`/
//! `Radial` are dimension-agnostic labels — a second, parallel enum for
//! 3D would be a duplicated concept with no different values). Just like
//! 2D's `GraphEngine<N, E, GraphLayoutMode>`, a
//! `GraphEngine3D<N, E, GraphLayoutMode3D>` instantiation gets runtime
//! layout switching for free — `GraphEngine3D`'s own design stays generic
//! over `Layout` and untouched.
//!
//! **Pause, at the mode-wrapper level, not the [`super::Layout`] trait**
//! (mirrors [`super::mode::GraphLayoutMode`]'s own identical choice — see
//! that module's doc comment for the full reasoning, repeated briefly
//! here since this is the 3D half of the same decision): pausing is a
//! demo/HUD-level "freeze the sim in place" concept that only the
//! currently-active kind's own physics genuinely has an opinion about
//! (Hierarchical/Radial are already one-shot — there is no ongoing motion
//! to freeze). Adding `paused`/`set_paused` to the `Layout` trait itself
//! would force EVERY layout (including ones with no concept of "settled
//! motion" at all) to carry pause plumbing; keeping it here means only
//! the ONE dispatcher that ever needs it does, and [`Layout::tick`]'s
//! trait contract stays exactly as narrow as it already was.

use crate::graph::SimTopology;
use crate::particle::Particle;

use super::force_directed_3d::{ForceDirectedLayout3D, ForceParams3D};
use super::hierarchical_3d::{HierarchicalLayout3D, HierarchicalParams3D};
use super::mode::LayoutKind;
use super::radial_3d::{RadialLayout3D, RadialParams3D};
use super::{Layout, LayoutTickResult};

pub struct GraphLayoutMode3D {
    kind: LayoutKind,
    force: ForceDirectedLayout3D,
    hierarchical: HierarchicalLayout3D,
    radial: RadialLayout3D,
    /// See this module's own doc comment for why pause lives here, not on
    /// [`super::Layout`]. While `true`, [`Layout::tick`] short-circuits to
    /// a `settled` no-op without touching the active kind's own state at
    /// all — a paused Force sim's `alpha`/velocities stay frozen exactly
    /// where they were, ready to resume from that same point.
    paused: bool,
}

impl Default for GraphLayoutMode3D {
    fn default() -> Self {
        Self {
            kind: LayoutKind::Force,
            force: ForceDirectedLayout3D::default(),
            hierarchical: HierarchicalLayout3D::default(),
            radial: RadialLayout3D::default(),
            paused: false,
        }
    }
}

impl GraphLayoutMode3D {
    pub fn kind(&self) -> LayoutKind {
        self.kind
    }

    /// Switch the active layout — ALSO clears [`GraphLayoutMode3D::paused`]
    /// unconditionally: a fresh kind starts running, never inheriting a
    /// stale freeze from whichever kind was active before (a paused flag
    /// left set after switching AWAY from Force would otherwise silently
    /// prevent a freshly-selected Hierarchical/Radial from ever computing
    /// its own one-shot layout, since [`Layout::tick`] checks `paused`
    /// before dispatching to any kind at all). Hierarchical/radial always
    /// recompute fresh on the next tick (one-shot semantics — `reheat`
    /// just clears their `computed` flag); force-directed reheats to full
    /// alpha so it visibly resettles instead of sitting frozen wherever
    /// the previous layout left it.
    pub fn set_kind(&mut self, kind: LayoutKind) {
        self.kind = kind;
        self.paused = false;
        match kind {
            LayoutKind::Force => self.force.reheat(1.0),
            LayoutKind::Hierarchical => self.hierarchical.reheat(1.0),
            LayoutKind::Radial => self.radial.reheat(1.0),
        }
    }

    /// Whether the active kind is currently frozen — see this module's
    /// own doc comment.
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Freeze (`true`) or resume (`false`) the active kind in place.
    /// Deliberately does NOT touch `reheat`/`set_alpha_target` state —
    /// pausing is a pure integration freeze, not a physics reset, so
    /// resuming continues exactly where the sim left off.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn force_params(&self) -> &ForceParams3D {
        self.force.params()
    }

    pub fn set_force_params(&mut self, params: ForceParams3D) {
        self.force.set_params(params);
    }

    pub fn hierarchical_params(&self) -> &HierarchicalParams3D {
        self.hierarchical.params()
    }

    pub fn set_hierarchical_params(&mut self, params: HierarchicalParams3D) {
        self.hierarchical.set_params(params);
    }

    pub fn radial_params(&self) -> &RadialParams3D {
        self.radial.params()
    }

    pub fn set_radial_params(&mut self, params: RadialParams3D) {
        self.radial.set_params(params);
    }
}

impl Layout for GraphLayoutMode3D {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult {
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
        let mut mode = GraphLayoutMode3D::default();
        assert_eq!(mode.kind(), LayoutKind::Force);

        mode.set_kind(LayoutKind::Hierarchical);
        assert_eq!(mode.kind(), LayoutKind::Hierarchical);
        let r = mode.tick(&t, &mut particles, 1.0 / 60.0);
        // Hierarchical/radial are one-shot — settled immediately.
        assert!(r.settled);
        assert!(particles[0].y > particles[1].y, "root must sit above its child in Y-up world");
    }

    #[test]
    fn kind_switch_round_trips_between_force_hierarchical_and_radial() {
        let mut mode = GraphLayoutMode3D::default();
        for kind in [LayoutKind::Hierarchical, LayoutKind::Radial, LayoutKind::Force, LayoutKind::Radial] {
            mode.set_kind(kind);
            assert_eq!(mode.kind(), kind);
        }
    }

    #[test]
    fn set_kind_clears_a_previous_pause() {
        let mut mode = GraphLayoutMode3D::default();
        mode.set_paused(true);
        assert!(mode.paused());
        mode.set_kind(LayoutKind::Hierarchical);
        assert!(!mode.paused(), "switching kind must clear a stale pause from the previous kind");
    }

    #[test]
    fn paused_force_layout_does_not_move_particles_and_reports_settled() {
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges);
        let mut particles = vec![Particle::at3(-10.0, 0.0, 0.0), Particle::at3(10.0, 0.0, 0.0), Particle::at3(0.0, 15.0, 5.0)];
        let mut mode = GraphLayoutMode3D::default();
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
        let r = mode.tick(&t, &mut particles, 1.0 / 60.0);
        assert_ne!(particles, before, "resuming must let the sim move particles again");
        let _ = r;
    }

    #[test]
    fn mode3d_is_deterministic_across_identical_runs() {
        let edges = [SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 1.0 }, SimEdge { from: NodeIndex(0), to: NodeIndex(2), weight: 1.0 }];
        let t = topo(3, &edges);

        let mut particles_a = vec![Particle::default(); 3];
        let mut mode_a = GraphLayoutMode3D::default();
        mode_a.set_kind(LayoutKind::Radial);
        mode_a.tick(&t, &mut particles_a, 1.0 / 60.0);

        let mut particles_b = vec![Particle::default(); 3];
        let mut mode_b = GraphLayoutMode3D::default();
        mode_b.set_kind(LayoutKind::Radial);
        mode_b.tick(&t, &mut particles_b, 1.0 / 60.0);

        assert_eq!(particles_a, particles_b);
    }
}
