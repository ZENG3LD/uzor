//! `GraphEngine` — the facade tying graph + particles + camera + layout
//! + interaction + render + agent surface together into one owned
//! object an app registers as a blackbox and drives from `App::ui`/
//! `App::on_event`/`App::regions`.

use std::time::Instant;

use std::collections::HashSet;

use uzor::input::{MouseButton, PlatformEvent};
use uzor::render::{RenderContext, RenderRegion, UNCAPPED_FPS};
use uzor::types::Rect;
use uzor_figures::interact::FocusSet;

use crate::camera::{Aabb, Camera2D};
use crate::cluster::{ClusterRegistry, GroupId};
use crate::graph::{Graph, NodeIndex};
use crate::interaction::drag::DragController;
use crate::interaction::pick;
use crate::layout::force_directed::ForceDirectedLayout;
use crate::layout::{Layout, LayoutTickResult};
use crate::particle::Particle;
use crate::render as gr_render;

/// Screen-space distance a `PointerDown`->`PointerUp` pair may travel
/// while panning the camera and still count as a click-to-deselect.
const CLICK_DRAG_THRESHOLD_PX: f64 = 4.0;
const ZOOM_SENSITIVITY: f64 = 0.0015;
/// Alpha to reheat the sim to when a node starts being dragged/unpinned
/// — enough to visibly resettle the local neighborhood without a full
/// restart-from-scratch jolt.
const DRAG_REHEAT_ALPHA: f32 = 0.35;

#[derive(Debug, Clone, Copy)]
enum PointerMode {
    Idle,
    PanningCamera { last: (f64, f64), total: f64 },
    DraggingNode,
}

/// Generic facts about one node, for a caller's sidebar/inspector —
/// engine-level, so no domain-specific field beyond what `Graph` itself
/// exposes (id/label/category/degree/position/pin state).
#[derive(Debug, Clone, Copy)]
pub struct NodeFacts<'a> {
    pub index: NodeIndex,
    pub label: &'a str,
    pub category: &'a str,
    pub degree: u32,
    pub position: (f32, f32),
    pub pinned: bool,
}

/// The engine's owned state: graph topology, simulated positions,
/// camera, the active layout algorithm, and interaction/selection
/// state. Implements [`uzor::layout::agent::BlackboxAgentSurface`] (see
/// `agent.rs`) so it can be registered directly as a blackbox — the
/// same `Arc<Mutex<...>>` a human-driven `App::ui`/`on_event` and the
/// HTTP agent control plane both mutate.
pub struct GraphEngine<N, E, L: Layout = ForceDirectedLayout> {
    pub graph: Graph<N, E>,
    pub particles: Vec<Particle>,
    pub camera: Camera2D,
    pub layout: L,
    pub selected: Option<NodeIndex>,
    pub hovered: Option<NodeIndex>,
    pub focus: FocusSet,
    pub clusters: ClusterRegistry,

    pinned: Vec<bool>,
    drag: DragController,
    mode: PointerMode,
    canvas_rect: Rect,
    last_pointer_screen: (f64, f64),
    visible: Vec<NodeIndex>,
    last_tick: LayoutTickResult,
    last_frame_at: Option<Instant>,
    dirty: bool,
    pub(crate) agent_slot_id: String,
}

impl<N, E, L: Layout> GraphEngine<N, E, L> {
    pub fn new(graph: Graph<N, E>, layout: L) -> Self {
        let n = graph.node_count();
        Self {
            graph,
            particles: vec![Particle::default(); n],
            camera: Camera2D::default(),
            layout,
            selected: None,
            hovered: None,
            focus: FocusSet::empty(),
            clusters: ClusterRegistry::default(),
            pinned: vec![false; n],
            drag: DragController::default(),
            mode: PointerMode::Idle,
            canvas_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            last_pointer_screen: (0.0, 0.0),
            visible: Vec::new(),
            last_tick: LayoutTickResult { alpha: 1.0, max_displacement: 0.0, settled: false },
            last_frame_at: None,
            dirty: true,
            agent_slot_id: "graph".to_owned(),
        }
    }

    pub fn set_agent_slot_id(&mut self, id: impl Into<String>) {
        self.agent_slot_id = id.into();
    }

    /// Seed initial particle positions in `NodeIndex` order. Entries
    /// past `positions.len()` stay at the origin (which the sim will
    /// spread out on its own — just slower to settle).
    pub fn seed_positions(&mut self, positions: &[(f32, f32)]) {
        for (p, &(x, y)) in self.particles.iter_mut().zip(positions.iter()) {
            p.x = x;
            p.y = y;
        }
        self.dirty = true;
    }

