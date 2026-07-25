//! Five-leg (tiny-skia / vello-cpu / vello-gpu / urx-cpu / urx-gpu)
//! cross-family proof coverage for `uzor-graph`'s own 2D draw surface
//! (`src/render.rs`) — `uzor-proof-harness`, the same instrument
//! `uzor-figures` already uses for its own proof suite.
//!
//! **Why this exists**: `uzor-examples/src/l4/force_graph_demo.rs`'s
//! existing backend-comparison coverage (`dump_comparison_pngs` +
//! `parity_harness`) is a WITHIN-FAMILY comparison — urx-cpu vs
//! urx-native both rasterize the SAME `Scene` recorded through ONE
//! `UrxRenderContext`, so a bug in the recording layer itself (wrong
//! coordinates, a dropped primitive, a mis-set color) would make both
//! legs agree WRONGLY; its vello leg is an unasserted visual dump nobody
//! has verified. This file drives `uzor-graph`'s real draw functions
//! through THREE independently-implemented rasterizer families instead
//! (tiny-skia, vello, urx), each from its own from-scratch recording of
//! the exact same draw closure, catching exactly the class of bug a
//! within-family comparison structurally cannot.
//!
//! **Determinism is mandatory** — every scene hand-places `Particle`
//! positions with literal world coordinates; nothing here ever runs the
//! force simulation (`Layout::tick`), so every composite PNG is
//! byte-reproducible across runs.
//!
//! Every scene uses `Camera2D { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }` (the
//! identity transform, so world coordinates ARE screen pixel
//! coordinates — easy to reason about node/edge/label placement by
//! hand) EXCEPT `graph_camera_zoomed_and_panned`, which deliberately
//! runs the identical baseline scene through a non-identity camera to
//! re-verify the CTM-composition path after the URX arc's own
//! world-frame-vs-local-frame `then_*`/`pre_*` fix (see that test's own
//! doc comment).
//!
//! Run: `cargo test -p uzor-graph --test five_leg_render -- --nocapture`

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use uzor::types::Rect;
use uzor_graph::camera::Camera2D;
use uzor_graph::cluster::ClusterRegistry;
use uzor_graph::graph::{Graph, NodeIndex};
use uzor_graph::label_grid::{LabelLodConfig, DEFAULT_LABEL_DENSITY};
use uzor_graph::particle::Particle;
use uzor_graph::render::{
    draw_box_select_rect, draw_cluster_edges, draw_cluster_supernodes, draw_edges, draw_hover_card, draw_nodes, DrawContext, HoverCardInfo,
};
use uzor_graph::theme::GraphTheme;
use uzor_graph::FocusSet;
use uzor_proof_harness::{ChannelTolerance, MultiLegDiff, MultiLegRender, RenderContext};

type G = Graph<(), ()>;

const CANVAS_W: u32 = 900;
const CANVAS_H: u32 = 650;

/// Node-label halo color — mirrors `uzor-graph::engine::DEFAULT_LABEL_HALO`
/// (`pub(crate)`, not reachable from this integration test crate) and the
/// canvas background every scene fills with — the exact real-demo
/// convention (a halo that matches the canvas so it "clears" an edge
/// stroke crossing a glyph instead of drawing a visible ring).
const LABEL_HALO: &str = "#0d0f14";
const CANVAS_BG: &str = "#0d0f14";

