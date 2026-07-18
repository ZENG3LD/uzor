//! `GraphEngine3D` — the 3D sibling of [`crate::engine::GraphEngine`]
//! (W3D arc plan §1.2: a separate engine, NOT a `Dimension` mode bolted
//! onto the 2D facade). Reuses `Graph`/`SimTopology`/`NodeIndex`/
//! `EdgeIndex` unchanged; owns its own [`Camera3D`] and a 3D-aware
//! [`Layout`] impl ([`crate::layout::force_directed_3d::ForceDirectedLayout3D`]
//! by default).
//!
//! Wave 1 (plan §4) proved the physics core only
//! ([`GraphEngine3D::new`]/[`GraphEngine3D::tick`]). Wave 2 lands the
//! render path ([`GraphEngine3D::build_scene`], wired into
//! [`crate::render3d::build_scene`]) and camera event dispatch
//! ([`GraphEngine3D::on_event`] — drag-orbit, wheel-dolly, shift-drag
//! pan, per plan §1.4). Picking (`hovered`/`selected` actually changing
//! on click) is Wave 3.

use std::sync::Arc;

use uzor::input::{ModifierKeys, MouseButton, PlatformEvent};
use uzor::types::Rect;
use uzor_urx_3d::{MeshLit, PerspectiveCamera, Scene3D};

use crate::camera3d::Camera3D;
use crate::graph::{Graph, NodeIndex};
use crate::layout::force_directed_3d::ForceDirectedLayout3D;
use crate::layout::{Layout, LayoutTickResult};
use crate::particle::Particle;

/// Wheel-to-dolly screen-delta sensitivity — mirrors
/// [`crate::engine::GraphEngine`]'s own `ZOOM_SENSITIVITY` (`engine.rs`)
/// in magnitude, but flipped in sign: 2D's `zoom` grows for "zoom in"
/// (multiply), 3D's orbit `distance` SHRINKS for "zoom in" — see
/// [`GraphEngine3D::on_scroll`].
const DOLLY_SENSITIVITY: f32 = 0.0015;
const DOLLY_FACTOR_MIN: f32 = 0.8;
const DOLLY_FACTOR_MAX: f32 = 1.25;

