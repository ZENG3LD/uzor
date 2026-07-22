//! `DagFigure` — a layered directed-acyclic-graph figure: horizontal
//! layers stacked top-down (Kahn longest-path depth + one down-sweep/
//! one up-sweep barycenter crossing reduction, see [`layering`]'s own
//! module docs for the full algorithm + its provenance), rounded boxes
//! per node, cubic-bezier edges between layer bands.
//!
//! **Scope note (arc B wave 2 shelf item):** this figure is deliberately
//! a FIRST version — one down-sweep/one up-sweep barycenter pass, no
//! dummy-node chain insertion for edges spanning more than one layer, no
//! iterate-to-convergence crossing minimization (same honestly-scoped
//! limitation [`layering`]'s own lifted algorithm documents). Edge
//! anchoring is also a fixed, simple convention (source anchors at its
//! own node's bottom-center, target at its own node's top-center,
//! regardless of which layer either sits in) — a back edge whose target
//! sits at an equal-or-shallower layer than its source (only possible
//! because [`layout_dag`] still draws EVERY caller-supplied edge, even
//! ones the layering pass itself ignored as cycle-breaking back edges —
//! see [`layering::compute_layering`]'s own docs) draws a visually
//! unusual curve/arrowhead rather than routing around the boxes it
//! crosses; this is a documented v1 simplification, not a bug, matching
//! this shelf item's own "faithful lift, not a full Sugiyama pass"
//! framing. `uzor-figures` must NOT depend on `uzor-graph` (see this
//! crate's own `CLAUDE.md` Forbidden list) — the layering algorithm is a
//! private, in-crate copy (see [`layering`]'s own provenance doc), not
//! an import.

mod layering;

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::figure::FigureOverlay;
use crate::guide::tooltip;
use crate::guide::wrap::truncate_ellipsis;
use crate::mark::text::draw_label_centered;
use crate::theme::FigureTheme;

const MARGIN: f64 = 12.0;
const TITLE_HEIGHT: f64 = 24.0;
/// Preferred node box width (px) — shrunk uniformly (never below
/// [`MIN_NODE_WIDTH`]) when a layer's own row can't fit every node at
/// this width, the same "the tightest row sets the shared scale" idiom
/// [`crate::figure::sankey::layout_sankey`] already uses for its own
/// per-column height (rotated onto this figure's horizontal axis).
const NODE_WIDTH: f64 = 120.0;
const MIN_NODE_WIDTH: f64 = 40.0;
/// Preferred node box height (px) — shrunk to fit a layer's own row
/// pitch when there isn't enough vertical room for every layer at this
/// height.
const NODE_HEIGHT: f64 = 36.0;
const MIN_NODE_HEIGHT: f64 = 18.0;
/// Horizontal gap (px) between two node boxes sharing the same layer row.
const NODE_GAP_X: f64 = 24.0;
const NODE_CORNER_RADIUS: f64 = 6.0;
/// Horizontal label padding (px) inside a node box — the measured-fit
/// budget [`truncate_ellipsis`] wraps a label into.
const LABEL_PAD: f64 = 8.0;
/// Half-width (px) of the small filled direction-indicator triangle
/// drawn at an edge's target end.
const ARROW_SIZE: f64 = 5.0;
const EDGE_ALPHA: f64 = 0.55;
const EDGE_HOVER_ALPHA: f64 = 0.95;
const EDGE_DIM_ALPHA: f64 = 0.14;
const NODE_DIM_ALPHA: f64 = 0.3;
const HOVER_STROKE_WIDTH: f64 = 1.5;
const SELECTED_STROKE_WIDTH: f64 = 2.0;

/// One node: `label` is drawn (measured-fit/ellipsis-truncated, see
/// [`truncate_ellipsis`]) inside its own layered box; `category` (if
/// present) indexes [`FigureTheme::palette`] for the box's own fill
/// color — a node with `category: None` uses `theme.label_color` (a
/// neutral default fill, same convention
/// [`crate::figure::sankey::SankeyFigure`]'s nodes use).
#[derive(Debug, Clone)]
pub struct DagNode {
    pub label: String,
    pub category: Option<usize>,
}

/// One directed edge between two [`DagNode`] indices (`from` = upstream,
/// `to` = downstream). A caller may supply a cyclic edge set — see
/// [`layering::compute_layering`]'s own docs for how cycles are broken
/// for the LAYERING computation only; every edge, cycle-causing or not,
/// is still drawn by [`layout_dag`]/[`DagFigure::render`].
#[derive(Debug, Clone, Copy)]
pub struct DagEdge {
    pub from: usize,
    pub to: usize,
}

