//! `GraphEngine` — the facade tying graph + particles + camera + layout
//! + interaction + render + agent surface together into one owned
//! object an app registers as a blackbox and drives from `App::ui`/
//! `App::on_event`/`App::regions`.

use std::time::Instant;

use uzor::input::{MouseButton, PlatformEvent};
use uzor::render::{RenderContext, RenderRegion, UNCAPPED_FPS};
use uzor::types::Rect;

use crate::camera::{Aabb, Camera2D};
use crate::graph::{Graph, NodeIndex};
use crate::interaction::drag::DragController;
use crate::interaction::focus::FocusSet;
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
        self.visible = gr_render::cull_visible(&self.graph, &self.particles, &self.camera, self.canvas_rect);
    }

    pub fn visible_nodes(&self) -> &[NodeIndex] {
        &self.visible
    }

    /// Refresh the culled/visible set and paint nodes+edges. Call once
    /// per frame while the canvas is on screen.
    pub fn draw(&mut self, render: &mut dyn RenderContext) {
        self.refresh_visible();
        let ctx = gr_render::DrawContext {
            camera: &self.camera,
            viewport: self.canvas_rect,
            visible: &self.visible,
            focus: &self.focus,
            selected: self.selected,
            hovered: self.hovered,
        };
        gr_render::draw_edges(render, &self.graph, &self.particles, &ctx);
        gr_render::draw_nodes(render, &self.graph, &self.particles, &ctx);
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
        self.focus = FocusSet::neighborhood(&self.graph, node);
        self.dirty = true;
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.focus = FocusSet::empty();
        self.dirty = true;
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
