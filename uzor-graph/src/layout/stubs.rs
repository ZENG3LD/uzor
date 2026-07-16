//! Alternate [`super::Layout`] implementations — explicitly deferred
//! this run (see the top-level task scope / engine design doc §2.4,
//! §6 M5). Both compile and behave as safe no-op layouts: particles
//! keep whatever position they were seeded at, and the tick immediately
//! reports settled. Wiring the real algorithms is future work that only
//! needs to fill in these two `impl` bodies — the surrounding
//! `Layout` trait contract, render pipeline, and interaction layer do
//! not change.

use crate::graph::SimTopology;
use crate::particle::Particle;

use super::{Layout, LayoutTickResult};

/// STUB — hop-depth column layout that auto-wraps overpopulated levels
/// into multiple sub-columns instead of one unbounded column (would
/// generalize/replace `layered_init` from the rejected MVP; see design
/// doc §2.4/§6). Not implemented this run.
#[derive(Debug, Clone, Copy, Default)]
pub struct HierarchicalLayout;

impl Layout for HierarchicalLayout {
    fn tick(&mut self, _topo: &SimTopology<'_>, _particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true }
    }

    fn reheat(&mut self, _alpha: f32) {}

    fn is_settled(&self) -> bool {
        true
    }
}

/// STUB — concentric-ring layout by hop-depth or distance-from-selection
/// (Obsidian's "local graph" isolate look; design doc §1.3/§2.4). Not
/// implemented this run.
#[derive(Debug, Clone, Copy, Default)]
pub struct RadialLayout;

impl Layout for RadialLayout {
    fn tick(&mut self, _topo: &SimTopology<'_>, _particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true }
    }

    fn reheat(&mut self, _alpha: f32) {}

    fn is_settled(&self) -> bool {
        true
    }
}
