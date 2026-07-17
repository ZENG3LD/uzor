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
use crate::label_grid;
use crate::layout::force_directed::ForceDirectedLayout;
use crate::layout::{ForceParams, GraphLayoutMode, Layout, LayoutTickResult};
use crate::particle::Particle;
use crate::render as gr_render;

/// Screen-space distance a `PointerDown`->`PointerUp` pair may travel
/// while panning the camera and still count as a click-to-deselect.
const CLICK_DRAG_THRESHOLD_PX: f64 = 4.0;
const ZOOM_SENSITIVITY: f64 = 0.0015;
/// Alpha to reheat the sim to when a node is unpinned, a cluster
/// expands, or force params change — enough to visibly resettle the
/// local neighborhood without a full restart-from-scratch jolt. NOT used
/// for drag-start any more — see [`DRAG_ALPHA_TARGET`] for the sustained
/// (not one-shot) reheat an active drag holds.
const DRAG_REHEAT_ALPHA: f32 = 0.35;

/// Sustained "`alphaTarget`"-equivalent held for the whole duration of an
/// active node drag (Wave 2.1 drag-physics contract — d3-force canon
/// range 0.1-0.3, obsidian doc §drag/d3-canon; picked the top of that
/// range so the local neighborhood keeps visibly simmering for the
/// entire gesture, not just at drag-start). Set via
/// [`Layout::set_alpha_target`] on drag-start, cleared back to `0.0` on
/// drag-end so alpha eases back down instead of free-decaying from
/// wherever it happened to be — the one-shot [`DRAG_REHEAT_ALPHA`] bump
/// this replaces for the drag path didn't hold a target, it just bumped
/// once and let ordinary decay take back over immediately.
const DRAG_ALPHA_TARGET: f32 = 0.3;

/// Screen-space distance the pointer must travel since the last hover
/// pick before `nearest_node` is re-run on `PointerMoved` (Wave 2.2 perf
/// guard — cosmos.gl/sigma's "skip the readback if the mouse hasn't
/// moved" idiom, adapted for CPU distance-scan picking: at 534+ nodes a
/// re-pick on every single-pixel jitter is wasted work the render loop
/// doesn't need). Below this threshold the previous hover result is kept
/// as-is.
const HOVER_PICK_MIN_MOVE_PX: f64 = 2.0;

/// Default hover-neighbor-highlight depth (Wave 2.2 — oss doc §2.1: "no
/// surveyed engine ships depth-2 as a built-in option... depth is a free
/// parameter"). `0` highlights only the hovered node itself; `1` (this
/// default) adds its direct neighbors + connecting edges.
const DEFAULT_HOVER_DEPTH: u8 = 1;

#[derive(Debug, Clone, Copy)]
enum PointerMode {
    Idle,
    PanningCamera { last: (f64, f64), total: f64 },
    DraggingNode,
}

