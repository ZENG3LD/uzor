//! Draw nodes/edges over `RenderContext`'s batch path
//! (`BatchPainter::draw_circle_batch`/`draw_line_batch`) plus viewport
//! culling. Node position/radius come from the SAME `Camera2D` helpers
//! [`crate::interaction::pick`] uses for hit-testing — one source of
//! truth for screen position, per the engine design doc §4.2/§0.3.
//!
//! Direct `uzor-render-wgpu-instanced` `QuadInstance`/`LineInstance`
//! integration is deliberately NOT wired this run — the engine design
//! doc (§6) frames that as an escape valve gated on measured frame time
//! on a fully-expanded worst case, not a speculative baseline build.
//! `BatchPainter` already gives one batched draw call per node color /
//! one per edge pass on every backend (tiny-skia/vello override it with
//! real batching); swapping in the instanced path later doesn't change
//! this module's public shape.

use std::collections::{BTreeSet, HashMap, HashSet};

use uzor::render::{CircleBatch, LineSegment, RenderContext};
use uzor::types::Rect;
use uzor_figures::guide::text_protect::fill_text_with_halo;
use uzor_figures::guide::tooltip::draw_tooltip;
use uzor_figures::interact::FocusSet;
use uzor_figures::theme::FigureTheme;

use crate::camera::Camera2D;
use crate::cluster::ClusterRegistry;
use crate::graph::{Graph, NodeIndex};
use crate::label_grid::{self, LabelCandidate};
use crate::particle::Particle;

/// Re-exported from `label_grid` (Wave 2.3 moved the constants there —
/// every label-LOD number lives in one module) so any existing
/// `crate::render::LOD_LABEL_FADE_{LOW,HIGH}` path keeps resolving.
pub use crate::label_grid::{LOD_LABEL_FADE_HIGH, LOD_LABEL_FADE_LOW};

const DIM_ALPHA: f64 = 0.15;

/// Deterministic category -> color mapping. No per-app configuration
/// needed for the default palette; category is an opaque string tag
/// (see [`crate::graph::GraphNode::category`]).
pub fn category_color(category: &str) -> &'static str {
    const PALETTE: &[&str] = &[
        "#4d90fe", "#e0703c", "#5cb87a", "#c94f7c", "#d9b64e",
        "#7e6bd9", "#3fb6c9", "#e0555a", "#8fbf5f", "#c78bd9",
    ];
    let mut hash: u32 = 2166136261;
    for b in category.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// Viewport-culled node list — `Graph`'s node positions intersected with
/// `camera.visible_world_aabb(viewport)` plus a world-unit margin so
/// nodes don't pop at the edge. Shared by render, the agent's
/// `visible_node_count`, and the pick candidate set.
pub fn cull_visible<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    camera: &Camera2D,
    viewport: Rect,
) -> Vec<NodeIndex> {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return Vec::new();
    }
    let aabb = camera.visible_world_aabb(viewport);
    const MARGIN: f64 = 64.0;
    graph
        .nodes()
        .filter_map(|(id, node)| {
            let p = particles.get(id.index())?;
            let x = p.x as f64;
            let y = p.y as f64;
            let r = node.radius as f64 + MARGIN;
            if x >= aabb.min_x - r && x <= aabb.max_x + r && y >= aabb.min_y - r && y <= aabb.max_y + r {
                Some(id)
            } else {
                None
            }
        })
        .collect()
}