fn identity_camera() -> Camera2D {
    Camera2D { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
}

fn viewport() -> Rect {
    Rect::new(0.0, 0.0, CANVAS_W as f64, CANVAS_H as f64)
}

fn out_dir() -> PathBuf {
    // Fixed path (not CARGO_MANIFEST_DIR-relative) — `uzor/out/` is the
    // shared human-eyeball drop point for every headless proof render in
    // this workspace (same convention `uzor-figures`/`uzor-graph`'s own
    // existing `proof_tests` modules use).
    PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
}

fn fill_background(ctx: &mut dyn RenderContext) {
    ctx.set_fill_color(CANVAS_BG);
    ctx.fill_rect(0.0, 0.0, CANVAS_W as f64, CANVAS_H as f64);
}

fn empty_focus() -> FocusSet {
    FocusSet::empty()
}

fn empty_selection() -> BTreeSet<NodeIndex> {
    BTreeSet::new()
}

fn empty_node_set() -> HashSet<NodeIndex> {
    HashSet::new()
}

/// A ~12-node graph, several categories (exercises `category_color`'s
/// hash-palette), varied node radii, a hub star + a small ring, plus two
/// long edges crossing the whole viewport diagonally. World coordinates
/// double as screen-pixel coordinates under [`identity_camera`], so
/// placement below can be reasoned about directly.
///
/// Returns `(graph, particles, nodes)` — `nodes[0..=10]` are the 11
/// non-hub nodes in declaration order, `nodes[11]` is the hub.
fn build_baseline_graph() -> (G, Vec<Particle>, Vec<NodeIndex>) {
    let mut graph = G::new();
    let mut particles = Vec::new();
    let mut nodes = Vec::new();

    // (label, category, radius, x, y)
    const SPECS: [(&str, &str, f32, f32, f32); 11] = [
        ("root", "alpha", 10.0, 80.0, 120.0),
        ("atlas", "beta", 6.0, 220.0, 80.0),
        ("beacon", "gamma", 5.0, 380.0, 140.0),
        ("cobalt", "alpha", 7.0, 520.0, 90.0),
        ("delta", "delta", 4.0, 660.0, 160.0),
        ("ember", "beta", 8.0, 800.0, 260.0),
        ("fjord", "gamma", 5.0, 700.0, 420.0),
        ("granite", "alpha", 6.0, 560.0, 520.0),
        ("harbor", "delta", 9.0, 380.0, 560.0),
        ("ion", "beta", 4.0, 200.0, 500.0),
        ("juniper", "gamma", 7.0, 60.0, 380.0),
    ];
    for &(label, category, radius, x, y) in &SPECS {
        let id = graph.push_node((), label, category, radius);
        nodes.push(id);
        particles.push(Particle::at(x, y));
    }
    let hub = graph.push_node((), "hub", "delta", 10.0);
    nodes.push(hub);
    particles.push(Particle::at(430.0, 300.0));

    // Hub star to the first 5 nodes.
    for &n in &nodes[0..5] {
        graph.push_edge(hub, n, 1.4, ());
    }
    // A small ring among the remaining 6 (indices 5..10).
    let ring = [nodes[5], nodes[6], nodes[7], nodes[8], nodes[9], nodes[10]];
    for i in 0..ring.len() {
        graph.push_edge(ring[i], ring[(i + 1) % ring.len()], 1.0, ());
    }
    // Two long edges crossing the viewport diagonally — the baseline
    // scene's own explicit requirement.
    graph.push_edge(nodes[0], nodes[5], 0.8, ());
    graph.push_edge(nodes[10], nodes[3], 0.8, ());

    (graph, particles, nodes)
}

fn draw_baseline_scene(ctx: &mut dyn RenderContext, graph: &G, particles: &[Particle], camera: &Camera2D) {
    fill_background(ctx);
    let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
    let focus = empty_focus();
    let selection = empty_selection();
    let hidden = empty_node_set();
    let forced = empty_node_set();
    let lod = LabelLodConfig::default();
    let theme = GraphTheme::dark();
    let dctx = DrawContext {
        camera,
        viewport: viewport(),
        visible: &visible,
        focus: &focus,
        selection: &selection,
        hovered: None,
        hidden: &hidden,
        label_density: 0.0,
        label_halo: LABEL_HALO,
        forced_labels: &forced,
        label_lod: &lod,
        theme: &theme,
    };
    draw_edges(ctx, graph, particles, &dctx);
    draw_nodes(ctx, graph, particles, &dctx);
}

fn run_proof(name: &str, render: &MultiLegRender) -> MultiLegDiff {
    let diff = MultiLegDiff::compute(render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[{name}] {line}");
    }
    uzor_proof_harness::write_composite_png(render, &out_dir().join(format!("{name}_backends.png")))
        .unwrap_or_else(|e| panic!("{name}: composite PNG should write: {e}"));
    diff
}

// ── Scene 1: baseline nodes + edges ─────────────────────────────────────

#[test]
fn graph_nodes_and_edges() {
    let (graph, particles, _nodes) = build_baseline_graph();
    let camera = identity_camera();
    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        draw_baseline_scene(ctx, &graph, &particles, &camera);
    });
    let diff = run_proof("graph_nodes_and_edges", &render);
    assert!(diff.all_within_budget(), "graph_nodes_and_edges: structural backend divergence detected");
}