/// What happens to a node's pin state when a drag ends. Wave 2.1 owner
/// order (2026-07-18): the DEFAULT is [`DragEndPolicy::Sticky`] ("хочу
/// вывести и оставить" — drag a node out, release, it stays exactly
/// there); [`DragEndPolicy::RestorePrior`] is the classic d3-force
/// convention (obsidian doc §drag/d3-canon) kept available as the
/// explicit non-default mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragEndPolicy {
    /// Drag-end always pins the node at the release position — an
    /// explicit unpin ([`GraphEngine::unpin_node`] / the `unpin_node`
    /// agent action) is required to rejoin the simulation.
    #[default]
    Sticky,
    /// Drag-end unfixes the node UNLESS it was already explicitly pinned
    /// (via [`GraphEngine::pin_node`]) before the drag started — pin
    /// composes with drag (a pre-pinned node stays pinned, now at the
    /// drag's release position) instead of every drag producing a pin.
    RestorePrior,
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
    /// Snapshot of [`GraphEngine::is_pinned`] for the node currently
    /// being dragged, taken at drag-start — what
    /// [`DragEndPolicy::RestorePrior`] restores on release.
    drag_prior_pinned: bool,
    drag_end_policy: DragEndPolicy,
    mode: PointerMode,
    canvas_rect: Rect,
    last_pointer_screen: (f64, f64),
    /// Screen position at the last `nearest_node` hover pick — `None`
    /// once the pointer has left the canvas (so re-entering always picks
    /// again immediately, regardless of where it left off). See
    /// [`HOVER_PICK_MIN_MOVE_PX`].
    last_hover_pick_screen: Option<(f64, f64)>,
    hover_depth: u8,
    hover_card: bool,
    /// Label-LOD density param (Wave 2.3 — `crate::label_grid`'s
    /// `labelDensity`, "labels per 100px cell at zoom 1.0"). See
    /// [`GraphEngine::label_density`]/[`GraphEngine::set_label_density`].
    label_density: f64,
    /// Labels actually drawn on the last [`GraphEngine::draw`] call —
    /// see [`GraphEngine::labels_drawn_last_frame`].
    labels_drawn_last_frame: usize,
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
            drag_prior_pinned: false,
            drag_end_policy: DragEndPolicy::default(),
            mode: PointerMode::Idle,
            canvas_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            last_pointer_screen: (0.0, 0.0),
            last_hover_pick_screen: None,
            hover_depth: DEFAULT_HOVER_DEPTH,
            hover_card: true,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
            labels_drawn_last_frame: 0,
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
        // Collapsed-cluster representatives always keep their label
        // (Wave 2.3 forced-label union) — hover/selection-neighbor
        // forcing needs no entry here, `draw_nodes` derives that
        // straight from `focus`.
        let forced_labels: HashSet<NodeIndex> = self.clusters.collapsed_clusters().map(|c| c.representative).collect();
        let ctx = gr_render::DrawContext {
            camera: &self.camera,
            viewport: self.canvas_rect,
            visible: &self.visible,
            focus: &self.focus,
            selected: self.selected,
            hovered: self.hovered,
            hidden: &hidden,
            label_density: self.label_density,
            forced_labels: &forced_labels,
        };
        gr_render::draw_edges(render, &self.graph, &self.particles, &ctx);
        gr_render::draw_cluster_edges(render, &self.particles, &ctx, &self.clusters);
        let node_stats = gr_render::draw_nodes(render, &self.graph, &self.particles, &ctx);
        self.labels_drawn_last_frame = node_stats.labels_drawn;
        gr_render::draw_cluster_supernodes(render, &self.graph, &self.particles, &ctx, &self.clusters);

        if self.hover_card {
            if let Some(id) = self.hovered {
                if let Some(facts) = self.node_facts(id) {
                    let anchor = self.camera.world_to_screen((facts.position.0 as f64, facts.position.1 as f64), self.canvas_rect);
                    let info = gr_render::HoverCardInfo {
                        label: facts.label,
                        category: facts.category,
                        degree: facts.degree,
                        pinned: facts.pinned,
                    };
                    gr_render::draw_hover_card(render, anchor, &info, self.canvas_rect);
                }
            }
        }
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

    /// Clears the persistent click-selection. A hover already in
    /// progress (the pointer never left the hovered node) resumes
    /// driving the highlight/dim focus set immediately — click-selection
    /// and hover share one `focus`, click just takes precedence while
    /// it's active (Orb's `isStateOverride` precedent, oss doc §2.1).
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.refresh_focus_from_hover();
        self.dirty = true;
    }

    /// Hover-neighbor-highlight depth (Wave 2.2 — oss doc §2.1). `0`
    /// highlights only the hovered node; `1` (default) adds its direct
    /// neighbors + connecting edges; higher values walk further hops
    /// (see [`Graph::neighborhood_focus_keys_depth`]).
    pub fn hover_depth(&self) -> u8 {
        self.hover_depth
    }

    /// Change the hover-neighbor-highlight depth, immediately
    /// recomputing the focus set from the CURRENT hover (if any and if
    /// no click-selection is overriding it) so the new depth is visible
    /// without waiting for the next pointer move.
    pub fn set_hover_depth(&mut self, depth: u8) {
        self.hover_depth = depth;
        if self.selected.is_none() {
            self.refresh_focus_from_hover();
            self.dirty = true;
        }
    }

    /// Whether [`GraphEngine::draw`] paints the floating hover info card
    /// (label/category/degree/pinned) near the hovered node. Default
    /// `true`.
    pub fn hover_card_enabled(&self) -> bool {
        self.hover_card
    }

    pub fn set_hover_card_enabled(&mut self, enabled: bool) {
        self.hover_card = enabled;
        self.dirty = true;
    }

    /// Label-LOD density param (Wave 2.3 — `crate::label_grid`'s sigma
    /// `LabelGrid` port). "Labels per 100px grid cell at zoom 1.0" —
    /// default [`label_grid::DEFAULT_LABEL_DENSITY`].
    pub fn label_density(&self) -> f64 {
        self.label_density
    }

    /// Negative values clamp to `0.0` (an empty per-cell quota — only
    /// forced labels, e.g. hover/selection/collapsed-cluster
    /// representatives, would show).
    pub fn set_label_density(&mut self, density: f64) {
        self.label_density = density.max(0.0);
        self.dirty = true;
    }

    /// Labels actually drawn on the last [`GraphEngine::draw`] call — a
    /// test/verification aid (Wave 2.3 gate) surfaced in `agent_state`'s
    /// `labels.drawn_last_frame` field.
    pub fn labels_drawn_last_frame(&self) -> usize {
        self.labels_drawn_last_frame
    }

    /// The currently hovered node, if any — same value `agent_state`'s
    /// `hover` field and [`GraphEngine::draw`]'s info card read.
    pub fn hovered(&self) -> Option<NodeIndex> {
        self.hovered
    }

    /// Set (or clear, `None`) the hovered node directly — the same path
    /// `on_pointer_moved`'s picking drives, exposed so the `hover_node`
    /// agent action (and any other headless driver) can reach the exact
    /// same behavior, including the reducer-style focus-set replace and
    /// the sigma-style "already this node — no-op" dedup guard.
    pub(crate) fn set_hovered(&mut self, hit: Option<NodeIndex>) {
        if hit == self.hovered {
            return;
        }
        self.hovered = hit;
        if self.selected.is_none() {
            self.refresh_focus_from_hover();
        }
        self.dirty = true;
    }

    /// Reducer-style paint override (oss doc §2.1: sigma `nodeReducer`/
    /// `edgeReducer`, replace-not-merge): recompute the WHOLE `focus`
    /// selection from `self.hovered` at `self.hover_depth` — pure
    /// function of current state, never an incremental patch. Only
    /// called where the caller has already confirmed no click-selection
    /// is overriding hover (`select`/click-selection always wins — see
    /// [`GraphEngine::clear_selection`]'s doc comment).
    fn refresh_focus_from_hover(&mut self) {
        match self.hovered {
            Some(node) => self.focus.select_many(self.graph.neighborhood_focus_keys_depth(node, self.hover_depth)),
            None => {
                self.focus.clear_selection();
            }
        }
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

    /// Current drag-release behavior — see [`DragEndPolicy`]. Defaults
    /// to [`DragEndPolicy::Sticky`].
    pub fn drag_end_policy(&self) -> DragEndPolicy {
        self.drag_end_policy
    }

    pub fn set_drag_end_policy(&mut self, policy: DragEndPolicy) {
        self.drag_end_policy = policy;
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

    /// Current force-model parameters, if the engine's active layout has
    /// one — a bare [`ForceDirectedLayout`] (`L = ForceDirectedLayout`),
    /// or a [`GraphLayoutMode`] dispatcher (`L = GraphLayoutMode`,
    /// regardless of which concrete mode is currently active — the force
    /// instance persists even while hierarchical/radial is selected,
    /// same as `GraphLayoutMode::hierarchical_params`/`radial_params`).
    /// `None` for any other `L` (hierarchical/radial layouts have their
    /// own differently-shaped `*Params`, not this one).
    pub fn force_params(&self) -> Option<ForceParams>
    where
        L: 'static,
    {
        let layout_any: &dyn std::any::Any = &self.layout;
        if let Some(force) = layout_any.downcast_ref::<ForceDirectedLayout>() {
            return Some(*force.params());
        }
        if let Some(mode) = layout_any.downcast_ref::<GraphLayoutMode>() {
            return Some(*mode.force_params());
        }
        None
    }

    /// Replace the active force-model parameters wholesale (Wave 2.1
    /// owner order — "хочу иметь возможность изменять силу притяжения")
    /// and reheat so the change is visible instead of sitting inert
    /// until the next unrelated wake. Returns `false` (no-op) if `L`
    /// isn't one of the two force-capable shapes
    /// [`GraphEngine::force_params`] documents.
    pub fn set_force_params(&mut self, params: ForceParams) -> bool
    where
        L: 'static,
    {
        let applied = {
            let layout_any: &mut dyn std::any::Any = &mut self.layout;
            if let Some(force) = layout_any.downcast_mut::<ForceDirectedLayout>() {
                force.set_params(params);
                true
            } else if let Some(mode) = layout_any.downcast_mut::<GraphLayoutMode>() {
                mode.set_force_params(params);
                true
            } else {
                false
            }
        };
        if applied {
            self.reheat(DRAG_REHEAT_ALPHA);
        }
        applied
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
            self.drag_prior_pinned = self.is_pinned(hit);
            self.drag.start(hit, (x, y));
            // Pin-during-drag: fix the node at the pointer's world
            // position for the whole gesture (d3-force canon — `fx`/`fy`
            // are the ONLY pin primitive, drag-in-progress and an
            // explicit persistent pin share the same mechanism).
            let world = self.camera.screen_to_world((x, y), self.canvas_rect);
            if let Some(p) = self.particles.get_mut(hit.index()) {
                p.fx = Some(world.0 as f32);
                p.fy = Some(world.1 as f32);
            }
            // Sustained reheat (obsidian doc §drag/d3-canon): hold alpha
            // at the drag target for the whole gesture instead of a
            // one-shot bump that starts cooling right away. `reheat`
            // jumps alpha straight to the target NOW; `set_alpha_target`
            // holds it there every subsequent tick until drag-end clears
            // it back to 0.0.
            self.layout.set_alpha_target(DRAG_ALPHA_TARGET);
            self.reheat(DRAG_ALPHA_TARGET);
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
                    // Screen -> world through the camera's own inverse
                    // transform (divides by `zoom` internally) — this IS
                    // the "delta ÷ zoom" canon, expressed as an absolute
                    // re-projection each tick instead of an accumulated
                    // delta (no drift, same result at zoom == 1).
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
            // Perf guard (Wave 2.2 §4): re-run the O(visible) nearest-node
            // scan only once the pointer has actually moved
            // `HOVER_PICK_MIN_MOVE_PX` since the last pick — a hot loop of
            // sub-pixel `PointerMoved` jitter at 534+ nodes must not
            // re-scan every single event.
            let moved_enough = match self.last_hover_pick_screen {
                Some((lx, ly)) => {
                    let dx = x - lx;
                    let dy = y - ly;
                    (dx * dx + dy * dy).sqrt() >= HOVER_PICK_MIN_MOVE_PX
                }
                None => true,
            };
            if moved_enough {
                self.last_hover_pick_screen = Some((x, y));
                let hit = pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible);
                self.set_hovered(hit);
            }
            handled = true;
        } else if self.hovered.is_some() {
            self.set_hovered(None);
            self.last_hover_pick_screen = None;
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
                    let stay_pinned = match self.drag_end_policy {
                        DragEndPolicy::Sticky => true,
                        DragEndPolicy::RestorePrior => self.drag_prior_pinned,
                    };
                    if stay_pinned {
                        // `fx`/`fy` already hold the release-time world
                        // position from the last drag-move tick — just
                        // flip the persistent-pin bookkeeping flag so
                        // `is_pinned`/`unpin_node` see it correctly.
                        if let Some(flag) = self.pinned.get_mut(node.index()) {
                            *flag = true;
                        }
                    } else {
                        if let Some(p) = self.particles.get_mut(node.index()) {
                            p.unpin();
                        }
                        if let Some(flag) = self.pinned.get_mut(node.index()) {
                            *flag = false;
                        }
                    }
                    // Drag-end: alphaTarget -> 0 so alpha eases back down
                    // instead of free-decaying from wherever it was held.
                    self.layout.set_alpha_target(0.0);
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

    // ── W2.1 drag-physics contract (sticky drag + alphaTarget) ─────────────
    //
    // A 2-node chain, camera left at its identity default (pan (0,0),
    // zoom 1.0) so screen == world and picking a node is just "click at
    // its seeded position". `refresh_visible()` (private, but reachable
    // here since `tests` is a descendant of this module) stands in for
    // the `draw()` call a real frame would make to populate the pick
    // candidate list.

    fn two_node_chain_engine() -> (TestEngine, NodeIndex, NodeIndex) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(100.0, 100.0), (300.0, 100.0)]);
        engine.refresh_visible();
        (engine, a, b)
    }

    #[test]
    fn sticky_drag_is_the_default_and_pins_the_node_at_the_release_position() {
        let (mut engine, a, _b) = two_node_chain_engine();
        assert_eq!(engine.drag_end_policy(), DragEndPolicy::Sticky, "Sticky must be the default policy");
        assert!(!engine.is_pinned(a));

        engine.on_event(&PlatformEvent::PointerDown { x: 100.0, y: 100.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 250.0, y: 220.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 250.0, y: 220.0, button: MouseButton::Left });
        // `x`/`y` only resync from the held `fx`/`fy` on the next
        // `tick()` (same as a real render frame would do) — one tick to
        // observe the release position land.
        engine.tick(1.0 / 60.0);

        assert!(engine.is_pinned(a), "a node dragged and released must be reported pinned under Sticky");
        let released = engine.particles[a.index()];
        assert!((released.x - 250.0).abs() < 1e-6);
        assert!((released.y - 220.0).abs() < 1e-6);

        // The pin must hold across many subsequent ticks, even with a
        // linked neighbor still under active force influence.
        for _ in 0..120 {
            engine.tick(1.0 / 60.0);
        }
        assert_eq!(engine.particles[a.index()].x, released.x);
        assert_eq!(engine.particles[a.index()].y, released.y);
    }

    #[test]
    fn restore_prior_policy_unfixes_a_previously_free_node_but_keeps_a_pre_pinned_one_pinned() {
        let (mut engine, a, b) = two_node_chain_engine();
        engine.set_drag_end_policy(DragEndPolicy::RestorePrior);

        // `a` was free before the drag -> released back into the sim.
        engine.on_event(&PlatformEvent::PointerDown { x: 100.0, y: 100.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 250.0, y: 220.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 250.0, y: 220.0, button: MouseButton::Left });
        assert!(!engine.is_pinned(a), "a node that was free before the drag must NOT stay pinned under RestorePrior");

        // `b` was explicitly pinned BEFORE the drag -> stays pinned
        // afterward, at the drag's release position (pin composes with
        // drag instead of the drag clobbering it).
        engine.pin_node(b);
        engine.on_event(&PlatformEvent::PointerDown { x: 300.0, y: 100.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 400.0, y: 150.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 400.0, y: 150.0, button: MouseButton::Left });
        engine.tick(1.0 / 60.0);
        assert!(engine.is_pinned(b), "a node pinned before the drag must stay pinned under RestorePrior");
        assert!((engine.particles[b.index()].x - 400.0).abs() < 1e-6);
        assert!((engine.particles[b.index()].y - 150.0).abs() < 1e-6);
    }

    #[test]
    fn drag_holds_alpha_near_the_sustained_target_and_decays_after_release() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        // Centered on the world origin: `center_strength` pulls the
        // settling pair toward world `(0, 0)`, so a canvas centered
        // there (rather than cornered at it) guarantees the click point
        // computed below — from wherever the pair actually settles —
        // stays inside the canvas regardless of drift direction.
        let canvas = Rect::new(-400.0, -300.0, 800.0, 600.0);
        engine.set_canvas_rect(canvas);
        engine.seed_positions(&[(100.0, 100.0), (300.0, 100.0)]);

        // Fully settle first — the "sustained, not one-shot" contract
        // only bites from a cold/settled start (a one-shot bump also
        // looks fine on the very first frame after seeding). Settling
        // moves `a` away from its seeded position, so the drag below
        // clicks its ACTUAL (post-settle) screen position, computed
        // through the real camera transform, not the stale seed.
        for _ in 0..600 {
            engine.tick(1.0 / 60.0);
        }
        assert!(!engine.is_hot(), "fixture must settle before the drag starts");
        engine.refresh_visible();
        let click = {
            let p = engine.particles[a.index()];
            engine.camera.world_to_screen((p.x as f64, p.y as f64), canvas)
        };

        engine.on_event(&PlatformEvent::PointerDown { x: click.0, y: click.1, button: MouseButton::Left });
        let mut min_alpha = f32::MAX;
        let mut max_alpha = f32::MIN;
        for i in 0..90 {
            if i % 10 == 0 {
                engine.on_event(&PlatformEvent::PointerMoved { x: click.0 + i as f64, y: click.1 });
            }
            let r = engine.tick(1.0 / 60.0);
            min_alpha = min_alpha.min(r.alpha);
            max_alpha = max_alpha.max(r.alpha);
        }
        assert!(
            (min_alpha - DRAG_ALPHA_TARGET).abs() < 0.01 && (max_alpha - DRAG_ALPHA_TARGET).abs() < 0.01,
            "alpha must hold near the sustained drag target for the whole gesture: min {min_alpha} max {max_alpha} (target {DRAG_ALPHA_TARGET})"
        );

        engine.on_event(&PlatformEvent::PointerUp { x: click.0 + 80.0, y: click.1, button: MouseButton::Left });
        let post_release = engine.tick(1.0 / 60.0);
        assert!(
            post_release.alpha < DRAG_ALPHA_TARGET - 1e-4,
            "alpha must start decaying immediately after drag-end clears alpha_target: {}",
            post_release.alpha
        );
        for _ in 0..600 {
            engine.tick(1.0 / 60.0);
        }
        assert!(!engine.is_hot(), "alpha must decay all the way back down once alpha_target is cleared");
    }

    // ── W2.2 hover system (reducer-style neighbor highlight, oss doc §2.1) ─

    /// `a - b - c - d` chain laid out on a line, 100 world units apart —
    /// with the default camera (pan (0,0), zoom 1.0) screen == world, so
    /// picking a node is just "hover at its seeded x".
    fn chain4_engine_on_a_line() -> (TestEngine, [NodeIndex; 4]) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        let d = graph.push_node((), "d", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        graph.push_edge(c, d, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (300.0, 0.0)]);
        engine.refresh_visible();
        (engine, [a, b, c, d])
    }

    #[test]
    fn pointer_moved_over_a_node_hovers_it_and_moving_to_empty_space_clears_it() {
        let (mut engine, [a, b, _c, _d]) = chain4_engine_on_a_line();
        assert!(engine.hovered().is_none());
        assert!(!engine.focus.is_active());

        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // exactly on b
        assert_eq!(engine.hovered(), Some(b));
        assert!(engine.focus.is_active());
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(engine.focus.is_selected(u64::from(a)), "b's depth-1 neighbor a must be highlighted too");

        engine.on_event(&PlatformEvent::PointerMoved { x: 700.0, y: 500.0 }); // empty space, far from every node
        assert!(engine.hovered().is_none());
        assert!(!engine.focus.is_active(), "moving off every node must clear the highlight entirely");
    }

    #[test]
    fn hover_neighborhood_highlights_exactly_the_hovered_node_and_its_depth_one_adjacency() {
        let (mut engine, [a, b, c, d]) = chain4_engine_on_a_line();
        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // b
        assert_eq!(engine.hovered(), Some(b));

        assert!(engine.focus.is_selected(u64::from(a)));
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(engine.focus.is_selected(u64::from(c)));
        assert!(!engine.focus.is_selected(u64::from(d)), "d is 2 hops from b — outside a depth-1 hover neighborhood");
    }

    #[test]
    fn hover_depth_zero_highlights_only_the_hovered_node_itself() {
        let (mut engine, [a, b, c, _d]) = chain4_engine_on_a_line();
        engine.set_hover_depth(0);
        assert_eq!(engine.hover_depth(), 0);

        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // b
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(!engine.focus.is_selected(u64::from(a)));
        assert!(!engine.focus.is_selected(u64::from(c)));
    }

    #[test]
    fn click_selection_takes_precedence_over_a_concurrent_hover_and_resumes_on_clear() {
        let (mut engine, [_a, b, _c, d]) = chain4_engine_on_a_line();
        engine.select(d); // click-select d — its 1-hop neighborhood is {c, d}
        assert!(engine.focus.is_selected(u64::from(d)));

        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // hover b, unrelated to the selection
        assert_eq!(engine.hovered(), Some(b));
        assert!(engine.focus.is_selected(u64::from(d)), "click-selection must win over a concurrent hover");
        assert!(!engine.focus.is_selected(u64::from(b)), "hover must not override an active click-selection");

        engine.clear_selection();
        // The pointer never left `b` — hover resumes driving the focus
        // set the instant the click-selection is no longer overriding it.
        assert!(engine.focus.is_selected(u64::from(b)), "clearing the selection must resume hover-driven focus for the node still under the cursor");
        assert!(!engine.focus.is_selected(u64::from(d)));
    }

    #[test]
    fn hover_pick_skips_recompute_for_sub_threshold_pointer_moves() {
        let (mut engine, [_a, b, _c, _d]) = chain4_engine_on_a_line();
        // b sits at world/screen (100, 0). `node_screen_radius(4.0)` at
        // zoom 1.0 is 4.0px, + `pick::HOVER_TOLERANCE_PX` (6.0) = a 10px
        // hit radius.
        engine.on_event(&PlatformEvent::PointerMoved { x: 109.9, y: 0.0 }); // 9.9px from b — inside
        assert_eq!(engine.hovered(), Some(b));

        // A <2px move that would, if re-picked, land JUST outside the hit
        // radius (10.9px from b) — the perf guard must keep the stale
        // hover instead of immediately re-scanning and clearing it.
        engine.on_event(&PlatformEvent::PointerMoved { x: 110.9, y: 0.0 });
        assert_eq!(engine.hovered(), Some(b), "a <2px move must not trigger a re-pick — stale hover kept");

        // A >=2px move (measured from the LAST PICK position, 109.9) does
        // trigger a fresh pick, which correctly clears the now-out-of-range hover.
        engine.on_event(&PlatformEvent::PointerMoved { x: 113.0, y: 0.0 });
        assert_eq!(engine.hovered(), None, "a >=2px move re-picks and correctly clears the hover");
    }

    // ── W2.3 label LOD (sigma LabelGrid port, oss doc §3/§label-lod) ───────

    #[test]
    fn label_density_defaults_and_set_label_density_updates_the_getter_and_marks_dirty() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(engine.label_density(), crate::label_grid::DEFAULT_LABEL_DENSITY);
        assert_eq!(engine.labels_drawn_last_frame(), 0);

        engine.clear_dirty();
        engine.set_label_density(2.5);
        assert_eq!(engine.label_density(), 2.5);
        assert!(engine.dirty(), "changing label_density must mark the canvas dirty");

        // Negative density clamps to 0.0 (an empty per-cell quota).
        engine.set_label_density(-4.0);
        assert_eq!(engine.label_density(), 0.0);
    }

    /// A real `draw()` call (through `uzor-export`'s headless render path,
    /// same as `lib.rs`'s `proof_tests`) must populate
    /// `labels_drawn_last_frame` from what `render::draw_nodes` actually
    /// drew — not stay stuck at its `0` initial value.
    #[test]
    fn draw_populates_labels_drawn_last_frame_from_the_render_pass() {
        use uzor_export::{render_to_png, ExportSpec};

        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 400.0, 300.0));
        engine.seed_positions(&[(0.0, 0.0), (50.0, 0.0)]);
        engine.camera.zoom = 2.0; // well above LOD_LABEL_FADE_HIGH — labels fully opaque

        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        render_to_png(&spec, |ctx| engine.draw(ctx)).expect("headless render must succeed");

        assert_eq!(engine.labels_drawn_last_frame(), 2, "both nodes sit in separate grid cells and must both draw a label");
    }
}