/// Shared per-frame draw parameters for [`draw_edges`]/[`draw_nodes`] —
/// bundled into one struct (rather than seven-plus loose arguments) so
/// both draw calls read from the exact same camera/viewport/visible-set/
/// focus snapshot.
pub struct DrawContext<'a> {
    pub camera: &'a Camera2D,
    pub viewport: Rect,
    pub visible: &'a [NodeIndex],
    pub focus: &'a FocusSet,
    /// Every node in the current multi-selection (Wave 2.4) — every
    /// member gets `draw_nodes`'s white selection ring. Supersedes the
    /// old `Option<NodeIndex>` `selected` field: a single click still
    /// lands here as a one-element set (`GraphEngine::select` sets
    /// BOTH `selected` and `selection`), so the visible ring behavior for
    /// a plain single click is unchanged — [`crate::engine::GraphEngine::
    /// selected`] (the "last individually clicked" facts-panel value)
    /// no longer needs its own `DrawContext` slot since ring-drawing
    /// generalized to the whole set.
    pub selection: &'a BTreeSet<NodeIndex>,
    pub hovered: Option<NodeIndex>,
    /// Nodes hidden by a collapsed cluster (every member except that
    /// cluster's representative — see `crate::cluster`). Empty when no
    /// cluster is collapsed. Edges touching a hidden node are skipped
    /// entirely by [`draw_edges`] — [`draw_cluster_edges`] draws the
    /// aggregated substitute instead.
    pub hidden: &'a HashSet<NodeIndex>,
    /// Label-LOD density param (Wave 2.3 — `crate::label_grid`'s
    /// `labelDensity`, "labels per 100px cell at zoom 1.0"). Passed
    /// through here rather than as a ninth loose argument to
    /// [`draw_nodes`].
    pub label_density: f64,
    /// Node-label halo color (owner defect report: thin edge strokes
    /// crossing node label text made it unreadable) — [`draw_nodes`]
    /// paints each label's own 4-direction offset-fill halo in this color
    /// before the real label fill, via
    /// `uzor_figures::guide::text_protect::fill_text_with_halo`. See
    /// [`crate::engine::GraphEngine::label_halo`]/[`crate::engine::
    /// GraphEngine::set_label_halo`] for the owning field this is read
    /// from every frame.
    pub label_halo: &'a str,
    /// Nodes that must show their label regardless of the grid quota —
    /// collapsed-cluster representatives. Hover/selection-neighbor
    /// forcing does NOT need a separate entry here: it's derived inline
    /// in [`labels_to_draw`] from `focus`, which already carries the
    /// full neighborhood key set.
    pub forced_labels: &'a HashSet<NodeIndex>,
}

/// Draw every visible edge, dimming any edge outside an active
/// [`FocusSet`]. Edges touching a node hidden by cluster collapse
/// (`ctx.hidden`) are skipped entirely — `draw_cluster_edges` draws the
/// aggregated substitute for those. Returns the number of edges drawn.
pub fn draw_edges<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> usize {
    let visible_set: HashSet<NodeIndex> = ctx.visible.iter().copied().collect();
    let mut segments: Vec<LineSegment> = Vec::new();
    let mut dim_segments: Vec<LineSegment> = Vec::new();

    for (eid, edge) in graph.edges() {
        if ctx.hidden.contains(&edge.from) || ctx.hidden.contains(&edge.to) {
            continue;
        }
        if !visible_set.contains(&edge.from) && !visible_set.contains(&edge.to) {
            continue;
        }
        let (Some(a), Some(b)) = (particles.get(edge.from.index()), particles.get(edge.to.index())) else {
            continue;
        };
        let (ax, ay) = ctx.camera.world_to_screen((a.x as f64, a.y as f64), ctx.viewport);
        let (bx, by) = ctx.camera.world_to_screen((b.x as f64, b.y as f64), ctx.viewport);
        let seg = LineSegment { x1: ax, y1: ay, x2: bx, y2: by };
        if ctx.focus.is_active() && !ctx.focus.is_selected(u64::from(eid)) {
            dim_segments.push(seg);
        } else {
            segments.push(seg);
        }
    }

    let drawn = segments.len() + dim_segments.len();
    // Round caps + >=1.5px width: sub-1.5px butt-capped hairlines at an
    // angle read as a beaded staircase on a standard-DPI display even
    // with correct AA (live-verified 2026-07-18); industry engines
    // (d3/sigma/obsidian) stroke edges at 1.5-2px for exactly this
    // reason.
    render.set_line_cap("round");
    if !dim_segments.is_empty() {
        render.set_global_alpha(DIM_ALPHA);
        render.draw_line_batch(&dim_segments, "#5a6070", 1.3);
        render.set_global_alpha(1.0);
    }
    if !segments.is_empty() {
        render.draw_line_batch(&segments, "#7c8496", 1.7);
    }
    render.set_line_cap("butt");
    drawn
}

/// Per-frame node/label draw counts. [`crate::engine::GraphEngine::draw`]
/// stashes `labels_drawn` for the `labels.drawn_last_frame` agent-state
/// field (Wave 2.3 gate — the drawn label count is a test/verification
/// aid, not just an internal detail).
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeDrawStats {
    pub nodes_drawn: usize,
    pub labels_drawn: usize,
}