/// One rendered edge's screen-pixel geometry — a cubic-bezier curve from
/// the source node's own bottom-center to the target node's own
/// top-center, control points at the vertical midpoint between the two
/// (the same "control points at the midpoint between the two edges"
/// idiom [`crate::figure::sankey::RibbonGeom`]/`draw_ribbon_path` use,
/// rotated 90° onto this figure's top-down layer axis). `edge_index`
/// indexes back into whatever `edges` slice [`layout_dag`] was called
/// with, same convention as `RibbonGeom::link_index`.
#[derive(Debug, Clone, Copy)]
pub struct DagEdgeGeom {
    pub edge_index: usize,
    pub src_x: f64,
    pub src_y: f64,
    pub dst_x: f64,
    pub dst_y: f64,
}

/// Pure layout geometry for a [`DagFigure`] — separate from painting
/// (design law #1) so [`hit_test_dag_node`] always agrees pixel-for-
/// pixel with what got drawn.
#[derive(Debug, Clone)]
pub struct DagLayout {
    /// One rect per input node, same index as the `nodes` slice passed
    /// to [`layout_dag`]. Empty only when `nodes` itself is empty.
    pub node_rects: Vec<Rect>,
    /// One layer index (`0` = root row) per input node, same index as
    /// `node_rects` — the result of [`layering::compute_layering`],
    /// exposed so a caller/tooltip can report which row a node landed
    /// in without re-deriving it.
    pub node_layer: Vec<u32>,
    /// One [`DagEdgeGeom`] per edge with valid, non-self `from`/`to`
    /// indices — an edge referencing an out-of-range node index, or a
    /// self-loop, is silently dropped (a caller data bug, not a panic —
    /// same convention [`crate::figure::sankey::layout_sankey`] uses for
    /// its own malformed links).
    pub edges: Vec<DagEdgeGeom>,
}

/// Layered DAG layout: Kahn longest-path depth assignment + barycenter
/// within-layer crossing reduction (see [`layering::compute_layering`]),
/// then a row-per-layer / column-per-slot geometric placement within
/// `rect` — layer `0` at the top, increasing layers stacking downward
/// ("horizontal layers top-down" per this figure's own brief).
///
/// Every layer shares ONE row pitch (`rect.height / layer_count`); every
/// node shares ONE box width, shrunk below [`NODE_WIDTH`] (never below
/// [`MIN_NODE_WIDTH`]) only for whichever row is too crowded to fit at
/// the preferred width — the same "tightest row sets the shared scale"
/// idiom [`crate::figure::sankey::layout_sankey`] already uses for its
/// own per-column height. A row's boxes are centered horizontally within
/// `rect`'s own width, gapped by [`NODE_GAP_X`].
pub fn layout_dag(nodes: &[DagNode], edges: &[DagEdge], explicit_roots: &[usize], rect: Rect) -> DagLayout {
    if nodes.is_empty() {
        return DagLayout { node_rects: Vec::new(), node_layer: Vec::new(), edges: Vec::new() };
    }

    // Only edges whose `from`/`to` are real, non-self node indices
    // participate — malformed input degrades to "this edge doesn't
    // exist" rather than an out-of-bounds panic anywhere below. The
    // FULL (unfiltered-for-cycles) valid edge set is what both the
    // layering pass AND the final drawn geometry are built from — see
    // this module's own doc comment for why cycle-breaking only affects
    // layer/slot assignment, never which edges get drawn.
    let valid_edges: Vec<(usize, &DagEdge)> =
        edges.iter().enumerate().filter(|(_, e)| e.from < nodes.len() && e.to < nodes.len() && e.from != e.to).collect();
    let pairs: Vec<(usize, usize)> = valid_edges.iter().map(|&(_, e)| (e.from, e.to)).collect();

    let layering = layering::compute_layering(nodes.len(), &pairs, explicit_roots);
    let layer_count = layering.layers.len().max(1);

    let available_h = rect.height.max(0.0);
    let row_pitch = available_h / layer_count as f64;
    let node_h = NODE_HEIGHT.min((row_pitch - 8.0).max(MIN_NODE_HEIGHT));

    let available_w = rect.width.max(0.0);
    let mut node_w = NODE_WIDTH;
    for row in &layering.layers {
        let count = row.len();
        if count == 0 {
            continue;
        }
        let full_w = NODE_WIDTH * count as f64 + NODE_GAP_X * (count.saturating_sub(1)) as f64;
        if full_w > available_w {
            let candidate = ((available_w - NODE_GAP_X * (count.saturating_sub(1)) as f64) / count as f64).max(MIN_NODE_WIDTH);
            node_w = node_w.min(candidate);
        }
    }

    let mut node_rects = vec![Rect::default(); nodes.len()];
    for (l, row) in layering.layers.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let count = row.len();
        let total_w = node_w * count as f64 + NODE_GAP_X * (count.saturating_sub(1)) as f64;
        let mut x = rect.x + ((available_w - total_w) / 2.0).max(0.0);
        let y = rect.y + row_pitch * l as f64 + (row_pitch - node_h) / 2.0;
        for &node_idx in row {
            node_rects[node_idx] = Rect::new(x, y, node_w, node_h);
            x += node_w + NODE_GAP_X;
        }
    }

    let edge_geoms: Vec<DagEdgeGeom> = valid_edges
        .iter()
        .map(|&(ei, e)| {
            let src = node_rects[e.from];
            let dst = node_rects[e.to];
            DagEdgeGeom { edge_index: ei, src_x: src.center_x(), src_y: src.bottom(), dst_x: dst.center_x(), dst_y: dst.y }
        })
        .collect();

    DagLayout { node_rects, node_layer: layering.layer, edges: edge_geoms }
}

