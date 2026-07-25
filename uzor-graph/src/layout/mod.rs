//! Pluggable layout algorithms over a [`crate::particle::Particle`] slice.

pub mod barnes_hut;
pub mod barnes_hut_3d;
pub mod force_directed;
pub mod force_directed_3d;
pub mod hierarchical;
pub mod hierarchical_3d;
pub mod layering;
pub mod mode;
pub mod mode3d;
pub mod radial;
pub mod radial_3d;

pub use force_directed::{ForceDirectedLayout, ForceParams};
pub use force_directed_3d::{ForceDirectedLayout3D, ForceParams3D};
pub use hierarchical::{HierarchicalLayout, HierarchicalParams};
pub use hierarchical_3d::{HierarchicalLayout3D, HierarchicalParams3D};
pub use layering::DepthMetric;
pub use mode::{GraphLayoutMode, LayoutKind};
pub use mode3d::GraphLayoutMode3D;
pub use radial::{RadialLayout, RadialParams};
pub use radial_3d::{RadialLayout3D, RadialParams3D};

use crate::graph::SimTopology;
use crate::particle::Particle;

/// Outcome of one [`Layout::tick`] call.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutTickResult {
    /// Current cooling parameter (1.0 = just reheated, `alpha_min` and
    /// below = settled). Algorithms that don't use alpha cooling (e.g.
    /// [`hierarchical::HierarchicalLayout`]/[`radial::RadialLayout`] —
    /// one-shot, no cooling schedule) report `0.0`.
    pub alpha: f32,
    /// Largest per-particle displacement this tick, in world units.
    pub max_displacement: f32,
    pub settled: bool,
}

/// A pluggable graph layout algorithm.
///
/// Deliberately does *not* carry a `pin`/`unpin` pair: [`Particle::fx`]/
/// [`Particle::fy`] (see [`crate::particle`]) are the single source of
/// pin truth, exactly like d3-force's own `fx`/`fy` node fields — any
/// code that can see the particle slice (the engine, an agent action, a
/// drag handler) can set or clear them directly. Every `Layout::tick`
/// implementation must honor a set `fx`/`fy` by holding that particle's
/// position fixed instead of integrating it.
pub trait Layout {
    /// Advance the simulation by one frame. `dt` is real elapsed seconds
    /// (frame-rate independent) — implementations that care about a
    /// fixed-step cadence normalize internally.
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult;

    /// Bump internal cooling back up (e.g. on drag-start or a structural
    /// change) so the layout "wakes" and resettles. For a one-shot
    /// layout (hierarchical/radial) this instead just clears the
    /// "already computed" flag, forcing a fresh compute on the next
    /// `tick`.
    fn reheat(&mut self, alpha: f32);

    fn is_settled(&self) -> bool;

    /// Sustained-reheat target ("`alphaTarget`" in d3-force parlance —
    /// Wave 2.1 drag-physics contract, obsidian doc §drag/d3-canon):
    /// while set above `0.0`, [`Layout::tick`] eases `alpha` TOWARD this
    /// value each tick instead of decaying it all the way to `0.0`. This
    /// is what keeps an interaction (e.g. an active node drag) feeling
    /// "warm" for its whole duration instead of a one-shot [`Layout::reheat`]
    /// bump that starts cooling back down immediately. Layouts without
    /// an alpha-decay schedule (hierarchical/radial — one-shot, no
    /// cooling) leave this the default no-op.
    fn set_alpha_target(&mut self, _target: f32) {}

    /// Current sustained-reheat target — see [`Layout::set_alpha_target`].
    /// Default `0.0` for layouts that don't override
    /// [`Layout::set_alpha_target`].
    fn alpha_target(&self) -> f32 {
        0.0
    }
}
