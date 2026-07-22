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

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use uzor::core::types::Rect;
use uzor::framework::app::{App, CursorCaptureMode, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::frame_profiler::FrameProfiler;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::input::{KeyCode, MouseButton, PlatformEvent};
use uzor::layout::agent::{AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface};
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager};
use uzor::platform::types::CornerStyle;
use uzor::render::{RenderContext, RenderRegion};
use uzor::types::unsafe_widget_id;
use uzor_desktop::{AppRun3D as _, Scene3DApp, Scene3DFrame};

use uzor_graph::interaction::fly::FlyController;
use uzor_graph::{FilterSpec, ForceDirectedLayout3D, Graph, GraphEngine, GraphEngine3D, GraphLayoutMode, GroupId, NodeIndex, SelectMode};

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

/// Overwrite every node's radius from its own degree (once all edges
/// are known) — the one radius formula every fixture below shares.
fn apply_degree_radius(graph: &mut DemoGraph) {
    let ids: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
    for id in ids {
        let degree = graph.degree(id);
        graph.set_radius(id, 3.0 + (degree as f32).sqrt() * 1.6);
    }
}

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
/// The `clusters` [`Fixture`] (default, matches the original ~534-node
/// shape) — see [`build_fixture`] for the other 3.
fn build_clusters_graph() -> (DemoGraph, Vec<(f32, f32)>, Vec<Vec<NodeIndex>>) {
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

    apply_degree_radius(&mut graph);

    (graph, positions, cluster_members)
}

// ── Wave (owner order: "хочу более древовидные визуализации, не только
// пятиугольник") — 3 additional deterministic fixtures ──────────────────

const TREE_NODE_BUDGET: usize = 300;
/// Every node below this depth always branches — guarantees the tree
/// actually reaches its target depth before the budget/taper can choke
/// it off early (a "few hundred nodes, deep" shape, not a shallow bush).
const TREE_MIN_BRANCH_DEPTH: u32 = 4;
/// Hard depth cap — with `TREE_MIN_BRANCH_DEPTH` this produces 5-6
/// branching levels beneath the root ("хочу более древовидные... branching
/// 4-5 levels").
const TREE_MAX_DEPTH: u32 = 6;

/// Root + branch-and-taper tree generator shared by the `tree` and
/// `hierarchy` fixtures — deterministic (index-seeded `DetRng`, same
/// convention as [`build_clusters_graph`]), stack-based (DFS) growth
/// capped by `node_budget`/[`TREE_MAX_DEPTH`]. Branching tapers from
/// `2..=4` children per node below [`TREE_MIN_BRANCH_DEPTH`] to `1..=3`
/// beyond it, so the tree keeps growing DEEPER as the budget is consumed,
/// not just wider at the root. Returns each node's own depth alongside
/// the usual graph/position pair — [`build_hierarchy_graph`] needs it to
/// pick same-depth cross-links.
fn build_tree_internal(node_budget: usize, category_prefix: &str) -> (DemoGraph, Vec<(f32, f32)>, Vec<u32>) {
    let mut graph = DemoGraph::new();
    let mut positions = Vec::with_capacity(node_budget);
    let mut depths = Vec::with_capacity(node_budget);

    let root = graph.push_node((), format!("{category_prefix}-root"), format!("{category_prefix}-0"), 6.0);
    positions.push((0.0, 0.0));
    depths.push(0u32);

    let mut stack: Vec<(NodeIndex, u32, f32, f32)> = vec![(root, 0, 0.0, 0.0)];
    let mut count = 1usize;
    let mut seq = 0u64;
    while let Some((parent, depth, px, py)) = stack.pop() {
        if depth >= TREE_MAX_DEPTH || count >= node_budget {
            continue;
        }
        let mut rng = DetRng::new(0xC0FF_EE01 ^ ((depth as u64) << 40) ^ seq);
        seq += 1;
        let children = if depth < TREE_MIN_BRANCH_DEPTH { 2 + rng.range_usize(3) } else { 1 + rng.range_usize(3) };
        for c in 0..children {
            if count >= node_budget {
                break;
            }
            let angle = (c as f32 / children as f32) * std::f32::consts::TAU + depth as f32 * 0.7;
            let step = 70.0 + depth as f32 * 12.0;
            let x = px + angle.cos() * step;
            let y = py + angle.sin() * step;
            let child_depth = depth + 1;
            let label = format!("{category_prefix}-{child_depth}-{count}");
            let category = format!("{category_prefix}-{}", child_depth.min(6));
            let child = graph.push_node((), label, category, 4.0);
            graph.push_edge(parent, child, 1.0, ());
            positions.push((x, y));
            depths.push(child_depth);
            stack.push((child, child_depth, x, y));
            count += 1;
        }
    }

    apply_degree_radius(&mut graph);
    (graph, positions, depths)
}

/// The `tree` [`Fixture`] — a deep rooted tree, no cross-links (a pure
/// hierarchy). See [`build_tree_internal`]'s own doc comment.
fn build_tree_graph() -> (DemoGraph, Vec<(f32, f32)>, Vec<Vec<NodeIndex>>) {
    let (graph, positions, _depths) = build_tree_internal(TREE_NODE_BUDGET, "tree");
    (graph, positions, Vec::new())
}

/// Fraction of nodes that get one extra same-depth "shortcut" edge on
/// top of the underlying tree — the owner's own spec ("tree + cross-links
/// ~5% — DAG-ish").
const HIERARCHY_CROSS_LINK_RATIO: f32 = 0.05;

/// The `hierarchy` [`Fixture`] — [`build_tree_internal`]'s SAME tree,
/// plus a deterministic ~5% pass of extra same-depth cross-links (a
/// shortcut between two siblings-of-siblings, never back to an ancestor)
/// so the result reads as a DAG-ish hierarchy with shortcuts, not a
/// random web.
fn build_hierarchy_graph() -> (DemoGraph, Vec<(f32, f32)>, Vec<Vec<NodeIndex>>) {
    let (mut graph, positions, depths) = build_tree_internal(TREE_NODE_BUDGET, "hier");
    let n = graph.node_count();

    let mut by_depth: std::collections::BTreeMap<u32, Vec<NodeIndex>> = std::collections::BTreeMap::new();
    for (i, &d) in depths.iter().enumerate() {
        by_depth.entry(d).or_default().push(NodeIndex(i as u32));
    }

    for i in 0..n {
        let mut rng = DetRng::new(0xDEC0_DE55 ^ i as u64);
        if rng.next_f32() >= HIERARCHY_CROSS_LINK_RATIO {
            continue;
        }
        let depth = depths[i];
        let candidates = by_depth.get(&depth).map(Vec::as_slice).unwrap_or(&[]);
        if candidates.len() < 2 {
            continue;
        }
        let j = candidates[rng.range_usize(candidates.len())];
        if j.index() != i {
            graph.push_edge(NodeIndex(i as u32), j, 0.6, ());
        }
    }

    (graph, positions, Vec::new())
}

const SPARSE_NODE_COUNT: usize = 360;
/// Probability each node originates one random long-range edge —
/// contributes an average degree around ~1.1 (low-degree, per spec),
/// most nodes end up with 0-2 edges total once both origination and
/// incoming edges are counted.
const SPARSE_LONG_RANGE_EDGE_PROBABILITY: f32 = 0.55;