    pub fn set_canvas_rect(&mut self, rect: Rect) {
        self.canvas_rect = rect;
    }

    pub fn canvas_rect(&self) -> Rect {
        self.canvas_rect
    }

    pub fn is_hot(&self) -> bool {
        !self.last_tick.settled
    }

    pub fn last_tick(&self) -> LayoutTickResult {
        self.last_tick
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// The `RenderRegion` this engine wants for its canvas: continuous
    /// while hot, dirty-driven (repaint only when [`GraphEngine::dirty`]
    /// is set) once settled — the freeze/wake split from the engine
    /// design doc §3.5.
    pub fn render_region(&self, id: &'static str) -> RenderRegion {
        if self.is_hot() {
            RenderRegion { id, rect: self.canvas_rect, target_fps: UNCAPPED_FPS, dirty: true }
        } else {
            RenderRegion { id, rect: self.canvas_rect, target_fps: 0, dirty: self.dirty }
        }
    }

    /// Advance the simulation by `dt` real seconds.
    pub fn tick(&mut self, dt: f32) -> LayoutTickResult {
        let topo = self.graph.topology();
        let was_hot = self.is_hot();
        self.last_tick = self.layout.tick(&topo, &mut self.particles, dt);
        if was_hot || self.is_hot() {
            self.dirty = true;
        }
        self.last_tick
    }

    /// [`GraphEngine::tick`] using wall-clock elapsed time since the
    /// last call (clamped so a stalled frame can't blow the sim up).
    pub fn tick_real_time(&mut self) -> LayoutTickResult {
        let now = Instant::now();
        let dt = match self.last_frame_at {
            Some(prev) => now.duration_since(prev).as_secs_f32().min(0.1),
            None => 1.0 / 60.0,
        };
        self.last_frame_at = Some(now);
        self.tick(dt)
    }

    pub fn reheat(&mut self, alpha: f32) {
        self.layout.reheat(alpha);
        self.dirty = true;
    }

    /// Mark the canvas dirty without touching the sim's cooling state —
    /// e.g. after an agent-driven camera move, which changes what's on
    /// screen but shouldn't wake the physics.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn refresh_visible(&mut self) {
        let culled = gr_render::cull_visible(&self.graph, &self.particles, &self.camera, self.canvas_rect);
        if self.clusters.any_collapsed() {
            let hidden: HashSet<NodeIndex> = self.clusters.hidden_nodes().collect();
            self.visible = culled.into_iter().filter(|id| !hidden.contains(id)).collect();
        } else {
            self.visible = culled;
        }
    }

    pub fn visible_nodes(&self) -> &[NodeIndex] {
        &self.visible
    }

    /// Refresh the culled/visible set and paint nodes+edges (plus, when
    /// any cluster is collapsed, the aggregated cross-cluster edges and
    /// the collapsed-supernode double-ring/count-label overlay — see
    /// `render::{draw_cluster_edges, draw_cluster_supernodes}`). Call
    /// once per frame while the canvas is on screen.
    pub fn draw(&mut self, render: &mut dyn RenderContext) {
        self.refresh_visible();
        let hidden: HashSet<NodeIndex> =
            if self.clusters.any_collapsed() { self.clusters.hidden_nodes().collect() } else { HashSet::new() };
        let ctx = gr_render::DrawContext {
            camera: &self.camera,
            viewport: self.canvas_rect,
            visible: &self.visible,
            focus: &self.focus,
            selected: self.selected,
            hovered: self.hovered,
            hidden: &hidden,
        };
        gr_render::draw_edges(render, &self.graph, &self.particles, &ctx);
        gr_render::draw_cluster_edges(render, &self.particles, &ctx, &self.clusters);
        gr_render::draw_nodes(render, &self.graph, &self.particles, &ctx);
        gr_render::draw_cluster_supernodes(render, &self.graph, &self.particles, &ctx, &self.clusters);
    }

    pub fn fit_view(&mut self) {
        let points: Vec<(f64, f64)> = self.particles.iter().map(|p| (p.x as f64, p.y as f64)).collect();
        if let Some(aabb) = Aabb::from_points(&points) {
            self.camera.fit_view(aabb, self.canvas_rect);
        }
        self.dirty = true;
    }

    pub fn select(&mut self, node: NodeIndex) {
        self.selected = Some(node);
        self.focus.select_many(self.graph.neighborhood_focus_keys(node));
        self.dirty = true;
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.focus.clear_selection();
        self.dirty = true;
    }