/// The exact set of `NodeIndex` that will draw a label this frame, given
/// `ctx` — extracted out of [`draw_nodes`] as a pure function (no
/// `RenderContext` needed) so the wave-2.3 gate's grid-quota/degree-
/// priority/forced-union/determinism/zoom-ramp tests can exercise it
/// directly without a mock renderer.
///
/// Two disjoint paths (Wave 2.3 dim-interaction fix):
/// - **`focus` active** (a hover or click-selection is live): the label
///   cloud must vanish for every dimmed node, not just fade — so the
///   ENTIRE `label_grid` quota pass is bypassed and the shown set is
///   exactly the focus-selected nodes (hovered/selected node + its
///   highlighted neighbors) unioned with `ctx.forced_labels` (a
///   collapsed-cluster representative stays labeled even while some
///   unrelated node is focused elsewhere).
/// - **no active focus**: the ordinary `label_grid::select_labels` LOD
///   pass, unioned with `ctx.forced_labels`.
fn labels_to_draw<N, E>(graph: &Graph<N, E>, particles: &[Particle], ctx: &DrawContext<'_>) -> HashSet<NodeIndex> {
    if ctx.focus.is_active() {
        let mut shown: HashSet<NodeIndex> =
            ctx.visible.iter().copied().filter(|&id| ctx.focus.is_selected(u64::from(id))).collect();
        shown.extend(ctx.forced_labels.iter().copied());
        return shown;
    }

    let candidates: Vec<LabelCandidate> = ctx
        .visible
        .iter()
        .filter_map(|&id| {
            let p = particles.get(id.index())?;
            let node = graph.get_node(id)?;
            Some(LabelCandidate {
                node: id,
                screen_pos: ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport),
                degree: graph.degree(id),
                screen_radius: ctx.camera.node_screen_radius(node.radius),
            })
        })
        .collect();

    label_grid::select_labels(&candidates, ctx.viewport, ctx.camera.zoom, ctx.label_density, ctx.forced_labels)
}

/// Draw every visible node as a circle (radius from `Camera2D::node_screen_radius`,
/// color from [`category_color`]), plus a selection/hover ring and
/// label-LOD labels (Wave 2.3 — `crate::label_grid`'s sigma `LabelGrid`
/// port, alpha boosted by degree). Returns node/label draw counts.
pub fn draw_nodes<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> NodeDrawStats {
    let mut by_color: HashMap<&'static str, Vec<CircleBatch>> = HashMap::new();
    let mut dim: Vec<CircleBatch> = Vec::new();

    for &id in ctx.visible {
        let (Some(p), Some(node)) = (particles.get(id.index()), graph.get_node(id)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);
        let circle = CircleBatch { cx: sx, cy: sy, r };
        if ctx.focus.is_active() && !ctx.focus.is_selected(u64::from(id)) {
            dim.push(circle);
        } else {
            by_color.entry(category_color(&node.category)).or_default().push(circle);
        }
    }

    if !dim.is_empty() {
        render.set_global_alpha(DIM_ALPHA);
        render.draw_circle_batch(&dim, "#6b7280");
        render.set_global_alpha(1.0);
    }
    for (color, circles) in &by_color {
        render.draw_circle_batch(circles, color);
    }

    let label_set = labels_to_draw(graph, particles, ctx);
    // Whole-graph max degree (not just the currently-visible subset) —
    // invariant across pan/zoom, so the same node always normalizes to
    // the same degree-boost regardless of what else happens to be on
    // screen this frame (Wave 2.3 determinism gate).
    let max_degree = graph.nodes().map(|(id, _)| graph.degree(id)).max().unwrap_or(0).max(1);
    let mut labels_drawn = 0usize;

    for &id in ctx.visible {
        let (Some(p), Some(node)) = (particles.get(id.index()), graph.get_node(id)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);

        if ctx.selection.contains(&id) {
            render.set_stroke_color("#ffffff");
            render.set_stroke_width(2.0);
            render.begin_path();
            render.arc(sx, sy, r + 2.0, 0.0, std::f64::consts::TAU);
            render.stroke();
        } else if Some(id) == ctx.hovered {
            render.set_stroke_color("#ffd76a");
            render.set_stroke_width(1.5);
            render.begin_path();
            render.arc(sx, sy, r + 1.5, 0.0, std::f64::consts::TAU);
            render.stroke();
        }

        if !label_set.contains(&id) {
            // Wave 2.3 dim-interaction fix: a dimmed node's label is
            // skipped entirely (no draw call at all), not just faded —
            // the label cloud vanishes along with the dimmed circle.
            continue;
        }

        // Forced (focus-active highlight, or a collapsed-cluster
        // representative) always draws at full opacity — the zoom+degree
        // fade curve only governs the ordinary, non-forced LOD-grid path.
        let is_forced = ctx.focus.is_active() || ctx.forced_labels.contains(&id);
        let alpha = if is_forced {
            1.0
        } else {
            let normalized_degree = graph.degree(id) as f64 / max_degree as f64;
            label_grid::label_alpha(ctx.camera.zoom, normalized_degree)
        };

        if alpha > 0.01 {
            render.set_global_alpha(alpha);
            render.set_font("11px sans-serif");
            // Halo (owner defect report: thin edge strokes crossing node
            // label text made it unreadable) — a 4-direction offset-fill
            // in `ctx.label_halo` under the real `"#e6e6ea"` label fill.
            fill_text_with_halo(render, &node.label, sx + r + 4.0, sy + 4.0, "#e6e6ea", ctx.label_halo);
            render.set_global_alpha(1.0);
            labels_drawn += 1;
        }
    }

    NodeDrawStats { nodes_drawn: ctx.visible.len(), labels_drawn }
}

