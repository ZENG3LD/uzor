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
//! Keybinds (owner control-HUD pass — see the control panel, docked
//! top-RIGHT in BOTH dimensions — owner defect fix 2026-07-23, it used
//! to float top-left in 3D while 2D extended the right sidebar — for
//! the same map with clickable buttons):
//! - `Tab` — toggle 2D <-> 3D (animated), works from EITHER dimension.
//! - `V` — toggle orbit/fly camera navigation (3D only).
//! - `Home` / `F` — fit view to the graph's bounds (either dimension).
//! - `G` — toggle the reference ground grid (3D only).
//! - `MMB` click (fly mode) — toggle captured cursor/look.
//! - `Esc` — extra escape hatch: releases fly mouse-look if captured.
//! - `Shift`+drag — box-select.
//! - `H` — show/hide the control-HUD panel (shown by default).
//!
//! `tree`/`hierarchy` load with the HUD's own LAYERED (hierarchical,
//! layered top-down) layout by default instead of FORCE — a tree-shaped
//! fixture used to always run through the force layout and ball up
//! instead of reading as a tree (owner defect fix 2026-07-23).
//! `clusters`/`sparse` keep FORCE as their own default. The default
//! applies to BOTH dimensions now (`GraphLayoutMode3D` — 2026-07-23 —
//! gave the 3D engine the same three runtime-switchable layouts 2D
//! already had: Force/Hierarchical/Radial). The HUD's LAYOUT section
//! (FORCE / LAYERED / RADIAL) shows all three in BOTH dimensions and
//! lets any fixture's layout be flipped by hand afterward. Re-clicking
//! the ALREADY-active button toggles: FORCE pauses/resumes the sim in
//! place (button reads "FORCE (PAUSED)", amber-highlighted, while
//! frozen); LAYERED/RADIAL (one-shot) re-run their layout and re-fit the
//! camera, a visibly honest action since a fresh recompute can genuinely
//! move every node. Clicking an INACTIVE button switches mode as before.
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
use uzor_desktop::{AppRun3D as _, Scene3DApp, Scene3DFrame};

use uzor_graph::interaction::fly::{FlyController, KEYBOARD_SENSITIVITY_MAX, KEYBOARD_SENSITIVITY_MIN, MOUSE_SENSITIVITY_MAX, MOUSE_SENSITIVITY_MIN};
use uzor_graph::{
    FilterSpec, Graph, GraphEngine, GraphEngine3D, GraphLayoutMode, GraphLayoutMode3D, GroupId, Layout, LayoutKind,
    NodeIndex, SelectMode, TransitionDirection,
};

const AGENT_PORT: u16 = 17481;
const BLACKBOX_SLOT: &str = "graph";
const SIDEBAR_SLOT: &str = "sidebar";
const SIDEBAR_WIDTH: f32 = 320.0;

/// Animated 2D<->3D dimension-transition duration — the owner's own spec
/// ("~600ms"). See [`DemoBlackbox::set_dimension_3d`]/
/// [`DemoBlackbox::set_dimension_2d`].
const DIMENSION_TRANSITION_MS: f64 = 600.0;

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

// `GraphLayoutMode`/`GraphLayoutMode3D` (not a bare
// `ForceDirectedLayout`/`ForceDirectedLayout3D`) so the `set_layout`
// agent action + the HUD's LAYOUT section have something to dispatch to
// — see `uzor-graph/src/layout/{mode,mode3d}.rs`.
type Engine = GraphEngine<(), (), GraphLayoutMode>;
type Engine3D = GraphEngine3D<(), (), GraphLayoutMode3D>;

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

/// Control-HUD panel visibility (owner defect fix, item 3 — "H hides/
/// shows it, default shown") — same cross-thread `Arc<AtomicBool>`
/// convention [`MouseLookState`] already established: read/written from
/// `DemoApp::on_event` on the winit thread, reported by
/// `DemoBlackbox::agent_state` on the agent-api HTTP thread.
#[derive(Clone)]
struct HudVisibleState(Arc<AtomicBool>);

impl HudVisibleState {
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

// ── Control HUD (owner defect fix — foxhound-style control panel) ───────
//
// Owner report: this demo was an agent-api test stand with nothing for a
// human — no dimension hotkey, no toggles/menu. This section builds a
// left/sidebar control panel mirroring `foxhound-app-shell-native`'s own
// HUD structure (title, FIXTURE/NAVIGATION button rows, SENSITIVITY
// sliders, MOUSE legend, STATUS line) at the APP level — `uzor-graph`
// itself gains no new API.
//
// **Placement, ONE consistent position — owner defect fix (2026-07-23):
// "тут слева, тут справа — что за хуйня"**. The panel used to float at a
// fixed top-LEFT origin in 3D while 2D extended the pre-existing
// RIGHT-docked sidebar (`SIDEBAR_SLOT`/`SIDEBAR_WIDTH`) — a real,
// reported left/right inconsistency, not a deliberate design choice
// worth keeping. Both dimensions now paint the SAME content-sized
// floating card (owner design order 2026-07-24: no "Graph" header
// strip, no title row, no dead full-height column — `HUD_MARGIN` air
// above/around it): 2D paints it directly into the docked
// `SIDEBAR_SLOT` edge area (the old `lm::sidebar` widget is gone); 3D —
// which has ZERO 2D chrome to extend (`Scene3DApp`'s own divergence
// log) — computes the equivalent right-docked card anchor from the
// last-known real 3D surface width and the SAME `SIDEBAR_WIDTH`/
// `HUD_MARGIN` constants (`surface_width_logical`/`DemoApp::hud_origin`'s
// own `Dimension::ThreeD` branch). Both dimensions share the exact same
// row/section LAYOUT (`build_hud_layout`), just a different `(origin_x,
// origin_y, width)` anchor.
//
// **Single source of truth, not stored-then-hit-tested**: [`HudLayout`]
// is a deterministic PURE function of a small state snapshot
// ([`HudSnapshot`]) plus an origin/width — recomputed fresh every paint
// AND every hit-test call (`DemoApp::hud_layout`), never cached from a
// previous frame. This is cheap (a few dozen `Rect` computations, no
// allocation-heavy work) and makes drift between "what's drawn" and
// "what's clickable" structurally impossible — they're always the exact
// same function call.
//
// **Coordinate-space reconciliation (item 3's own explicit ask)**: hit
// zones are stored/compared in LOGICAL pixels — the SAME space every
// `PlatformEvent::Pointer*` coordinate already arrives in
// (`uzor-window-desktop::EventMapper` divides physical by the real OS
// scale factor before this app ever sees an event, confirmed by that
// crate's own divergence log). The 3D floating panel, however, is
// painted through `Scene3DFrame::overlay`'s render context, which
// operates in the SAME 1:1 PHYSICAL-pixel space as `surf_w`/`surf_h`
// (per that field's own doc comment) — DIFFERENT from the logical event
// space whenever the OS scale factor isn't exactly `1.0`. Reconciled by
// scaling ONLY the DRAW call sites (`draw_hud_static`/`draw_hud_status`'s
// own `scale` parameter, `DemoApp::scale_factor`) — hit-testing itself
// needs no scale factor at all, since it stays entirely in logical space
// end to end. `DemoApp::scale_factor` defaults to `1.0` and is kept live
// via `PlatformEvent::ScaleFactorChanged` — see that field's own doc
// comment for the one known startup-value gap this leaves (still
// correct by construction for the common 100%-scale case). The 2D
// sidebar panel needs NO scaling at all (`scale: 1.0` always) — `ui()`'s
// `RenderContext` is already logical-native (the docked column rect it
// paints into is in the identical logical space `PointerDown` events
// arrive in).
const HUD_PAD: f64 = 12.0;
/// Empty-space margin around the floating HUD card (owner design order
/// 2026-07-24: the panel is NOT a full-height column and has NO title —
/// a content-sized card with air above/below/around it, docked toward
/// the right edge in both dimensions).
const HUD_MARGIN: f64 = 12.0;
const HUD_HEADING_H: f64 = 16.0;
const HUD_BUTTON_H: f64 = 27.0;
const HUD_BUTTON_GAP: f64 = 5.0;
const HUD_SECTION_GAP: f64 = 10.0;
const HUD_SLIDER_ROW_H: f64 = 32.0;
const HUD_TEXT_LINE_H: f64 = 16.0;
const HUD_STATUS_LINE_COUNT: usize = 2;

/// Fallback logical viewport width for deriving the 3D panel's
/// right-docked origin ([`surface_width_logical`]) before any real 3D
/// surface size is known yet — the window's own initial logical size
/// (`WindowSpec::new(...).size(1400, 900)`, see `main()`), the same
/// "no real viewport yet" convention [`full_window_viewport`]/
/// [`FALLBACK_SURFACE_ASPECT`] already established.
const FALLBACK_SURFACE_WIDTH_LOGICAL: f64 = 1400.0;

/// Logical viewport width for the CURRENT last-known 3D surface size —
/// physical px / OS scale factor, landing the right-docked HUD origin in
/// the SAME logical space `PlatformEvent::Pointer*` coordinates already
/// arrive in (this section's own module doc, "coordinate-space
/// reconciliation"). Falls back to [`FALLBACK_SURFACE_WIDTH_LOGICAL`]
/// before the first 3D frame has ever rendered.
fn surface_width_logical(size: Option<(u32, u32)>, scale_factor: f64) -> f64 {
    match size {
        Some((w, _)) if scale_factor > 0.0 => w as f64 / scale_factor,
        _ => FALLBACK_SURFACE_WIDTH_LOGICAL,
    }
}

/// Height sibling of [`surface_width_logical`] — backs the 3D card's
/// vertical centering (owner design order 2026-07-24: equal air above
/// and below the card). Fallback = the window's own initial logical
/// height (`WindowSpec::new(...).size(1400, 900)`).
const FALLBACK_SURFACE_HEIGHT_LOGICAL: f64 = 900.0;

fn surface_height_logical(size: Option<(u32, u32)>, scale_factor: f64) -> f64 {
    match size {
        Some((_, h)) if scale_factor > 0.0 => h as f64 / scale_factor,
        _ => FALLBACK_SURFACE_HEIGHT_LOGICAL,
    }
}

/// Vertically-centered card origin y for a viewport of `viewport_h`
/// logical px — equal empty space above and below the content-sized
/// card (owner design order 2026-07-24), floored at [`HUD_MARGIN`] so a
/// card taller than the window still starts with its top row reachable.
fn hud_card_y(viewport_h: f64, card_h: f64) -> f64 {
    ((viewport_h - card_h) / 2.0).max(HUD_MARGIN)
}

/// Which control the HUD panel currently exposes as a clickable button —
/// resolved by [`hit_button`], dispatched by [`DemoApp::apply_hud_control`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HudControl {
    Fixture(Fixture),
    /// LAYOUT section button — switches the ACTIVE dimension's own
    /// `GraphLayoutMode`/`GraphLayoutMode3D` kind (owner defect fix
    /// 2026-07-23: both dimensions expose the same three modes now). See
    /// [`build_hud_layout`]'s own LAYOUT section doc.
    SetLayout(LayoutKind),
    ToggleDimension,
    ToggleNavMode,
    FitView,
    ToggleGrid,
}

/// Which sensitivity slider a HUD hit-test resolved — see
/// [`hit_slider`]/[`DemoApp::apply_hud_slider`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HudSliderId {
    Keyboard,
    Mouse,
}

/// One laid-out, clickable button row — geometry (`rect`, LOGICAL px),
/// what it does (`control`), its drawn label, and whether it should
/// paint in the "active" highlighted style. `paused` is a further,
/// LAYOUT-section-only refinement of `active` (owner rule «если 1
/// вариант — должен тоглиться», generalized to re-click-toggles-pause for
/// FORCE) — always `false` for every other button.
struct HudButtonRect {
    control: HudControl,
    rect: Rect,
    label: String,
    active: bool,
    paused: bool,
}

/// One laid-out sensitivity slider — `track` is the thin visual bar,
/// `hit` a taller (18px) surrounding hit-test band around it (matching
/// the foxhound reference app's own `TOOLBAR_SLIDER_HIT_HEIGHT`
/// convention — a bare 4px track would be nearly unclickable).
struct HudSliderRect {
    id: HudSliderId,
    track: Rect,
    hit: Rect,
    label: &'static str,
    value: f32,
    min: f32,
    max: f32,
}