// ── Scene 2: focus dimming ───────────────────────────────────────────────

/// Same graph as [`graph_nodes_and_edges`], with an active `FocusSet`: the
/// hub is hovered and its depth-1 neighborhood (its 5 star edges + their
/// far endpoints) is highlighted at full opacity, everything else dims to
/// `DIM_ALPHA` (0.15) — a good divergence detector since alpha blending
/// (not just AA/text shaping) is exactly what differs here.
#[test]
fn graph_focus_dimming() {
    let (graph, particles, nodes) = build_baseline_graph();
    let camera = identity_camera();
    let hub = nodes[11];
    let mut focus = FocusSet::empty();
    focus.select_many(graph.neighborhood_focus_keys_depth(hub, 1));

    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        fill_background(ctx);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let selection = empty_selection();
        let hidden = empty_node_set();
        let forced = empty_node_set();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let dctx = DrawContext {
            camera: &camera,
            viewport: viewport(),
            visible: &visible,
            focus: &focus,
            selection: &selection,
            hovered: Some(hub),
            hidden: &hidden,
            label_density: DEFAULT_LABEL_DENSITY,
            label_halo: LABEL_HALO,
            forced_labels: &forced,
            label_lod: &lod,
            theme: &theme,
        };
        draw_edges(ctx, &graph, &particles, &dctx);
        draw_nodes(ctx, &graph, &particles, &dctx);
    });
    let diff = run_proof("graph_focus_dimming", &render);
    assert!(diff.all_within_budget(), "graph_focus_dimming: structural backend divergence detected");
}

// ── Scene 3: labels + halo ───────────────────────────────────────────────

/// Dedicated fixture (not the baseline graph) built specifically to
/// exercise the label-LOD path at a high `label_density` (so every real
/// node's label draws) AND `fill_text_with_halo` — the highest-risk
/// primitive across families, per the harness's own findings (glyph-
/// outline rounding, rotation ignored, `measure_text` divergence). Two
/// small zero-radius/zero-label anchor nodes (`cross-a`/`cross-b`) carry
/// one deliberate edge routed to pass straight through `edge-crosser`'s
/// own label text footprint (`render::draw_nodes`'s label anchor formula
/// is `(sx + r + 4, sy + 4)` — with `edge-crosser` at `(650, 150)` r=6,
/// that's `(660, 154)`; the crossing edge runs the horizontal line
/// `y = 154` from `x = 500` to `x = 820`, squarely through it).
fn build_label_scene_graph() -> (G, Vec<Particle>) {
    let mut graph = G::new();
    let mut particles = Vec::new();

    const SPECS: [(&str, &str, f32, f32, f32); 8] = [
        ("north-relay", "alpha", 8.0, 120.0, 100.0),
        ("south-bridge", "beta", 6.0, 120.0, 300.0),
        ("east-gate", "gamma", 7.0, 420.0, 100.0),
        ("west-vault", "delta", 5.0, 420.0, 300.0),
        ("core-index", "alpha", 9.0, 270.0, 200.0),
        ("edge-crosser", "beta", 6.0, 650.0, 150.0),
        ("outer-ring", "gamma", 5.0, 650.0, 350.0),
        ("deep-node", "delta", 4.0, 800.0, 250.0),
    ];
    let mut ids = Vec::new();
    for &(label, category, radius, x, y) in &SPECS {
        let id = graph.push_node((), label, category, radius);
        ids.push(id);
        particles.push(Particle::at(x, y));
    }
    let core = ids[4];
    for &target in &[ids[0], ids[1], ids[2], ids[3], ids[5]] {
        graph.push_edge(core, target, 1.0, ());
    }
    graph.push_edge(ids[5], ids[6], 1.0, ());
    graph.push_edge(ids[6], ids[7], 1.0, ());

    let cross_a = graph.push_node((), "", "", 0.5);
    let cross_b = graph.push_node((), "", "", 0.5);
    particles.push(Particle::at(500.0, 154.0));
    particles.push(Particle::at(820.0, 154.0));
    graph.push_edge(cross_a, cross_b, 0.6, ());

    (graph, particles)
}

