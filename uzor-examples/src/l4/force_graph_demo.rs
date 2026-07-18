//! # `uzor-graph` engine demo — force-directed, live, draggable
//!
//! Synthetic clustered graph (~530 nodes, 6 clusters bridged by a hub
//! ring), generated **deterministically** (index-seeded, no
//! time/`Math.random`) so the engine is proven against a graph shape
//! that isn't pathologically skewed toward the one forensic case that
//! motivated it (see `uzor-graph/RUN.md` / the engine design doc).
//!
//! Watch it settle live, drag a node (it pins + the sim re-settles
//! around it), scroll to zoom, drag empty canvas to pan, click a node
//! to select it and see facts in the right sidebar.
//!
//! Run:
//! ```sh
//! cargo run -p uzor-examples --bin force-graph-demo
//! ```
//!
//! Agent-api verification: see `uzor-graph/RUN.md`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use uzor::core::types::Rect;
use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::input::PlatformEvent;
use uzor::layout::agent::{AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface};
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager};
use uzor::platform::types::CornerStyle;
use uzor::render::{RenderContext, RenderRegion};
use uzor::types::unsafe_widget_id;
use uzor_desktop::{AppRun3D as _, Scene3DApp, Scene3DFrame};

use uzor_graph::{ForceDirectedLayout3D, Graph, GraphEngine, GraphEngine3D, GraphLayoutMode, NodeIndex};

const AGENT_PORT: u16 = 17481;
const BLACKBOX_SLOT: &str = "graph";
const SIDEBAR_SLOT: &str = "sidebar";
const SIDEBAR_WIDTH: f32 = 320.0;

const NUM_CLUSTERS: usize = 6;
const CLUSTER_SIZE: usize = 88;
const EDGES_PER_NODE: usize = 3;

// ── Deterministic PRNG — index-seeded, no time/OS randomness ────────────

/// splitmix64-style generator. Seeded purely by caller-supplied indices
/// so the whole demo graph is byte-for-byte reproducible.
struct DetRng(u64);

impl DetRng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range_usize(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as usize }
    }
}

// ── Synthetic graph ──────────────────────────────────────────────────────

type DemoGraph = Graph<(), ()>;

/// 6 clusters of `CLUSTER_SIZE` members each, plus one hub node per
/// cluster. Intra-cluster edges are deterministic-random; the hub fans
/// out to a subset of its cluster; hubs form a ring — the only
/// inter-cluster edges, so clusters stay visually separable once
/// settled (unlike the rejected MVP's hop-depth-0 megacolumn).
///
/// Also returns each cluster's member list — `DemoApp::new` declares 3
/// collapsible clusters (Phase D's "3 clusters x ~8 nodes" fixture) over
/// the first 8 members of clusters 0/1/2, so the `collapse`/`expand`
/// agent actions have something concrete to act on in the live demo.
fn build_demo_graph() -> (DemoGraph, Vec<(f32, f32)>, Vec<Vec<NodeIndex>>) {
    let mut graph = DemoGraph::new();
    let mut positions = Vec::new();
    let mut cluster_members: Vec<Vec<NodeIndex>> = vec![Vec::new(); NUM_CLUSTERS];
    let mut hubs = Vec::with_capacity(NUM_CLUSTERS);

    for cluster in 0..NUM_CLUSTERS {
        // Golden-angle ring placement spreads cluster centers apart
        // before the sim even starts.
        let angle = cluster as f32 * 2.399_963;
        let cx = angle.cos() * 420.0;
        let cy = angle.sin() * 420.0;

        for member in 0..CLUSTER_SIZE {
            let mut rng = DetRng::new((cluster as u64) << 32 | member as u64);
            let jitter_r = rng.next_f32() * 140.0;
            let jitter_a = rng.next_f32() * std::f32::consts::TAU;
            let x = cx + jitter_a.cos() * jitter_r;
            let y = cy + jitter_a.sin() * jitter_r;

            let id = graph.push_node((), format!("c{cluster}n{member}"), format!("cluster-{cluster}"), 4.0);
            cluster_members[cluster].push(id);
            positions.push((x, y));
        }

        let hub_id = graph.push_node((), format!("hub-{cluster}"), "hub", 4.0);
        hubs.push(hub_id);
        positions.push((cx, cy));
    }

    for cluster in 0..NUM_CLUSTERS {
        let members = &cluster_members[cluster];
        for (i, &node) in members.iter().enumerate() {
            let mut rng = DetRng::new(0xC0FF_EE00 ^ ((cluster as u64) << 20) ^ i as u64);
            for _ in 0..EDGES_PER_NODE {
                let j = rng.range_usize(members.len());
                if j != i {
                    graph.push_edge(node, members[j], 1.0, ());
                }
            }
        }
        for (i, &node) in members.iter().enumerate() {
            if i % 6 == 0 {
                graph.push_edge(hubs[cluster], node, 1.0, ());
            }
        }
    }

    for cluster in 0..NUM_CLUSTERS {
        let next = (cluster + 1) % NUM_CLUSTERS;
        graph.push_edge(hubs[cluster], hubs[next], 0.6, ());
    }

    // Radius by degree, computed after all edges are known.
    let ids: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
    for id in ids {
        let degree = graph.degree(id);
        graph.set_radius(id, 3.0 + (degree as f32).sqrt() * 1.6);
    }

    (graph, positions, cluster_members)
}