/// Index of the node whose rect contains `(px, py)` — hit-tests through
/// the EXACT same `node_rects` geometry [`layout_dag`] just produced
/// (design law #1). `None` when no node rect contains the point.
pub fn hit_test_dag_node(layout: &DagLayout, px: f64, py: f64) -> Option<usize> {
    layout.node_rects.iter().position(|r| r.contains(px, py))
}

/// A layered directed-acyclic-graph figure — see this module's own docs
/// for the layering algorithm + its documented v1 scope boundaries.
pub struct DagFigure {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
    /// Explicit layer-0 root set — see
    /// [`layering::compute_layering`]'s own docs for the auto-root /
    /// fallback-root rule used when this is empty (the default).
    roots: Vec<usize>,
    pub title: String,
}

impl DagFigure {
    pub fn new(nodes: Vec<DagNode>, edges: Vec<DagEdge>) -> Self {
        Self { nodes, edges, roots: Vec::new(), title: String::new() }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Force `roots` as the layer-0 set instead of auto-detecting every
    /// zero-in-degree node — see [`layering::compute_layering`]'s own
    /// docs.
    pub fn with_roots(mut self, roots: Vec<usize>) -> Self {
        self.roots = roots;
        self
    }

    fn plot_rect(&self, rect: Rect) -> Rect {
        let title_h = if self.title.is_empty() { 0.0 } else { TITLE_HEIGHT };
        Rect::new(
            rect.x + MARGIN,
            rect.y + title_h + MARGIN,
            (rect.width - MARGIN * 2.0).max(0.0),
            (rect.height - title_h - MARGIN * 2.0).max(0.0),
        )
    }

    /// This figure's plot rect for `rect` — exposed for the same reason
    /// as every other figure's `plot_area`/`plot_rect` accessor: a
    /// caller driving hover routing from outside needs the EXACT rect
    /// [`DagFigure::layout`] was computed against.
    pub fn plot_rect_for(&self, rect: Rect) -> Rect {
        self.plot_rect(rect)
    }

    /// This figure's own layout geometry for `rect` — exposed for the
    /// same reason as [`DagFigure::plot_rect_for`].
    pub fn layout(&self, rect: Rect) -> DagLayout {
        layout_dag(&self.nodes, &self.edges, &self.roots, self.plot_rect(rect))
    }

    fn node_color<'a>(&self, theme: &'a FigureTheme, i: usize) -> &'a str {
        match self.nodes.get(i).and_then(|n| n.category) {
            Some(cat) if !theme.palette.is_empty() => &theme.palette[cat % theme.palette.len()],
            _ => &theme.label_color,
        }
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a
    /// node highlights that node + every edge touching it (dimming
    /// everything else) and shows a node/layer tooltip;
    /// `overlay.focus`-selected nodes (keyed by index into
    /// [`DagFigure::nodes`]) get a persistent accent outline, the same
    /// convention [`crate::figure::SankeyFigure`]/[`crate::figure::BarFigure`]
    /// use. `overlay.brush` is not consumed by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let plot_rect = self.plot_rect(rect);
        let layout = layout_dag(&self.nodes, &self.edges, &self.roots, plot_rect);

