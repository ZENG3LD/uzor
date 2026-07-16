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

use std::collections::HashMap;

use uzor::render::{CircleBatch, LineSegment, RenderContext};
use uzor::types::Rect;

use crate::camera::Camera2D;
use crate::graph::{Graph, NodeIndex};
use crate::interaction::focus::FocusSet;
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
}

/// Draw every visible edge, dimming any edge outside an active
/// [`FocusSet`]. Returns the number of edges drawn.
pub fn draw_edges<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> usize {
    let visible_set: std::collections::HashSet<NodeIndex> = ctx.visible.iter().copied().collect();
    let mut segments: Vec<LineSegment> = Vec::new();
    let mut dim_segments: Vec<LineSegment> = Vec::new();

    for (eid, edge) in graph.edges() {
        if !visible_set.contains(&edge.from) && !visible_set.contains(&edge.to) {
            continue;
        }
        let (Some(a), Some(b)) = (particles.get(edge.from.index()), particles.get(edge.to.index())) else {
            continue;
        };
        let (ax, ay) = ctx.camera.world_to_screen((a.x as f64, a.y as f64), ctx.viewport);
        let (bx, by) = ctx.camera.world_to_screen((b.x as f64, b.y as f64), ctx.viewport);
        let seg = LineSegment { x1: ax, y1: ay, x2: bx, y2: by };
        if ctx.focus.is_active() && !ctx.focus.contains_edge(eid) {
            dim_segments.push(seg);
        } else {
            segments.push(seg);
        }
    }

    let drawn = segments.len() + dim_segments.len();
    if !dim_segments.is_empty() {
        render.set_global_alpha(DIM_ALPHA);
        render.draw_line_batch(&dim_segments, "#5a6070", 1.0);
        render.set_global_alpha(1.0);
    }
    if !segments.is_empty() {
        render.draw_line_batch(&segments, "#7c8496", 1.2);
    }
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
        if ctx.focus.is_active() && !ctx.focus.contains_node(id) {
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