// ── App ───────────────────────────────────────────────────────────────

struct SelectedFacts {
    label: String,
    category: String,
    degree: u32,
    position: (f32, f32),
    pinned: bool,
}

// `GraphLayoutMode` (not a bare `ForceDirectedLayout`) so the
// `set_layout` agent action has something to dispatch to — see
// `uzor-graph/src/layout/mode.rs`.
type Engine = GraphEngine<(), (), GraphLayoutMode>;
type Engine3D = GraphEngine3D<(), (), ForceDirectedLayout3D>;

// ── Dimension switching (W3D arc plan §1.6, Wave 2) ──────────────────────

/// Which engine currently owns rendering/event dispatch — decided at the
/// DEMO layer, not inside `uzor-graph` itself (`GraphEngine`/
/// `GraphEngine3D` stay fully independent library types, per the plan's
/// own §1.2/§1.6 reasoning).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dimension {
    TwoD,
    ThreeD,
}

impl Dimension {
    fn from_code(code: u8) -> Self {
        if code == 3 { Dimension::ThreeD } else { Dimension::TwoD }
    }

    fn code(self) -> u8 {
        match self {
            Dimension::TwoD => 2,
            Dimension::ThreeD => 3,
        }
    }
}

/// Shared, thread-safe active-dimension flag — cloned into both
/// `DemoApp` (read every frame on the winit thread) and `DemoBlackbox`
/// (read/written from the agent-api HTTP thread, see
/// `uzor::layout::agent::blackbox`'s own "Threading" doc section: the
/// registry stores `Arc<Mutex<dyn BlackboxAgentSurface>>`, but the flag
/// itself needs to be readable from `DemoApp::on_event`/`scene3d`
/// without going through that same lock). `Relaxed` ordering is
/// sufficient — this is a single independent flag, not synchronizing
/// access to any other shared data.
#[derive(Clone)]
struct DimState(Arc<AtomicU8>);

impl DimState {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(Dimension::TwoD.code())))
    }

    fn get(&self) -> Dimension {
        Dimension::from_code(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, dim: Dimension) {
        self.0.store(dim.code(), Ordering::Relaxed);
    }
}

/// Wave 2's placeholder: `GraphEngine3D::on_event` used its `viewport`
/// argument ONLY for a `contains()` gate (never for coordinate math), so
/// a maximal rect was the honest "whole window, no narrower rect to
/// check against" answer. **Wave 3 divergence**: hover/click picking
/// (`pick3d::screen_to_ray`/`project_world_to_screen`) needs the REAL
/// pixel viewport to build a correct NDC mapping — this placeholder's
/// `2e9`-wide extent would compress every real cursor position to
/// `ndc ~= (0, 0)` and break picking outright. Kept only as the
/// before-the-first-3D-frame fallback (see [`DemoApp::dim3d_viewport`]).
fn full_window_viewport() -> Rect {
    Rect::new(-1.0e9, -1.0e9, 2.0e9, 2.0e9)
}