#[test]
fn graph_labels() {
    let (graph, particles) = build_label_scene_graph();
    let camera = identity_camera();
    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        fill_background(ctx);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let focus = empty_focus();
        let selection = empty_selection();
        let hidden = empty_node_set();
        let forced = empty_node_set();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let dctx = DrawContext {
            camera: &camera,
            viewport: viewport(),
            visible: &visible,
            focus: &focus,
            selection: &selection,
            hovered: None,
            hidden: &hidden,
            // High density guarantees every node's label clears the
            // grid-cell quota, exercising the LOD path's "show" branch
            // rather than its skip branch.
            label_density: 20.0,
            label_halo: LABEL_HALO,
            forced_labels: &forced,
            label_lod: &lod,
            theme: &theme,
        };
        draw_edges(ctx, &graph, &particles, &dctx);
        draw_nodes(ctx, &graph, &particles, &dctx);
    });
    let diff = run_proof("graph_labels", &render);
    assert!(diff.all_within_budget(), "graph_labels: structural backend divergence detected");
}

// ── Scene 4: selection rings + box-select overlay ───────────────────────

#[test]
fn graph_selection_and_box_select() {
    let (graph, particles, nodes) = build_baseline_graph();
    let camera = identity_camera();
    let selection: BTreeSet<NodeIndex> = [nodes[2], nodes[7]].into_iter().collect();

    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        fill_background(ctx);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let focus = empty_focus();
        let hidden = empty_node_set();
        let forced = empty_node_set();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let dctx = DrawContext {
            camera: &camera,
            viewport: viewport(),
            visible: &visible,
            focus: &focus,
            selection: &selection,
            hovered: None,
            hidden: &hidden,
            label_density: 0.0,
            label_halo: LABEL_HALO,
            forced_labels: &forced,
            label_lod: &lod,
            theme: &theme,
        };
        draw_edges(ctx, &graph, &particles, &dctx);
        draw_nodes(ctx, &graph, &particles, &dctx);
        draw_box_select_rect(ctx, Rect::new(150.0, 150.0, 320.0, 220.0), &theme);
    });
    let diff = run_proof("graph_selection_and_box_select", &render);
    assert!(diff.all_within_budget(), "graph_selection_and_box_select: structural backend divergence detected");
}

// ── Scene 5: hover card ──────────────────────────────────────────────────

#[test]
fn graph_hover_card() {
    let (graph, particles, nodes) = build_baseline_graph();
    let camera = identity_camera();
    let hovered = nodes[3]; // "cobalt"
    let node = graph.get_node(hovered).expect("hovered node exists in the baseline fixture");
    let p = particles[hovered.index()];
    let anchor = camera.world_to_screen((p.x as f64, p.y as f64), viewport());
    let info =
        HoverCardInfo { label: &node.label, category: &node.category, degree: graph.degree(hovered), pinned: false };

    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        fill_background(ctx);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let focus = empty_focus();
        let selection = empty_selection();
        let hidden = empty_node_set();
        let forced = empty_node_set();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let dctx = DrawContext {
            camera: &camera,
            viewport: viewport(),
            visible: &visible,
            focus: &focus,
            selection: &selection,
            hovered: Some(hovered),
            hidden: &hidden,
            label_density: 0.0,
            label_halo: LABEL_HALO,
            forced_labels: &forced,
            label_lod: &lod,
            theme: &theme,
        };
        draw_edges(ctx, &graph, &particles, &dctx);
        draw_nodes(ctx, &graph, &particles, &dctx);
        draw_hover_card(ctx, anchor, &info, viewport(), &theme.hover_card);
    });
    let diff = run_proof("graph_hover_card", &render);
    assert!(diff.all_within_budget(), "graph_hover_card: structural backend divergence detected");
}

// ── Scene 6: collapsed cluster + aggregated cluster edges ───────────────