/// One deterministic layout pass over the panel's own content — see
/// this section's module doc for why this is recomputed fresh rather
/// than cached. `legend`/`status_y` are LOGICAL-space anchors for
/// plain informational text (no hit-test needed for either).
struct HudLayout {
    panel: Rect,
    headings: Vec<(&'static str, f64)>,
    buttons: Vec<HudButtonRect>,
    sliders: Vec<HudSliderRect>,
    /// `(input label, action label, y)` — mirrors the foxhound
    /// reference app's own two-column `draw_toolbar_hint` convention.
    legend: Vec<(&'static str, &'static str, f64)>,
    /// y of the FIRST status text line; each subsequent line advances by
    /// [`HUD_TEXT_LINE_H`]. Exactly [`HUD_STATUS_LINE_COUNT`] lines are
    /// ever drawn there (`draw_hud_status`'s own contract).
    status_y: f64,
    /// y of the FIRST selection-facts line (SELECTION section — owner
    /// defect fix 2026-07-24, selection info lives INSIDE the panel now);
    /// exactly `HudSnapshot::selection_line_count` lines are drawn there,
    /// same line advance as `status_y`.
    selection_y: f64,
}

/// Small, cheap-to-snapshot slice of live app state the HUD layout/paint
/// needs — deliberately NOT the whole `DemoApp` (keeps `build_hud_layout`
/// a pure function, directly unit-testable with no engine/window setup).
#[derive(Clone, Copy)]
struct HudSnapshot {
    dim: Dimension,
    fixture: Fixture,
    nav_mode: NavMode,
    grid_enabled: bool,
    keyboard_sensitivity: f32,
    mouse_sensitivity: f32,
    /// The ACTIVE dimension's own layout kind (`engine.layout.kind()` in
    /// 2D, `engine3d.layout.kind()` in 3D — owner defect fix 2026-07-23:
    /// previously this always read the 2D engine regardless of `dim`,
    /// which is why 3D's LAYOUT section could only ever show a single
    /// hardcoded always-active FORCE button). See [`build_hud_layout`]'s
    /// own LAYOUT section doc.
    layout_kind: LayoutKind,
    /// The active dimension's own layout pause state — same source engine
    /// as `layout_kind` above; only meaningful (and only ever `true`)
    /// while `layout_kind == LayoutKind::Force`.
    layout_paused: bool,
    /// How many SELECTION-section text lines the layout must reserve
    /// space for — [`selection_lines_2d`]/[`selection_lines_3d`]'s own
    /// `.len()` for the active dimension (5 facts while a node is
    /// selected, 1 "no selection" hint line otherwise). Count only — the
    /// TEXT is per-frame dynamic, see [`HudLayout::selection_y`].
    selection_line_count: usize,
}

/// Build the panel's full layout at `(origin_x, origin_y)` with content
/// `width` — the ONE function both painting (`draw_hud_static`/
/// `draw_hud_status`) and hit-testing (`DemoApp::on_event_hud`) call, so
/// drawn and clickable geometry can never drift apart. Section order
/// mirrors the foxhound reference app's own HUD: FIXTURE (4
/// buttons, always), LAYOUT (ONLY [`available_layout_kinds`]'s modes for
/// the current fixture — tree/hierarchy get FORCE/LAYERED/RADIAL,
/// clusters/sparse get FORCE alone; re-clicking the ACTIVE button
/// toggles FORCE pause/resume or re-runs + re-fits a one-shot
/// LAYERED/RADIAL layout, see [`HudButtonRect::paused`] and
/// [`DemoApp::apply_hud_control`]), NAVIGATION (2-4 buttons — Orbit/Fly
/// and Grid are 3D only), SENSITIVITY (2 sliders, 3D **fly** mode only),
/// MOUSE (a per-mode legend), STATUS (2 numeric lines, filled in by the
/// caller — see [`draw_hud_status`]), SELECTION
/// (`HudSnapshot::selection_line_count` lines, also caller-filled).
fn build_hud_layout(origin_x: f64, origin_y: f64, width: f64, snap: &HudSnapshot) -> HudLayout {
    let content_w = (width - 2.0 * HUD_PAD).max(0.0);
    let mut y = origin_y + HUD_PAD;

    let mut headings: Vec<(&'static str, f64)> = Vec::new();
    let mut buttons: Vec<HudButtonRect> = Vec::new();
    let mut sliders: Vec<HudSliderRect> = Vec::new();

    // FIXTURE — always 4 buttons, one per deterministic demo shape.
    headings.push(("FIXTURE", y));
    y += HUD_HEADING_H;
    for fixture in [Fixture::Clusters, Fixture::Tree, Fixture::Hierarchy, Fixture::Sparse] {
        let rect = Rect::new(origin_x + HUD_PAD, y, content_w, HUD_BUTTON_H);
        buttons.push(HudButtonRect {
            control: HudControl::Fixture(fixture),
            rect,
            label: fixture.as_str().to_ascii_uppercase(),
            active: fixture == snap.fixture,
            paused: false,
        });
        y += HUD_BUTTON_H + HUD_BUTTON_GAP;
    }
    y += HUD_SECTION_GAP;

    // LAYOUT — ONLY the modes that genuinely exist for the current
    // fixture's data shape ([`available_layout_kinds`]'s own doc — the
    // owner rule «показывать только те режимы которые там есть»):
    // tree/hierarchy show FORCE/LAYERED/RADIAL, clusters/sparse show
    // FORCE alone (which still acts — re-clicking it toggles pause). A
    // button whose kind is already active AND is FORCE AND is currently
    // paused gets the distinct `paused` visual treatment (amber,
    // "(PAUSED)" label suffix) — see `draw_hud_static`'s own color table.
    headings.push(("LAYOUT", y));
    y += HUD_HEADING_H;
    for &kind in available_layout_kinds(snap.fixture) {
        let rect = Rect::new(origin_x + HUD_PAD, y, content_w, HUD_BUTTON_H);
        let active = kind == snap.layout_kind;
        let paused = active && kind == LayoutKind::Force && snap.layout_paused;
        let base = layout_kind_label(kind);
        let label = if paused { format!("{base} (PAUSED)") } else { base.to_owned() };
        buttons.push(HudButtonRect { control: HudControl::SetLayout(kind), rect, label, active, paused });
        y += HUD_BUTTON_H + HUD_BUTTON_GAP;
    }
    y += HUD_SECTION_GAP;

    // NAVIGATION — dimension toggle + Fit always show; Orbit/Fly and
    // Grid only while 3D is active (both are purely 3D camera concepts).
    headings.push(("NAVIGATION", y));
    y += HUD_HEADING_H;
    {
        let rect = Rect::new(origin_x + HUD_PAD, y, content_w, HUD_BUTTON_H);
        buttons.push(HudButtonRect {
            control: HudControl::ToggleDimension,
            rect,
            label: "2D / 3D  —  TAB".to_owned(),
            active: snap.dim == Dimension::ThreeD,
            paused: false,
        });
        y += HUD_BUTTON_H + HUD_BUTTON_GAP;
    }
    if snap.dim == Dimension::ThreeD {
        let rect = Rect::new(origin_x + HUD_PAD, y, content_w, HUD_BUTTON_H);
        buttons.push(HudButtonRect {
            control: HudControl::ToggleNavMode,
            rect,
            label: "ORBIT / FLY  —  V".to_owned(),
            active: snap.nav_mode == NavMode::Fly,
            paused: false,
        });
        y += HUD_BUTTON_H + HUD_BUTTON_GAP;
    }
    {
        let rect = Rect::new(origin_x + HUD_PAD, y, content_w, HUD_BUTTON_H);
        buttons.push(HudButtonRect {
            control: HudControl::FitView,
            rect,
            label: "FIT  —  HOME / F".to_owned(),
            active: false,
            paused: false,
        });
        y += HUD_BUTTON_H + HUD_BUTTON_GAP;
    }
    if snap.dim == Dimension::ThreeD {
        let rect = Rect::new(origin_x + HUD_PAD, y, content_w, HUD_BUTTON_H);
        buttons.push(HudButtonRect {
            control: HudControl::ToggleGrid,
            rect,
            label: "GRID  —  G".to_owned(),
            active: snap.grid_enabled,
            paused: false,
        });
        y += HUD_BUTTON_H + HUD_BUTTON_GAP;
    }
    y += HUD_SECTION_GAP;

    // SENSITIVITY — fly-mode-only section (FlyController's own
    // keyboard/mouse sensitivity scalars mean nothing outside fly nav).
    if snap.dim == Dimension::ThreeD && snap.nav_mode == NavMode::Fly {
        headings.push(("SENSITIVITY", y));
        y += HUD_HEADING_H;
        for (id, label, value, min, max) in [
            (HudSliderId::Keyboard, "KEYBOARD SPEED", snap.keyboard_sensitivity, KEYBOARD_SENSITIVITY_MIN, KEYBOARD_SENSITIVITY_MAX),
            (HudSliderId::Mouse, "MOUSE LOOK", snap.mouse_sensitivity, MOUSE_SENSITIVITY_MIN, MOUSE_SENSITIVITY_MAX),
        ] {
            let track = Rect::new(origin_x + HUD_PAD, y + 14.0, content_w, 4.0);
            let hit = Rect::new(track.x, track.y - 9.0, track.width, 18.0);
            sliders.push(HudSliderRect { id, track, hit, label, value, min, max });
            y += HUD_SLIDER_ROW_H;
        }
        y += HUD_SECTION_GAP;
    }

    // MOUSE — a per-(dimension, nav_mode) legend, plain text.
    headings.push(("MOUSE", y));
    y += HUD_HEADING_H;
    let legend_pairs: &[(&str, &str)] = match (snap.dim, snap.nav_mode) {
        (Dimension::TwoD, _) => &[("DRAG", "PAN / MOVE NODE"), ("WHEEL", "ZOOM"), ("SHIFT+DRAG", "BOX SELECT")],
        (Dimension::ThreeD, NavMode::Orbit) => {
            &[("LMB DRAG", "ORBIT / MOVE NODE"), ("MMB DRAG", "PAN"), ("WHEEL", "DOLLY"), ("SHIFT+DRAG", "BOX SELECT")]
        }
        (Dimension::ThreeD, NavMode::Fly) => &[("WASD", "MOVE"), ("MOUSE", "LOOK"), ("MMB CLICK", "CURSOR / LOOK")],
    };
    let mut legend = Vec::with_capacity(legend_pairs.len());
    for &(input, action) in legend_pairs {
        legend.push((input, action, y));
        y += HUD_TEXT_LINE_H;
    }
    y += HUD_SECTION_GAP;

    // STATUS — heading + [`HUD_STATUS_LINE_COUNT`] numeric lines, text
    // supplied by the caller (`draw_hud_status`) since it changes every
    // frame (node/visible counts, frame timing).
    headings.push(("STATUS", y));
    y += HUD_HEADING_H;
    let status_y = y;
    y += HUD_STATUS_LINE_COUNT as f64 * HUD_TEXT_LINE_H;
    y += HUD_SECTION_GAP;

    // SELECTION — the selected node's facts, INSIDE the panel (owner
    // defect fix 2026-07-24: these used to be loose text rows dangling
    // in the dead black area under the old sidebar-widget body). Line
    // COUNT comes from the snapshot so this layout stays a pure
    // function; line TEXT is per-frame dynamic and drawn by the caller
    // via `draw_hud_status` at `selection_y` (positions change while
    // dragging).
    headings.push(("SELECTION", y));
    y += HUD_HEADING_H;
    let selection_y = y;
    y += snap.selection_line_count as f64 * HUD_TEXT_LINE_H;
    y += HUD_PAD;

    HudLayout { panel: Rect::new(origin_x, origin_y, width, y - origin_y), headings, buttons, sliders, legend, status_y, selection_y }
}

/// Resolve a LOGICAL `(x, y)` against every button's own `rect` — the
/// one hit-test both `DemoApp::on_event_hud` and this module's own tests
/// use, so "what's clickable" can never diverge from `hit_button`'s own
/// logic living in two places.
fn hit_button(layout: &HudLayout, x: f64, y: f64) -> Option<HudControl> {
    layout.buttons.iter().find(|b| b.rect.contains(x, y)).map(|b| b.control)
}

/// Resolve a LOGICAL `(x, y)` against every slider's own (taller) `hit`
/// band.
fn hit_slider(layout: &HudLayout, x: f64, y: f64) -> Option<HudSliderId> {
    layout.sliders.iter().find(|s| s.hit.contains(x, y)).map(|s| s.id)
}

/// Uniformly scale every field of a `Rect` by `scale` — the ONE place
/// the logical-to-physical conversion for painting happens (see this
/// section's own module doc on coordinate-space reconciliation).
/// `scale == 1.0` is a pure no-op, exactly the 2D sidebar's own case.
fn scaled_rect(r: Rect, scale: f64) -> Rect {
    Rect::new(r.x * scale, r.y * scale, r.width * scale, r.height * scale)
}

/// Paint the card's chrome — background, border, section headings,
/// fixture/layout/navigation buttons (with active-state highlight),
/// sensitivity sliders, and the mouse legend. Painted fresh every frame
/// in BOTH dimensions (the former 3D `CachedOverlayJob` static/dynamic
/// split is gone — owner z-order rule 2026-07-24, «менюшка всегда выше
/// сцены»: render-hub blits a cached job UNDER the plain overlay, which
/// put scene labels above the card; see `DemoApp::scene3d`'s own
/// z-order comment). Does NOT draw the STATUS/SELECTION line content —
/// [`draw_hud_status`] paints those after this, above the card bg.
fn draw_hud_static(ctx: &mut dyn RenderContext, layout: &HudLayout, scale: f64) {
    // A content-sized floating CARD (owner design order 2026-07-24: no
    // title row, not a full-height column — air above/below it), with
    // its own background + border box.
    let panel = scaled_rect(layout.panel, scale);
    ctx.save();
    ctx.set_global_alpha(0.96);
    ctx.set_fill_color("#0b1019");
    ctx.fill_rect(panel.x, panel.y, panel.width, panel.height);
    ctx.set_global_alpha(1.0);
    ctx.set_stroke_color("#2e3a4d");
    ctx.set_stroke_width(1.0);
    ctx.stroke_rect(panel.x, panel.y, panel.width, panel.height);

    for &(label, y) in &layout.headings {
        ctx.set_font("bold 10px sans-serif");
        ctx.set_fill_color("#687b96");
        ctx.fill_text(label, panel.x + HUD_PAD * scale, (y + 11.0) * scale);
    }

    for b in &layout.buttons {
        let r = scaled_rect(b.rect, scale);
        // `paused` (LAYOUT section, active FORCE re-clicked) gets a
        // distinct amber palette — visually different from BOTH plain
        // "active/running" (green) and "inactive" (gray), so a frozen
        // sim reads as neither "off" nor "running normally".
        let (fill, stroke, text) = if b.paused {
            ("#3c3319", "#dba85c", "#e7c98b")
        } else if b.active {
            ("#193c31", "#75dba0", "#8be7b2")
        } else {
            ("#151d2a", "#36445a", "#b7c2d4")
        };
        ctx.set_fill_color(fill);
        ctx.fill_rect(r.x, r.y, r.width, r.height);
        ctx.set_stroke_color(stroke);
        ctx.set_stroke_width(1.0);
        ctx.stroke_rect(r.x, r.y, r.width, r.height);
        ctx.set_font("bold 10px sans-serif");
        ctx.set_fill_color(text);
        ctx.fill_text(&b.label, r.x + 8.0 * scale, r.y + r.height * 0.65);
    }

    for s in &layout.sliders {
        let track = scaled_rect(s.track, scale);
        let normalized = (((s.value - s.min) / (s.max - s.min)) as f64).clamp(0.0, 1.0);
        let knob_x = track.x + track.width * normalized;
        ctx.set_font("bold 9px sans-serif");
        ctx.set_fill_color("#b7c2d4");
        ctx.fill_text(s.label, track.x, track.y - 8.0 * scale);
        ctx.set_fill_color("#8be7b2");
        ctx.fill_text(&format!("{:.2}x", s.value), track.x + track.width - 32.0 * scale, track.y - 8.0 * scale);
        ctx.set_fill_color("#263246");
        ctx.fill_rect(track.x, track.y, track.width, track.height);
        ctx.set_fill_color("#4d90fe");
        ctx.fill_rect(track.x, track.y, knob_x - track.x, track.height);
        ctx.begin_path();
        ctx.arc(knob_x, track.y + track.height / 2.0, 4.0 * scale, 0.0, std::f64::consts::TAU);
        ctx.set_fill_color("#edf2fa");
        ctx.fill();
    }

    ctx.set_font("bold 10px sans-serif");
    for &(input, action, y) in &layout.legend {
        ctx.set_fill_color("#b7c2d4");
        ctx.fill_text(input, panel.x + HUD_PAD * scale, (y + 11.0) * scale);
        ctx.set_font("10px sans-serif");
        ctx.set_fill_color("#73839b");
        ctx.fill_text(action, panel.x + 90.0 * scale, (y + 11.0) * scale);
        ctx.set_font("bold 10px sans-serif");
    }
    ctx.restore();
}

/// Paint a run of dynamic text lines starting at `status_y` — the
/// DYNAMIC half of the split `draw_hud_static` documents. Used for BOTH
/// the STATUS section (exactly [`HUD_STATUS_LINE_COUNT`] lines) and the
/// SELECTION section (`HudSnapshot::selection_line_count` lines, at
/// `HudLayout::selection_y`) — the layout reserved exactly `lines.len()`
/// worth of vertical space in either case.
fn draw_hud_status(ctx: &mut dyn RenderContext, panel_x: f64, status_y: f64, lines: &[String], scale: f64) {
    ctx.save();
    ctx.set_font("11px sans-serif");
    ctx.set_fill_color("#aebbd0");
    let x = (panel_x + HUD_PAD) * scale;
    for (i, line) in lines.iter().enumerate() {
        let y = (status_y + i as f64 * HUD_TEXT_LINE_H + 11.0) * scale;
        ctx.fill_text(line, x, y);
    }
    ctx.restore();
}

/// The SELECTION section's text lines for the 2D engine — the same five
/// facts the old under-panel text rows showed (owner defect fix
/// 2026-07-24: they live INSIDE the panel now), or the single "no
/// selection" hint. Shared shape with [`selection_lines_3d`]; both the
/// snapshot (line COUNT, reserves layout space) and the per-frame draw
/// (line TEXT) call these, so reserved and drawn line counts can never
/// drift apart.
fn selection_lines_2d(engine: &Engine) -> Vec<String> {
    match engine.selected_facts() {
        Some(f) => vec![
            format!("selected: {}", f.label),
            format!("category: {}", f.category),
            format!("degree: {}", f.degree),
            format!("pos: ({:.1}, {:.1})", f.position.0, f.position.1),
            format!("pinned: {}", f.pinned),
        ],
        None => vec!["no selection — click a node".to_owned()],
    }
}

/// 3D sibling of [`selection_lines_2d`] — same five facts via
/// `GraphEngine3D::node_facts` (`NodeFacts` reports the 2D `(x, y)`
/// position shape by design, see that method's own doc), same hint line
/// when nothing is selected.
fn selection_lines_3d(engine3d: &Engine3D) -> Vec<String> {
    match engine3d.selected().and_then(|id| engine3d.node_facts(id)) {
        Some(f) => vec![
            format!("selected: {}", f.label),
            format!("category: {}", f.category),
            format!("degree: {}", f.degree),
            format!("pos: ({:.1}, {:.1})", f.position.0, f.position.1),
            format!("pinned: {}", f.pinned),
        ],
        None => vec!["no selection — click a node".to_owned()],
    }
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

/// Shared "an animated `Out` dimension transition is flattening toward
/// 2D" flag (dimension-transition wave) — same cross-thread
/// `Arc<Atomic*>` convention [`CameraFitFlag`]/[`DimState`] already
/// established. `DimState` itself stays `ThreeD` for the WHOLE duration
/// of an animated 3D->2D switch (the flattening volume is still `engine3d`'s
/// own scene) — this flag is the separate signal `DemoApp::scene3d`
/// polls every tick, on the winit thread, to know once the transition
/// completes (`GraphEngine3D::transition_active()` itself clears the
/// instant the ease finishes, losing which DIRECTION just completed) that
/// it must copy the settled 3D positions back into the 2D engine and
/// flip `DimState` to `TwoD` — see [`DemoBlackbox::set_dimension_2d`] and
/// [`DemoApp::scene3d`]'s own doc comments.
#[derive(Clone)]
struct FlattenPendingFlag(Arc<AtomicBool>);

impl FlattenPendingFlag {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    fn is_pending(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn request(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    fn clear(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Per-fixture default `GraphLayoutMode`/`GraphLayoutMode3D` kind (owner
/// defect fix — tree/hierarchy fixtures are tree-SHAPED graphs but always
/// ran through the force layout, which balls them up instead of reading
/// as a tree). `clusters`/`sparse` are genuinely mesh-shaped (a force
/// layout is the right default), `tree`/`hierarchy` are genuinely
/// hierarchical (a layered top-down layout is the right default) — this
/// mapping now applies to BOTH dimensions (`rebuild_engines` calls it for
/// `engine` AND `engine3d`, owner defect fix 2026-07-23 — previously 3D's
/// engine had no runtime-switchable layout at all to default). The
/// owner's own explicit HUD LAYOUT buttons (`HudControl::SetLayout`) let
/// this default be overridden by hand afterward — within
/// [`available_layout_kinds`]'s own per-fixture set; this is only the
/// INITIAL pick on a fixture switch, not a hard rule.
fn default_layout_kind_for_fixture(fixture: Fixture) -> LayoutKind {
    match fixture {
        Fixture::Clusters | Fixture::Sparse => LayoutKind::Force,
        Fixture::Tree | Fixture::Hierarchy => LayoutKind::Hierarchical,
    }
}

/// Which layout modes genuinely EXIST for a fixture's data shape — the
/// owner's HUD rule («показывать только те режимы которые там есть, чужие
/// не показывать»): `tree`/`hierarchy` are rooted hierarchical data, so
/// Layered/Radial are real modes for them; `clusters`/`sparse` are
/// mesh-shaped (cyclic, no root, no levels), so a layered/radial pass
/// over them isn't a mode that exists, just garbage rows — those
/// fixtures expose FORCE alone (a single-variant control still acts:
/// re-clicking it toggles pause, per [`DemoApp::apply_hud_control`]).
/// Identical in both dimensions (availability is a property of the DATA,
/// not of 2D vs 3D — both engines implement all three kinds). The HUD
/// (`build_hud_layout`), the click dispatcher (`apply_hud_control`) and
/// the agent-api `set_layout` gate (`DemoBlackbox::apply_agent_action`)
/// all consult this ONE function, so what's shown, what's clickable and
/// what's scriptable can never drift apart.
fn available_layout_kinds(fixture: Fixture) -> &'static [LayoutKind] {
    match fixture {
        Fixture::Clusters | Fixture::Sparse => &[LayoutKind::Force],
        Fixture::Tree | Fixture::Hierarchy => &[LayoutKind::Force, LayoutKind::Hierarchical, LayoutKind::Radial],
    }
}

/// Drawn HUD label for a layout kind — the LAYOUT section's button text.
fn layout_kind_label(kind: LayoutKind) -> &'static str {
    match kind {
        LayoutKind::Force => "FORCE",
        LayoutKind::Hierarchical => "LAYERED",
        LayoutKind::Radial => "RADIAL",
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
    // Owner defect fix — a tree-shaped fixture defaults to a tree-shaped
    // LAYOUT, not the force layout every fixture ran through before. See
    // `default_layout_kind_for_fixture`'s own doc comment; the camera-fit
    // request below (unconditional, pre-existing) re-frames the view for
    // whichever layout just got picked.
    new_engine.layout.set_kind(default_layout_kind_for_fixture(fixture));
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
    let mut new_engine3d = Engine3D::new(graph3d, GraphLayoutMode3D::default());
    new_engine3d.seed_positions(&positions3d);
    // Owner defect fix 2026-07-23 — the SAME per-fixture default now
    // applies to 3D too (`GraphLayoutMode3D` gave 3D real Hierarchical/
    // Radial modes to default INTO). `camera_fit.request()` below
    // (unconditional, pre-existing) re-frames BOTH dimensions after.
    new_engine3d.layout.set_kind(default_layout_kind_for_fixture(fixture));
    for members in cluster_members3d.iter().take(COLLAPSIBLE_CLUSTERS) {
        new_engine3d.define_cluster(members[..COLLAPSIBLE_CLUSTER_SIZE.min(members.len())].to_vec());
    }

    *DemoApp::lock(engine) = new_engine;
    *DemoApp::lock3d(engine3d) = new_engine3d;
    camera_fit.request();
}

/// Copy the 3D engine's CURRENT `(x, y)` positions into the 2D engine and
/// flip `dim` to `TwoD` (dimension-transition wave) — the "3D->2D handoff"
/// half of `set_dimension`, shared by [`DemoBlackbox::set_dimension_2d`]'s
/// instant (`animate: false`) path and [`DemoApp::scene3d`]'s own
/// animated-completion poll, so there is exactly ONE "flatten 3D into 2D"
/// code path regardless of which thread triggers it.
fn flatten_3d_into_2d(engine: &Arc<Mutex<Engine>>, engine3d: &Arc<Mutex<Engine3D>>, dim: &DimState) {
    let positions: Vec<(f32, f32)> = {
        let engine3d = DemoApp::lock3d(engine3d);
        engine3d.particles.iter().map(|p| (p.x, p.y)).collect()
    };
    DemoApp::lock(engine).seed_positions(&positions);
    dim.set(Dimension::TwoD);
}

/// Shared state-mutation core of the 2D->3D dimension switch — the SAME
/// logic `DemoBlackbox::set_dimension_3d` (the `set_dimension` agent
/// action) and `DemoApp::toggle_dimension` (the Tab keyboard shortcut,
/// owner defect fix item 2) both drive, so there's exactly ONE "flip to
/// 3D" code path regardless of which thread/input source triggers it.
/// See `DemoBlackbox::set_dimension_3d`'s own (now-forwarding) doc
/// comment for the full In/Out mechanics this implements.
fn apply_dimension_3d_transition(
    engine: &Arc<Mutex<Engine>>,
    engine3d: &Arc<Mutex<Engine3D>>,
    dim: &DimState,
    flatten_pending: &FlattenPendingFlag,
    animate: bool,
) {
    if dim.get() == Dimension::TwoD {
        let positions: Vec<(f32, f32)> = {
            let engine = DemoApp::lock(engine);
            engine.particles.iter().map(|p| (p.x, p.y)).collect()
        };
        DemoApp::lock3d(engine3d).seed_positions(&positions);
        dim.set(Dimension::ThreeD);
    }
    flatten_pending.clear();
    if animate {
        DemoApp::lock3d(engine3d).start_transition(TransitionDirection::In, DIMENSION_TRANSITION_MS);
    }
}

/// Shared state-mutation core of the 3D->2D dimension switch — see
/// [`apply_dimension_3d_transition`]'s own doc comment for why this is a
/// free fn shared by `DemoBlackbox::set_dimension_2d` and
/// `DemoApp::toggle_dimension`.
fn apply_dimension_2d_transition(
    engine: &Arc<Mutex<Engine>>,
    engine3d: &Arc<Mutex<Engine3D>>,
    dim: &DimState,
    flatten_pending: &FlattenPendingFlag,
    animate: bool,
) {
    if dim.get() == Dimension::ThreeD {
        if animate {
            flatten_pending.request();
            DemoApp::lock3d(engine3d).start_transition(TransitionDirection::Out, DIMENSION_TRANSITION_MS);
        } else {
            flatten_3d_into_2d(engine, engine3d, dim);
            flatten_pending.clear();
        }
    }
}

struct DemoApp {
    engine: Arc<Mutex<Engine>>,
    engine3d: Arc<Mutex<Engine3D>>,
    dim: DimState,
    /// Which deterministic graph shape is currently loaded — see
    /// [`Fixture`]/[`rebuild_engines`].
    fixture: FixtureState,
    camera_fit: CameraFitFlag,
    /// Deferred 3D fit — consumed in `scene3d()` AFTER the engine tick
    /// (one-shot layouts apply their positions during a tick; fitting
    /// before it frames the stale extent — the same ordering rule 2D's
    /// `camera_fit` already follows) and only while no dimension
    /// transition is interpolating the camera.
    fit3d_pending: CameraFitFlag,
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
    /// Dimension-transition wave — see [`FlattenPendingFlag`]'s own doc
    /// comment. Polled every `scene3d()` tick.
    flatten_pending: FlattenPendingFlag,
    /// Control-HUD panel visibility (owner defect fix, item 3/4) — see
    /// [`HudVisibleState`]'s own doc comment.
    hud_visible: HudVisibleState,
    /// Tracked OS DPI scale factor, winit-thread-only (both reader and
    /// writer are `App`/`Scene3DApp` methods `Manager` calls on the same
    /// thread every frame — unlike `DimState`/`NavModeState`/etc, this
    /// never needs to cross to the agent-api HTTP thread, so a plain
    /// field is correct here, not an `Arc<Atomic*>`). See the "Control
    /// HUD" section's own module doc for the full coordinate-space
    /// reconciliation this backs and its one known startup-value gap.
    scale_factor: f64,
    /// Last-known 2D sidebar body rect (`ui()`'s own `body_rect`,
    /// LOGICAL px) — `on_event`'s HUD hit-test needs the SAME rect the
    /// panel was actually painted into, but `on_event` and `ui()` run at
    /// different times within a frame; mirrors this file's own
    /// established "last-known value from the most recent paint" idiom
    /// ([`SurfaceSizeState`]/`dim3d_viewport`'s own convention). Defaults
    /// to a zero-size rect (`Rect::contains` on a zero rect only ever
    /// matches the single point `(0, 0)`) — the same honest "nothing
    /// painted yet, no click zones" answer `full_window_viewport`'s own
    /// doc comment already established for the 3D viewport case.
    last_sidebar_body: Rect,
    /// Which HUD button is currently pressed (`PointerDown` matched, not
    /// yet released) — mirrors the foxhound reference app's own
    /// press-then-release-confirm button convention (`pressed_control`):
    /// the action only actually fires on `PointerUp` if the cursor is
    /// STILL over the same button, so a drag-off cancels the click.
    hud_pressed_button: Option<HudControl>,
    /// Which HUD sensitivity slider is currently being dragged, if any.
    hud_dragging_slider: Option<HudSliderId>,
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
    fit3d_pending: CameraFitFlag,
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
    /// Same `flatten_pending` `Arc` `DemoApp` owns — see
    /// [`FlattenPendingFlag`]'s own doc comment.
    flatten_pending: FlattenPendingFlag,
    /// Same `hud_visible` `Arc` `DemoApp` owns — see
    /// [`HudVisibleState`]'s own doc comment.
    hud_visible: HudVisibleState,
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

/// Stable lowercase string form for `LayoutKind` — mirrors
/// `uzor_graph::agent`'s own private `layout_mode_name`'s match arms
/// (re-implemented here since `GraphEngine3D` has no `BlackboxAgentSurface`
/// impl of its own to reuse that helper through), the 3D `agent_state`'s
/// `"layout"` field + the `set_layout` action's own reply both use this.
fn layout_kind_str(kind: LayoutKind) -> &'static str {
    match kind {
        LayoutKind::Force => "force",
        LayoutKind::Hierarchical => "hierarchical",
        LayoutKind::Radial => "radial",
    }
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
        let engine3d = Arc::new(Mutex::new(Engine3D::new(DemoGraph::new(), GraphLayoutMode3D::default())));
        let camera_fit = CameraFitFlag::new();
        let fixture = FixtureState::new();
        rebuild_engines(&engine, &engine3d, fixture.get(), &camera_fit);

        Self {
            engine,
            engine3d,
            dim: DimState::new(),
            fixture,
            camera_fit,
            fit3d_pending: CameraFitFlag::new(),
            nav_mode: NavModeState::new(),
            mouse_look: MouseLookState::new(),
            fly: Arc::new(Mutex::new(FlyController::new())),
            frame_profiler: Arc::new(Mutex::new(FrameProfiler::default())),
            last_3d_frame_at: None,
            last_3d_surface_px: SurfaceSizeState::new(),
            flatten_pending: FlattenPendingFlag::new(),
            hud_visible: HudVisibleState::new(),
            scale_factor: 1.0,
            last_sidebar_body: Rect::new(0.0, 0.0, 0.0, 0.0),
            hud_pressed_button: None,
            hud_dragging_slider: None,
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

    /// Toggle between orbit and fly navigation (`V`, 3D only — REBOUND
    /// from `Tab`, owner defect fix item 2: `Tab` is now the 2D<->3D
    /// dimension toggle) — always stops the `FlyController` on either
    /// transition so held keys/
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
        // the owner's expected "one keypress drops me straight into
        // mouse-look" flow (originally bound to `Tab`, now `V` — see
        // this method's own doc comment); a stale OFF from the previous
        // fly session would read as a broken toggle.
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
    /// Deferred: raises [`DemoApp::fit3d_pending`], consumed in
    /// `scene3d()` AFTER the engine tick — see that field's doc comment
    /// for the ordering rule (one-shot layouts apply positions during
    /// the tick, so an immediate fit here would frame the stale extent).
    fn fit_view_3d(&self) {
        self.fit3d_pending.request();
    }

    /// Wave 5 ground-reference grid toggle — the demo-level entry point
    /// both the G keyboard shortcut and the `set_grid` agent action call.
    fn toggle_grid(&self) {
        let mut engine3d = Self::lock3d(&self.engine3d);
        let next = !engine3d.grid_enabled();
        engine3d.set_grid_enabled(next);
    }

    /// Tab (owner defect fix item 2) — 2D<->3D dimension toggle, works
    /// from EITHER dimension. Shares its state-mutation core with
    /// `DemoBlackbox::set_dimension_3d`/`set_dimension_2d` (the
    /// `set_dimension` agent action) via `apply_dimension_3d_transition`/
    /// `apply_dimension_2d_transition` — see those free fns' own doc
    /// comments. Always animated, mirroring `set_dimension`'s own default.
    fn toggle_dimension(&mut self) {
        match self.dim.get() {
            Dimension::TwoD => apply_dimension_3d_transition(&self.engine, &self.engine3d, &self.dim, &self.flatten_pending, true),
            Dimension::ThreeD => apply_dimension_2d_transition(&self.engine, &self.engine3d, &self.dim, &self.flatten_pending, true),
        }
    }

    /// Home/F fit-to-bounds (owner defect fix item 2), dispatched to
    /// whichever dimension is currently active. 3D reuses the existing
    /// `fit_view_3d()`; 2D reuses the SAME `camera_fit` flag `ui()`'s own
    /// initial-fit/`set_fixture` paths already drive
    /// (`GraphEngine::fit_view()` runs on the next 2D frame). Previously
    /// Home/F only worked in 3D — extended to 2D here because the
    /// control-HUD's own "FIT — Home/F" button (item 3) is NOT marked 3D
    /// only, unlike "Orbit/Fly"/"Grid".
    fn fit_view_current_dimension(&mut self) {
        match self.dim.get() {
            Dimension::TwoD => self.camera_fit.request(),
            Dimension::ThreeD => self.fit_view_3d(),
        }
    }

    /// Small, cheap snapshot of exactly the state [`build_hud_layout`]
    /// needs — see [`HudSnapshot`]'s own doc comment. `layout_kind`/
    /// `layout_paused` read whichever dimension's engine is CURRENTLY
    /// active (owner defect fix 2026-07-23 — this used to always read the
    /// 2D engine regardless of `dim`, which is the root cause of 3D's
    /// LAYOUT section only ever being able to show one hardcoded
    /// always-active button).
    fn hud_snapshot(&self) -> HudSnapshot {
        let grid_enabled = Self::lock3d(&self.engine3d).grid_enabled();
        let (keyboard_sensitivity, mouse_sensitivity) = {
            let fly = Self::lock_fly(&self.fly);
            (fly.keyboard_sensitivity(), fly.mouse_sensitivity())
        };
        let (layout_kind, layout_paused, selection_line_count) = match self.dim.get() {
            Dimension::TwoD => {
                let engine = Self::lock(&self.engine);
                (engine.layout.kind(), engine.layout.paused(), selection_lines_2d(&engine).len())
            }
            Dimension::ThreeD => {
                let engine3d = Self::lock3d(&self.engine3d);
                (engine3d.layout.kind(), engine3d.layout.paused(), selection_lines_3d(&engine3d).len())
            }
        };
        HudSnapshot {
            dim: self.dim.get(),
            fixture: self.fixture.get(),
            nav_mode: self.nav_mode.get(),
            grid_enabled,
            keyboard_sensitivity,
            mouse_sensitivity,
            layout_kind,
            layout_paused,
            selection_line_count,
        }
    }

    /// Panel origin + content width for the CURRENT dimension — owner
    /// defect fix (panel-placement consistency): BOTH dimensions dock
    /// top-RIGHT now, at the SAME `SIDEBAR_WIDTH`, per this section's own
    /// module doc. 2D reads the real, already-right-docked sidebar body
    /// rect `ui()` last painted into; 3D has no such chrome to read, so
    /// it computes an equivalent right-docked rect from the last-known
    /// real 3D surface width (falling back to
    /// [`FALLBACK_SURFACE_WIDTH_LOGICAL`] before the first 3D frame ever
    /// renders).
    fn hud_origin(&self, snap: &HudSnapshot) -> (f64, f64, f64) {
        match self.dim.get() {
            Dimension::ThreeD => {
                let viewport_w = surface_width_logical(self.last_3d_surface_px.get(), self.scale_factor);
                let viewport_h = surface_height_logical(self.last_3d_surface_px.get(), self.scale_factor);
                // Floating card, right-docked with `HUD_MARGIN` air and
                // VERTICALLY CENTERED (owner design order 2026-07-24) —
                // the content-sized height comes from a zero-origin
                // probe layout of the same snapshot, so paint and
                // hit-test center on the identical card height.
                let card_h = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, snap).panel.height;
                (viewport_w - HUD_MARGIN - SIDEBAR_WIDTH as f64, hud_card_y(viewport_h, card_h), SIDEBAR_WIDTH as f64)
            }
            Dimension::TwoD => (self.last_sidebar_body.x, self.last_sidebar_body.y, self.last_sidebar_body.width),
        }
    }

    /// The full HUD layout for the CURRENT dimension/state — the one
    /// call `on_event_hud`'s own hit-testing makes; `ui()`/`scene3d()`
    /// build their own copy from the identical `build_hud_layout` with
    /// whatever origin their own paint call site already has on hand
    /// (the centered card rect in 2D, the centered float origin in 3D)
    /// — always the SAME function, so drawn and clickable geometry
    /// never drift apart.
    fn hud_layout(&self) -> HudLayout {
        let snap = self.hud_snapshot();
        let (origin_x, origin_y, width) = self.hud_origin(&snap);
        build_hud_layout(origin_x, origin_y, width, &snap)
    }

    /// App-level HUD hit-testing (owner defect fix item 3) — checked at
    /// the very top of `App::on_event`, before ANY dimension/nav-mode
    /// dispatch, so a click on the panel NEVER reaches
    /// `GraphEngine`/`GraphEngine3D::on_event` (satisfies "clicks on
    /// panel consume the event, no click-through" AND "panel excluded
    /// from box-select origination" as the exact SAME mechanism — a
    /// box-select can only ever START from a `PointerDown` the graph
    /// engine actually sees, which a panel-consumed `PointerDown` never
    /// reaches). Mirrors the foxhound reference app's own press-then-
    /// release-confirm button convention (a drag-off before release
    /// cancels the click) and its own live slider-drag-follows-the-
    /// cursor convention. Returns `Some(consumed)` once this event is
    /// fully handled by the HUD; `None` means "not the HUD's concern,
    /// let the caller's existing dispatch run."
    fn on_event_hud(&mut self, event: &PlatformEvent) -> Option<bool> {
        match event {
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => {
                let layout = self.hud_layout();
                if let Some(id) = hit_slider(&layout, *x, *y) {
                    self.hud_dragging_slider = Some(id);
                    if let Some(slider) = layout.sliders.iter().find(|s| s.id == id) {
                        self.apply_hud_slider(id, *x, slider.track);
                    }
                    return Some(true);
                }
                if let Some(control) = hit_button(&layout, *x, *y) {
                    self.hud_pressed_button = Some(control);
                    return Some(true);
                }
                if layout.panel.contains(*x, *y) {
                    // Inside the panel but not over any control (padding/
                    // heading/legend text) — still consumed, so a drag
                    // starting here can never fall through into a
                    // background pan/orbit/box-select gesture either.
                    return Some(true);
                }
                None
            }
            PlatformEvent::PointerMoved { x, .. } => {
                if let Some(id) = self.hud_dragging_slider {
                    let layout = self.hud_layout();
                    if let Some(slider) = layout.sliders.iter().find(|s| s.id == id) {
                        self.apply_hud_slider(id, *x, slider.track);
                    }
                    return Some(true);
                }
                if self.hud_pressed_button.is_some() {
                    return Some(true);
                }
                None
            }
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => {
                if self.hud_dragging_slider.take().is_some() {
                    return Some(true);
                }
                if let Some(pressed) = self.hud_pressed_button.take() {
                    let layout = self.hud_layout();
                    if hit_button(&layout, *x, *y) == Some(pressed) {
                        self.apply_hud_control(pressed);
                    }
                    return Some(true);
                }
                None
            }
            _ => None,
        }
    }

    /// Drive a sensitivity slider from a pointer x position (start-drag
    /// or continued-drag alike) — mirrors the foxhound reference app's
    /// own `apply_slider_at`.
    fn apply_hud_slider(&mut self, id: HudSliderId, x: f64, track: Rect) {
        let t = (((x - track.x) / track.width) as f32).clamp(0.0, 1.0);
        let mut fly = Self::lock_fly(&self.fly);
        match id {
            HudSliderId::Keyboard => {
                fly.set_keyboard_sensitivity(KEYBOARD_SENSITIVITY_MIN + t * (KEYBOARD_SENSITIVITY_MAX - KEYBOARD_SENSITIVITY_MIN));
            }
            HudSliderId::Mouse => {
                fly.set_mouse_sensitivity(MOUSE_SENSITIVITY_MIN + t * (MOUSE_SENSITIVITY_MAX - MOUSE_SENSITIVITY_MIN));
            }
        }
    }

    /// Dispatch a resolved HUD button click.
    fn apply_hud_control(&mut self, control: HudControl) {
        match control {
            HudControl::Fixture(fixture) => {
                self.fixture.set(fixture);
                rebuild_engines(&self.engine, &self.engine3d, fixture, &self.camera_fit);
            }
            // Owner defect fix 2026-07-23 — re-click semantics generalize
            // the owner's own rule «если 1 вариант — должен тоглиться»:
            // clicking the ALREADY-active mode toggles instead of being a
            // silent no-op (the exact "зажат форс, нихуя не
            // переключается" defect — FORCE was hardcoded active in 3D
            // and its own button click did nothing at all). FORCE
            // pauses/resumes the sim IN PLACE (no camera jump — nothing
            // actually moved); LAYERED/RADIAL (one-shot) re-run their
            // layout and re-fit, a visibly honest action since a fresh
            // recompute genuinely can move every node. Clicking an
            // INACTIVE button switches mode as before (+ fit). Dispatches
            // to whichever dimension's engine is CURRENTLY active — both
            // now expose the identical `GraphLayoutMode`/`GraphLayoutMode3D`
            // surface (`kind`/`paused`/`set_kind`/`set_paused`/`reheat`).
            HudControl::SetLayout(kind) => {
                // Defense in depth for the owner's availability rule —
                // `build_hud_layout` never emits a button for a kind
                // outside `available_layout_kinds(fixture)`, so a click
                // can't normally reach here with one; a stale hit-test
                // race right after a fixture switch could, and must be a
                // no-op, not a garbage layout.
                if !available_layout_kinds(self.fixture.get()).contains(&kind) {
                    return;
                }
                match self.dim.get() {
                    Dimension::TwoD => {
                        let mut engine = Self::lock(&self.engine);
                        if engine.layout.kind() == kind {
                            if kind == LayoutKind::Force {
                                let next = !engine.layout.paused();
                                engine.layout.set_paused(next);
                            } else {
                                engine.layout.reheat(1.0);
                                drop(engine);
                                self.camera_fit.request();
                            }
                        } else {
                            engine.layout.set_kind(kind);
                            // A layout switch can move every node to a wildly
                            // different extent (e.g. force's settled cluster
                            // spread vs. layered's compact rows) — re-frame
                            // the camera so the owner actually sees the new
                            // shape, same "camera fit after" convention a
                            // fixture switch already follows
                            // (`rebuild_engines`'s own `camera_fit.request()`).
                            drop(engine);
                            self.camera_fit.request();
                        }
                    }
                    Dimension::ThreeD => {
                        let mut engine3d = Self::lock3d(&self.engine3d);
                        if engine3d.layout.kind() == kind {
                            if kind == LayoutKind::Force {
                                let next = !engine3d.layout.paused();
                                engine3d.layout.set_paused(next);
                            } else {
                                engine3d.layout.reheat(1.0);
                                drop(engine3d);
                                self.fit_view_3d();
                            }
                        } else {
                            engine3d.layout.set_kind(kind);
                            drop(engine3d);
                            self.fit_view_3d();
                        }
                    }
                }
            }
            HudControl::ToggleDimension => self.toggle_dimension(),
            HudControl::ToggleNavMode => {
                if self.dim.get() == Dimension::ThreeD {
                    self.toggle_nav_mode();
                }
            }
            HudControl::FitView => self.fit_view_current_dimension(),
            HudControl::ToggleGrid => {
                if self.dim.get() == Dimension::ThreeD {
                    self.toggle_grid();
                }
            }
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
                    // Cluster/selection wave — same `{count, indices
                    // capped 50, collapsed_group}` shape `uzor_graph::agent`'s
                    // own 2D `selection_json` reports.
                    "selection": selection_json_3d(&engine3d),
                    // Filter/local-subgraph wave — same field names/shapes
                    // `uzor_graph::agent`'s own 2D `agent_state` reports.
                    "local_root": engine3d.local_root().map(|(node, depth)| json!({ "index": node.index(), "depth": depth })),
                    "filter": engine3d.filter().map(filter_json_3d),
                    // Owner defect fix 2026-07-23 — same `layout`/
                    // `layout_paused` field names `uzor_graph::agent`'s
                    // own 2D `agent_state` reports (see that crate's
                    // `agent.rs`), now meaningful in 3D too since
                    // `GraphLayoutMode3D` gave 3D real runtime-switchable
                    // layouts.
                    "layout": layout_kind_str(engine3d.layout.kind()),
                    "layout_paused": engine3d.layout.paused(),
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
            // Dimension-transition wave — always reported regardless of
            // which dimension is active (same convention `fixture`/
            // `nav_mode`/`grid` already use): the transition lives on
            // `engine3d` regardless.
            {
                let engine3d = DemoApp::lock3d(&self.engine3d);
                map.insert(
                    "dimension_transition".to_owned(),
                    json!({
                        "active": engine3d.transition_active(),
                        "direction": engine3d.transition_direction().map(TransitionDirection::as_str),
                        "progress": engine3d.transition_progress(),
                    }),
                );
            }
            // 2026-07-22 (3D-parity-arc tail) — same field name the
            // foxhound source app's own `FrameProfile` publishes under
            // (`uzor::framework::frame_profiler::FrameProfiler::to_json`'s
            // own doc comment), reported unconditionally like `fixture`/
            // `nav_mode`/`grid` above: `scene3d()` only ever records into
            // it while 3D is active, but it's meaningful to query either
            // way (an empty `stages` map, `frames: 0`, before the first
            // 3D frame ever renders).
            map.insert("frame_profile_ema_ms".to_owned(), DemoApp::lock_profiler(&self.frame_profiler).to_json());
            // Owner defect fix item 4 — "panel sections reflect the same
            // state already reported": no new engine API, just the
            // panel's own visibility flag; every section it draws
            // (fixture/nav-mode/grid/dimension) is already reported by
            // the fields above.
            map.insert("hud".to_owned(), json!({ "visible": self.hud_visible.get() }));
        }
        state
    }

    fn apply_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        if action.name == "set_dimension" {
            // Dimension-transition wave: animated by default (the owner's
            // own spec — "the last 3D-shelf UX item"); `args.animate:
            // false` keeps the pre-existing instant switch for headless
            // tests/scripts that want determinism. See
            // `DemoBlackbox::set_dimension_3d`/`set_dimension_2d`'s own
            // doc comments for the full In/Out mechanics.
            let animate = action.args.get("animate").and_then(Value::as_bool).unwrap_or(true);
            return match action.args.get("dim").and_then(Value::as_u64) {
                Some(2) => self.set_dimension_2d(animate),
                Some(3) => self.set_dimension_3d(animate),
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
            self.fit3d_pending.request();
            return AgentActionReply::ok_with_log(json!({ "fit_view_3d": true }));
        }
        if action.name == "set_grid" {
            let Some(on) = action.args.get("on").and_then(Value::as_bool) else {
                return AgentActionReply::err("set_grid requires args.on to be true or false");
            };
            DemoApp::lock3d(&self.engine3d).set_grid_enabled(on);
            return AgentActionReply::ok_with_log(json!({ "grid": on }));
        }
        if action.name == "set_hud" {
            // Owner defect fix item 4 — headless twin of the H keyboard
            // shortcut, mirroring `set_mouse_look`'s own shape, so an
            // agent can toggle the panel and verify it via a screenshot
            // without synthesizing a real keypress.
            let Some(on) = action.args.get("visible").and_then(Value::as_bool) else {
                return AgentActionReply::err("set_hud requires args.visible to be true or false");
            };
            self.hud_visible.set(on);
            return AgentActionReply::ok_with_log(json!({ "hud": { "visible": on } }));
        }
        if action.name == "set_layout" {
            // Owner availability rule, agent-api mirror of the HUD gate
            // (`available_layout_kinds`'s own doc): a mode that doesn't
            // exist for the current fixture's data shape is a typed
            // rejection here too, in ONE place for both dimensions —
            // the per-dimension handlers below only ever see a kind the
            // fixture genuinely has.
            if let Some(mode) = action.args.get("mode").and_then(Value::as_str) {
                let kind = match mode {
                    "force" => Some(LayoutKind::Force),
                    "hierarchical" => Some(LayoutKind::Hierarchical),
                    "radial" => Some(LayoutKind::Radial),
                    _ => None, // unknown string — let the handler's own error reply fire
                };
                let fixture = self.fixture.get();
                if let Some(kind) = kind {
                    if !available_layout_kinds(fixture).contains(&kind) {
                        return AgentActionReply::err(format!(
                            "layout mode {mode:?} does not exist for fixture {:?} (available: {:?})",
                            fixture.as_str(),
                            available_layout_kinds(fixture).iter().map(|k| layout_kind_str(*k)).collect::<Vec<_>>(),
                        ));
                    }
                }
            }
        }
        match self.dim.get() {
            Dimension::TwoD => DemoApp::lock(&self.engine).apply_agent_action(action),
            Dimension::ThreeD => self.apply_3d_agent_action(action),
        }
    }
}

impl DemoBlackbox {
    /// `set_dimension {"dim": 3}` — 2D->3D (dimension-transition wave).
    /// Re-seeds `engine3d`'s `x`/`y` from the CURRENT 2D particle layout
    /// (`GraphEngine3D::seed_positions` also jitters `z` per its own
    /// existing z-plane-degeneracy fix) ONLY on a genuinely fresh switch
    /// (`self.dim.get() == TwoD` — reseeding while already `ThreeD` would
    /// stomp the live 3D sim mid-flight, e.g. during a reversal of an
    /// in-flight `Out`), flips `DimState` to `ThreeD` immediately, clears
    /// any pending flatten-to-2D (a reversal), then starts (or reverses
    /// into) an animated `In` transition — see
    /// `GraphEngine3D::start_transition`'s own doc comment for the
    /// no-snap reversal mechanics. `animate: false` skips
    /// `start_transition` entirely (an instant switch, the pre-existing
    /// behavior — headless tests/scripts want determinism).
    ///
    /// The actual state mutation now lives in the shared free fn
    /// [`apply_dimension_3d_transition`] (owner defect fix item 2) — this
    /// method is a thin agent-api wrapper around it, so the Tab keyboard
    /// shortcut (`DemoApp::toggle_dimension`) and this action can never
    /// drift onto two different "flip to 3D" implementations.
    fn set_dimension_3d(&mut self, animate: bool) -> AgentActionReply {
        apply_dimension_3d_transition(&self.engine, &self.engine3d, &self.dim, &self.flatten_pending, animate);
        if !animate {
            // The animated path frames the flat layout itself (the In
            // transition starts from a fit_bounds front-on pose); the
            // instant path used to keep the STALE orbit pose from the
            // previous 3D session — fit explicitly, same aspect source
            // as the fit_view_3d action.
            self.fit3d_pending.request();
        }
        AgentActionReply::ok_with_log(json!({ "dimension": 3, "animate": animate }))
    }

    /// `set_dimension {"dim": 2}` — 3D->2D (dimension-transition wave).
    /// While ALREADY `TwoD`, a no-op success (matches the pre-existing
    /// idempotent behavior). While `ThreeD` and `animate`: starts an
    /// `Out` transition and sets [`FlattenPendingFlag`] — `DimState`
    /// DELIBERATELY stays `ThreeD` for the whole flattening (the volume
    /// is still `engine3d`'s own scene to render); `DemoApp::scene3d`'s
    /// own completion poll is what actually copies the settled 3D
    /// positions into the 2D engine and flips `DimState` once the ease
    /// finishes (`GraphEngine3D::transition_active()` alone can't tell
    /// `scene3d` which direction just completed, since it clears either
    /// way — the flag is the persistent signal). `animate: false`
    /// completes the flip synchronously right here instead (the
    /// pre-existing instant-switch behavior).
    ///
    /// Delegates the actual state mutation to the shared free fn
    /// [`apply_dimension_2d_transition`] — see [`Self::set_dimension_3d`]'s
    /// own updated doc comment for why. The reply shape is IDENTICAL on
    /// both branches of the pre-existing match (`{"dimension":2,
    /// "animate":animate}`), so collapsing to one reply after the shared
    /// call is a pure refactor, not a behavior change.
    fn set_dimension_2d(&mut self, animate: bool) -> AgentActionReply {
        apply_dimension_2d_transition(&self.engine, &self.engine3d, &self.dim, &self.flatten_pending, animate);
        AgentActionReply::ok_with_log(json!({ "dimension": 2, "animate": animate }))
    }

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
    ///
    /// **Owner defect fix 2026-07-23**: `set_layout` (mirrors 2D's own
    /// `args.mode`/`args.paused` arg shape, `uzor-graph/src/agent.rs`)
    /// now also forwards onto `engine3d.layout` (`GraphLayoutMode3D` —
    /// 3D's own runtime-switchable Force/Hierarchical/Radial dispatcher,
    /// closing the "3D has no `set_layout` to dispatch to" gap every
    /// earlier wave's own doc comment used to note).
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
                    self.fit3d_pending.request();
                    return AgentActionReply::ok_with_log(json!({ "local_root": Value::Null }));
                }
                let Some(node) = resolve_node_3d(&engine3d, &action) else {
                    return AgentActionReply::err(
                        "set_local_root requires args.index (u32), args.label (string), or {} / null to clear",
                    );
                };
                let depth = action.args.get("depth").and_then(Value::as_u64).map(|d| d as u8);
                engine3d.set_local_root(Some(node), depth);
                self.fit3d_pending.request();
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
            // Owner defect fix 2026-07-23 — mirrors `uzor_graph::agent`'s
            // own 2D `set_layout` action arg shape exactly (`args.mode`
            // switches kind, `args.paused` freezes/resumes in place,
            // either or both may be present in one call — see that
            // crate's own `agent.rs` for the identical "fold pause into
            // the existing action shape" decision, applied here too so a
            // caller driving both dimensions sees the same action name
            // and args shape either way).
            "set_layout" => {
                let mode_arg = action.args.get("mode").and_then(Value::as_str);
                let paused_arg = action.args.get("paused").and_then(Value::as_bool);
                if mode_arg.is_none() && paused_arg.is_none() {
                    return AgentActionReply::err(
                        "set_layout requires args.mode (\"force\"|\"hierarchical\"|\"radial\") and/or args.paused (bool)",
                    );
                }
                let kind = match mode_arg {
                    Some("force") => Some(LayoutKind::Force),
                    Some("hierarchical") => Some(LayoutKind::Hierarchical),
                    Some("radial") => Some(LayoutKind::Radial),
                    Some(other) => return AgentActionReply::err(format!("unknown layout mode {other:?}")),
                    None => None,
                };
                if let Some(kind) = kind {
                    if engine3d.layout.kind() != kind {
                        engine3d.layout.set_kind(kind);
                        // Same "re-frame after a layout switch" rule the
                        // HUD button follows — deferred so the one-shot
                        // layout has applied by the time the fit reads
                        // positions.
                        self.fit3d_pending.request();
                    } else {
                        engine3d.layout.set_kind(kind);
                    }
                }
                if let Some(paused) = paused_arg {
                    engine3d.layout.set_paused(paused);
                }
                AgentActionReply::ok_with_log(json!({
                    "layout": layout_kind_str(engine3d.layout.kind()),
                    "layout_paused": engine3d.layout.paused(),
                }))
            }
            _ => AgentActionReply::err(
                "3D dimension supports hover_node/select_node/clear_selection/select_nodes/box_select/collapse_selection/pin_selection/unpin_selection/collapse/expand/set_local_root/set_filter/set_layout besides set_dimension",
            ),
        }
    }
}


impl App<NoPanel> for DemoApp {
    fn init(&mut self, key: &WindowKey, layout: &mut LayoutManager<NoPanel>) {
        // Kill the LayoutManager's DEFAULT chrome strip (owner defect
        // report 2026-07-24: «невидимая полоска за которую я могу
        // драгать» — `ChromeSlot::default()` is `visible: true`, so the
        // solver reserved an unpainted 32px band under the REAL OS
        // titlebar this window already has, and `Manager`'s
        // `handle_chrome_press` hit-tested its caption zone into
        // `drag_window()`). This demo uses standard OS decorations —
        // the LM's own custom-titlebar system must be fully OFF.
        layout.chrome_mut_for(key).visible = false;
        // Wave 2 (W3D arc plan §1.6): register the `DemoBlackbox`
        // dimension-aware wrapper instead of `self.engine` directly —
        // see that struct's own doc comment.
        let blackbox = DemoBlackbox {
            engine: self.engine.clone(),
            engine3d: self.engine3d.clone(),
            dim: self.dim.clone(),
            fixture: self.fixture.clone(),
            camera_fit: self.camera_fit.clone(),
            fit3d_pending: self.fit3d_pending.clone(),
            nav_mode: self.nav_mode.clone(),
            mouse_look: self.mouse_look.clone(),
            fly: self.fly.clone(),
            frame_profiler: self.frame_profiler.clone(),
            surface_size: self.last_3d_surface_px.clone(),
            flatten_pending: self.flatten_pending.clone(),
            hud_visible: self.hud_visible.clone(),
        };
        layout.register_blackbox_agent(BLACKBOX_SLOT, Arc::new(Mutex::new(blackbox)));
    }

    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        // The dock slot itself follows HUD visibility (owner defect fix
        // 2026-07-24): a hidden panel (H) releases the whole column back
        // to the graph viewport instead of leaving a dead black strip.
        let hud_visible = self.hud_visible.get();
        win.layout.edges_mut().clear();
        win.layout.edges_mut().add(EdgeSlot {
            id: SIDEBAR_SLOT.to_owned(),
            side: EdgeSide::Right,
            // Card width + air on BOTH sides of it (owner design order
            // 2026-07-24: the panel floats with empty space around it,
            // it's not a flush full-height column).
            thickness: SIDEBAR_WIDTH + 2.0 * HUD_MARGIN as f32,
            visible: hud_visible,
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

        let (hot, alpha, node_count, visible_count, selection_lines) = {
            let mut engine = Self::lock(&self.engine);
            engine.set_canvas_rect(canvas_rect);

            engine.tick_real_time();

            // Fit AFTER the tick, not before: the one-shot layouts
            // (Hierarchical/Radial) only apply their positions during a
            // tick — fitting first framed the stale seed layout (owner
            // report: layered tree rendered with most layers off-canvas,
            // "visible 78" of 300).
            if self.camera_fit.needs_fit() && canvas_rect.width > 0.0 {
                engine.fit_view();
                self.camera_fit.clear();
            }

            if canvas_rect.width > 0.0 && canvas_rect.height > 0.0 {
                win.render.save();
                win.render.clip_rect(canvas_rect.x, canvas_rect.y, canvas_rect.width, canvas_rect.height);
                engine.draw(win.render);
                win.render.restore();
            }

            let lines = selection_lines_2d(&engine);
            let snapshot = (engine.is_hot(), engine.last_tick().alpha, engine.graph.node_count(), engine.visible_nodes().len(), lines);
            engine.clear_dirty();
            snapshot
        };

        // Control-HUD card — painted DIRECTLY into the docked right-edge
        // area (owner defect fix 2026-07-24: the old `lm::sidebar` widget
        // put a "Graph" header strip ABOVE the panel and left a dead
        // black body area BELOW it; owner design order same day: a
        // content-sized floating card with `HUD_MARGIN` air around it,
        // no title row — the selection facts are a SELECTION section
        // inside it instead of loose text rows). `scale: 1.0` — this
        // `RenderContext` is already logical-native (see the "Control
        // HUD" section's own module doc).
        if hud_visible && win_rect.width > 0.0 && win_rect.height > 0.0 {
            let hud_snapshot = self.hud_snapshot();
            // Vertically CENTERED content-sized card (owner design order
            // 2026-07-24: equal air above and below) — height from a
            // zero-origin probe of the same snapshot.
            let card_h = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &hud_snapshot).panel.height;
            let card_origin = Rect::new(
                win_rect.x + win_rect.width - HUD_MARGIN - SIDEBAR_WIDTH as f64,
                win_rect.y + hud_card_y(win_rect.height, card_h),
                SIDEBAR_WIDTH as f64,
                card_h,
            );
            self.last_sidebar_body = card_origin;
            let hud_layout = build_hud_layout(card_origin.x, card_origin.y, card_origin.width, &hud_snapshot);
            draw_hud_static(win.render, &hud_layout, 1.0);
            let status_lines =
                [format!("nodes {node_count}  visible {visible_count}"), format!("alpha {alpha:.4}  hot {hot}")];
            draw_hud_status(win.render, hud_layout.panel.x, hud_layout.status_y, &status_lines, 1.0);
            draw_hud_status(win.render, hud_layout.panel.x, hud_layout.selection_y, &selection_lines, 1.0);
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
        // Keep the tracked DPI scale factor current — see the "Control
        // HUD" section's own module doc for what this backs and its one
        // known startup-value gap. Not consumed: falls through so the
        // engines can still see it (neither currently reacts to it, but
        // there's no reason to hide the event from them either).
        if let PlatformEvent::ScaleFactorChanged { scale } = event {
            self.scale_factor = *scale;
        }

        // Owner defect fix item 3 — control-HUD hit-testing, checked
        // FIRST so a panel click never reaches
        // `GraphEngine`/`GraphEngine3D::on_event` at all. See
        // `on_event_hud`'s own doc comment.
        if self.hud_visible.get() {
            if let Some(consumed) = self.on_event_hud(event) {
                return consumed;
            }
        }

        // `H` (item 3) — HUD visibility toggle, works in EITHER
        // dimension, checked regardless of current visibility so a
        // hidden panel can be brought back.
        if let PlatformEvent::KeyDown { key: KeyCode::H, .. } = event {
            self.hud_visible.toggle();
            return true;
        }

        // `Tab` (item 2) — 2D<->3D dimension toggle, works from EITHER
        // dimension (REBOUND from the pre-existing orbit/fly toggle,
        // which moved to `V` below).
        if let PlatformEvent::KeyDown { key: KeyCode::Tab, .. } = event {
            self.toggle_dimension();
            return true;
        }

        // `Home`/`F` (item 2) — fit-to-bounds, now EITHER dimension (was
        // 3D-only) — see `fit_view_current_dimension`'s own doc comment
        // for why.
        if let PlatformEvent::KeyDown { key: KeyCode::Home | KeyCode::F, .. } = event {
            self.fit_view_current_dimension();
            return true;
        }

        // `Esc` (item 2) — extra escape hatch out of fly mouse-look, on
        // top of the pre-existing MMB toggle. A no-op (falls through)
        // whenever fly mouse-look isn't actually active, so Escape still
        // does whatever else it might mean elsewhere.
        if let PlatformEvent::KeyDown { key: KeyCode::Escape, .. } = event {
            if self.dim.get() == Dimension::ThreeD && self.nav_mode.get() == NavMode::Fly && self.mouse_look.get() {
                self.mouse_look.set(false);
                return true;
            }
        }

        if self.dim.get() == Dimension::ThreeD {
            // `V` (item 2, REBOUND from Tab) — orbit/fly toggle, 3D only,
            // since orbit/fly are both purely 3D camera-control concepts.
            if let PlatformEvent::KeyDown { key: KeyCode::V, .. } = event {
                self.toggle_nav_mode();
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

        // Deferred 3D fit — consumed AFTER the tick so a one-shot layout
        // (Layered/Radial) has applied its positions before the camera
        // frames them (same ordering rule 2D's `camera_fit` follows);
        // skipped while a dimension transition owns the camera — the
        // flag simply survives until the transition ends.
        if self.fit3d_pending.needs_fit() && !engine3d.transition_active() {
            engine3d.fit_view(surf_w as f32 / (surf_h.max(1) as f32));
            self.fit3d_pending.clear();
        }

        // Dimension-transition wave — poll for an animated `Out`
        // flattening's completion: `GraphEngine3D::tick` (above) just
        // advanced it, and `transition_active()` clears the INSTANT the
        // ease finishes. `DimState` deliberately stayed `ThreeD` for the
        // whole flattening (see `DemoBlackbox::set_dimension_2d`'s own
        // doc comment) — this is the one place that actually hands
        // control back to the 2D engine, THIS SAME frame, once the
        // volume has genuinely finished collapsing to the plane.
        if self.flatten_pending.is_pending() && !engine3d.transition_active() {
            drop(engine3d);
            flatten_3d_into_2d(&self.engine, &self.engine3d, &self.dim);
            self.flatten_pending.clear();
            self.last_3d_frame_at = None;
            return None;
        }

        let scene_build_started = std::time::Instant::now();
        let scene = engine3d.build_scene(surf_h as f64);
        let scene_build_ms = scene_build_started.elapsed().as_secs_f64() * 1000.0;
        let aspect = surf_w as f32 / (surf_h.max(1) as f32);
        let camera = engine3d.camera(aspect);
        let node_count = engine3d.graph.node_count();
        drop(engine3d);

        let frame_ms = {
            let mut profiler = Self::lock_profiler(&self.frame_profiler);
            profiler.record_ms("tick", tick_ms);
            profiler.record_ms("scene_build", scene_build_ms);
            profiler.end_frame();
            profiler.stage_ms("tick") + profiler.stage_ms("scene_build")
        };

        let hud_snapshot = self.hud_snapshot();
        let hud_visible = self.hud_visible.get();
        let hud_scale = self.scale_factor;
        // Right-docked, vertically-centered card origin for THIS tick's
        // real surface size (already recorded above via
        // `self.last_3d_surface_px.set(...)`) — see `DemoApp::hud_origin`'s
        // own `Dimension::ThreeD` branch (`dim` is confirmed `ThreeD` by
        // the early-return guard at the top of this function, so this
        // always resolves the 3D branch).
        let (hud_origin_x, hud_origin_y, hud_width) = self.hud_origin(&hud_snapshot);

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
        //
        // Z-ORDER RULE (owner defect fix 2026-07-24: «менюшка всегда
        // выше сцены» — node labels used to leak straight through the
        // HUD card): EVERYTHING scene-space (engine overlay labels,
        // rings, grid ticks, hover card, crosshair) paints FIRST, the
        // WHOLE HUD card (static chrome + dynamic status/selection
        // text) paints LAST, all inside this ONE closure. The former
        // `CachedOverlayJob` split (static chrome cached, dynamic text
        // in the plain overlay) is deliberately GONE — render-hub's
        // Phase 4.5 blits the cached texture BEFORE the plain overlay,
        // which structurally puts scene labels ABOVE the card chrome;
        // no cache key can fix an ordering problem, and the card's
        // static paint (a few dozen rects + short strings) is trivial
        // next to the per-node label pass that already repaints every
        // frame anyway.
        let engine3d_for_overlay = self.engine3d.clone();
        let overlay_viewport = Rect::new(0.0, 0.0, surf_w as f64, surf_h as f64);
        let crosshair_armed = self.nav_mode.get() == NavMode::Fly && self.mouse_look.get();
        let overlay: Box<dyn FnMut(&mut dyn RenderContext)> = Box::new(move |ctx: &mut dyn RenderContext| {
            let engine3d = Self::lock3d(&engine3d_for_overlay);
            engine3d.draw_overlay(ctx, &camera, overlay_viewport);
            let selection_lines = selection_lines_3d(&engine3d);
            if crosshair_armed {
                draw_fly_crosshair(ctx, overlay_viewport);
            }
            drop(engine3d);
            // The HUD card — chrome first, then its dynamic STATUS/
            // SELECTION text, both ABOVE everything scene-space painted
            // just before (the z-order rule above). 3D has no separate
            // "visible" (frustum-culled) node count of its own to
            // report — see `draw_hud_status`'s caller-supplied `lines`,
            // this is the honest simplification.
            if hud_visible {
                let layout = build_hud_layout(hud_origin_x, hud_origin_y, hud_width, &hud_snapshot);
                draw_hud_static(ctx, &layout, hud_scale);
                let lines = [format!("nodes {node_count}  visible {node_count}"), format!("frame {frame_ms:.2}ms")];
                draw_hud_status(ctx, layout.panel.x, layout.status_y, &lines, hud_scale);
                draw_hud_status(ctx, layout.panel.x, layout.selection_y, &selection_lines, hud_scale);
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
                // Owner defect fix — this demo is a human-facing control
                // surface now (item 3's own control HUD), not a bare
                // agent-api test stand, so it gets STANDARD OS window
                // chrome (titlebar + min/max/close) like every other
                // window on the desktop. `WindowSpec::new`'s own default
                // is `decorations: false` (borderless, no OS chrome) —
                // this demo used to just restate that default explicitly;
                // overriding it here to `true` is the ENTIRE chrome fix,
                // `uzor-desktop::manager.rs` already forwards
                // `spec.decorations` straight into winit's own
                // `.with_decorations(...)`. `corner_style`/`border_color`
                // are independent DWM window-attribute overrides (not
                // decorations) and stay as-is — they still apply on a
                // decorated window.
                .decorations(true)
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

    // ── Owner defect fix: control-HUD hit-zone mapping + keybind dispatch ──

    #[test]
    fn hit_button_resolves_the_correct_fixture_button_and_none_outside_any_zone() {
        let snap = HudSnapshot {
            dim: Dimension::ThreeD,
            fixture: Fixture::Clusters,
            nav_mode: NavMode::Orbit,
            grid_enabled: false,
            keyboard_sensitivity: 1.0,
            mouse_sensitivity: 1.0,
            layout_kind: LayoutKind::Force,
            layout_paused: false,
            selection_line_count: 1,
        };
        let layout = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap);
        let tree_button = layout.buttons.iter().find(|b| b.control == HudControl::Fixture(Fixture::Tree)).expect("tree button must exist");
        let (cx, cy) = (tree_button.rect.center_x(), tree_button.rect.center_y());
        assert_eq!(hit_button(&layout, cx, cy), Some(HudControl::Fixture(Fixture::Tree)));

        // A point in the panel's own padding gutter, left of every
        // button's own rect (which starts at `HUD_PAD` in from the
        // panel edge) — no button should claim it.
        assert_eq!(hit_button(&layout, layout.panel.x + 1.0, cy), None);

        // Nothing outside the panel bounds at all.
        assert_eq!(hit_button(&layout, layout.panel.x - 50.0, layout.panel.y - 50.0), None);
    }

    #[test]
    fn hud_layout_omits_3d_only_controls_while_2d_is_active() {
        let snap_2d = HudSnapshot {
            dim: Dimension::TwoD,
            fixture: Fixture::Clusters,
            nav_mode: NavMode::Orbit,
            grid_enabled: false,
            keyboard_sensitivity: 1.0,
            mouse_sensitivity: 1.0,
            layout_kind: LayoutKind::Force,
            layout_paused: false,
            selection_line_count: 1,
        };
        let layout_2d = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap_2d);
        assert!(!layout_2d.buttons.iter().any(|b| b.control == HudControl::ToggleNavMode), "Orbit/Fly button must not appear in 2D");
        assert!(!layout_2d.buttons.iter().any(|b| b.control == HudControl::ToggleGrid), "Grid button must not appear in 2D");
        assert!(layout_2d.sliders.is_empty(), "sensitivity sliders are a 3D fly-mode-only section");
        assert!(layout_2d.buttons.iter().any(|b| b.control == HudControl::ToggleDimension), "the 2D/3D toggle must always appear");
        assert!(layout_2d.buttons.iter().any(|b| b.control == HudControl::FitView), "Fit must always appear (not 3D-only)");
        assert!(layout_2d.buttons.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Force)), "LAYOUT buttons must appear in 2D");

        let snap_3d_fly = HudSnapshot { dim: Dimension::ThreeD, nav_mode: NavMode::Fly, ..snap_2d };
        let layout_3d_fly = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap_3d_fly);
        assert!(layout_3d_fly.buttons.iter().any(|b| b.control == HudControl::ToggleNavMode));
        assert!(layout_3d_fly.buttons.iter().any(|b| b.control == HudControl::ToggleGrid));
        assert_eq!(layout_3d_fly.sliders.len(), 2, "keyboard + mouse sensitivity sliders while 3D fly is active");
    }

    /// The owner's availability rule (2026-07-24): the LAYOUT section
    /// shows ONLY the modes that exist for the current fixture's data
    /// shape — never a foreign mode, never an empty section. Mesh-shaped
    /// fixtures (clusters/sparse) expose FORCE alone; tree-shaped ones
    /// (tree/hierarchy) expose all three. Identical in both dimensions.
    #[test]
    fn hud_layout_shows_only_the_layout_modes_that_exist_for_the_fixture() {
        let base = HudSnapshot {
            dim: Dimension::TwoD,
            fixture: Fixture::Clusters,
            nav_mode: NavMode::Orbit,
            grid_enabled: false,
            keyboard_sensitivity: 1.0,
            mouse_sensitivity: 1.0,
            layout_kind: LayoutKind::Force,
            layout_paused: false,
            selection_line_count: 1,
        };
        for dim in [Dimension::TwoD, Dimension::ThreeD] {
            for fixture in [Fixture::Clusters, Fixture::Sparse] {
                let snap = HudSnapshot { dim, fixture, ..base };
                let layout = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap);
                let modes: Vec<_> = layout.buttons.iter().filter(|b| matches!(b.control, HudControl::SetLayout(_))).collect();
                assert_eq!(modes.len(), 1, "{fixture:?}/{dim:?}: mesh-shaped fixtures have exactly one layout mode");
                assert!(modes[0].control == HudControl::SetLayout(LayoutKind::Force) && modes[0].active);
            }
            for fixture in [Fixture::Tree, Fixture::Hierarchy] {
                let snap = HudSnapshot { dim, fixture, layout_kind: LayoutKind::Hierarchical, ..base };
                let layout = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap);
                let modes: Vec<_> = layout.buttons.iter().filter(|b| matches!(b.control, HudControl::SetLayout(_))).collect();
                assert_eq!(modes.len(), 3, "{fixture:?}/{dim:?}: tree-shaped fixtures have all three layout modes");
                assert!(modes.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Force) && !b.active));
                assert!(modes.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Hierarchical) && b.active));
                assert!(modes.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Radial) && !b.active));
            }
        }
    }

    /// The task's own explicit ask: prove the coordinate-space
    /// reconciliation with a synthetic click, not just by inspection. A
    /// LOGICAL click point strictly inside a button's own (logical) hit
    /// rect must map to a PHYSICAL point (`logical * scale`) that also
    /// falls strictly inside that SAME button's rect after
    /// `scaled_rect` — proving `draw_hud_static`'s painted geometry and
    /// `hit_button`'s hit-tested geometry agree at ANY scale factor, not
    /// just `1.0`.
    #[test]
    fn hud_layout_scale_invariant_holds_for_a_synthetic_click_at_an_arbitrary_scale() {
        let snap = HudSnapshot {
            dim: Dimension::ThreeD,
            fixture: Fixture::Sparse,
            nav_mode: NavMode::Orbit,
            grid_enabled: true,
            keyboard_sensitivity: 1.2,
            mouse_sensitivity: 0.8,
            layout_kind: LayoutKind::Force,
            layout_paused: false,
            selection_line_count: 5,
        };
        let layout = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap);
        let button = layout.buttons.first().expect("at least one button");
        let logical = (button.rect.center_x(), button.rect.center_y());
        assert_eq!(hit_button(&layout, logical.0, logical.1), Some(button.control));

        for scale in [1.0_f64, 1.25, 1.5, 2.0] {
            let physical = (logical.0 * scale, logical.1 * scale);
            let drawn = scaled_rect(button.rect, scale);
            assert!(
                drawn.contains(physical.0, physical.1),
                "at scale {scale}: physical click {physical:?} must land inside the scaled draw rect {drawn:?}"
            );
        }
    }

    #[test]
    fn tab_key_toggles_dimension_from_2d_to_3d_and_back() {
        let mut app = DemoApp::new();
        assert_eq!(app.dim.get(), Dimension::TwoD);
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::Tab, modifiers: uzor::input::ModifierKeys::default() });
        assert_eq!(app.dim.get(), Dimension::ThreeD, "Tab must switch 2D -> 3D");
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::Tab, modifiers: uzor::input::ModifierKeys::default() });
        // Dimension-transition wave: an animated 3D->2D switch keeps
        // `DimState` at `ThreeD` until `scene3d()`'s own completion poll
        // flattens it (see `apply_dimension_2d_transition`'s own doc
        // comment) — headless tests never call `scene3d()`, so the
        // observable, testable claim here is that flattening was
        // correctly REQUESTED, not that `dim` already flipped back.
        assert!(app.flatten_pending.is_pending(), "Tab from 3D must request the animated flatten-to-2D");
    }

    #[test]
    fn v_key_toggles_nav_mode_only_while_3d_is_active_and_is_a_noop_in_2d() {
        let mut app = DemoApp::new();
        assert_eq!(app.nav_mode.get(), NavMode::Orbit);
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::V, modifiers: uzor::input::ModifierKeys::default() });
        assert_eq!(app.nav_mode.get(), NavMode::Orbit, "V must be a no-op while 2D is active");

        app.dim.set(Dimension::ThreeD);
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::V, modifiers: uzor::input::ModifierKeys::default() });
        assert_eq!(app.nav_mode.get(), NavMode::Fly, "V must toggle orbit -> fly while 3D is active");
    }

    #[test]
    fn h_key_toggles_hud_visibility_and_defaults_to_shown() {
        let mut app = DemoApp::new();
        assert!(app.hud_visible.get());
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::H, modifiers: uzor::input::ModifierKeys::default() });
        assert!(!app.hud_visible.get());
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::H, modifiers: uzor::input::ModifierKeys::default() });
        assert!(app.hud_visible.get());
    }

    #[test]
    fn escape_key_exits_fly_mouse_look_only_when_active_in_3d_fly() {
        let mut app = DemoApp::new();
        // Not yet in 3D fly — Escape must be a complete no-op on mouse_look.
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::Escape, modifiers: uzor::input::ModifierKeys::default() });
        assert!(app.mouse_look.get(), "mouse_look defaults true and Escape must not touch it outside 3D fly");

        app.dim.set(Dimension::ThreeD);
        app.nav_mode.set(NavMode::Fly);
        assert!(app.mouse_look.get());
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::Escape, modifiers: uzor::input::ModifierKeys::default() });
        assert!(!app.mouse_look.get(), "Escape must release fly mouse-look");

        // A second Escape while already released is a no-op, not a
        // re-toggle back on — this is an escape HATCH, not a toggle.
        app.on_event(&PlatformEvent::KeyDown { key: KeyCode::Escape, modifiers: uzor::input::ModifierKeys::default() });
        assert!(!app.mouse_look.get());
    }

    #[test]
    fn pointer_down_inside_the_hud_panel_is_consumed_by_on_event() {
        let mut app = DemoApp::new();
        app.dim.set(Dimension::ThreeD);
        // Well inside the right-docked 3D panel's title area (no real
        // surface size recorded yet in this headless test, so
        // `hud_origin` falls back to `FALLBACK_SURFACE_WIDTH_LOGICAL` —
        // read the REAL computed layout rather than a stale hardcoded
        // top-left constant, since the panel no longer floats there —
        // owner defect fix, panel-placement consistency) — no button/
        // slider under it, just plain panel padding.
        let panel = app.hud_layout().panel;
        let consumed =
            app.on_event(&PlatformEvent::PointerDown { x: panel.x + 20.0, y: panel.y + 5.0, button: MouseButton::Left });
        assert!(consumed, "a PointerDown inside the HUD panel must be consumed, never fall through to the graph engine");
        // The panel-consumed click must not have started an orbit-drag
        // (no `Pointer3DMode` state this test can inspect directly, but
        // the nav mode/dimension must be completely untouched by a
        // plain panel click with no button under it).
        assert_eq!(app.dim.get(), Dimension::ThreeD);
        assert_eq!(app.nav_mode.get(), NavMode::Orbit);
    }

    #[test]
    fn fixture_button_click_rebuilds_both_engines_from_the_clicked_fixture() {
        let mut app = DemoApp::new();
        app.dim.set(Dimension::ThreeD);
        let layout = app.hud_layout();
        let tree_button = layout.buttons.iter().find(|b| b.control == HudControl::Fixture(Fixture::Tree)).expect("tree button must exist");
        let (x, y) = (tree_button.rect.center_x(), tree_button.rect.center_y());

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));

        assert_eq!(app.fixture.get(), Fixture::Tree);
        let node_count = DemoApp::lock3d(&app.engine3d).graph.node_count();
        assert!(node_count > 100, "the tree fixture has a few hundred nodes, got {node_count}");
    }

    // ── Owner defect fix 1: HUD panel position consistency (both dims dock top-right) ──

    #[test]
    fn hud_panel_rect_consistency_both_dimensions_dock_flush_right_with_the_same_width() {
        let mut app = DemoApp::new();
        let snap = app.hud_snapshot();

        // 2D: `hud_origin` reads back whatever `ui()` last painted the
        // real, already-centered card rect at.
        app.last_sidebar_body = Rect::new(1080.0, 40.0, SIDEBAR_WIDTH as f64, 860.0);
        assert_eq!(app.hud_origin(&snap), (1080.0, 40.0, SIDEBAR_WIDTH as f64));

        // 3D: the card docks the SAME right edge, at the SAME
        // `SIDEBAR_WIDTH`, derived from the last-known real 3D surface
        // size — and is VERTICALLY CENTERED (owner design order
        // 2026-07-24: equal air above and below the content-sized card).
        app.dim.set(Dimension::ThreeD);
        app.last_3d_surface_px.set(1400, 900);
        let snap3 = app.hud_snapshot();
        let card_h = build_hud_layout(0.0, 0.0, SIDEBAR_WIDTH as f64, &snap3).panel.height;
        let (x3, y3, w3) = app.hud_origin(&snap3);
        assert_eq!(w3, SIDEBAR_WIDTH as f64, "3D card width must match the 2D card's own width");
        assert_eq!(x3, 1400.0 - HUD_MARGIN - SIDEBAR_WIDTH as f64, "3D card docks toward the right edge with HUD_MARGIN air");
        assert_eq!(y3, (900.0 - card_h) / 2.0, "the card is vertically centered — equal air above and below");
        assert!((y3 + card_h / 2.0 - 450.0).abs() < 1e-9, "card center must sit at the viewport's vertical center");
        assert!(x3 > 0.0, "must not sit at a left-anchored origin — the exact defect the owner reported");

        // A wider surface pushes the dock further right, proportionally
        // — proves this is a REAL right-dock derived from the viewport,
        // not a second hardcoded left-ish constant in disguise.
        app.last_3d_surface_px.set(1920, 1080);
        let (x3_wide, y3_tall, w3_wide) = app.hud_origin(&snap3);
        assert_eq!(w3_wide, SIDEBAR_WIDTH as f64);
        assert_eq!(x3_wide, 1920.0 - HUD_MARGIN - SIDEBAR_WIDTH as f64);
        assert!(x3_wide > x3, "a wider window must dock the panel further right, not leave it in place");
        assert!(y3_tall > y3, "a taller window must re-center the card lower, not leave it pinned at a fixed top offset");
    }

    // ── Owner defect fix 2: per-fixture default 2D layout + HUD LAYOUT section ──

    #[test]
    fn default_layout_kind_matches_the_owner_specified_per_fixture_mapping() {
        assert_eq!(default_layout_kind_for_fixture(Fixture::Clusters), LayoutKind::Force);
        assert_eq!(default_layout_kind_for_fixture(Fixture::Sparse), LayoutKind::Force);
        assert_eq!(default_layout_kind_for_fixture(Fixture::Tree), LayoutKind::Hierarchical);
        assert_eq!(default_layout_kind_for_fixture(Fixture::Hierarchy), LayoutKind::Hierarchical);
    }

    #[test]
    fn rebuild_engines_applies_the_default_layout_kind_to_both_engines_per_fixture() {
        let engine = Arc::new(Mutex::new(Engine::new(DemoGraph::new(), GraphLayoutMode::default())));
        let engine3d = Arc::new(Mutex::new(Engine3D::new(DemoGraph::new(), GraphLayoutMode3D::default())));
        let camera_fit = CameraFitFlag::new();

        for (fixture, expected) in [
            (Fixture::Clusters, LayoutKind::Force),
            (Fixture::Tree, LayoutKind::Hierarchical),
            (Fixture::Hierarchy, LayoutKind::Hierarchical),
            (Fixture::Sparse, LayoutKind::Force),
        ] {
            rebuild_engines(&engine, &engine3d, fixture, &camera_fit);
            assert_eq!(DemoApp::lock(&engine).layout.kind(), expected, "2D {fixture:?} must default to {expected:?}");
            assert_eq!(DemoApp::lock3d(&engine3d).layout.kind(), expected, "3D {fixture:?} must default to {expected:?}");
        }
    }

    /// The live-app twin of the pure availability test above: a REAL
    /// `DemoApp` on the tree fixture shows all 3 modes in both
    /// dimensions with the fixture default (LAYERED) highlighted, and
    /// the default clusters fixture shows FORCE alone.
    #[test]
    fn hud_layout_reflects_per_fixture_layout_availability_in_both_dimensions() {
        let mut app = DemoApp::new();
        // Fresh app defaults to `Clusters` — a single FORCE mode, active.
        let layout = app.hud_layout();
        let modes: Vec<_> = layout.buttons.iter().filter(|b| matches!(b.control, HudControl::SetLayout(_))).collect();
        assert_eq!(modes.len(), 1, "clusters must expose exactly one layout mode");
        assert!(modes[0].control == HudControl::SetLayout(LayoutKind::Force) && modes[0].active);

        app.fixture.set(Fixture::Tree);
        rebuild_engines(&app.engine, &app.engine3d, Fixture::Tree, &app.camera_fit);
        let layout = app.hud_layout();
        let layered_button = layout.buttons.iter().find(|b| b.control == HudControl::SetLayout(LayoutKind::Hierarchical));
        assert!(layered_button.is_some_and(|b| b.active), "LAYERED must exist and be the tree fixture's active default");
        assert!(layout.buttons.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Radial)), "RADIAL button must exist too");

        app.dim.set(Dimension::ThreeD);
        let layout_3d = app.hud_layout();
        let layout_buttons_3d: Vec<_> =
            layout_3d.buttons.iter().filter(|b| matches!(b.control, HudControl::SetLayout(_))).collect();
        assert_eq!(layout_buttons_3d.len(), 3, "3D must show the same 3 tree-fixture modes as 2D");
        assert!(layout_buttons_3d.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Force) && !b.active));
        assert!(layout_buttons_3d.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Hierarchical) && b.active));
        assert!(layout_buttons_3d.iter().any(|b| b.control == HudControl::SetLayout(LayoutKind::Radial) && !b.active));
    }

    #[test]
    fn layout_button_click_switches_the_2d_engines_layout_kind_and_requests_a_camera_fit() {
        let mut app = DemoApp::new();
        // The tree fixture defaults to LAYERED — RADIAL is the inactive
        // mode a click genuinely SWITCHES to (clusters would have no
        // second mode to switch to at all, per the availability rule).
        app.fixture.set(Fixture::Tree);
        rebuild_engines(&app.engine, &app.engine3d, Fixture::Tree, &app.camera_fit);
        app.camera_fit.clear();
        let layout = app.hud_layout();
        let radial_button = layout
            .buttons
            .iter()
            .find(|b| b.control == HudControl::SetLayout(LayoutKind::Radial))
            .expect("RADIAL button must exist for the tree fixture while 2D is active");
        let (x, y) = (radial_button.rect.center_x(), radial_button.rect.center_y());

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));

        assert_eq!(DemoApp::lock(&app.engine).layout.kind(), LayoutKind::Radial);
        assert!(app.camera_fit.needs_fit(), "switching layout must request a camera re-fit, same convention a fixture switch already follows");
    }

    /// Owner defect fix 2026-07-23 — the exact defect report: the LAYOUT
    /// section's FORCE button in 3D used to be a single, hardcoded,
    /// always-active, ALWAYS-DEAD button — clicking it did nothing at
    /// all («зажат форс, нихуя не переключается, не отключается»). Now
    /// it toggles pause/resume in place.
    #[test]
    fn re_clicking_the_active_force_button_pauses_and_resumes_in_place_in_3d() {
        let mut app = DemoApp::new();
        app.dim.set(Dimension::ThreeD);
        assert!(!DemoApp::lock3d(&app.engine3d).layout.paused());

        let layout = app.hud_layout();
        let force_button = layout
            .buttons
            .iter()
            .find(|b| b.control == HudControl::SetLayout(LayoutKind::Force))
            .expect("FORCE button must exist in 3D");
        assert!(force_button.active, "FORCE must be the default active mode");
        let (x, y) = (force_button.rect.center_x(), force_button.rect.center_y());

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));
        assert!(DemoApp::lock3d(&app.engine3d).layout.paused(), "re-clicking the active FORCE button must pause the sim");

        // The paused state must show up in the HUD's own button
        // (distinct visual state) and in agent_state.
        let paused_layout = app.hud_layout();
        let paused_button =
            paused_layout.buttons.iter().find(|b| b.control == HudControl::SetLayout(LayoutKind::Force)).expect("still there");
        assert!(paused_button.paused, "the HUD button itself must report the paused visual state");
        assert!(paused_button.label.contains("PAUSED"));

        // Re-click again resumes.
        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));
        assert!(!DemoApp::lock3d(&app.engine3d).layout.paused(), "a second re-click must resume the sim");
    }

    #[test]
    fn re_clicking_the_active_force_button_pauses_and_resumes_in_place_in_2d() {
        let mut app = DemoApp::new();
        assert!(!DemoApp::lock(&app.engine).layout.paused());

        let layout = app.hud_layout();
        let force_button = layout
            .buttons
            .iter()
            .find(|b| b.control == HudControl::SetLayout(LayoutKind::Force))
            .expect("FORCE button must exist in 2D");
        let (x, y) = (force_button.rect.center_x(), force_button.rect.center_y());

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));
        assert!(DemoApp::lock(&app.engine).layout.paused(), "re-clicking the active FORCE button must pause the sim");

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));
        assert!(!DemoApp::lock(&app.engine).layout.paused(), "a second re-click must resume the sim");
    }

    /// Re-clicking an already-active ONE-SHOT mode (LAYERED/RADIAL) must
    /// re-run the layout and re-fit — a visibly honest action, not a
    /// silent no-op, and NOT a pause (one-shot layouts have nothing
    /// ongoing to freeze).
    #[test]
    fn re_clicking_the_active_layered_button_re_runs_the_layout_and_requests_a_fit() {
        let mut app = DemoApp::new();
        app.fixture.set(Fixture::Tree);
        rebuild_engines(&app.engine, &app.engine3d, Fixture::Tree, &app.camera_fit);
        assert_eq!(DemoApp::lock(&app.engine).layout.kind(), LayoutKind::Hierarchical);
        app.camera_fit.clear();

        // Hand-nudge a settled position so the re-run is provably fresh.
        {
            let mut engine = DemoApp::lock(&app.engine);
            engine.particles[0].x = 123_456.0;
        }

        let layout = app.hud_layout();
        let layered_button = layout
            .buttons
            .iter()
            .find(|b| b.control == HudControl::SetLayout(LayoutKind::Hierarchical))
            .expect("LAYERED must be active for the tree fixture");
        assert!(layered_button.active);
        let (x, y) = (layered_button.rect.center_x(), layered_button.rect.center_y());

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));

        assert_eq!(DemoApp::lock(&app.engine).layout.kind(), LayoutKind::Hierarchical, "re-click must not change kind");
        assert!(app.camera_fit.needs_fit(), "re-running a one-shot layout must request a camera re-fit");
        // Ticking once must overwrite the hand-nudged position (the
        // one-shot layout genuinely recomputed, `reheat` cleared its
        // `computed` flag).
        DemoApp::lock(&app.engine).tick(1.0 / 60.0);
        assert_ne!(DemoApp::lock(&app.engine).particles[0].x, 123_456.0);
    }

    #[test]
    fn clicking_an_inactive_layout_button_in_3d_switches_kind_and_fits() {
        let mut app = DemoApp::new();
        // RADIAL only exists for tree-shaped fixtures (availability
        // rule) — the tree default is LAYERED, so RADIAL is the
        // inactive button this test clicks.
        app.fixture.set(Fixture::Tree);
        rebuild_engines(&app.engine, &app.engine3d, Fixture::Tree, &app.camera_fit);
        app.dim.set(Dimension::ThreeD);
        assert_eq!(DemoApp::lock3d(&app.engine3d).layout.kind(), LayoutKind::Hierarchical);

        let layout = app.hud_layout();
        let radial_button = layout
            .buttons
            .iter()
            .find(|b| b.control == HudControl::SetLayout(LayoutKind::Radial))
            .expect("RADIAL button must exist in 3D");
        let (x, y) = (radial_button.rect.center_x(), radial_button.rect.center_y());

        assert!(app.on_event(&PlatformEvent::PointerDown { x, y, button: MouseButton::Left }));
        assert!(app.on_event(&PlatformEvent::PointerUp { x, y, button: MouseButton::Left }));

        assert_eq!(DemoApp::lock3d(&app.engine3d).layout.kind(), LayoutKind::Radial);
        assert!(!DemoApp::lock3d(&app.engine3d).layout.paused(), "switching kind must not leave the new kind paused");
    }

    /// The task's own explicit gate: prove the hierarchical layout
    /// actually produces a LAYERED tree for the `tree` fixture through
    /// this demo's own engine wiring (not just `uzor-graph`'s own
    /// isolated `HierarchicalLayout` unit tests) — root strictly above
    /// EVERY one of its children, across the whole tree, and frozen
    /// (one-shot) on a second tick.
    #[test]
    fn hierarchical_layout_settles_the_tree_fixture_into_strict_top_down_layers() {
        let (graph, positions, _clusters) = build_fixture(Fixture::Tree);
        let mut engine = Engine::new(graph, GraphLayoutMode::default());
        engine.seed_positions(&positions);
        engine.layout.set_kind(LayoutKind::Hierarchical);
        engine.tick(1.0 / 60.0);

        // `build_tree_internal` pushes the root as the very first node
        // of a fresh graph, so it is always `NodeIndex(0)` — and every
        // edge in the plain `tree` fixture is parent->child in
        // construction order (no cross-links, unlike `hierarchy`).
        let root = NodeIndex(0);
        let root_y = engine.particles[root.index()].y;
        let mut child_edges_checked = 0usize;
        for (_, edge) in engine.graph.edges() {
            if edge.from == root {
                child_edges_checked += 1;
                let child_y = engine.particles[edge.to.index()].y;
                assert!(
                    root_y < child_y,
                    "hierarchical layout must place the root strictly ABOVE (a shallower layer than) its child \
                     (root_y={root_y}, child_y={child_y})"
                );
            }
        }
        assert!(child_edges_checked > 0, "the tree fixture's root must have at least one child edge to prove layering against");

        // One-shot layout — a second tick must not move the root away
        // from its computed layer.
        engine.tick(1.0 / 60.0);
        assert_eq!(engine.particles[root.index()].y, root_y, "hierarchical layout is one-shot — a second tick must not move the root");
    }
}
