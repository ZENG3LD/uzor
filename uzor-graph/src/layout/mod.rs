//! Pluggable layout algorithms over a [`crate::particle::Particle`] slice.

pub mod barnes_hut;
pub mod force_directed;
pub mod stubs;

pub use force_directed::{ForceDirectedLayout, ForceParams};
pub use stubs::{HierarchicalLayout, RadialLayout};

use crate::graph::SimTopology;
use crate::particle::Particle;

/// Outcome of one [`Layout::tick`] call.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutTickResult {
    /// Current cooling parameter (1.0 = just reheated, `alpha_min` and
    /// below = settled). Algorithms that don't use alpha cooling (e.g.
    /// [`stubs::HierarchicalLayout`]) report `0.0`.
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
    /// change) so the layout "wakes" and resettles. No-op for algorithms
    /// without a cooling schedule.
    fn reheat(&mut self, alpha: f32);

    fn is_settled(&self) -> bool;
}