const CLUSTER_ACCENT: &str = "#c9a94e";

/// Draw the aggregated cross-cluster edges for every collapsed cluster —
/// one synthetic line per outside neighbor (summed weight, thicker line
/// for a heavier aggregate), replacing the N raw overlapping lines
/// [`draw_edges`] already excludes via `ctx.hidden`. Returns the number
/// of synthetic edges drawn.
pub fn draw_cluster_edges(
    render: &mut dyn RenderContext,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
    clusters: &ClusterRegistry,
) -> usize {
    let mut drawn = 0;
    for cluster in clusters.collapsed_clusters() {
        let Some(rep) = particles.get(cluster.representative.index()) else { continue };
        let (rx, ry) = ctx.camera.world_to_screen((rep.x as f64, rep.y as f64), ctx.viewport);
        for edge in cluster.aggregated_edges() {
            let Some(other) = particles.get(edge.outside.index()) else { continue };
            let (ox, oy) = ctx.camera.world_to_screen((other.x as f64, other.y as f64), ctx.viewport);
            let width = (1.0 + (edge.weight as f64).sqrt()).min(6.0);
            render.draw_line_batch(&[LineSegment { x1: rx, y1: ry, x2: ox, y2: oy }], CLUSTER_ACCENT, width);
            drawn += 1;
        }
    }
    drawn
}

/// Draw a distinct double-ring + member-count label over every collapsed
/// cluster's representative — the "this circle is actually N nodes"
/// affordance. Returns the number of super-nodes drawn.
pub fn draw_cluster_supernodes<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
    clusters: &ClusterRegistry,
) -> usize {
    let mut drawn = 0;
    for cluster in clusters.collapsed_clusters() {
        let rep = cluster.representative;
        let (Some(p), Some(node)) = (particles.get(rep.index()), graph.get_node(rep)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);

        render.set_stroke_color(CLUSTER_ACCENT);
        render.set_stroke_width(2.0);
        render.begin_path();
        render.arc(sx, sy, r + 3.0, 0.0, std::f64::consts::TAU);
        render.stroke();
        render.begin_path();
        render.arc(sx, sy, r + 7.0, 0.0, std::f64::consts::TAU);
        render.stroke();

        render.set_fill_color("#f0e6c0");
        render.set_font("11px sans-serif");
        render.fill_text(&format!("×{}", cluster.member_count()), sx + r + 10.0, sy + 4.0);
        drawn += 1;
    }
    drawn
}