/// In-progress pointer gesture (plan §1.4) — resolved once at
/// `PointerDown` from the held modifiers (mirrors the 2D engine's own
/// `PointerMode`, `engine.rs`), never re-resolved mid-drag even if a
/// modifier is pressed/released while dragging.
#[derive(Debug, Clone, Copy)]
enum Pointer3DMode {
    Idle,
    Orbiting { last: (f64, f64) },
    /// Shift-drag pan (plan §1.4) — middle-drag is not wired this wave
    /// (the `PlatformEvent` stream this engine consumes only carries a
    /// `MouseButton::Left`-gated `PointerDown`/`Up`, matching the 2D
    /// engine's own `on_event` match arms; a middle-button chord is a
    /// natural, small follow-up if ever requested).
    Panning { last: (f64, f64) },
}

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
    /// Held keyboard-modifier state (Wave 2) — mirrors the 2D engine's
    /// own `PlatformEvent::ModifiersChanged` tracking pattern
    /// (`engine.rs`), read by [`GraphEngine3D::on_pointer_down`] to pick
    /// orbit vs. shift-drag pan (plan §1.4).
    modifiers: ModifierKeys,
    mode: Pointer3DMode,
    /// Last pointer position in screen px — [`GraphEngine3D::on_scroll`]'s
    /// "is the cursor currently over this viewport" gate (mirrors
    /// `GraphEngine::on_scroll`, which reads its own
    /// `last_pointer_screen` the same way).
    last_pointer_screen: (f64, f64),
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
            modifiers: ModifierKeys::default(),
            mode: Pointer3DMode::Idle,
            last_pointer_screen: (0.0, 0.0),
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

    /// Raw `PlatformEvent` handler — orbit-drag, wheel-dolly, shift-drag
    /// pan (plan §1.4). Wire this from the app's 3D dispatch (see
    /// `uzor-desktop::scene3d_app`'s divergence log for how
    /// `force_graph_demo` routes events to whichever dimension is
    /// active). Returns `true` if the event was consumed. Hover/click
    /// picking (`hovered`/`selected` actually changing) is Wave 3 — this
    /// wave never mutates either field.
    pub fn on_event(&mut self, event: &PlatformEvent, viewport: Rect) -> bool {
        match event {
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => self.on_pointer_down(*x, *y, viewport),
            PlatformEvent::PointerMoved { x, y } => self.on_pointer_moved(*x, *y),
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => self.on_pointer_up(*x, *y),
            PlatformEvent::Scroll { dy, .. } => self.on_scroll(*dy, viewport),
            PlatformEvent::ModifiersChanged { modifiers } => {
                self.modifiers = *modifiers;
                true
            }
            PlatformEvent::KeyDown { modifiers, .. } | PlatformEvent::KeyUp { modifiers, .. } => {
                self.modifiers = *modifiers;
                false
            }
            _ => false,
        }
    }

    /// Plain left-drag orbits; shift-held left-drag pans instead (plan
    /// §1.4) — the drag KIND is resolved once here, at drag-start, and
    /// held for the whole gesture (mirrors the 2D engine's own
    /// box-select-mode resolution, `box_select_mode_for` in `engine.rs`).
    /// `false` (event not consumed) if `(x, y)` lands outside `viewport`.
    fn on_pointer_down(&mut self, x: f64, y: f64, viewport: Rect) -> bool {
        if !viewport.contains(x, y) {
            return false;
        }
        self.mode = if self.modifiers.shift { Pointer3DMode::Panning { last: (x, y) } } else { Pointer3DMode::Orbiting { last: (x, y) } };
        true
    }

    fn on_pointer_moved(&mut self, x: f64, y: f64) -> bool {
        self.last_pointer_screen = (x, y);
        match self.mode {
            Pointer3DMode::Orbiting { last } => {
                self.camera.orbit((x - last.0) as f32, (y - last.1) as f32);
                self.mode = Pointer3DMode::Orbiting { last: (x, y) };
                true
            }
            Pointer3DMode::Panning { last } => {
                self.camera.pan((x - last.0) as f32, (y - last.1) as f32);
                self.mode = Pointer3DMode::Panning { last: (x, y) };
                true
            }
            Pointer3DMode::Idle => false,
        }
    }

    fn on_pointer_up(&mut self, _x: f64, _y: f64) -> bool {
        let was_dragging = !matches!(self.mode, Pointer3DMode::Idle);
        self.mode = Pointer3DMode::Idle;
        was_dragging
    }

    /// Wheel-to-dolly, gated on the cursor currently sitting over
    /// `viewport` (mirrors `GraphEngine::on_scroll`'s own gate). Positive
    /// `dy` means "zoom in" (matches the 2D engine's own convention) —
    /// in orbit-camera terms that means a SMALLER `distance`, the
    /// inverse of 2D's "bigger `zoom`", so the sign is flipped relative
    /// to the 2D formula this mirrors.
    fn on_scroll(&mut self, dy: f64, viewport: Rect) -> bool {
        if !viewport.contains(self.last_pointer_screen.0, self.last_pointer_screen.1) {
            return false;
        }
        let factor = (1.0 - dy as f32 * DOLLY_SENSITIVITY).clamp(DOLLY_FACTOR_MIN, DOLLY_FACTOR_MAX);
        self.camera.dolly(factor);
        true
    }

    /// Instanced sphere nodes + cylinder edges (plan §1.3) — wires
    /// straight into [`crate::render3d::build_scene`], which is
    /// independently unit-tested (no GPU needed) for the node/edge
    /// instance construction itself; see `uzor-graph/tests/render3d_gpu.rs`
    /// for the headless-GPU proof that the result actually renders
    /// visually-distinct pixels.
    pub fn build_scene(&self) -> Scene3D {
        crate::render3d::build_scene(&self.graph, &self.particles, &self.node_mesh, &self.edge_mesh, crate::render3d::DEFAULT_EDGE_WIDTH)
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
    fn build_scene_returns_one_instanced_sphere_per_node_and_one_cylinder_per_edge() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        // `new()` seeds every particle at the origin — non-coincident
        // positions are needed here so none of the 3 edges gets skipped
        // as degenerate (see `render3d.rs`'s own
        // `build_edge_instances_skips_a_coincident_degenerate_edge`).
        engine.particles[0] = Particle::at3(-4.0, 0.0, 0.0);
        engine.particles[1] = Particle::at3(4.0, 0.0, 0.0);
        engine.particles[2] = Particle::at3(0.0, 4.0, 0.0);

        let scene = engine.build_scene();

        // Triangle fixture: 3 nodes, 3 edges.
        assert_eq!(scene.nodes.len(), 6, "build_scene must emit one Node per graph node plus one per edge (Wave 2)");
        assert!(!scene.lights.is_empty(), "build_scene must light the scene so MeshLit tints are visible");
    }

    #[test]
    fn on_event_plain_drag_orbits_the_camera_and_is_consumed() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_yaw = engine.camera.yaw;

        assert!(engine.on_event(&PlatformEvent::PointerDown { x: 50.0, y: 50.0, button: uzor::input::MouseButton::Left }, viewport));
        assert!(engine.on_event(&PlatformEvent::PointerMoved { x: 150.0, y: 50.0 }, viewport));
        assert!(engine.camera.yaw != before_yaw, "a plain left-drag must orbit the camera (plan §1.4)");
        assert!(engine.on_event(&PlatformEvent::PointerUp { x: 150.0, y: 50.0, button: uzor::input::MouseButton::Left }, viewport));

        let handled_outside =
            engine.on_event(&PlatformEvent::PointerDown { x: -10.0, y: -10.0, button: uzor::input::MouseButton::Left }, viewport);
        assert!(!handled_outside, "a pointer-down outside the viewport must not start a drag");
    }

    #[test]
    fn on_event_shift_drag_pans_instead_of_orbiting() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_yaw = engine.camera.yaw;
        let before_target = engine.camera.target;

        engine.on_event(&PlatformEvent::ModifiersChanged { modifiers: uzor::input::ModifierKeys::shift() }, viewport);
        engine.on_event(&PlatformEvent::PointerDown { x: 50.0, y: 50.0, button: uzor::input::MouseButton::Left }, viewport);
        engine.on_event(&PlatformEvent::PointerMoved { x: 150.0, y: 50.0 }, viewport);

        assert_eq!(engine.camera.yaw, before_yaw, "shift-drag must pan, not orbit — yaw stays put");
        assert!(engine.camera.target != before_target, "shift-drag must move the pan target");
    }

    #[test]
    fn on_event_scroll_dollies_only_when_the_cursor_is_over_the_viewport() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_distance = engine.camera.distance;

        engine.on_event(&PlatformEvent::PointerMoved { x: 200.0, y: 150.0 }, viewport);
        assert!(engine.on_event(&PlatformEvent::Scroll { dx: 0.0, dy: 10.0 }, viewport));
        assert!(engine.camera.distance < before_distance, "positive scroll dy must dolly in (smaller distance)");

        let before_distance = engine.camera.distance;
        engine.on_event(&PlatformEvent::PointerMoved { x: -50.0, y: -50.0 }, viewport);
        let handled = engine.on_event(&PlatformEvent::Scroll { dx: 0.0, dy: 10.0 }, viewport);
        assert!(!handled, "scroll must not be consumed while the cursor sits outside the viewport");
        assert_eq!(engine.camera.distance, before_distance);
    }

    #[test]
    fn camera_reflects_the_default_orbit_state() {
        let engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let persp = engine.camera(16.0 / 9.0);
        assert!((persp.eye - engine.camera.eye()).length() < 1e-4);
    }
}