/// The `sparse` [`Fixture`] — low-degree random graph, nodes spread
/// uniformly (by AREA, not radius — `sqrt(u)` sampling) over a wide disk
/// with edges wired to a RANDOM other node (not a nearest neighbor), so
/// the typical edge spans a large fraction of the whole layout — "shows
/// long-range structure," and not incidentally the fixture that most
/// directly exercises the Wave C edge-quality overhaul's own target case
/// (long, generically-angled edges).
fn build_sparse_graph() -> (DemoGraph, Vec<(f32, f32)>, Vec<Vec<NodeIndex>>) {
    let mut graph = DemoGraph::new();
    let mut positions = Vec::with_capacity(SPARSE_NODE_COUNT);
    let mut ids = Vec::with_capacity(SPARSE_NODE_COUNT);
    for i in 0..SPARSE_NODE_COUNT {
        let mut rng = DetRng::new(0xBADC_0FFE ^ i as u64);
        let r = rng.next_f32().sqrt() * 900.0;
        let a = rng.next_f32() * std::f32::consts::TAU;
        let x = a.cos() * r;
        let y = a.sin() * r;
        let category = format!("sparse-{}", i % 8);
        let id = graph.push_node((), format!("s{i}"), category, 3.5);
        ids.push(id);
        positions.push((x, y));
    }
    for i in 0..SPARSE_NODE_COUNT {
        let mut rng = DetRng::new(0xFACE_B00C ^ ((i as u64) << 16));
        if rng.next_f32() >= SPARSE_LONG_RANGE_EDGE_PROBABILITY {
            continue;
        }
        let j = rng.range_usize(SPARSE_NODE_COUNT);
        if j != i {
            graph.push_edge(ids[i], ids[j], 1.0, ());
        }
    }
    apply_degree_radius(&mut graph);
    (graph, positions, Vec::new())
}

/// Which deterministic demo graph shape is currently loaded — the
/// owner's own live-verdict order ("хочу более древовидные
/// визуализации, не только пятиугольник"). `Clusters` is the ORIGINAL
/// ~534-node fixture (unchanged) and stays the default at launch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fixture {
    Clusters,
    Tree,
    Hierarchy,
    Sparse,
}

impl Fixture {
    fn as_str(self) -> &'static str {
        match self {
            Fixture::Clusters => "clusters",
            Fixture::Tree => "tree",
            Fixture::Hierarchy => "hierarchy",
            Fixture::Sparse => "sparse",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "clusters" => Some(Fixture::Clusters),
            "tree" => Some(Fixture::Tree),
            "hierarchy" => Some(Fixture::Hierarchy),
            "sparse" => Some(Fixture::Sparse),
            _ => None,
        }
    }

    fn code(self) -> u8 {
        match self {
            Fixture::Clusters => 0,
            Fixture::Tree => 1,
            Fixture::Hierarchy => 2,
            Fixture::Sparse => 3,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Fixture::Tree,
            2 => Fixture::Hierarchy,
            3 => Fixture::Sparse,
            _ => Fixture::Clusters,
        }
    }
}

/// One dispatcher every fixture-consuming call site goes through — see
/// each generator's own doc comment for its shape.
fn build_fixture(fixture: Fixture) -> (DemoGraph, Vec<(f32, f32)>, Vec<Vec<NodeIndex>>) {
    match fixture {
        Fixture::Clusters => build_clusters_graph(),
        Fixture::Tree => build_tree_graph(),
        Fixture::Hierarchy => build_hierarchy_graph(),
        Fixture::Sparse => build_sparse_graph(),
    }
}

/// Shared, thread-safe current-fixture flag — same `Arc<AtomicU8>`
/// cross-thread convention [`DimState`] already established (`state()`/
/// `set_fixture` run on the agent-api HTTP thread; `DemoApp` reads it
/// from the winit thread only for the initial `new()` build).
#[derive(Clone)]
struct FixtureState(Arc<AtomicU8>);

impl FixtureState {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(Fixture::Clusters.code())))
    }

    fn get(&self) -> Fixture {
        Fixture::from_code(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, fixture: Fixture) {
        self.0.store(fixture.code(), Ordering::Relaxed);
    }
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

/// Which 3D camera-control scheme is active — orbit (the pre-existing
/// `GraphEngine3D::on_event` drag-orbit/wheel-dolly/shift-drag-pan
/// behavior, default) or fly (WASD/arrow-key + captured-mouse-look via
/// `uzor_graph::interaction::fly::FlyController`, proving out that
/// lifted engine capability — see `uzor-graph/CLAUDE.md`'s divergence
/// log). Decided at the DEMO layer, same convention as [`Dimension`]:
/// `GraphEngine3D` itself stays fully independent of this choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NavMode {
    Orbit,
    Fly,
}

impl NavMode {
    fn as_str(self) -> &'static str {
        match self {
            NavMode::Orbit => "orbit",
            NavMode::Fly => "fly",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "orbit" => Some(NavMode::Orbit),
            "fly" => Some(NavMode::Fly),
            _ => None,
        }
    }

    fn code(self) -> u8 {
        match self {
            NavMode::Orbit => 0,
            NavMode::Fly => 1,
        }
    }

    fn from_code(code: u8) -> Self {
        if code == 1 { NavMode::Fly } else { NavMode::Orbit }
    }
}

/// Shared, thread-safe active-nav-mode flag — same cross-thread
/// `Arc<AtomicU8>` convention as [`DimState`]: read/written from the
/// agent-api HTTP thread (`set_nav_mode` action, `agent_state()`) and
/// read every tick from the winit thread (`on_event`/`scene3d`).
#[derive(Clone)]
struct NavModeState(Arc<AtomicU8>);

impl NavModeState {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(NavMode::Orbit.code())))
    }

    fn get(&self) -> NavMode {
        NavMode::from_code(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, mode: NavMode) {
        self.0.store(mode.code(), Ordering::Relaxed);
    }
}

/// Fly-mode mouse-look sub-state (owner order 2026-07-19: "прицел и
/// сброс прицела по МКМ") — the foxhound source app's own "MMB CLICK:
/// CURSOR / LOOK" toggle, which the original lift skipped. While ON (the
/// default whenever fly mode is entered) the app requests
/// `CursorCaptureMode::LockedHidden`, `PointerDelta` drives free look,
/// and the overlay paints a center crosshair; a middle-click flips it
/// OFF — capture releases, the crosshair disappears, the ordinary OS
/// cursor returns (WASD movement stays live either way). Another
/// middle-click re-arms look. Same cross-thread `Arc<Atomic*>`
/// convention as [`NavModeState`]/[`DimState`].
#[derive(Clone)]
struct MouseLookState(Arc<AtomicBool>);

impl MouseLookState {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn set(&self, on: bool) {
        self.0.store(on, Ordering::Relaxed);
    }