/// Plain display fields for [`draw_hover_card`] — deliberately NOT
/// `crate::engine::NodeFacts` itself (this module stays the lower layer
/// `engine.rs` calls into, not the reverse): `GraphEngine::draw` builds
/// this fresh each frame from the exact same `node_facts()` data source
/// the demo's own sidebar facts panel already reads, so the hover card
/// and the click-select sidebar can never disagree about what a node's
/// facts are.
pub struct HoverCardInfo<'a> {
    pub label: &'a str,
    pub category: &'a str,
    pub degree: u32,
    pub pinned: bool,
}

/// Floating hover info card (Wave 2.2 — «справка»): label/category/
/// degree/pinned, anchored near `anchor_px` (the hovered node's screen
/// position). Flips to stay inside `bounds` exactly like
/// `uzor_figures::guide::tooltip::draw_tooltip` (the SAME function,
/// reused verbatim — one flip-to-fit implementation for the whole demo
/// suite, not a second one invented here) already does for figures'
/// hover tooltips; `bounds` is the graph canvas's own viewport, so the
/// card never clips past the canvas edge even in a multi-panel layout.
pub fn draw_hover_card(render: &mut dyn RenderContext, anchor_px: (f64, f64), info: &HoverCardInfo<'_>, bounds: Rect) {
    let lines = [
        ("label".to_owned(), info.label.to_owned()),
        ("category".to_owned(), info.category.to_owned()),
        ("degree".to_owned(), info.degree.to_string()),
        ("pinned".to_owned(), info.pinned.to_string()),
    ];
    draw_tooltip(render, &FigureTheme::dark(), anchor_px, &lines, bounds);
}

const BOX_SELECT_FILL: &str = "#4d90fe";
const BOX_SELECT_FILL_ALPHA: f64 = 0.15;
const BOX_SELECT_BORDER: &str = "#7fb2ff";
const BOX_SELECT_BORDER_WIDTH: f64 = 1.0;

/// Live rubber-band overlay for an in-progress box-select drag (Wave 2.4
/// — oss doc §2.3: "a translucent rectangle overlay drawn from
/// `select[0..3]` each frame"). `rect` is already corner-normalized
/// screen-space (see [`crate::engine::GraphEngine::box_select_rect`]) —
/// this function is pure paint, no selection logic of its own.
pub fn draw_box_select_rect(render: &mut dyn RenderContext, rect: Rect) {
    render.set_global_alpha(BOX_SELECT_FILL_ALPHA);
    render.set_fill_color(BOX_SELECT_FILL);
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render.set_global_alpha(1.0);
    render.set_stroke_color(BOX_SELECT_BORDER);
    render.set_stroke_width(BOX_SELECT_BORDER_WIDTH);
    render.stroke_rect(rect.x, rect.y, rect.width, rect.height);
}

#[cfg(test)]
mod tests {
    //! `labels_to_draw` exercised at the full `Graph`/`Camera2D`/
    //! `DrawContext` level (Wave 2.3) — deliberately WITHOUT a
    //! `RenderContext` mock, since the label-SELECTION logic
    //! (`labels_to_draw`) never touches the renderer; `label_grid.rs`'s
    //! own test module covers the pure grid/quota/degree-priority/
    //! zoom-ramp/determinism math in isolation.

    use super::*;
    use crate::graph::Graph;
    use crate::particle::Particle;

    type G = Graph<(), ()>;

    /// A hub with 5 depth-1 neighbors (degree 1 each), plus an unrelated
    /// 15-node clique (degree 14 each) packed into the SAME screen cell —
    /// the clique always wins the LOD grid's priority ranking over the
    /// hub/its neighbors when nothing is forced.
    fn hub_and_noise_clique() -> (G, NodeIndex, Vec<NodeIndex>, Vec<NodeIndex>) {
        let mut graph = G::new();
        let hub = graph.push_node((), "hub", "x", 4.0);
        let neighbors: Vec<NodeIndex> = (0..5).map(|i| graph.push_node((), format!("nbr{i}"), "x", 4.0)).collect();
        for &n in &neighbors {
            graph.push_edge(hub, n, 1.0, ());
        }
        let noise: Vec<NodeIndex> = (0..15).map(|i| graph.push_node((), format!("noise{i}"), "x", 4.0)).collect();
        for i in 0..noise.len() {
            for j in (i + 1)..noise.len() {
                graph.push_edge(noise[i], noise[j], 1.0, ());
            }
        }
        (graph, hub, neighbors, noise)
    }