    /// Declare a cluster over `members` (first member becomes the
    /// collapse representative — see `cluster.rs` module docs). `None`
    /// if `members` is empty.
    pub fn define_cluster(&mut self, members: Vec<NodeIndex>) -> Option<GroupId> {
        self.clusters.define(&self.graph, members)
    }

    pub fn is_collapsed(&self, id: GroupId) -> bool {
        self.clusters.is_collapsed(id)
    }

    /// Collapse `id` into one super-node — centroid position, member-
    /// count radius, aggregated cross-cluster edge weights (see
    /// `cluster.rs`). No-op (`false`) if `id` is unknown or already
    /// collapsed.
    pub fn collapse_cluster(&mut self, id: GroupId) -> bool {
        let ok = self.clusters.collapse(id, &mut self.graph, &mut self.particles);
        if ok {
            self.dirty = true;
        }
        ok
    }

    /// Expand `id` back to its individual members, restoring EXACT
    /// pre-collapse positions (round-trip identity — see `cluster.rs`),
    /// then reheats so the layout visibly resettles around them (same
    /// convention as [`GraphEngine::unpin_node`]). No-op (`false`) if
    /// `id` is unknown or not currently collapsed.
    pub fn expand_cluster(&mut self, id: GroupId) -> bool {
        let ok = self.clusters.expand(id, &mut self.graph, &mut self.particles);
        if ok {
            self.reheat(DRAG_REHEAT_ALPHA);
        }
        ok
    }

    pub fn is_pinned(&self, node: NodeIndex) -> bool {
        self.pinned.get(node.index()).copied().unwrap_or(false)
    }

    /// Persistently pin `node` at its current position (survives drag
    /// release — Obsidian's explicit-pin affordance, distinct from
    /// "drag holds position while the button is down").
    pub fn pin_node(&mut self, node: NodeIndex) {
        if let Some(p) = self.particles.get_mut(node.index()) {
            p.pin(p.x, p.y);
        }
        if let Some(flag) = self.pinned.get_mut(node.index()) {
            *flag = true;
        }
        self.dirty = true;
    }

    /// Release a persistent pin and reheat so the node visibly rejoins
    /// the simulation instead of sitting frozen with nothing to prove
    /// it's alive again.
    pub fn unpin_node(&mut self, node: NodeIndex) {
        if let Some(p) = self.particles.get_mut(node.index()) {
            p.unpin();
        }
        if let Some(flag) = self.pinned.get_mut(node.index()) {
            *flag = false;
        }
        self.reheat(DRAG_REHEAT_ALPHA);
    }