struct DemoApp {
    engine: Arc<Mutex<Engine>>,
    engine3d: Arc<Mutex<Engine3D>>,
    dim: DimState,
    did_init_camera: bool,
    /// Wall-clock timestamp of the last `scene3d()` tick — mirrors
    /// `GraphEngine::tick_real_time`'s own clamped-dt convention so the
    /// 3D sim settles at a real-time rate regardless of the render
    /// loop's actual frame rate. Reset to `None` whenever 3D goes
    /// inactive so re-activating doesn't apply one huge stale-dt jump.
    last_3d_frame_at: Option<std::time::Instant>,
    /// Last real 3D surface size in px, set every `scene3d()` call
    /// (Wave 3) — `on_event`'s picking math (`GraphEngine3D::on_event` ->
    /// `pick3d::screen_to_ray`/`project_world_to_screen`) needs the
    /// ACTUAL rendered pixel dimensions, unlike Wave 2's pure-gating
    /// `viewport.contains()` use, which tolerated
    /// [`full_window_viewport`]'s placeholder extent. See
    /// [`DemoApp::dim3d_viewport`].
    last_3d_surface_px: Option<(u32, u32)>,
}

/// Thin `BlackboxAgentSurface` wrapper the demo registers instead of
/// `engine` directly (W3D arc plan §1.6) — forwards `set_dimension
/// {"dim": 2|3}` to flip `DimState` locally, forwards every other action
/// to the 2D engine while it's active. This keeps `GraphEngine`'s own
/// agent vocabulary clean and dimension-switching entirely a demo
/// concern, per the plan's own reasoning for why dimension state lives
/// here and not inside `uzor-graph`.
///
/// **Wave 3**: `GraphEngine3D` still does not implement
/// `BlackboxAgentSurface` itself (a full 3D agent-action vocabulary —
/// pin/cluster/filter/etc-equivalents — is out of this arc's scope, plan
/// §5) — but the plan's own Wave 3 item 4 asks for "hover_node/
/// select_node equivalents working in 3D", so THIS wrapper now forwards
/// exactly `hover_node`/`select_node`/`clear_selection` straight onto
/// `engine3d` while 3D is active (see [`DemoBlackbox::apply_3d_agent_action`]).
/// Every other action while 3D is active stays a clean, typed rejection.
struct DemoBlackbox {
    engine: Arc<Mutex<Engine>>,
    engine3d: Arc<Mutex<Engine3D>>,
    dim: DimState,
}

/// `NodeIndex` resolution for the 3D agent-forwarding path (Wave 3) —
/// mirrors `uzor_graph::agent`'s own private `resolve_node` (index or
/// label lookup), re-implemented here since that helper isn't exported
/// (`GraphEngine3D` has its own `graph`/`particles`, not a `GraphEngine`,
/// so the 2D helper doesn't apply directly).
fn resolve_node_3d(engine3d: &Engine3D, action: &AgentAction) -> Option<NodeIndex> {
    if let Some(idx) = action.args.get("index").and_then(Value::as_u64) {
        let node = NodeIndex(idx as u32);
        return (node.index() < engine3d.graph.node_count()).then_some(node);
    }
    if let Some(label) = action.args.get("label").and_then(Value::as_str) {
        return engine3d.graph.find_by_label(label);
    }
    None
}

/// JSON facts for one 3D node — mirrors `uzor_graph::agent`'s own
/// 2D-engine `selected`/`hover` shape closely enough for a caller driving
/// both dimensions to expect the same field names.
fn node_facts_json_3d(engine3d: &Engine3D, id: NodeIndex) -> Option<Value> {
    engine3d.node_facts(id).map(|f| {
        json!({
            "index": f.index.index(),
            "label": f.label,
            "category": f.category,
            "degree": f.degree,
            "position": { "x": f.position.0, "y": f.position.1 },
            "pinned": f.pinned,
        })
    })
}