    /// Every node placed a couple of world units apart, all landing
    /// inside the SAME single 100px screen cell at the zoom this test
    /// module uses.
    fn particles_all_in_one_cell(graph: &G) -> Vec<Particle> {
        (0..graph.node_count()).map(|i| Particle::at(i as f32 * 2.0, i as f32 * 2.0)).collect()
    }

    /// Wave 2.3's literal forced-union gate: a hovered node and its
    /// depth-1 neighbors must keep their labels even when the LOD grid's
    /// quota is entirely exhausted by unrelated higher-degree candidates
    /// — AND (the dim-interaction fix) every unrelated, unfocused node
    /// must NOT show a label at all while the focus is active.
    #[test]
    fn hovered_node_and_its_neighbors_keep_labels_even_when_the_grid_quota_is_exhausted() {
        let (graph, hub, neighbors, noise) = hub_and_noise_clique();
        let particles = particles_all_in_one_cell(&graph);
        let camera = Camera2D { pan_x: 0.0, pan_y: 0.0, zoom: 2.0 }; // quota = ceil(1.0 * 2.0^2) = 4
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let empty_focus = FocusSet::empty();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let empty_selection = BTreeSet::new();

        let ctx_no_focus = DrawContext {
            camera: &camera,
            viewport,
            visible: &visible,
            focus: &empty_focus,
            selection: &empty_selection,
            hovered: None,
            hidden: &hidden,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
            label_halo: crate::engine::DEFAULT_LABEL_HALO,
            forced_labels: &forced,
        };
        let shown_no_focus = labels_to_draw(&graph, &particles, &ctx_no_focus);
        assert_eq!(shown_no_focus.len(), 4, "the quota's 4 slots all go to the degree-14 noise clique");
        assert!(!shown_no_focus.contains(&hub), "without focus, the hub (degree 5) loses the crowded cell to the noise clique");
        for &n in &neighbors {
            assert!(!shown_no_focus.contains(&n), "without focus, a degree-1 neighbor never wins the crowded cell");
        }

        // Simulate a hover on the hub at depth 1 — the SAME
        // `neighborhood_focus_keys_depth` + `FocusSet::select_many`
        // `GraphEngine::set_hovered` drives.
        let mut focus = FocusSet::empty();
        focus.select_many(graph.neighborhood_focus_keys_depth(hub, 1));
        let ctx_focused = DrawContext { focus: &focus, ..ctx_no_focus };

        let shown_focused = labels_to_draw(&graph, &particles, &ctx_focused);
        assert!(shown_focused.contains(&hub), "the hovered node itself must always show its label");
        for &n in &neighbors {
            assert!(shown_focused.contains(&n), "every depth-1 neighbor of the hovered node must show its label");
        }
        for &noisy in &noise {
            assert!(
                !shown_focused.contains(&noisy),
                "an unrelated, unfocused node must NOT show a label while focus is active — the dim-interaction fix"
            );
        }
    }

    /// Repeated calls against the exact same `DrawContext`/`Graph`/
    /// `Particle` state must always produce the identical label set —
    /// no `HashMap`-iteration-order leak through the full
    /// `Graph`->`LabelCandidate`->`select_labels` pipeline.
    #[test]
    fn labels_to_draw_is_deterministic_across_repeated_calls_with_unchanged_state() {
        let (graph, _hub, _neighbors, _noise) = hub_and_noise_clique();
        let particles = particles_all_in_one_cell(&graph);
        let camera = Camera2D { pan_x: 3.0, pan_y: -7.0, zoom: 1.4 };
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let empty_focus = FocusSet::empty();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let empty_selection = BTreeSet::new();
        let ctx = DrawContext {
            camera: &camera,
            viewport,
            visible: &visible,
            focus: &empty_focus,
            selection: &empty_selection,
            hovered: None,
            hidden: &hidden,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
            label_halo: crate::engine::DEFAULT_LABEL_HALO,
            forced_labels: &forced,
        };

        let first = labels_to_draw(&graph, &particles, &ctx);
        for _ in 0..5 {
            assert_eq!(labels_to_draw(&graph, &particles, &ctx), first, "identical state must yield an identical label set every call");
        }
    }
}