    pub fn node_facts(&self, node: NodeIndex) -> Option<NodeFacts<'_>> {
        let n = self.graph.get_node(node)?;
        let p = self.particles.get(node.index())?;
        Some(NodeFacts {
            index: node,
            label: &n.label,
            category: &n.category,
            degree: self.graph.degree(node),
            position: (p.x, p.y),
            pinned: self.is_pinned(node),
        })
    }

    pub fn selected_facts(&self) -> Option<NodeFacts<'_>> {
        self.selected.and_then(|id| self.node_facts(id))
    }

    /// Raw `PlatformEvent` handler — canvas pan/zoom, node pick/drag,
    /// click-select, hover. Wire this from `App::on_event`. Returns
    /// `true` if the event was consumed.
    pub fn on_event(&mut self, event: &PlatformEvent) -> bool {
        match event {
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => self.on_pointer_down(*x, *y),
            PlatformEvent::PointerMoved { x, y } => self.on_pointer_moved(*x, *y),
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => self.on_pointer_up(*x, *y),
            PlatformEvent::Scroll { dy, .. } => self.on_scroll(*dy),
            _ => false,
        }
    }

    fn on_pointer_down(&mut self, x: f64, y: f64) -> bool {
        if !self.canvas_rect.contains(x, y) {
            return false;
        }
        if let Some(hit) = pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible) {
            self.mode = PointerMode::DraggingNode;
            self.drag.start(hit, (x, y));
            let world = self.camera.screen_to_world((x, y), self.canvas_rect);
            if let Some(p) = self.particles.get_mut(hit.index()) {
                p.fx = Some(world.0 as f32);
                p.fy = Some(world.1 as f32);
            }
            self.reheat(DRAG_REHEAT_ALPHA);
        } else {
            self.mode = PointerMode::PanningCamera { last: (x, y), total: 0.0 };
        }
        self.dirty = true;
        true
    }

    fn on_pointer_moved(&mut self, x: f64, y: f64) -> bool {
        self.last_pointer_screen = (x, y);
        let mut handled = false;

        match self.mode {
            PointerMode::DraggingNode => {
                self.drag.update((x, y));
                if let Some(node) = self.drag.dragging_node() {
                    let world = self.camera.screen_to_world((x, y), self.canvas_rect);
                    if let Some(p) = self.particles.get_mut(node.index()) {
                        p.fx = Some(world.0 as f32);
                        p.fy = Some(world.1 as f32);
                    }
                }
                handled = true;
            }
            PointerMode::PanningCamera { last, total } => {
                let dx = x - last.0;
                let dy = y - last.1;
                self.camera.pan_x += dx;
                self.camera.pan_y += dy;
                self.mode = PointerMode::PanningCamera { last: (x, y), total: total + (dx * dx + dy * dy).sqrt() };
                handled = true;
            }
            PointerMode::Idle => {}
        }

        if self.canvas_rect.contains(x, y) {
            let hit = pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible);
            if hit != self.hovered {
                self.hovered = hit;
                self.dirty = true;
            }
            handled = true;
        } else if self.hovered.is_some() {
            self.hovered = None;
            self.dirty = true;
        }

        if handled {
            self.dirty = true;
        }
        handled
    }

    fn on_pointer_up(&mut self, x: f64, y: f64) -> bool {
        match self.mode {
            PointerMode::DraggingNode => {
                if let Some((node, _response)) = self.drag.stop() {
                    if !self.is_pinned(node) {
                        if let Some(p) = self.particles.get_mut(node.index()) {
                            p.unpin();
                        }
                    }
                    self.select(node);
                }
                self.mode = PointerMode::Idle;
                self.dirty = true;
                true
            }
            PointerMode::PanningCamera { total, .. } => {
                self.mode = PointerMode::Idle;
                if total < CLICK_DRAG_THRESHOLD_PX && self.canvas_rect.contains(x, y) {
                    match pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible) {
                        // A collapsed super-node's designated expand
                        // affordance is a single click on it (this raw-
                        // `PlatformEvent` canvas path has no double-click
                        // in its input vocabulary — see `cluster.rs`
                        // module docs / the demo's `expand` agent action
                        // for the alternative route).
                        Some(hit) if self.clusters.cluster_of(hit).is_some_and(|id| self.clusters.is_collapsed(id)) => {
                            if let Some(id) = self.clusters.cluster_of(hit) {
                                self.expand_cluster(id);
                            }
                        }
                        Some(hit) => self.select(hit),
                        None => self.clear_selection(),
                    }
                }
                self.dirty = true;
                true
            }
            PointerMode::Idle => false,
        }
    }

    fn on_scroll(&mut self, dy: f64) -> bool {
        if !self.canvas_rect.contains(self.last_pointer_screen.0, self.last_pointer_screen.1) {
            return false;
        }
        let factor = (1.0 + dy * ZOOM_SENSITIVITY).clamp(0.8, 1.25);
        self.camera.zoom_at(self.last_pointer_screen, self.canvas_rect, factor);
        self.dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    type TestEngine = GraphEngine<(), (), ForceDirectedLayout>;

    /// `a - b - c` chain — `b` is `a`'s only 1-hop neighbor, `c` is 2
    /// hops away (outside the neighborhood `select` focuses on).
    fn chain_graph() -> (Graph<(), ()>, NodeIndex, NodeIndex, NodeIndex) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        (graph, a, b, c)
    }

    #[test]
    fn select_populates_the_repointed_figures_focus_set_with_neighborhood_keys() {
        let (graph, a, b, c) = chain_graph();
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.select(a);

        assert!(engine.focus.is_active());
        assert!(engine.focus.is_selected(u64::from(a)));
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(!engine.focus.is_selected(u64::from(c)), "c is 2 hops away — outside the 1-hop neighborhood");

        engine.clear_selection();
        assert!(!engine.focus.is_active());
    }

    #[test]
    fn reselecting_a_different_node_replaces_the_whole_focus_set() {
        let (graph, a, _b, c) = chain_graph();
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.select(a);
        assert!(engine.focus.is_selected(u64::from(a)));

        engine.select(c);
        assert!(!engine.focus.is_selected(u64::from(a)), "stale selection from the previous select() must not leak");
        assert!(engine.focus.is_selected(u64::from(c)));
    }

    // ── P0 pointer-plumbing gate (uzor-window-desktop mapper fix) ──────────
    //
    // These inject a full `PlatformEvent` sequence directly (as the engine
    // is wired from `App::on_event`), asserting the camera pans by exactly
    // the moved delta with no jump — the behavior downstream of the fixed
    // `EventMapper` (which used to stamp `PointerDown`/`Up` at hardcoded
    // `(0.0, 0.0)`, see `uzor-window-desktop/src/event_mapper.rs`). The
    // engine's own pan/pick code was already correct; these prove it stays
    // correct end-to-end once fed real coordinates, and document what the
    // old stale-zero bug looked like from the engine's point of view.

    fn empty_engine_with_canvas(rect: Rect) -> TestEngine {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        engine.set_canvas_rect(rect);
        engine
    }

    /// Down(bg point) -> Moved xN -> Up: each Move pans the camera by
    /// exactly that step's screen-space delta (zoom stays 1.0, so
    /// dividing by it is a no-op) — no jump from a stale (0,0) origin.
    #[test]
    fn background_drag_sequence_pans_camera_by_exact_per_step_delta() {
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        let mut engine = empty_engine_with_canvas(canvas);

        let grab = (300.0, 300.0);
        assert!(engine.on_event(&PlatformEvent::PointerDown {
            x: grab.0,
            y: grab.1,
            button: MouseButton::Left,
        }));
        assert_eq!((engine.camera.pan_x, engine.camera.pan_y), (0.0, 0.0),
            "PointerDown alone must not move the camera");

        let moves = [(310.0, 300.0), (325.0, 305.0), (325.0, 320.0)];
        let mut last = grab;
        for &(mx, my) in &moves {
            let before = (engine.camera.pan_x, engine.camera.pan_y);
            engine.on_event(&PlatformEvent::PointerMoved { x: mx, y: my });
            let (expected_dx, expected_dy) = (mx - last.0, my - last.1);
            assert!((engine.camera.pan_x - (before.0 + expected_dx)).abs() < 1e-9);
            assert!((engine.camera.pan_y - (before.1 + expected_dy)).abs() < 1e-9);
            last = (mx, my);
        }

        engine.on_event(&PlatformEvent::PointerUp { x: last.0, y: last.1, button: MouseButton::Left });
        // Total travel from (300,300) to (325,320): pan_x += 25, pan_y += 20.
        assert!((engine.camera.pan_x - 25.0).abs() < 1e-9);
        assert!((engine.camera.pan_y - 20.0).abs() < 1e-9);
    }

    /// Regression-shaped: a Down at the true grab point followed by a
    /// +10px Move pans by exactly +10px (screen-space, ÷ zoom == 1.0 here
    /// so it's a no-op) — contrasted against the OLD bug shape, where
    /// every `PointerDown` was stamped at `(0.0, 0.0)` regardless of the
    /// real cursor position, so the same physical move read as a
    /// multi-hundred-pixel teleport instead of a +10px pan.
    #[test]
    fn regression_stale_zero_down_would_teleport_vs_fixed_pipeline_pans_by_delta() {
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        let grab = (300.0, 300.0);

        // Fixed pipeline: Down carries the real grab point (what the
        // corrected stateful mapper now stamps from the last CursorMoved).
        let mut fixed = empty_engine_with_canvas(canvas);
        fixed.on_event(&PlatformEvent::PointerDown { x: grab.0, y: grab.1, button: MouseButton::Left });
        fixed.on_event(&PlatformEvent::PointerMoved { x: grab.0 + 10.0, y: grab.1 });
        assert!((fixed.camera.pan_x - 10.0).abs() < 1e-9,
            "a +10px move after a correctly-stamped Down pans by exactly +10px");
        assert!((fixed.camera.pan_y - 0.0).abs() < 1e-9);

        // Old bug shape: EventMapper::map_window_event stamped every
        // PointerDown at (0.0, 0.0) ("position will be updated by cursor
        // moved event" — nothing did). Reproduce that input shape directly
        // against the engine to document the failure it caused downstream.
        let mut buggy = empty_engine_with_canvas(canvas);
        buggy.on_event(&PlatformEvent::PointerDown { x: 0.0, y: 0.0, button: MouseButton::Left });
        buggy.on_event(&PlatformEvent::PointerMoved { x: grab.0 + 10.0, y: grab.1 });

        assert!(buggy.camera.pan_x > 100.0,
            "stale (0,0) Down turns the same +10px physical move into a \
             camera teleport of ~grab_x pixels — this is the P0 bug: {} \
             (fixed pipeline pans by exactly 10.0)", buggy.camera.pan_x);
    }
}