/// Cluster indices given a collapsible cluster (Phase D's "3 clusters x
/// ~8 nodes" fixture) — a subset (first 8 members) of clusters 0/1/2 of
/// the full demo graph, not a separate graph. Driving
/// `{"name":"collapse","args":{"cluster":0}}` etc. over agent-api
/// exercises `GraphEngine::collapse_cluster` on the live demo.
const COLLAPSIBLE_CLUSTERS: usize = 3;
const COLLAPSIBLE_CLUSTER_SIZE: usize = 8;

impl DemoApp {
    fn new() -> Self {
        let (graph, positions, cluster_members) = build_demo_graph();
        let mut engine = GraphEngine::new(graph, GraphLayoutMode::default());
        engine.seed_positions(&positions);
        engine.set_agent_slot_id(BLACKBOX_SLOT);
        for members in cluster_members.iter().take(COLLAPSIBLE_CLUSTERS) {
            engine.define_cluster(members[..COLLAPSIBLE_CLUSTER_SIZE.min(members.len())].to_vec());
        }

        // Wave 2 (W3D arc plan §1.6) — the SAME graph fixture, seeded
        // into a second, independent 3D engine. `build_demo_graph` is
        // deterministic (index-seeded `DetRng`, no time/RNG), so calling
        // it a second time reproduces the byte-identical topology;
        // `Graph` itself isn't `Clone` (no coordinate state, but no
        // derived `Clone` either — `graph.rs`), so a fresh construction
        // is the straightforward way to get a second independent value.
        let (graph3d, positions3d, _cluster_members3d) = build_demo_graph();
        let mut engine3d = Engine3D::new(graph3d, ForceDirectedLayout3D::default());
        // `seed_positions` (not a manual `p.x = x; p.y = y;` loop) — the
        // engine-level fix for a live-caught defect: `build_demo_graph`'s
        // positions are 2D-only (`z` stays at `Particle::default()`'s
        // `0.0` for every node), and every 3D force is z-symmetric, so a
        // manual x/y-only seed left the sim permanently confined to the
        // z = 0 plane. `seed_positions` detects that degenerate z-extent
        // and jitters it deterministically — see `uzor-graph/CLAUDE.md`'s
        // divergence log and `GraphEngine3D::ensure_z_variance`.
        engine3d.seed_positions(&positions3d);

        Self {
            engine: Arc::new(Mutex::new(engine)),
            engine3d: Arc::new(Mutex::new(engine3d)),
            dim: DimState::new(),
            did_init_camera: false,
            last_3d_frame_at: None,
            last_3d_surface_px: None,
        }
    }

    fn lock(engine: &Arc<Mutex<Engine>>) -> MutexGuard<'_, Engine> {
        match engine.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn lock3d(engine3d: &Arc<Mutex<Engine3D>>) -> MutexGuard<'_, Engine3D> {
        match engine3d.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// The REAL 3D viewport (Wave 3) — the last surface size `scene3d()`
    /// was actually called with, or [`full_window_viewport`]'s
    /// placeholder before the very first 3D frame has rendered (there's
    /// nothing better to gate `on_pointer_down`'s `contains()` check
    /// against yet, and no picking has anything to hit before then
    /// either).
    fn dim3d_viewport(&self) -> Rect {
        match self.last_3d_surface_px {
            Some((w, h)) => Rect::new(0.0, 0.0, w as f64, h as f64),
            None => full_window_viewport(),
        }
    }
}

impl BlackboxAgentSurface for DemoBlackbox {
    fn agent_slot_id(&self) -> &str {
        BLACKBOX_SLOT
    }

    fn agent_kind(&self) -> &str {
        "graph"
    }

