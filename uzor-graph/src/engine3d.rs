//! `GraphEngine3D` — the 3D sibling of [`crate::engine::GraphEngine`]
//! (W3D arc plan §1.2: a separate engine, NOT a `Dimension` mode bolted
//! onto the 2D facade). Reuses `Graph`/`SimTopology`/`NodeIndex`/
//! `EdgeIndex` unchanged; owns its own [`Camera3D`] and a 3D-aware
//! [`Layout`] impl ([`crate::layout::force_directed_3d::ForceDirectedLayout3D`]
//! by default).
//!
//! Wave 1 (plan §4) proves the physics core only:
//! [`GraphEngine3D::new`]/[`GraphEngine3D::tick`] are real,
//! [`GraphEngine3D::build_scene`]/[`GraphEngine3D::on_event`] are
//! documented no-op stubs — render-path construction (plan §1.3) and
//! event dispatch (plan §1.4/§1.5) land in Wave 2/3.

use std::sync::Arc;

use uzor::input::PlatformEvent;
use uzor::types::Rect;
use uzor_urx_3d::{MeshLit, PerspectiveCamera, Scene3D};

use crate::camera3d::Camera3D;
use crate::graph::{Graph, NodeIndex};
use crate::layout::force_directed_3d::ForceDirectedLayout3D;
use crate::layout::{Layout, LayoutTickResult};
use crate::particle::Particle;

/// Shared unit-sphere node mesh geometry (plan §1.3) — latitude/longitude
/// resolution tuned for a smooth silhouette at typical node screen sizes
/// without an excessive vertex count. One shared mesh serves every node
/// via `uzor-urx-3d`'s Arc-identity instancing, so this cost is paid
/// once per engine, not once per node.
const NODE_SPHERE_RINGS: u32 = 12;
const NODE_SPHERE_SLICES: u32 = 16;
/// Shared unit-cylinder edge mesh geometry (plan §1.3).
const EDGE_CYLINDER_SLICES: u32 = 8;

/// The 3D sibling of [`crate::engine::GraphEngine`] — see the module doc.
pub struct GraphEngine3D<N, E, L: Layout = ForceDirectedLayout3D> {
    pub graph: Graph<N, E>,
    pub particles: Vec<Particle>,
    pub layout: L,
    pub camera: Camera3D,
    pub hovered: Option<NodeIndex>,
    pub selected: Option<NodeIndex>,
    /// Shared unit sphere every node instances from (plan §1.3) — built
    /// once at construction, never mutated.
    node_mesh: Arc<MeshLit>,
    /// Shared unit cylinder every edge instances from (plan §1.3).
    edge_mesh: Arc<MeshLit>,
}

impl<N, E, L: Layout> GraphEngine3D<N, E, L> {
    pub fn new(graph: Graph<N, E>, layout: L) -> Self {
        let n = graph.node_count();
        Self {
            graph,
            particles: vec![Particle::default(); n],
            layout,
            camera: Camera3D::default(),
            hovered: None,
            selected: None,
            node_mesh: Arc::new(MeshLit::sphere(1.0, NODE_SPHERE_RINGS, NODE_SPHERE_SLICES, [1.0, 1.0, 1.0, 1.0])),
            edge_mesh: Arc::new(MeshLit::cylinder(1.0, 1.0, EDGE_CYLINDER_SLICES, [1.0, 1.0, 1.0, 1.0])),
        }
    }

    /// Shared node-sphere mesh — exposed read-only so a future render
    /// pass (Wave 2) can reuse it without re-generating geometry.
    pub fn node_mesh(&self) -> &Arc<MeshLit> {
        &self.node_mesh
    }

    /// Shared edge-cylinder mesh — see [`GraphEngine3D::node_mesh`].
    pub fn edge_mesh(&self) -> &Arc<MeshLit> {
        &self.edge_mesh
    }

    /// Advance the 3D force simulation by `dt` real seconds — the whole
    /// of this wave's proof surface (plan §4 Wave 1).
    pub fn tick(&mut self, dt: f32) -> LayoutTickResult {
        let topo = self.graph.topology();
        self.layout.tick(&topo, &mut self.particles, dt)
    }

    /// Wave 2/3 stub (plan §1.4/§1.5 — orbit/dolly/pan drag, hover/click
    /// pick). Always reports "unhandled" this wave — Wave 1 proves
    /// physics only (plan §4).
    pub fn on_event(&mut self, _event: &PlatformEvent, _viewport: Rect) -> bool {
        false
    }

    /// Wave 2 stub (plan §1.3 — instanced sphere nodes + cylinder
    /// edges). Returns an empty scene this wave — Wave 1 proves physics
    /// only (plan §4).
    pub fn build_scene(&self) -> Scene3D {
        Scene3D::default()
    }

    /// Fresh `PerspectiveCamera` for the current orbit-camera state.
    pub fn camera(&self, aspect: f32) -> PerspectiveCamera {
        self.camera.to_perspective(aspect)
    }

    pub fn hovered(&self) -> Option<NodeIndex> {
        self.hovered
    }

    pub fn selected(&self) -> Option<NodeIndex> {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    type DemoGraph = Graph<(), ()>;

    fn triangle() -> DemoGraph {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        graph.push_edge(c, a, 1.0, ());
        graph
    }

    #[test]
    fn new_seeds_one_particle_per_node_and_ticks_without_panicking() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        assert_eq!(engine.particles.len(), 3);
        let result = engine.tick(1.0 / 60.0);
        assert!(result.alpha <= 1.0);
    }

    #[test]
    fn build_scene_and_on_event_are_wave_1_no_op_stubs() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let scene = engine.build_scene();
        assert!(scene.nodes.is_empty(), "Wave 1 build_scene stub must return an empty scene");

        let handled = engine.on_event(&PlatformEvent::PointerMoved { x: 0.0, y: 0.0 }, Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(!handled, "Wave 1 on_event stub must never claim to handle an event");
    }

    #[test]
    fn camera_reflects_the_default_orbit_state() {
        let engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let persp = engine.camera(16.0 / 9.0);
        assert!((persp.eye - engine.camera.eye()).length() < 1e-4);
    }
}