/// Collapses `fjord`/`granite`/`harbor`/`ion` (baseline nodes 6-9, a
/// contiguous run of the ring) into one super-node. `fjord`'s own ring
/// edge to `ember` (outside the cluster) and `ion`'s own ring edge to
/// `juniper` (also outside) both survive collapse as two aggregated
/// cross-cluster edges drawn from the representative (`fjord`, the first
/// member) — exercising `draw_cluster_edges`/`draw_cluster_supernodes`'s
/// double-ring + "×N" halo-protected label together.
#[test]
fn graph_cluster_supernodes() {
    let (mut graph, mut particles, nodes) = build_baseline_graph();
    let camera = identity_camera();
    let members = vec![nodes[6], nodes[7], nodes[8], nodes[9]];
    let mut clusters = ClusterRegistry::default();
    let group = clusters.define(&graph, members).expect("cluster over a non-empty member list");
    assert!(clusters.collapse(group, &mut graph, &mut particles), "collapse should succeed on a freshly-defined cluster");

    let hidden: HashSet<NodeIndex> = clusters.hidden_nodes().collect();
    let forced: HashSet<NodeIndex> = clusters.collapsed_clusters().map(|c| c.representative).collect();

    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        fill_background(ctx);
        // `draw_nodes` iterates `ctx.visible` directly with no `hidden`
        // check of its own (`ctx.hidden` is only consulted by
        // `draw_edges`'s own edge-skip test) — the REAL engine's
        // `refresh_visible` excludes the cluster-hidden set from
        // `self.visible` upstream (`engine.rs::refresh_visible`), so this
        // fixture must do the same or every pinned-onto-the-centroid
        // hidden member (still a real, visible-by-default node otherwise)
        // draws its own full-size circle stacked exactly on top of the
        // representative at the collapse centroid.
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).filter(|id| !hidden.contains(id)).collect();
        let focus = empty_focus();
        let selection = empty_selection();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let dctx = DrawContext {
            camera: &camera,
            viewport: viewport(),
            visible: &visible,
            focus: &focus,
            selection: &selection,
            hovered: None,
            hidden: &hidden,
            label_density: 6.0,
            label_halo: LABEL_HALO,
            forced_labels: &forced,
            label_lod: &lod,
            theme: &theme,
        };
        draw_edges(ctx, &graph, &particles, &dctx);
        draw_cluster_edges(ctx, &particles, &dctx, &clusters);
        draw_nodes(ctx, &graph, &particles, &dctx);
        draw_cluster_supernodes(ctx, &graph, &particles, &dctx, &clusters);
    });
    let diff = run_proof("graph_cluster_supernodes", &render);
    assert!(diff.all_within_budget(), "graph_cluster_supernodes: structural backend divergence detected");
}

// ── Scene 7: camera zoom + pan (CTM composition re-verification) ────────

/// The exact same baseline scene as [`graph_nodes_and_edges`], through a
/// [`Camera2D`] with a non-1.0 zoom AND a non-zero pan. Deliberate: the
/// URX arc fixed a CTM-composition bug where several crates composed
/// transforms in the WORLD frame (`kurbo`'s `then_*`) instead of the
/// LOCAL frame (`pre_*`) — `uzor-graph`'s own camera path (`ctx.
/// translate`/`ctx.scale`-free; it pre-transforms every coordinate in
/// Rust via `Camera2D::world_to_screen`/`node_screen_radius` before ever
/// calling into a `RenderContext`) was never re-verified against that
/// fix. If a residual defect existed here, this is exactly where it
/// would show: node centers, edge endpoints, and stroke widths all have
/// to land in the SAME place across every backend despite the extra
/// pan+zoom composition step.
#[test]
fn graph_camera_zoomed_and_panned() {
    let (graph, particles, _nodes) = build_baseline_graph();
    let camera = Camera2D { pan_x: -140.0, pan_y: 95.0, zoom: 1.6 };
    let render = MultiLegRender::capture(CANVAS_W, CANVAS_H, |ctx| {
        draw_baseline_scene(ctx, &graph, &particles, &camera);
    });
    let diff = run_proof("graph_camera_zoomed_and_panned", &render);
    assert!(diff.all_within_budget(), "graph_camera_zoomed_and_panned: structural backend divergence detected");
}