    fn list_agent_widgets(&self) -> Vec<AgentWidget> {
        match self.dim.get() {
            Dimension::TwoD => DemoApp::lock(&self.engine).list_agent_widgets(),
            // Agent-addressable node RECTS in 3D would need a live
            // camera/viewport threaded into this wrapper (this struct
            // only holds `engine3d`/`dim`, not `DemoApp`'s own
            // `last_3d_surface_px`) — a real but small follow-up, not
            // this wave's own item 4 ask (hover_node/select_node
            // equivalents + agent_state reporting, both done below).
            // Flagged for Wave 4+.
            Dimension::ThreeD => Vec::new(),
        }
    }

    fn agent_state(&self) -> Value {
        let mut state = match self.dim.get() {
            Dimension::TwoD => DemoApp::lock(&self.engine).agent_state(),
            // Wave 3: report the same hover/selected shape the 2D engine
            // does (plan §4 item 4: "state() while 3D reports
            // hovered/selected/dimension").
            Dimension::ThreeD => {
                let engine3d = DemoApp::lock3d(&self.engine3d);
                json!({
                    "node_count": engine3d.graph.node_count(),
                    "edge_count": engine3d.graph.edge_count(),
                    "hovered": engine3d.hovered().map(NodeIndex::index),
                    "hover": engine3d.hovered().and_then(|id| node_facts_json_3d(&engine3d, id)),
                    "selected": engine3d.selected().map(NodeIndex::index),
                    "selected_facts": engine3d.selected().and_then(|id| node_facts_json_3d(&engine3d, id)),
                })
            }
        };
        if let Value::Object(ref mut map) = state {
            map.insert("dimension".to_owned(), json!(self.dim.get().code()));
        }
        state
    }

    fn apply_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        if action.name == "set_dimension" {
            return match action.args.get("dim").and_then(Value::as_u64) {
                Some(2) => {
                    self.dim.set(Dimension::TwoD);
                    AgentActionReply::ok_with_log(json!({ "dimension": 2 }))
                }
                Some(3) => {
                    self.dim.set(Dimension::ThreeD);
                    AgentActionReply::ok_with_log(json!({ "dimension": 3 }))
                }
                _ => AgentActionReply::err("set_dimension requires args.dim to be 2 or 3"),
            };
        }
        match self.dim.get() {
            Dimension::TwoD => DemoApp::lock(&self.engine).apply_agent_action(action),
            Dimension::ThreeD => self.apply_3d_agent_action(action),
        }
    }
}

impl DemoBlackbox {
    /// Wave 3 — `hover_node`/`select_node`/`clear_selection` forwarded
    /// straight onto `engine3d` (plan §4 item 4: "hover_node/select_node
    /// equivalents working in 3D"). Mirrors `uzor_graph::agent`'s own 2D
    /// `hover_node` convention exactly: `{}`/explicit `null` clears the
    /// hover, an out-of-range index or unknown label is an error reply,
    /// not a silent clear. Everything else stays a typed rejection — a
    /// full 3D agent vocabulary (pin/cluster/filter-equivalents) is out
    /// of this arc (plan §5).
    fn apply_3d_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        let mut engine3d = DemoApp::lock3d(&self.engine3d);
        match action.name.as_str() {
            "hover_node" => {
                let index_arg = action.args.get("index");
                let explicit_clear =
                    matches!(index_arg, Some(Value::Null)) || (index_arg.is_none() && action.args.get("label").is_none());
                if explicit_clear {
                    engine3d.hovered = None;
                    return AgentActionReply::ok_with_log(json!({ "hover": Value::Null }));
                }
                let Some(node) = resolve_node_3d(&engine3d, &action) else {
                    return AgentActionReply::err("hover_node requires args.index (u32), args.label (string), or {} / null to clear");
                };
                engine3d.hovered = Some(node);
                AgentActionReply::ok_with_log(json!({ "hover": { "index": node.index() } }))
            }
            "select_node" => {
                let Some(node) = resolve_node_3d(&engine3d, &action) else {
                    return AgentActionReply::err("select_node requires args.index (u32) or args.label (string)");
                };
                engine3d.selected = Some(node);
                AgentActionReply::ok_with_log(json!({ "selected": node.index() }))
            }
            "clear_selection" => {
                engine3d.selected = None;
                AgentActionReply::ok_with_log(json!({ "selected": Value::Null }))
            }
            _ => AgentActionReply::err(
                "3D dimension only supports hover_node/select_node/clear_selection besides set_dimension (Wave 3)",
            ),
        }
    }
}

