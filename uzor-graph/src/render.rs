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

use std::collections::{HashMap, HashSet};

use uzor::render::{CircleBatch, LineSegment, RenderContext};
use uzor::types::Rect;
use uzor_figures::guide::tooltip::draw_tooltip;
use uzor_figures::interact::FocusSet;
use uzor_figures::theme::FigureTheme;

use crate::camera::Camera2D;
use crate::cluster::ClusterRegistry;
use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;

/// Zoom at/above which node labels start fading in (Obsidian's "Text
/// Fade Threshold" precedent — a continuous dial keyed to zoom, not a
/// binary show/hide).
pub const LOD_LABEL_FADE_LOW: f64 = 0.45;
pub const LOD_LABEL_FADE_HIGH: f64 = 0.9;

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
    pub selected: Option<NodeIndex>,
    pub hovered: Option<NodeIndex>,
    /// Nodes hidden by a collapsed cluster (every member except that
    /// cluster's representative — see `crate::cluster`). Empty when no
    /// cluster is collapsed. Edges touching a hidden node are skipped
    /// entirely by [`draw_edges`] — [`draw_cluster_edges`] draws the
    /// aggregated substitute instead.
    pub hidden: &'a HashSet<NodeIndex>,
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

/// Draw every visible node as a circle (radius from `Camera2D::node_screen_radius`,
/// color from [`category_color`]), plus a selection/hover ring and
/// zoom-faded labels. Returns the number of nodes drawn.
pub fn draw_nodes<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> usize {
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

    let label_alpha = ((ctx.camera.zoom - LOD_LABEL_FADE_LOW) / (LOD_LABEL_FADE_HIGH - LOD_LABEL_FADE_LOW))
        .clamp(0.0, 1.0);

    for &id in ctx.visible {
        let (Some(p), Some(node)) = (particles.get(id.index()), graph.get_node(id)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);

        if Some(id) == ctx.selected {
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

        if label_alpha > 0.01 {
            render.set_global_alpha(label_alpha);
            render.set_fill_color("#e6e6ea");
            render.set_font("11px sans-serif");
            render.fill_text(&node.label, sx + r + 4.0, sy + 4.0);
            render.set_global_alpha(1.0);
        }
    }

    ctx.visible.len()
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