    fn toggle(&self) -> bool {
        let next = !self.get();
        self.set(next);
        next
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

/// The REAL 3D viewport `Rect` for a last-known surface size, or
/// [`full_window_viewport`]'s placeholder before the first 3D frame ever
/// renders — shared by [`DemoApp::dim3d_viewport`] (winit thread, reads
/// `DemoApp::last_3d_surface_px`) and [`DemoBlackbox::apply_3d_agent_action`]'s
/// own `box_select` forwarding (agent-api HTTP thread, reads
/// `DemoBlackbox::surface_size` — the SAME last-known size, a separate
/// `Arc` clone of it, per [`SurfaceSizeState`]'s own doc comment), so
/// both threads derive the identical viewport from the identical source.
fn viewport_from_surface_size(size: Option<(u32, u32)>) -> Rect {
    match size {
        Some((w, h)) => Rect::new(0.0, 0.0, w as f64, h as f64),
        None => full_window_viewport(),
    }
}

/// Shared last-known 3D render-surface size in px (Wave 5 fit-to-bounds)
/// — `scene3d()` writes it every tick on the winit thread; the
/// `fit_view_3d` agent action (`DemoBlackbox::apply_agent_action`) reads
/// it from the agent-api HTTP thread to derive the aspect ratio
/// `GraphEngine3D::fit_view` needs, mirroring [`DimState`]/[`NavModeState`]'s
/// own cross-thread `Arc<Atomic*>` convention (a `(u32, u32)` doesn't fit
/// one atomic, so this holds width/height as a pair of `AtomicU32`s
/// instead of `Mutex<Option<(u32, u32)>>` — same lock-free spirit). `0`
/// in either slot means "no 3D frame has rendered yet" — see
/// [`surface_aspect`] for the fallback.
#[derive(Clone)]
struct SurfaceSizeState(Arc<(AtomicU32, AtomicU32)>);

impl SurfaceSizeState {
    fn new() -> Self {
        Self(Arc::new((AtomicU32::new(0), AtomicU32::new(0))))
    }

    fn set(&self, w: u32, h: u32) {
        self.0 .0.store(w, Ordering::Relaxed);
        self.0 .1.store(h, Ordering::Relaxed);
    }

    fn get(&self) -> Option<(u32, u32)> {
        let w = self.0 .0.load(Ordering::Relaxed);
        let h = self.0 .1.load(Ordering::Relaxed);
        if w == 0 || h == 0 {
            None
        } else {
            Some((w, h))
        }
    }
}

/// Fallback aspect ratio (Wave 5) for the Home/F fit-to-bounds shortcut
/// and the `fit_view_3d` agent action, while no real 3D frame has
/// rendered yet — an ordinary 16:9 widescreen ratio, the same "no real
/// viewport yet" convention [`full_window_viewport`]'s own doc comment
/// already established for picking.
const FALLBACK_SURFACE_ASPECT: f32 = 16.0 / 9.0;

fn surface_aspect(size: Option<(u32, u32)>) -> f32 {
    match size {
        Some((w, h)) if h > 0 => w as f32 / h as f32,
        _ => FALLBACK_SURFACE_ASPECT,
    }
}

/// Center crosshair painted into the 3D overlay while fly-mode
/// mouse-look is active (owner order 2026-07-19) — the captured-cursor
/// aim marker, the foxhound source app's own convention (its HUD's
/// "MMB CLICK: CURSOR / LOOK" pairing). Four short bars around a small
/// center gap plus a center dot, drawn with plain `fill_rect` (no
/// stroke-path machinery needed for axis-aligned bars); light gray at
/// partial alpha so it reads over both the dark background and a bright
/// node sphere.
fn draw_fly_crosshair(ctx: &mut dyn RenderContext, viewport: Rect) {
    const ARM: f64 = 9.0;
    const GAP: f64 = 4.0;
    const THICK: f64 = 1.5;
    let cx = viewport.x + viewport.width / 2.0;
    let cy = viewport.y + viewport.height / 2.0;
    ctx.set_global_alpha(0.85);
    ctx.set_fill_color("#e6e6ea");
    ctx.fill_rect(cx - GAP - ARM, cy - THICK / 2.0, ARM, THICK);
    ctx.fill_rect(cx + GAP, cy - THICK / 2.0, ARM, THICK);
    ctx.fill_rect(cx - THICK / 2.0, cy - GAP - ARM, THICK, ARM);
    ctx.fill_rect(cx - THICK / 2.0, cy + GAP, THICK, ARM);
    ctx.fill_rect(cx - 1.0, cy - 1.0, 2.0, 2.0);
    ctx.set_global_alpha(1.0);
}

/// Shared "the 2D camera needs an initial `fit_view()`" flag — same
/// cross-thread `Arc<Atomic*>` convention [`DimState`] already
/// established. Was a private `DemoApp`-only `bool` before `set_fixture`
/// needed to request a re-fit from the AGENT thread after a live graph
/// rebuild (`DemoBlackbox::apply_agent_action` runs on the agent-api
/// HTTP thread; `ui()`'s own `fit_view()` call runs on the winit thread).
#[derive(Clone)]
struct CameraFitFlag(Arc<AtomicBool>);

impl CameraFitFlag {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    fn needs_fit(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn request(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    fn clear(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Rebuild BOTH the 2D and 3D engines IN PLACE from `fixture` —
/// `set_fixture`'s own implementation, also used by `DemoApp::new()` for
/// the initial build (exactly ONE "build a fixture into these engines"
/// code path). Replaces the `Mutex`-guarded VALUE, not the `Arc` itself,
/// so every existing clone of these Arcs (`DemoBlackbox`'s own,
/// registered once in `init()`) keeps pointing at the rebuilt engine
/// transparently — no re-registration needed.
fn rebuild_engines(engine: &Arc<Mutex<Engine>>, engine3d: &Arc<Mutex<Engine3D>>, fixture: Fixture, camera_fit: &CameraFitFlag) {
    let (graph, positions, cluster_members) = build_fixture(fixture);
    let mut new_engine = Engine::new(graph, GraphLayoutMode::default());
    new_engine.seed_positions(&positions);
    new_engine.set_agent_slot_id(BLACKBOX_SLOT);
    for members in cluster_members.iter().take(COLLAPSIBLE_CLUSTERS) {
        new_engine.define_cluster(members[..COLLAPSIBLE_CLUSTER_SIZE.min(members.len())].to_vec());
    }

    // Same fixture, called a SECOND time (deterministic — `DetRng`'s own
    // doc comment) for the independent 3D engine, mirroring
    // `DemoApp::new()`'s original Wave 2 convention. Cluster/selection
    // wave: the SAME collapsible clusters are ALSO defined on the 3D
    // engine now (its own independent `ClusterRegistry` — see
    // `GraphEngine3D::clusters`'s own doc comment), so the demo's
    // `collapse`/`expand`/`collapse_selection` blackbox actions work
    // identically while `dimension=3`.
    let (graph3d, positions3d, cluster_members3d) = build_fixture(fixture);
    let mut new_engine3d = Engine3D::new(graph3d, ForceDirectedLayout3D::default());
    new_engine3d.seed_positions(&positions3d);
    for members in cluster_members3d.iter().take(COLLAPSIBLE_CLUSTERS) {
        new_engine3d.define_cluster(members[..COLLAPSIBLE_CLUSTER_SIZE.min(members.len())].to_vec());
    }

    *DemoApp::lock(engine) = new_engine;
    *DemoApp::lock3d(engine3d) = new_engine3d;
    camera_fit.request();
}

struct DemoApp {
    engine: Arc<Mutex<Engine>>,
    engine3d: Arc<Mutex<Engine3D>>,
    dim: DimState,
    /// Which deterministic graph shape is currently loaded — see
    /// [`Fixture`]/[`rebuild_engines`].
    fixture: FixtureState,
    camera_fit: CameraFitFlag,
    /// Active 3D camera-control scheme — see [`NavMode`].
    nav_mode: NavModeState,
    /// Fly-mode mouse-look sub-state — see [`MouseLookState`].
    mouse_look: MouseLookState,
    /// The `FlyController` instance itself — `Arc<Mutex<_>>` (not a bare
    /// field) so `DemoBlackbox::agent_state` can read its current
    /// velocity from the agent-api HTTP thread without needing a
    /// `DemoApp`-only accessor (mirrors why `engine`/`engine3d` are
    /// themselves `Arc<Mutex<_>>`).
    fly: Arc<Mutex<FlyController>>,
    /// 2026-07-22 (3D-parity-arc tail) — named-stage frame timings for
    /// `scene3d()`'s own `tick`/`scene_build` stages, `Arc<Mutex<_>>` for
    /// the SAME cross-thread reason `fly` is (`DemoBlackbox::agent_state`
    /// reads it from the agent-api HTTP thread; `scene3d()` writes it
    /// from the winit thread). Lifted mechanism, app-specific stage
    /// names — see `uzor::framework::frame_profiler`'s own module doc.
    frame_profiler: Arc<Mutex<FrameProfiler>>,
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
    /// [`DemoApp::dim3d_viewport`]. Wave 5 promoted this from a plain
    /// `Option<(u32, u32)>` to [`SurfaceSizeState`] so `fit_view_3d`'s
    /// agent-api action (`DemoBlackbox`, a different struct on a
    /// different thread) can read the same last-known size.
    last_3d_surface_px: SurfaceSizeState,
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
    fixture: FixtureState,
    camera_fit: CameraFitFlag,
    nav_mode: NavModeState,
    mouse_look: MouseLookState,
    fly: Arc<Mutex<FlyController>>,
    /// Same `frame_profiler` `Arc` `DemoApp` owns — see that field's own
    /// doc comment.
    frame_profiler: Arc<Mutex<FrameProfiler>>,
    /// Wave 5 fit-to-bounds — last-known 3D surface size, so `fit_view_3d`
    /// can derive an aspect ratio from the agent-api HTTP thread. See
    /// [`SurfaceSizeState`]'s own doc comment.
    surface_size: SurfaceSizeState,
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

/// `GroupId` resolution for the 3D agent-forwarding path (cluster/
/// selection wave) — mirrors `uzor_graph::agent`'s own private
/// `resolve_cluster` (`args.cluster` as a u32), re-implemented here since
/// that helper isn't exported.
fn resolve_cluster_3d(action: &AgentAction) -> Option<GroupId> {
    action.args.get("cluster").and_then(Value::as_u64).map(|v| GroupId(v as u32))
}

/// Parse `args.mode` for the 3D `select_nodes`/`box_select` forwarding —
/// mirrors `uzor_graph::agent`'s own private `parse_select_mode` byte-for-
/// byte (re-implemented here since that helper isn't exported): missing
/// `mode` defaults to [`SelectMode::Replace`], an unrecognized string is
/// an error, not a silent fallback.
fn parse_select_mode_3d(action: &AgentAction) -> Result<SelectMode, String> {
    match action.args.get("mode").and_then(Value::as_str) {
        None | Some("replace") => Ok(SelectMode::Replace),
        Some("union") => Ok(SelectMode::Union),
        Some("diff") => Ok(SelectMode::Diff),
        Some(other) => Err(format!("unknown select mode {other:?} (expected \"replace\"|\"union\"|\"diff\")")),
    }
}

/// [`FilterSpec`] snapshot as JSON — mirrors `uzor_graph::agent`'s own
/// private `filter_json` exactly (re-implemented here since that helper
/// isn't exported), shared by `agent_state`'s 3D `filter` field and the
/// `set_filter` action's reply.
fn filter_json_3d(filter: &FilterSpec) -> Value {
    json!({
        "label_substring": filter.label_substring,
        "categories": filter.categories,
        "min_degree": filter.min_degree,
    })
}

/// `agent_state`'s 3D `selection` field shape — mirrors `uzor_graph::agent`'s
/// own 2D `selection_json` exactly (`{count, indices (capped at 50),
/// collapsed_group}`).
fn selection_json_3d(engine3d: &Engine3D) -> Value {
    const MAX_REPORTED_INDICES: usize = 50;
    let indices: Vec<u32> = engine3d.selection.iter().take(MAX_REPORTED_INDICES).map(|n| n.0).collect();
    json!({
        "count": engine3d.selection.len(),
        "indices": indices,
        "collapsed_group": engine3d.selection_collapsed_group().map(|id| id.0),
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
        // Placeholder empty graphs — immediately overwritten by
        // `rebuild_engines` below, which is also `set_fixture`'s own
        // implementation (exactly ONE "build a fixture into these
        // engines" code path, see its own doc comment). `seed_positions`
        // fixes the z-plane-degeneracy defect that a manual `p.x = x;
        // p.y = y;` loop would reintroduce — `build_fixture`'s positions
        // are 2D-only (`z` stays at `Particle::default()`'s `0.0` for
        // every node), and every 3D force is z-symmetric, so a naive
        // x/y-only seed would leave the sim permanently confined to the
        // z = 0 plane. See `uzor-graph/CLAUDE.md`'s divergence log and
        // `GraphEngine3D::ensure_z_variance`.
        let engine = Arc::new(Mutex::new(GraphEngine::new(DemoGraph::new(), GraphLayoutMode::default())));
        let engine3d = Arc::new(Mutex::new(Engine3D::new(DemoGraph::new(), ForceDirectedLayout3D::default())));
        let camera_fit = CameraFitFlag::new();
        let fixture = FixtureState::new();
        rebuild_engines(&engine, &engine3d, fixture.get(), &camera_fit);

        Self {
            engine,
            engine3d,
            dim: DimState::new(),
            fixture,
            camera_fit,
            nav_mode: NavModeState::new(),
            mouse_look: MouseLookState::new(),
            fly: Arc::new(Mutex::new(FlyController::new())),
            frame_profiler: Arc::new(Mutex::new(FrameProfiler::default())),
            last_3d_frame_at: None,
            last_3d_surface_px: SurfaceSizeState::new(),
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

    fn lock_fly(fly: &Arc<Mutex<FlyController>>) -> MutexGuard<'_, FlyController> {
        match fly.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn lock_profiler(profiler: &Arc<Mutex<FrameProfiler>>) -> MutexGuard<'_, FrameProfiler> {
        match profiler.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Toggle between orbit and fly navigation (Tab, 3D only) — always
    /// stops the `FlyController` on either transition so held keys/
    /// inertia can never leak across the boundary (mirrors the source
    /// app's own `stop_movement`/`release_viewport_cursor` discipline on
    /// any navigation-mode change).
    fn toggle_nav_mode(&mut self) {
        let next = match self.nav_mode.get() {
            NavMode::Orbit => NavMode::Fly,
            NavMode::Fly => NavMode::Orbit,
        };
        self.nav_mode.set(next);
        // Entering fly always starts in LOOK state (crosshair armed) —
        // the owner's expected "Tab drops me straight into mouse-look"
        // flow; a stale OFF from the previous fly session would read as
        // a broken toggle.
        self.mouse_look.set(true);
        Self::lock_fly(&self.fly).stop();
    }

    /// Fly-mode event routing (3D + `NavMode::Fly` only) — WASD/arrow
    /// `KeyDown`/`KeyUp` feed the `FlyController`'s held intent;
    /// `PointerDelta` applies free look (the app requests
    /// `CursorCaptureMode::LockedHidden` while flying, see
    /// `App::cursor_capture_mode` below, so the OS cursor is already
    /// locked/hidden by the time these deltas arrive). Anything the
    /// controller doesn't recognize (zoom via scroll, modifier tracking,
    /// ...) still falls through to `GraphEngine3D::on_event` unchanged.
    fn on_event_fly(&mut self, event: &PlatformEvent) -> bool {
        match event {
            PlatformEvent::KeyDown { key, .. } => {
                if Self::lock_fly(&self.fly).set_key(*key, true) {
                    return true;
                }
            }
            PlatformEvent::KeyUp { key, .. } => {
                if Self::lock_fly(&self.fly).set_key(*key, false) {
                    return true;
                }
            }
            // Owner order 2026-07-19 — the foxhound source app's own
            // "MMB CLICK: CURSOR / LOOK" mechanic the original lift
            // skipped: a middle-click toggles mouse-look (crosshair +
            // captured cursor <-> ordinary free cursor). Both halves of
            // the click are consumed so the middle-drag PAN gesture the
            // 3D engine would otherwise start can never fire while
            // flying — in fly mode MMB IS the look toggle, nothing else.
            PlatformEvent::PointerDown { button: MouseButton::Middle, .. } => {
                self.mouse_look.toggle();
                return true;
            }
            PlatformEvent::PointerUp { button: MouseButton::Middle, .. } => {
                return true;
            }
            PlatformEvent::PointerDelta { dx, dy } => {
                // Deltas only ever arrive while the cursor is actually
                // captured, but the look gate is checked anyway so a
                // straggler delta from the release frame can't turn the
                // camera after the crosshair is already gone.
                if self.mouse_look.get() {
                    let mut engine3d = Self::lock3d(&self.engine3d);
                    Self::lock_fly(&self.fly).apply_look_delta(*dx as f32, *dy as f32, &mut engine3d.camera);
                }
                return true;
            }
            PlatformEvent::WindowFocused(false) => {
                Self::lock_fly(&self.fly).stop();
                return false;
            }
            _ => {}
        }
        let mut engine3d = Self::lock3d(&self.engine3d);
        engine3d.on_event(event, self.dim3d_viewport())
    }

    /// The REAL 3D viewport (Wave 3) — the last surface size `scene3d()`
    /// was actually called with, or [`full_window_viewport`]'s
    /// placeholder before the very first 3D frame has rendered (there's
    /// nothing better to gate `on_pointer_down`'s `contains()` check
    /// against yet, and no picking has anything to hit before then
    /// either).
    fn dim3d_viewport(&self) -> Rect {
        viewport_from_surface_size(self.last_3d_surface_px.get())
    }

    /// Wave 5 fit-to-bounds — recenters/redistances the 3D camera to
    /// frame the whole graph, keeping the current yaw/pitch. The demo-level
    /// entry point both the Home/F keyboard shortcut (`App::on_event`
    /// below) and the `fit_view_3d` agent action
    /// (`DemoBlackbox::apply_agent_action`) call.
    fn fit_view_3d(&self) {
        let aspect = surface_aspect(self.last_3d_surface_px.get());
        Self::lock3d(&self.engine3d).fit_view(aspect);
    }

    /// Wave 5 ground-reference grid toggle — the demo-level entry point
    /// both the G keyboard shortcut and the `set_grid` agent action call.
    fn toggle_grid(&self) {
        let mut engine3d = Self::lock3d(&self.engine3d);
        let next = !engine3d.grid_enabled();
        engine3d.set_grid_enabled(next);
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
                    // Cluster/selection wave — same `{count, indices
                    // capped 50, collapsed_group}` shape `uzor_graph::agent`'s
                    // own 2D `selection_json` reports.
                    "selection": selection_json_3d(&engine3d),
                    // Filter/local-subgraph wave — same field names/shapes
                    // `uzor_graph::agent`'s own 2D `agent_state` reports.
                    "local_root": engine3d.local_root().map(|(node, depth)| json!({ "index": node.index(), "depth": depth })),
                    "filter": engine3d.filter().map(filter_json_3d),
                })
            }
        };
        if let Value::Object(ref mut map) = state {
            map.insert("dimension".to_owned(), json!(self.dim.get().code()));
            map.insert("fixture".to_owned(), json!(self.fixture.get().as_str()));
            map.insert("nav_mode".to_owned(), json!(self.nav_mode.get().as_str()));
            map.insert("mouse_look".to_owned(), json!(self.mouse_look.get()));
            let fly_velocity = DemoApp::lock_fly(&self.fly).velocity();
            map.insert("fly_velocity".to_owned(), json!([fly_velocity[0], fly_velocity[1]]));
            // Wave 5 — always reported regardless of which dimension is
            // active (same convention `fixture`/`nav_mode` already use):
            // the grid toggle lives on `engine3d`, but it's meaningful to
            // query even while 2D is the active dimension.
            map.insert("grid".to_owned(), json!(DemoApp::lock3d(&self.engine3d).grid_enabled()));
            // 2026-07-22 (3D-parity-arc tail) — same field name the
            // foxhound source app's own `FrameProfile` publishes under
            // (`uzor::framework::frame_profiler::FrameProfiler::to_json`'s
            // own doc comment), reported unconditionally like `fixture`/
            // `nav_mode`/`grid` above: `scene3d()` only ever records into
            // it while 3D is active, but it's meaningful to query either
            // way (an empty `stages` map, `frames: 0`, before the first
            // 3D frame ever renders).
            map.insert("frame_profile_ema_ms".to_owned(), DemoApp::lock_profiler(&self.frame_profiler).to_json());
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
        if action.name == "set_nav_mode" {
            let Some(mode) = action.args.get("mode").and_then(Value::as_str).and_then(NavMode::from_str) else {
                return AgentActionReply::err("set_nav_mode requires args.mode to be one of: orbit, fly");
            };
            self.nav_mode.set(mode);
            // Mirror `DemoApp::toggle_nav_mode` — entering fly always
            // starts in LOOK state (crosshair armed).
            self.mouse_look.set(true);
            DemoApp::lock_fly(&self.fly).stop();
            return AgentActionReply::ok_with_log(json!({ "nav_mode": mode.as_str(), "mouse_look": true }));
        }
        if action.name == "set_mouse_look" {
            // Headless twin of the middle-click toggle (owner order
            // 2026-07-19) — lets an agent flip look/cursor state and
            // verify the crosshair via a screenshot without synthesizing
            // a real MMB click.
            let Some(on) = action.args.get("on").and_then(Value::as_bool) else {
                return AgentActionReply::err("set_mouse_look requires args.on to be true or false");
            };
            self.mouse_look.set(on);
            return AgentActionReply::ok_with_log(json!({ "mouse_look": on }));
        }
        if action.name == "set_fixture" {
            let Some(fixture) = action.args.get("name").and_then(Value::as_str).and_then(Fixture::from_str) else {
                return AgentActionReply::err("set_fixture requires args.name to be one of: clusters, tree, hierarchy, sparse");
            };
            self.fixture.set(fixture);
            rebuild_engines(&self.engine, &self.engine3d, fixture, &self.camera_fit);
            return AgentActionReply::ok_with_log(json!({ "fixture": fixture.as_str() }));
        }
        if action.name == "fit_view_3d" {
            // Wave 5 — headless twin of the Home/F keyboard shortcut,
            // driven from the agent-api HTTP thread; uses the last
            // surface size `scene3d()` recorded (fallback 16:9 before the
            // very first 3D frame — see `surface_aspect`).
            let aspect = surface_aspect(self.surface_size.get());
            DemoApp::lock3d(&self.engine3d).fit_view(aspect);
            return AgentActionReply::ok_with_log(json!({ "fit_view_3d": true }));
        }
        if action.name == "set_grid" {
            let Some(on) = action.args.get("on").and_then(Value::as_bool) else {
                return AgentActionReply::err("set_grid requires args.on to be true or false");
            };
            DemoApp::lock3d(&self.engine3d).set_grid_enabled(on);
            return AgentActionReply::ok_with_log(json!({ "grid": on }));
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
    /// not a silent clear.
    ///
    /// **Cluster/selection wave**: `collapse`/`expand` (mirrors 2D's own
    /// args shape, `{"cluster": <u32 GroupId>}`) and `select_nodes`/
    /// `box_select`/`collapse_selection`/`pin_selection`/`unpin_selection`
    /// (mirror the 2D arg shapes exactly, `uzor_graph::agent`'s own doc
    /// comments) now also forward onto `engine3d`. `box_select` needs a
    /// real viewport this wrapper doesn't itself track — it reads
    /// [`DemoBlackbox::surface_size`], the SAME last-known 3D surface size
    /// [`DemoApp::dim3d_viewport`] reads from a separate `Arc` clone (see
    /// [`viewport_from_surface_size`]'s own doc comment).
    ///
    /// **Filter/local-subgraph wave (3D interaction parity, item 4)**:
    /// `set_local_root`/`set_filter` (mirror the 2D arg shapes exactly,
    /// `uzor_graph::agent`'s own doc comments) now also forward onto
    /// `engine3d`. `set_local_root` additionally calls
    /// `engine3d.fit_view(...)` right after, with the SAME real surface
    /// aspect [`DemoApp::fit_view_3d`] uses — `GraphEngine3D::
    /// set_local_root` itself does NOT auto-fit (documented on that
    /// method), so the demo/caller layer does it instead, mirroring 2D's
    /// own "activation frames the local neighborhood" convention.
    /// Everything else stays a typed rejection — pin_node/unpin_node
    /// remain out of this arc.
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
                engine3d.select(node);
                AgentActionReply::ok_with_log(json!({ "selected": node.index() }))
            }
            "clear_selection" => {
                engine3d.clear_selection();
                AgentActionReply::ok_with_log(json!({ "selected": Value::Null }))
            }
            "select_nodes" => {
                let Some(indices) = action.args.get("indices").and_then(Value::as_array) else {
                    return AgentActionReply::err("select_nodes requires args.indices (array of u32 node indices)");
                };
                let mode = match parse_select_mode_3d(&action) {
                    Ok(m) => m,
                    Err(e) => return AgentActionReply::err(e),
                };
                let mut nodes = Vec::with_capacity(indices.len());
                for v in indices {
                    let Some(idx) = v.as_u64() else {
                        return AgentActionReply::err("select_nodes args.indices entries must all be u32");
                    };
                    let node = NodeIndex(idx as u32);
                    if node.index() >= engine3d.graph.node_count() {
                        return AgentActionReply::err(format!("select_nodes index {idx} out of range"));
                    }
                    nodes.push(node);
                }
                engine3d.apply_selection(nodes, mode);
                AgentActionReply::ok_with_log(json!({ "selection": selection_json_3d(&engine3d) }))
            }
            "box_select" => {
                let coord = |k: &str| action.args.get(k).and_then(Value::as_f64);
                let (Some(x0), Some(y0), Some(x1), Some(y1)) = (coord("x0"), coord("y0"), coord("x1"), coord("y1")) else {
                    return AgentActionReply::err("box_select requires args.x0/y0/x1/y1 (f64 screen coords)");
                };
                let mode = match parse_select_mode_3d(&action) {
                    Ok(m) => m,
                    Err(e) => return AgentActionReply::err(e),
                };
                let viewport = viewport_from_surface_size(self.surface_size.get());
                engine3d.box_select((x0, y0), (x1, y1), mode, viewport);
                AgentActionReply::ok_with_log(json!({ "selection": selection_json_3d(&engine3d) }))
            }
            "collapse_selection" => match engine3d.collapse_selection() {
                Some(id) => AgentActionReply::ok_with_log(json!({ "collapsed": id.0 })),
                None => AgentActionReply::err("collapse_selection requires a non-empty selection"),
            },
            "pin_selection" => {
                engine3d.pin_selection();
                AgentActionReply::ok_with_log(json!({ "selection": selection_json_3d(&engine3d) }))
            }
            "unpin_selection" => {
                engine3d.unpin_selection();
                AgentActionReply::ok_with_log(json!({ "selection": selection_json_3d(&engine3d) }))
            }
            "collapse" => {
                let Some(id) = resolve_cluster_3d(&action) else {
                    return AgentActionReply::err("collapse requires args.cluster (u32 GroupId)");
                };
                if engine3d.collapse_cluster(id) {
                    AgentActionReply::ok_with_log(json!({ "collapsed": id.0 }))
                } else {
                    AgentActionReply::err(format!("cluster {} not found or already collapsed", id.0))
                }
            }
            "expand" => {
                let Some(id) = resolve_cluster_3d(&action) else {
                    return AgentActionReply::err("expand requires args.cluster (u32 GroupId)");
                };
                if engine3d.expand_cluster(id) {
                    AgentActionReply::ok_with_log(json!({ "expanded": id.0 }))
                } else {
                    AgentActionReply::err(format!("cluster {} not found or not collapsed", id.0))
                }
            }
            // Filter/local-subgraph wave (3D interaction parity, item 4)
            // — mirrors `uzor_graph::agent`'s own 2D `set_local_root`
            // action arg shape exactly (`{}`/`{"index": null}` clears,
            // same convention `hover_node`/`select_node` already use).
            // `GraphEngine3D::set_local_root` itself does NOT auto-fit
            // (it owns no viewport — see its own doc comment); the demo
            // is the caller with a REAL surface size, so it fits here,
            // mirroring 2D's own "activation frames the local
            // neighborhood" convention at this layer instead.
            "set_local_root" => {
                let index_arg = action.args.get("index");
                let explicit_clear =
                    matches!(index_arg, Some(Value::Null)) || (index_arg.is_none() && action.args.get("label").is_none());
                if explicit_clear {
                    engine3d.set_local_root(None, None);
                    engine3d.fit_view(surface_aspect(self.surface_size.get()));
                    return AgentActionReply::ok_with_log(json!({ "local_root": Value::Null }));
                }
                let Some(node) = resolve_node_3d(&engine3d, &action) else {
                    return AgentActionReply::err(
                        "set_local_root requires args.index (u32), args.label (string), or {} / null to clear",
                    );
                };
                let depth = action.args.get("depth").and_then(Value::as_u64).map(|d| d as u8);
                engine3d.set_local_root(Some(node), depth);
                engine3d.fit_view(surface_aspect(self.surface_size.get()));
                AgentActionReply::ok_with_log(json!({
                    "local_root": engine3d.local_root().map(|(n, d)| json!({ "index": n.index(), "depth": d })),
                }))
            }
            // Mirrors `uzor_graph::agent`'s own 2D `set_filter` action arg
            // shape exactly — an args object with NONE of the 3
            // recognized clauses clears the filter.
            "set_filter" => {
                let has_clause = ["label_substring", "categories", "min_degree"].iter().any(|k| action.args.get(*k).is_some());
                if !has_clause {
                    engine3d.set_filter(None);
                    return AgentActionReply::ok_with_log(json!({ "filter": Value::Null }));
                }
                let label_substring = action.args.get("label_substring").and_then(Value::as_str).map(str::to_owned);
                let categories = action
                    .args
                    .get("categories")
                    .and_then(Value::as_array)
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect::<Vec<String>>());
                let min_degree = action.args.get("min_degree").and_then(Value::as_u64).map(|v| v as u32);
                let spec = FilterSpec { label_substring, categories, min_degree };
                engine3d.set_filter(Some(spec));
                AgentActionReply::ok_with_log(json!({ "filter": engine3d.filter().map(filter_json_3d) }))
            }
            _ => AgentActionReply::err(
                "3D dimension supports hover_node/select_node/clear_selection/select_nodes/box_select/collapse_selection/pin_selection/unpin_selection/collapse/expand/set_local_root/set_filter besides set_dimension",
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
        let blackbox = DemoBlackbox {
            engine: self.engine.clone(),
            engine3d: self.engine3d.clone(),
            dim: self.dim.clone(),
            fixture: self.fixture.clone(),
            camera_fit: self.camera_fit.clone(),
            nav_mode: self.nav_mode.clone(),
            mouse_look: self.mouse_look.clone(),
            fly: self.fly.clone(),
            frame_profiler: self.frame_profiler.clone(),
            surface_size: self.last_3d_surface_px.clone(),
        };
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

            if self.camera_fit.needs_fit() && canvas_rect.width > 0.0 {
                engine.fit_view();
                self.camera_fit.clear();
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
        // `Tab` toggles orbit/fly (item 5 — a human-owner keyboard
        // shortcut alongside the `set_nav_mode` agent action); only
        // meaningful while 3D is active, since orbit/fly are both purely
        // 3D camera-control concepts.
        if self.dim.get() == Dimension::ThreeD {
            if let PlatformEvent::KeyDown { key: KeyCode::Tab, .. } = event {
                self.toggle_nav_mode();
                return true;
            }
            // Wave 5 fit-to-bounds — Home or F, in EITHER nav mode
            // (orbit or fly): frame the whole graph, keeping the current
            // yaw/pitch. Checked here, ahead of the orbit/fly dispatch
            // below, so it works regardless of which nav mode is active.
            if let PlatformEvent::KeyDown { key: KeyCode::Home | KeyCode::F, .. } = event {
                self.fit_view_3d();
                return true;
            }
            // Wave 5 ground-reference grid toggle — G, in either nav mode.
            if let PlatformEvent::KeyDown { key: KeyCode::G, .. } = event {
                self.toggle_grid();
                return true;
            }
        }
        match self.dim.get() {
            Dimension::TwoD => {
                let mut engine = Self::lock(&self.engine);
                engine.on_event(event)
            }
            Dimension::ThreeD => match self.nav_mode.get() {
                NavMode::Orbit => {
                    let mut engine3d = Self::lock3d(&self.engine3d);
                    engine3d.on_event(event, self.dim3d_viewport())
                }
                NavMode::Fly => self.on_event_fly(event),
            },
        }
    }

    /// While 3D + fly navigation is active, requests a locked/hidden OS
    /// cursor so `PlatformEvent::PointerDelta` carries raw mouse-look
    /// motion (`uzor-desktop::Manager::sync_cursor_capture` handles the
    /// actual OS grab, plus its own focus-loss safety net — item 3 of
    /// this same lift). Every other state (2D, or 3D orbit) stays
    /// `Free`, matching the pre-existing default.
    fn cursor_capture_mode(&self) -> CursorCaptureMode {
        if self.dim.get() == Dimension::ThreeD
            && self.nav_mode.get() == NavMode::Fly
            && self.mouse_look.get()
        {
            CursorCaptureMode::LockedHidden
        } else {
            CursorCaptureMode::Free
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
        self.last_3d_surface_px.set(surf_w, surf_h);
        let now = std::time::Instant::now();
        let dt = match self.last_3d_frame_at {
            Some(prev) => now.duration_since(prev).as_secs_f32().min(0.1),
            None => 1.0 / 60.0,
        };
        self.last_3d_frame_at = Some(now);

        let mut engine3d = Self::lock3d(&self.engine3d);
        // 2026-07-22 (3D-parity-arc tail) — `Instant`-bracket `tick`/
        // `build_scene` independently and feed both into `frame_profiler`,
        // mirroring foxhound's own `FrameProfile` stage-naming
        // (`uzor::framework::frame_profiler`'s own module doc).
        let tick_started = std::time::Instant::now();
        engine3d.tick(dt);
        let tick_ms = tick_started.elapsed().as_secs_f64() * 1000.0;
        if self.nav_mode.get() == NavMode::Fly {
            Self::lock_fly(&self.fly).tick(dt, &mut engine3d.camera);
        }
        let scene_build_started = std::time::Instant::now();
        let scene = engine3d.build_scene(surf_h as f64);
        let scene_build_ms = scene_build_started.elapsed().as_secs_f64() * 1000.0;
        let aspect = surf_w as f32 / (surf_h.max(1) as f32);
        let camera = engine3d.camera(aspect);
        drop(engine3d);

        {
            let mut profiler = Self::lock_profiler(&self.frame_profiler);
            profiler.record_ms("tick", tick_ms);
            profiler.record_ms("scene_build", scene_build_ms);
            profiler.end_frame();
        }

        // Wave 4 (W3D arc plan §1.3 label-overlay gap, closed here): hand
        // `Manager` a real 2D overlay closure — node labels + the hover
        // info card, painted ON TOP of the composed 3D frame this same
        // tick via `GraphEngine3D::draw_overlay`. See
        // `uzor-graph/CLAUDE.md`'s Wave 4 divergence log for why this
        // couldn't land in Wave 3 (no post-3D render-hub surface existed
        // yet — `uzor-render-hub::compose`'s new Phase 4.5 is that
        // surface). The closure re-locks `engine3d` when `Manager`
        // actually calls it (immediately after this `scene3d()` call
        // returns, same tick, before the next frame) — `camera` is
        // `Copy` so capturing it here doesn't disturb the `camera` value
        // returned below in `Scene3DFrame`.
        let engine3d_for_overlay = self.engine3d.clone();
        let overlay_viewport = Rect::new(0.0, 0.0, surf_w as f64, surf_h as f64);
        let crosshair_armed = self.nav_mode.get() == NavMode::Fly && self.mouse_look.get();
        let overlay: Box<dyn FnMut(&mut dyn RenderContext)> = Box::new(move |ctx: &mut dyn RenderContext| {
            let engine3d = Self::lock3d(&engine3d_for_overlay);
            engine3d.draw_overlay(ctx, &camera, overlay_viewport);
            if crosshair_armed {
                draw_fly_crosshair(ctx, overlay_viewport);
            }
        });

        Some(Scene3DFrame { scene, camera, cached_overlay: None, overlay: Some(overlay) })
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

// ── Tests: demo fixture builders (deterministic shapes) ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clusters_fixture_matches_the_original_534_node_shape() {
        let (graph, positions, cluster_members) = build_clusters_graph();
        // NUM_CLUSTERS * (CLUSTER_SIZE members + 1 hub) = 6 * 89 = 534.
        assert_eq!(graph.node_count(), NUM_CLUSTERS * (CLUSTER_SIZE + 1));
        assert_eq!(positions.len(), graph.node_count());
        assert_eq!(cluster_members.len(), NUM_CLUSTERS);
        assert!(cluster_members.iter().all(|m| m.len() == CLUSTER_SIZE));
    }

    #[test]
    fn tree_fixture_is_connected_acyclic_and_reaches_the_requested_depth() {
        let (graph, positions, depths) = build_tree_internal(TREE_NODE_BUDGET, "t");
        assert!(graph.node_count() > 100, "expected a few hundred nodes, got {}", graph.node_count());
        assert!(graph.node_count() <= TREE_NODE_BUDGET);
        assert_eq!(positions.len(), graph.node_count());
        assert_eq!(depths.len(), graph.node_count());
        // A tree has exactly n-1 edges — no cross-links yet.
        assert_eq!(graph.edge_count(), graph.node_count() - 1, "the plain tree fixture must have zero cross-links (n-1 edges)");
        let max_depth = depths.iter().copied().max().unwrap_or(0);
        assert!(
            max_depth >= TREE_MIN_BRANCH_DEPTH,
            "expected the tree to reach at least {TREE_MIN_BRANCH_DEPTH} branching levels, got max depth {max_depth}"
        );
        assert!(max_depth <= TREE_MAX_DEPTH);
        // Root is depth 0 and unique.
        assert_eq!(depths.iter().filter(|&&d| d == 0).count(), 1);
    }

    #[test]
    fn tree_fixture_via_build_fixture_has_no_collapsible_clusters() {
        let (graph, positions, cluster_members) = build_tree_graph();
        assert!(graph.node_count() > 100);
        assert_eq!(positions.len(), graph.node_count());
        assert!(cluster_members.is_empty(), "the tree fixture has no cluster-collapse concept");
    }

    #[test]
    fn hierarchy_fixture_adds_roughly_5_percent_cross_links_over_the_underlying_tree() {
        let (graph, positions, cluster_members) = build_hierarchy_graph();
        assert_eq!(positions.len(), graph.node_count());
        assert!(cluster_members.is_empty());
        let n = graph.node_count();
        let tree_edges = n - 1;
        let cross_links = graph.edge_count() - tree_edges;
        // HIERARCHY_CROSS_LINK_RATIO applies per-node with a coin flip,
        // some flips land on leaves/singleton depth levels with no valid
        // candidate and are skipped — a generous ±60% tolerance band
        // around the nominal ratio proves cross-links actually landed
        // (not zero, not close to every node) without over-fitting the
        // exact RNG sequence.
        let expected = n as f32 * HIERARCHY_CROSS_LINK_RATIO;
        assert!(
            (cross_links as f32) > expected * 0.4 && (cross_links as f32) < expected * 1.6,
            "expected roughly {expected:.0} cross-link edges (~{:.0}% of {n} nodes), got {cross_links}",
            HIERARCHY_CROSS_LINK_RATIO * 100.0
        );
    }

    #[test]
    fn sparse_fixture_has_low_average_degree_and_a_wide_spatial_spread() {
        let (graph, positions, cluster_members) = build_sparse_graph();
        assert_eq!(graph.node_count(), SPARSE_NODE_COUNT);
        assert_eq!(positions.len(), SPARSE_NODE_COUNT);
        assert!(cluster_members.is_empty());

        let total_degree: u32 = (0..graph.node_count()).map(|i| graph.degree(NodeIndex(i as u32))).sum();
        let avg_degree = total_degree as f32 / graph.node_count() as f32;
        assert!(avg_degree < 2.0, "sparse fixture should be low-degree, got avg degree {avg_degree:.2}");

        // "shows long-range structure" — at least one edge must span a
        // large fraction of the whole layout's own spatial extent (not
        // just nearest-neighbor links).
        let max_extent = positions
            .iter()
            .flat_map(|&(x, y)| [x.abs(), y.abs()])
            .fold(0.0_f32, f32::max);
        let mut longest_edge = 0.0_f32;
        for (_, edge) in graph.edges() {
            let (ax, ay) = positions[edge.from.index()];
            let (bx, by) = positions[edge.to.index()];
            let d = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
            longest_edge = longest_edge.max(d);
        }
        assert!(
            longest_edge > max_extent * 0.5,
            "expected at least one long-range edge spanning >50% of the layout's own extent ({max_extent:.1}), longest was {longest_edge:.1}"
        );
    }

    #[test]
    fn every_fixture_is_deterministic_across_repeated_calls() {
        for fixture in [Fixture::Clusters, Fixture::Tree, Fixture::Hierarchy, Fixture::Sparse] {
            let (g1, p1, _) = build_fixture(fixture);
            let (g2, p2, _) = build_fixture(fixture);
            assert_eq!(g1.node_count(), g2.node_count(), "{fixture:?} node_count must be deterministic");
            assert_eq!(g1.edge_count(), g2.edge_count(), "{fixture:?} edge_count must be deterministic");
            assert_eq!(p1, p2, "{fixture:?} positions must be byte-identical across repeated calls");
            for (id, node) in g1.nodes() {
                let other = g2.nodes().nth(id.index()).expect("same node_count implies same index range").1;
                assert_eq!(node.label, other.label, "{fixture:?} label must be deterministic at index {}", id.index());
                assert_eq!(node.category, other.category, "{fixture:?} category must be deterministic at index {}", id.index());
            }
        }
    }

    #[test]
    fn fixture_name_round_trips_through_as_str_and_from_str() {
        for fixture in [Fixture::Clusters, Fixture::Tree, Fixture::Hierarchy, Fixture::Sparse] {
            assert_eq!(Fixture::from_str(fixture.as_str()), Some(fixture));
            assert_eq!(Fixture::from_code(fixture.code()), fixture);
        }
        assert_eq!(Fixture::from_str("not-a-real-fixture"), None);
    }

    #[test]
    fn fixture_state_defaults_to_clusters_and_get_reflects_the_last_set() {
        let state = FixtureState::new();
        assert_eq!(state.get(), Fixture::Clusters);
        state.set(Fixture::Sparse);
        assert_eq!(state.get(), Fixture::Sparse);
    }
}