fn draw_row(
    layout: &mut LayoutManager<NoPanel>,
    render: &mut dyn RenderContext,
    body_rect: Rect,
    cy: &mut f64,
    row_h: f64,
    pad: f64,
    w: f64,
    id: &str,
    text: &str,
) {
    let r = Rect { x: body_rect.x + pad, y: *cy, width: w, height: row_h };
    uzor::framework::widgets::lm::text(unsafe_widget_id(id), r, text).build(layout, render);
    *cy += row_h;
}

impl App<NoPanel> for DemoApp {
    fn init(&mut self, _key: &WindowKey, layout: &mut LayoutManager<NoPanel>) {
        // Wave 2 (W3D arc plan §1.6): register the `DemoBlackbox`
        // dimension-aware wrapper instead of `self.engine` directly —
        // see that struct's own doc comment.
        let blackbox = DemoBlackbox { engine: self.engine.clone(), engine3d: self.engine3d.clone(), dim: self.dim.clone() };
        layout.register_blackbox_agent(BLACKBOX_SLOT, Arc::new(Mutex::new(blackbox)));
    }

    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        win.layout.edges_mut().clear();
        win.layout.edges_mut().add(EdgeSlot {
            id: SIDEBAR_SLOT.to_owned(),
            side: EdgeSide::Right,
            thickness: SIDEBAR_WIDTH,
            visible: true,
            order: 0,
            ..Default::default()
        });

        let win_rect = win.layout.last_window().unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
        if win_rect.width > 0.0 && win_rect.height > 0.0 {
            win.layout.solve(win_rect);
        }

        let canvas_rect = win.layout.last_solved().map(|s| s.dock_area).unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));

        win.render.set_fill_color("#0d0f14");
        win.render.fill_rect(canvas_rect.x, canvas_rect.y, canvas_rect.width, canvas_rect.height);

        let (hot, alpha, node_count, visible_count, facts_owned) = {
            let mut engine = Self::lock(&self.engine);
            engine.set_canvas_rect(canvas_rect);

            if !self.did_init_camera && canvas_rect.width > 0.0 {
                engine.fit_view();
                self.did_init_camera = true;
            }

            engine.tick_real_time();

            if canvas_rect.width > 0.0 && canvas_rect.height > 0.0 {
                win.render.save();
                win.render.clip_rect(canvas_rect.x, canvas_rect.y, canvas_rect.width, canvas_rect.height);
                engine.draw(win.render);
                win.render.restore();
            }

            let facts = engine.selected_facts().map(|f| SelectedFacts {
                label: f.label.to_owned(),
                category: f.category.to_owned(),
                degree: f.degree,
                position: f.position,
                pinned: f.pinned,
            });
            let snapshot = (engine.is_hot(), engine.last_tick().alpha, engine.graph.node_count(), engine.visible_nodes().len(), facts);
            engine.clear_dirty();
            snapshot
        };

        let sb_handle = win.layout.add_sidebar(SIDEBAR_SLOT);
        {
            let layout = &mut *win.layout;
            let render = &mut *win.render;
            uzor::framework::widgets::lm::sidebar(&sb_handle, SIDEBAR_SLOT)
                .header_title("Graph")
                .content_height(360.0)
                .build_with_body(layout, render, |layout, render, body_rect| {
                    let pad = 12.0_f64;
                    let row_h = 20.0_f64;
                    let w = body_rect.width - 2.0 * pad;
                    let mut cy = body_rect.y + pad;

                    draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:nodes", &format!("nodes: {node_count}  visible: {visible_count}"));
                    draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:alpha", &format!("alpha: {alpha:.4}  hot: {hot}"));
                    cy += 8.0;

                    match &facts_owned {
                        Some(f) => {
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:label", &format!("selected: {}", f.label));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:category", &format!("category: {}", f.category));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:degree", &format!("degree: {}", f.degree));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:pos", &format!("pos: ({:.1}, {:.1})", f.position.0, f.position.1));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:pinned", &format!("pinned: {}", f.pinned));
                        }
                        None => {
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:none", "no selection — click a node");
                        }
                    }
                });
        }
    }

    fn regions(&mut self) -> Vec<RenderRegion> {
        let engine = Self::lock(&self.engine);
        vec![engine.render_region("force-graph-demo:main")]
    }

    /// Dispatches to whichever dimension is currently active (W3D arc
    /// plan §1.6) — `ui()`/`draw_region()` above are ONLY ever called by
    /// `Manager` while 2D is active (`scene3d()` returning `None` is
    /// exactly what routes the frame there — see `uzor-desktop`'s
    /// `Manager` divergence log), so they don't need their own dimension
    /// check; `on_event` is called every tick regardless of which
    /// dimension is rendering, so it does.
    fn on_event(&mut self, event: &PlatformEvent) -> bool {
        match self.dim.get() {
            Dimension::TwoD => {
                let mut engine = Self::lock(&self.engine);
                engine.on_event(event)
            }
            Dimension::ThreeD => {
                let mut engine3d = Self::lock3d(&self.engine3d);
                engine3d.on_event(event, self.dim3d_viewport())
            }
        }
    }
}