        let hovered = overlay.hover_px.and_then(|(hx, hy)| hit_test_dag_node(&layout, hx, hy));
        let connected = |i: usize| -> bool {
            match hovered {
                None => true,
                Some(h) => i == h || self.edges.iter().any(|e| (e.from == h && e.to == i) || (e.to == h && e.from == i)),
            }
        };

        self.draw_edges(ctx, &layout, theme, hovered);
        self.draw_nodes(ctx, &layout, theme, overlay, hovered, &connected);
        self.draw_labels(ctx, &layout, theme, &connected);

        if let (Some(h), Some((hx, hy))) = (hovered, overlay.hover_px) {
            let label = self.nodes.get(h).map(|n| n.label.clone()).unwrap_or_default();
            let layer = layout.node_layer.get(h).copied().unwrap_or(0);
            let lines = vec![("node".to_owned(), label), ("layer".to_owned(), layer.to_string())];
            tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, plot_rect);
        }

        if !self.title.is_empty() {
            crate::figure::draw_title(ctx, rect, &self.title, theme);
        }
    }

    fn draw_edges(&self, ctx: &mut dyn RenderContext, layout: &DagLayout, theme: &FigureTheme, hovered: Option<usize>) {
        for geom in &layout.edges {
            let Some(edge) = self.edges.get(geom.edge_index) else { continue };
            let touches_hovered = hovered.is_some_and(|h| edge.from == h || edge.to == h);
            let alpha = match hovered {
                None => EDGE_ALPHA,
                Some(_) if touches_hovered => EDGE_HOVER_ALPHA,
                Some(_) => EDGE_DIM_ALPHA,
            };

            ctx.set_stroke_color(&theme.axis_color);
            ctx.set_stroke_width(1.4);
            ctx.set_global_alpha(alpha);
            let mid_y = (geom.src_y + geom.dst_y) / 2.0;
            ctx.begin_path();
            ctx.move_to(geom.src_x, geom.src_y);
            ctx.bezier_curve_to(geom.src_x, mid_y, geom.dst_x, mid_y, geom.dst_x, geom.dst_y);
            ctx.stroke();

            // Small direction-indicator triangle at the target end — its
            // apex sits at the target anchor, base perpendicular to the
            // curve's own end tangent (which is purely vertical here,
            // since the second control point shares the target's own
            // x). `dir_y` orients it for the common downward case AND
            // the (documented v1-simplified, see this module's own
            // doc comment) upward back-edge case.
            let dir_y = if geom.dst_y >= geom.src_y { 1.0 } else { -1.0 };
            ctx.set_fill_color(&theme.axis_color);
            ctx.begin_path();
            ctx.move_to(geom.dst_x, geom.dst_y);
            ctx.line_to(geom.dst_x - ARROW_SIZE, geom.dst_y - dir_y * ARROW_SIZE);
            ctx.line_to(geom.dst_x + ARROW_SIZE, geom.dst_y - dir_y * ARROW_SIZE);
            ctx.close_path();
            ctx.fill();
            ctx.set_global_alpha(1.0);
        }
    }

    fn draw_nodes(
        &self,
        ctx: &mut dyn RenderContext,
        layout: &DagLayout,
        theme: &FigureTheme,
        overlay: &FigureOverlay<'_>,
        hovered: Option<usize>,
        connected: &dyn Fn(usize) -> bool,
    ) {
        for (i, r) in layout.node_rects.iter().enumerate() {
            if r.width <= 0.0 || r.height <= 0.0 {
                continue;
            }
            let alpha = if connected(i) { 1.0 } else { NODE_DIM_ALPHA };
            ctx.set_fill_color(self.node_color(theme, i));
            ctx.set_global_alpha(alpha);
            ctx.fill_rounded_rect(r.x, r.y, r.width, r.height, NODE_CORNER_RADIUS);
            ctx.set_global_alpha(1.0);

            if hovered == Some(i) {
                ctx.set_stroke_color(&theme.background);
                ctx.set_stroke_width(HOVER_STROKE_WIDTH);
                ctx.stroke_rounded_rect(r.x, r.y, r.width, r.height, NODE_CORNER_RADIUS);
            }

            if let Some(focus) = overlay.focus {
                if focus.is_selected(i as u64) {
                    let accent = if theme.palette.len() > 1 { &theme.palette[1] } else { &theme.axis_color };
                    ctx.set_stroke_color(accent);
                    ctx.set_stroke_width(SELECTED_STROKE_WIDTH);
                    ctx.stroke_rounded_rect(r.x, r.y, r.width, r.height, NODE_CORNER_RADIUS);
                }
            }
        }
    }

    fn draw_labels(&self, ctx: &mut dyn RenderContext, layout: &DagLayout, theme: &FigureTheme, connected: &dyn Fn(usize) -> bool) {
        ctx.set_font(&theme.label_font);
        for (i, node) in self.nodes.iter().enumerate() {
            if node.label.is_empty() {
                continue;
            }
            let Some(r) = layout.node_rects.get(i) else { continue };
            if r.width <= 0.0 {
                continue;
            }
            let max_width = (r.width - LABEL_PAD * 2.0).max(0.0);
            let text = truncate_ellipsis(&node.label, max_width, |s| ctx.measure_text(s));
            let alpha = if connected(i) { 1.0 } else { NODE_DIM_ALPHA };
            ctx.set_global_alpha(alpha);
            // Text color deliberately follows `theme.background` (not
            // `theme.label_color`) — box fill is EITHER a mid-saturation
            // palette color or the neutral `theme.label_color` itself
            // (see `node_color`), and the background hue contrasts
            // against both in this crate's dark/light themes alike (a
            // dark background on a mid-bright box, or a bright
            // background on a darker neutral box).
            draw_label_centered(ctx, &text, r.center_x(), r.center_y(), &theme.background, &theme.label_font);
            ctx.set_global_alpha(1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(label: &str) -> DagNode {
        DagNode { label: label.to_owned(), category: None }
    }

    fn edge(from: usize, to: usize) -> DagEdge {
        DagEdge { from, to }
    }

    #[test]
    fn empty_nodes_produce_an_empty_layout_without_panicking() {
        let layout = layout_dag(&[], &[], &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert!(layout.node_rects.is_empty());
        assert!(layout.node_layer.is_empty());
        assert!(layout.edges.is_empty());
    }

    #[test]
    fn single_node_with_no_edges_lays_out_without_panicking() {
        let nodes = vec![node("solo")];
        let layout = layout_dag(&nodes, &[], &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.node_rects.len(), 1);
        assert_eq!(layout.node_layer, vec![0]);
        assert!(layout.node_rects[0].width > 0.0 && layout.node_rects[0].height > 0.0);
    }

    #[test]
    fn a_chain_places_each_node_in_its_own_successive_layer_row() {
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![edge(0, 1), edge(1, 2)];
        let layout = layout_dag(&nodes, &edges, &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.node_layer, vec![0, 1, 2]);
        // Layer stacks TOP-DOWN — a deeper layer's box must sit strictly
        // below a shallower layer's box.
        assert!(layout.node_rects[0].y < layout.node_rects[1].y);
        assert!(layout.node_rects[1].y < layout.node_rects[2].y);
    }

    #[test]
    fn out_of_range_and_self_loop_edges_are_dropped_from_the_drawn_geometry() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![edge(0, 0), edge(0, 99), edge(99, 1), edge(0, 1)];
        let layout = layout_dag(&nodes, &edges, &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.edges.len(), 1, "only the single valid, non-self edge (0 -> 1) should survive");
        assert_eq!(layout.edges[0].edge_index, 3);
    }

    #[test]
    fn a_cyclic_edge_set_still_lays_out_deterministically_and_draws_every_edge() {
        // 0 -> 1 -> 2 -> 0, a pure cycle — layering breaks the 2->0 back
        // edge internally, but every ORIGINAL edge (including 2->0)
        // still appears in the drawn geometry.
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![edge(0, 1), edge(1, 2), edge(2, 0)];
        let layout = layout_dag(&nodes, &edges, &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.edges.len(), 3, "the back edge must still be DRAWN even though it's excluded from layering math");
        assert_eq!(layout.node_layer, vec![0, 1, 2]);
    }

    #[test]
    fn layout_is_deterministic_across_repeated_calls() {
        let nodes = vec![node("a"), node("b"), node("c"), node("d"), node("e")];
        let edges = vec![edge(0, 2), edge(1, 2), edge(2, 3), edge(2, 4)];
        let rect = Rect::new(0.0, 0.0, 500.0, 400.0);
        let first = layout_dag(&nodes, &edges, &[], rect);
        let second = layout_dag(&nodes, &edges, &[], rect);
        assert_eq!(first.node_rects, second.node_rects);
        assert_eq!(first.node_layer, second.node_layer);
        let first_geo: Vec<(usize, f64, f64, f64, f64)> =
            first.edges.iter().map(|g| (g.edge_index, g.src_x, g.src_y, g.dst_x, g.dst_y)).collect();
        let second_geo: Vec<(usize, f64, f64, f64, f64)> =
            second.edges.iter().map(|g| (g.edge_index, g.src_x, g.src_y, g.dst_x, g.dst_y)).collect();
        assert_eq!(first_geo, second_geo);
    }

    #[test]
    fn a_crowded_layer_shrinks_the_shared_node_width_but_never_below_the_floor() {
        // 20 nodes all fanning out from one root into a single layer —
        // far too many to fit at NODE_WIDTH inside a narrow rect.
        let mut nodes = vec![node("root")];
        let mut edges = Vec::new();
        for i in 1..=20 {
            nodes.push(node(&format!("child-{i}")));
            edges.push(edge(0, i));
        }
        let layout = layout_dag(&nodes, &edges, &[], Rect::new(0.0, 0.0, 300.0, 300.0));
        // Every child sits in layer 1 — assert the shared box width was
        // shrunk (all children share one width) and never collapsed
        // below the documented floor.
        let child_widths: Vec<f64> = (1..nodes.len()).map(|i| layout.node_rects[i].width).collect();
        for &w in &child_widths {
            assert!(w >= MIN_NODE_WIDTH - 1e-9, "shrunk node width must never drop below MIN_NODE_WIDTH, got {w}");
            assert!(w < NODE_WIDTH, "a 20-node crowded layer inside a 300px-wide rect must shrink below the preferred width");
        }
    }

    #[test]
    fn hit_test_dag_node_resolves_a_point_inside_its_own_rect() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![edge(0, 1)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let layout = layout_dag(&nodes, &edges, &[], rect);

        let r = layout.node_rects[1];
        let cx = r.x + r.width / 2.0;
        let cy = r.y + r.height / 2.0;
        assert_eq!(hit_test_dag_node(&layout, cx, cy), Some(1));
        assert_eq!(hit_test_dag_node(&layout, -1000.0, -1000.0), None);
    }

    #[test]
    fn explicit_roots_are_forwarded_into_the_layering_pass() {
        // Without an explicit root, node 1 (in-degree 0) would ALSO be a
        // layer-0 root; forcing root = [0] still layers node 1 at 0 (its
        // own root), same fixture/assertion as `layering`'s own test.
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![edge(0, 2), edge(1, 2)];
        let layout = layout_dag(&nodes, &edges, &[0], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.node_layer[0], 0);
        assert_eq!(layout.node_layer[1], 0);
        assert_eq!(layout.node_layer[2], 1);
    }

    #[test]
    fn empty_figure_renders_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};

        let figure = DagFigure::new(Vec::new(), Vec::new());
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 120.0), &theme);
        });
        assert!(result.is_ok(), "empty dag figure must render without panicking");
    }

    #[test]
    fn a_small_dag_renders_with_hover_and_focus_overlay_without_panicking() {
        use uzor_export::{render_to_png, ExportSpec};
        use crate::interact::focus::FocusSet;

        let nodes =
            vec![node("root"), node("left"), node("right"), node("leaf-a"), node("leaf-b")].into_iter().enumerate().map(|(i, mut n)| {
                n.category = Some(i);
                n
            }).collect();
        let edges = vec![edge(0, 1), edge(0, 2), edge(1, 3), edge(2, 3), edge(2, 4)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let figure = DagFigure::new(nodes, edges).with_title("small dag");
        let layout = figure.layout(rect);
        let r = layout.node_rects[3];
        let hover_px = (r.center_x(), r.center_y());

        let mut focus = FocusSet::empty();
        focus.select(1);
        let overlay = FigureOverlay { hover_px: Some(hover_px), brush: None, focus: Some(&focus) };

        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        });
        assert!(result.is_ok(), "a small DAG with hover + focus overlay must render without panicking");
    }
}