impl Scene3DApp<NoPanel> for DemoApp {
    /// `None` while 2D is active — the 2D `App::ui` path (unchanged
    /// above) keeps rendering the window. `Some(frame)` while 3D is
    /// active ticks the 3D sim at a real-time rate (mirrors
    /// `GraphEngine::tick_real_time`'s own clamped-dt convention, which
    /// `GraphEngine3D` doesn't have its own copy of — Wave 2's own scope
    /// is `tick(dt)`, not a real-time wrapper) and builds the composed
    /// frame `Manager` will paint this tick.
    fn scene3d(&mut self, surf_w: u32, surf_h: u32) -> Option<Scene3DFrame> {
        if self.dim.get() != Dimension::ThreeD {
            self.last_3d_frame_at = None;
            return None;
        }
        // Wave 3: record the REAL surface size so `on_event`'s picking
        // math (`dim3d_viewport`) uses the actual rendered viewport
        // instead of `full_window_viewport`'s placeholder.
        self.last_3d_surface_px = Some((surf_w, surf_h));
        let now = std::time::Instant::now();
        let dt = match self.last_3d_frame_at {
            Some(prev) => now.duration_since(prev).as_secs_f32().min(0.1),
            None => 1.0 / 60.0,
        };
        self.last_3d_frame_at = Some(now);

        let mut engine3d = Self::lock3d(&self.engine3d);
        engine3d.tick(dt);
        let scene = engine3d.build_scene();
        let aspect = surf_w as f32 / (surf_h.max(1) as f32);
        let camera = engine3d.camera(aspect);
        Some(Scene3DFrame { scene, camera })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DemoApp::new())
        .agent_api(AGENT_PORT)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor-graph — force graph demo")
                .size(1400, 900)
                .min_size(900, 600)
                .decorations(false)
                .background(0xFF_0d_0f_14)
                .corner_style(CornerStyle::Rounded)
                .border_color(0x00_4d_90_fe),
        )
        .icon_from_png(include_bytes!("../../assets/icon.png"))?
        // Wave 2 (W3D arc plan §1.6/§1.7): `.run_with_3d()` instead of
        // `.run()` — additive entry point, 2D remains the default at
        // launch (`Dimension::TwoD`, `DimState::new()`); `set_dimension
        // {"dim": 3}` over agent-api switches live.
        .run_with_3d()?;
    Ok(())
}
