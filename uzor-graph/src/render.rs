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

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use uzor::render::{CircleBatch, LineSegment, RenderContext};
use uzor::types::Rect;
use uzor_figures::guide::text_protect::fill_text_with_halo;
use uzor_figures::guide::tooltip::draw_tooltip;
use uzor_figures::interact::FocusSet;
use uzor_figures::scale::color::CategoricalScale;
use uzor_figures::theme::FigureTheme;

use crate::camera::Camera2D;
use crate::cluster::ClusterRegistry;
use crate::graph::{EdgeIndex, Graph, NodeIndex};
use crate::label_grid::{self, LabelCandidate, LabelLodConfig};
use crate::particle::Particle;
use crate::style::{DashPattern, EdgeVisualStyle, NodeMarker, NodeVisualStyle};
use crate::theme::{default_category_palette, GraphTheme};

/// Re-exported from `label_grid` (Wave 2.3 moved the constants there —
/// every label-LOD number lives in one module) so any existing
/// `crate::render::LOD_LABEL_FADE_{LOW,HIGH}` path keeps resolving.
pub use crate::label_grid::{LOD_LABEL_FADE_HIGH, LOD_LABEL_FADE_LOW};

/// Deterministic category -> color mapping, hashed into `palette` (FNV-1a
/// over the category's own UTF-8 bytes, modulo the palette length — the
/// SAME algorithm this crate used before graph-strengthening arc Wave G2,
/// just against a caller-supplied [`CategoricalScale`] instead of a fixed
/// baked-in 10-color array). Returns an OWNED `String`, not `&'static
/// str` — the pre-Wave-G2 signature could only ever return that because
/// its palette was a compile-time constant; a caller-supplied
/// [`CategoricalScale`] holds runtime `String`s, so there is no `'static`
/// reference this function could hand back without leaking (explicitly
/// ruled out — see `uzor-graph/CLAUDE.md`'s graph-strengthening-arc entry
/// for the constraint this was weighed against). The extra per-call
/// allocation is accepted under this crate's own pre-existing "small
/// per-frame allocations are acceptable" convention (`engine3d.rs`'s
/// `compute_excluded_nodes_3d`'s own `HashSet`s already established it).
pub fn category_color(category: &str, palette: &CategoricalScale) -> String {
    let mut hash: u32 = 2166136261;
    for b in category.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    palette.color_for(hash as usize).to_owned()
}

/// [`category_color`] against this crate's own pre-existing default
/// 10-color palette (byte-identical to the pre-Wave-G2 hardcoded
/// mapping) — the convenience wrapper `render3d::category_tint` (3D, out
/// of this 2D-only wave's scope) keeps calling so its own node-tint
/// colors are unaffected by this wave.
pub fn category_color_default(category: &str) -> String {
    category_color(category, &default_category_palette())
}

/// Viewport-culled node list — `Graph`'s node positions intersected with
/// `camera.visible_world_aabb(viewport)` plus a world-unit `margin` (was
/// the fixed `const MARGIN: f64 = 64.0`, now caller-supplied — see
/// `crate::engine::GraphEngine::cull_margin_world`/`set_cull_margin_world`)
/// so nodes don't pop at the edge. Shared by render, the agent's
/// `visible_node_count`, and the pick candidate set.
pub fn cull_visible<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    camera: &Camera2D,
    viewport: Rect,
    margin: f64,
) -> Vec<NodeIndex> {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return Vec::new();
    }
    let aabb = camera.visible_world_aabb(viewport);
    let margin = margin.max(0.0);
    graph
        .nodes()
        .filter_map(|(id, node)| {
            let p = particles.get(id.index())?;
            let x = p.x as f64;
            let y = p.y as f64;
            let r = node.radius as f64 + margin;
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
    /// Label-LOD tuning (grid cell size, zoom fade window, degree shift —
    /// graph-strengthening arc Wave G2). See [`crate::engine::GraphEngine::
    /// label_lod`]/[`crate::engine::GraphEngine::set_label_lod`].
    pub label_lod: &'a LabelLodConfig,
    /// Every paint color/font this module's draw functions use (graph-
    /// strengthening arc Wave G2 / 2D quality audit A3) — see
    /// [`crate::theme::GraphTheme`]'s own doc comment, and
    /// [`crate::engine::GraphEngine::theme`]/[`crate::engine::GraphEngine::
    /// set_theme`] for the owning field this is read from every frame.
    pub theme: &'a GraphTheme,
}

struct VisibleEdgeCandidates {
    edge_ids: Vec<EdgeIndex>,
    candidates_scanned: usize,
    used_full_scan: bool,
    used_bitmap_order: bool,
    order_slots_scanned: usize,
}

const BITMAP_ORDER_MIN_CANDIDATES: usize = 256;

/// Collect the union of the visible nodes' adjacency lists, restoring
/// `Graph::edges()`'s ascending-index order after deduplication. Sorting
/// is required because `ctx.visible` is a viewport result rather than a
/// topology-order contract; the old full scan always painted a lower
/// `EdgeIndex` first within each edge batch. Dense views fall back to
/// the old ordered scan when their adjacency entries would equal or
/// exceed the graph's entire edge count.
fn visible_edge_candidates<N, E>(
    graph: &Graph<N, E>,
    visible: &[NodeIndex],
) -> VisibleEdgeCandidates {
    let adjacency_candidate_count: usize = visible
        .iter()
        .map(|&node| graph.incident_edges(node).len())
        .sum();
    if adjacency_candidate_count >= graph.edge_count() {
        let visible_set: HashSet<NodeIndex> = visible.iter().copied().collect();
        let edge_ids = graph
            .edges()
            .filter_map(|(edge_id, edge)| {
                (visible_set.contains(&edge.from) || visible_set.contains(&edge.to))
                    .then_some(edge_id)
            })
            .collect();
        return VisibleEdgeCandidates {
            edge_ids,
            candidates_scanned: graph.edge_count(),
            used_full_scan: true,
            used_bitmap_order: false,
            order_slots_scanned: graph.edge_count(),
        };
    }

    if adjacency_candidate_count >= BITMAP_ORDER_MIN_CANDIDATES
        && graph.edge_count() <= adjacency_candidate_count.saturating_mul(4)
    {
        let mut seen = vec![false; graph.edge_count()];
        for &node in visible {
            for edge in graph.incident_edges(node) {
                seen[edge.index()] = true;
            }
        }
        let edge_ids = seen
            .into_iter()
            .enumerate()
            .filter_map(|(index, present)| present.then_some(EdgeIndex(index as u32)))
            .collect();
        return VisibleEdgeCandidates {
            edge_ids,
            candidates_scanned: adjacency_candidate_count,
            used_full_scan: false,
            used_bitmap_order: true,
            order_slots_scanned: graph.edge_count(),
        };
    }

    let mut edge_ids = Vec::new();
    for &node in visible {
        let incident = graph.incident_edges(node);
        edge_ids.extend_from_slice(incident);
    }
    edge_ids.sort_unstable_by_key(|edge| edge.index());
    edge_ids.dedup_by_key(|edge| edge.index());
    VisibleEdgeCandidates {
        edge_ids,
        candidates_scanned: adjacency_candidate_count,
        used_full_scan: false,
        used_bitmap_order: false,
        order_slots_scanned: adjacency_candidate_count,
    }
}

/// Draw every visible edge, dimming any edge outside an active
/// [`FocusSet`]. Edges touching a node hidden by cluster collapse
/// (`ctx.hidden`) are skipped entirely — `draw_cluster_edges` draws the
/// aggregated substitute for those. Returns the number of edges drawn.
///
/// Bracketed in `render.save()`/`render.restore()` (2D quality audit A5 /
/// graph-strengthening arc G1.3) — `draw_line_batch` mutates persistent
/// stroke color/width state with no restore of its own, and this function
/// also flips the line-cap style; without the bracket that state leaks
/// past this call into whatever the caller paints next.
pub fn draw_edges<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> usize {
    let prepare_started = std::time::Instant::now();
    render.save();
    let candidate_scan = visible_edge_candidates(graph, ctx.visible);
    let mut segments: Vec<LineSegment> = Vec::new();
    let mut dim_segments: Vec<LineSegment> = Vec::new();
    let estimated_style_groups = (candidate_scan.edge_ids.len() / 32)
        .clamp(8, 2048)
        .min(candidate_scan.edge_ids.len().max(1));
    let mut styled_body_batches = StyledEdgeBatchTable::with_capacity(estimated_style_groups);
    let mut styled_arrow_batches = StyledEdgeBatchTable::with_capacity(estimated_style_groups);
    let mut screen_nodes = vec![CachedEdgeScreenNode::missing(); graph.node_count()];
    let mut screen_nodes_transformed = 0;
    let mut styled_count = 0;
    let focus_active = ctx.focus.is_active();
    let has_hidden = !ctx.hidden.is_empty();

    for &eid in &candidate_scan.edge_ids {
        let edge = graph.edge(eid);
        if has_hidden
            && (ctx.hidden.contains(&edge.from) || ctx.hidden.contains(&edge.to))
        {
            continue;
        }
        let Some(a) = cached_edge_screen_node(
            graph,
            particles,
            ctx,
            edge.from,
            &mut screen_nodes,
            &mut screen_nodes_transformed,
        ) else {
            continue;
        };
        let Some(b) = cached_edge_screen_node(
            graph,
            particles,
            ctx,
            edge.to,
            &mut screen_nodes,
            &mut screen_nodes_transformed,
        ) else {
            continue;
        };
        let seg = LineSegment { x1: a.x, y1: a.y, x2: b.x, y2: b.y };
        let dimmed = focus_active && !ctx.focus.is_selected(u64::from(eid));
        if let Some(style) = &edge.style {
            let resolved = ResolvedEdgeStyle::new(style, dimmed, ctx.theme);
            let (body, arrowhead) =
                styled_edge_geometry(seg, style, b.radius, resolved.width());
            styled_body_batches.push_segment(resolved, body);
            if let Some(arrowhead) = arrowhead {
                let arrow_style = resolved.with_solid_dash();
                styled_arrow_batches.push_pair(arrow_style, arrowhead);
            }
            styled_count += 1;
        } else if dimmed {
            dim_segments.push(seg);
        } else {
            segments.push(seg);
        }
    }

    let drawn = segments.len() + dim_segments.len() + styled_count;
    uzor::diagnostics::stage(
        "graph_2d_edges",
        "prepare_end",
        format_args!(
            "drawn={} plain={} dim={} styled={} graph_edges={} candidate_scan_mode={} candidates_scanned={} order_slots_scanned={} unique_candidates={} screen_nodes_transformed={} body_batches={} body_style_fast_hits={} body_style_hash_lookups={} arrow_batches={} arrow_style_fast_hits={} arrow_style_hash_lookups={} duration_us={}",
            drawn,
            segments.len(),
            dim_segments.len(),
            styled_count,
            graph.edge_count(),
            if candidate_scan.used_full_scan {
                "full"
            } else if candidate_scan.used_bitmap_order {
                "adjacency_bitmap"
            } else {
                "adjacency_sort"
            },
            candidate_scan.candidates_scanned,
            candidate_scan.order_slots_scanned,
            candidate_scan.edge_ids.len(),
            screen_nodes_transformed,
            styled_body_batches.batches.len(),
            styled_body_batches.fast_hits,
            styled_body_batches.hash_lookups,
            styled_arrow_batches.batches.len(),
            styled_arrow_batches.fast_hits,
            styled_arrow_batches.hash_lookups,
            prepare_started.elapsed().as_micros(),
        ),
    );
    let emit_started = std::time::Instant::now();
    // Round caps + >=1.5px width: sub-1.5px butt-capped hairlines at an
    // angle read as a beaded staircase on a standard-DPI display even
    // with correct AA (live-verified 2026-07-18); industry engines
    // (d3/sigma/obsidian) stroke edges at 1.5-2px for exactly this
    // reason.
    render.set_line_cap("round");
    if !dim_segments.is_empty() {
        render.set_global_alpha(ctx.theme.dim_alpha);
        render.draw_line_batch(&dim_segments, &ctx.theme.edge_dim_color, ctx.theme.edge_dim_width);
        render.set_global_alpha(1.0);
    }
    if !segments.is_empty() {
        render.draw_line_batch(&segments, &ctx.theme.edge_color, ctx.theme.edge_width);
    }
    for batch in &styled_body_batches.batches {
        draw_styled_edge_batch(render, batch);
    }
    // Arrowheads are a separate solid pass so they remain legible over
    // every body and edges with the same resolved arrow style collapse
    // into one backend call even when their body dash patterns differ.
    for batch in &styled_arrow_batches.batches {
        draw_styled_edge_batch(render, batch);
    }
    render.set_line_dash(&[]);
    render.set_global_alpha(1.0);
    render.set_line_cap("butt");
    render.restore();
    uzor::diagnostics::stage(
        "graph_2d_edges",
        "emit_end",
        format_args!("drawn={} duration_us={}", drawn, emit_started.elapsed().as_micros()),
    );
    drawn
}

const DEFAULT_EDGE_DASH: [f32; 2] = [8.0, 5.0];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ResolvedEdgeDash<'a> {
    Solid,
    Values(ResolvedEdgeDashValues<'a>),
}

#[derive(Clone, Copy)]
struct ResolvedEdgeDashValues<'a>(&'a [f32]);

impl PartialEq for ResolvedEdgeDashValues<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0)
                .all(|(left, right)| left.to_bits() == right.to_bits())
    }
}

impl Eq for ResolvedEdgeDashValues<'_> {}

impl std::hash::Hash for ResolvedEdgeDashValues<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.0.len(), state);
        for value in self.0 {
            std::hash::Hash::hash(&value.to_bits(), state);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ResolvedEdgeStyle<'a> {
    color: &'a str,
    width_bits: u64,
    alpha_bits: u64,
    dash: ResolvedEdgeDash<'a>,
}

impl<'a> ResolvedEdgeStyle<'a> {
    fn new(style: &'a EdgeVisualStyle, dimmed: bool, theme: &'a GraphTheme) -> Self {
        let color = style
            .tint
            .as_deref()
            .unwrap_or(if dimmed { &theme.edge_dim_color } else { &theme.edge_color });
        let width = style
            .width
            .map(|value| value.max(0.1) as f64)
            .unwrap_or(if dimmed { theme.edge_dim_width } else { theme.edge_width });
        let alpha = style.alpha.unwrap_or(1.0).clamp(0.0, 1.0) as f64
            * if dimmed { theme.dim_alpha } else { 1.0 };
        let dash = match style.dash.as_ref() {
            Some(DashPattern::Dashed) => ResolvedEdgeDash::Values(ResolvedEdgeDashValues(&DEFAULT_EDGE_DASH)),
            Some(DashPattern::Pattern(values))
                if !values.is_empty()
                    && values.iter().all(|value| value.is_finite() && *value > 0.0) =>
            {
                ResolvedEdgeDash::Values(ResolvedEdgeDashValues(values))
            }
            _ => ResolvedEdgeDash::Solid,
        };
        Self {
            color,
            width_bits: width.to_bits(),
            alpha_bits: alpha.to_bits(),
            dash,
        }
    }

    fn with_solid_dash(mut self) -> Self {
        self.dash = ResolvedEdgeDash::Solid;
        self
    }

    fn width(&self) -> f64 {
        f64::from_bits(self.width_bits)
    }

    fn alpha(&self) -> f64 {
        f64::from_bits(self.alpha_bits)
    }

    fn dash(&self) -> Vec<f64> {
        match self.dash {
            ResolvedEdgeDash::Solid => Vec::new(),
            ResolvedEdgeDash::Values(values) => values.0.iter().copied().map(f64::from).collect(),
        }
    }
}

struct StyledEdgeBatch<'a> {
    style: ResolvedEdgeStyle<'a>,
    segments: Vec<LineSegment>,
}

struct FastStyleHasher(u64);

impl Default for FastStyleHasher {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for FastStyleHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

type StyleBatchIndices<'a> =
    HashMap<ResolvedEdgeStyle<'a>, usize, BuildHasherDefault<FastStyleHasher>>;

struct StyledEdgeBatchTable<'a> {
    batches: Vec<StyledEdgeBatch<'a>>,
    indices: StyleBatchIndices<'a>,
    last: Option<(ResolvedEdgeStyle<'a>, usize)>,
    fast_hits: usize,
    hash_lookups: usize,
}

impl<'a> StyledEdgeBatchTable<'a> {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            batches: Vec::with_capacity(capacity),
            indices: HashMap::with_capacity_and_hasher(
                capacity,
                BuildHasherDefault::<FastStyleHasher>::default(),
            ),
            last: None,
            fast_hits: 0,
            hash_lookups: 0,
        }
    }

    fn batch_index(&mut self, style: ResolvedEdgeStyle<'a>) -> usize {
        if let Some((last_style, index)) = self.last {
            if last_style == style {
                self.fast_hits += 1;
                return index;
            }
        }

        self.hash_lookups += 1;
        let index = match self.indices.entry(style) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let index = self.batches.len();
                entry.insert(index);
                self.batches.push(StyledEdgeBatch {
                    style,
                    segments: Vec::with_capacity(4),
                });
                index
            }
        };
        self.last = Some((style, index));
        index
    }

    fn push_segment(&mut self, style: ResolvedEdgeStyle<'a>, segment: LineSegment) {
        let index = self.batch_index(style);
        self.batches[index].segments.push(segment);
    }

    fn push_pair(&mut self, style: ResolvedEdgeStyle<'a>, segments: [LineSegment; 2]) {
        let index = self.batch_index(style);
        self.batches[index].segments.extend_from_slice(&segments);
    }
}

#[derive(Clone, Copy)]
struct CachedEdgeScreenNode {
    x: f64,
    y: f64,
    radius: f64,
    present: bool,
}

impl CachedEdgeScreenNode {
    const fn missing() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            radius: 0.0,
            present: false,
        }
    }
}

fn cached_edge_screen_node<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
    node_id: NodeIndex,
    cache: &mut [CachedEdgeScreenNode],
    transformed: &mut usize,
) -> Option<CachedEdgeScreenNode> {
    let slot = cache.get_mut(node_id.index())?;
    if slot.present {
        return Some(*slot);
    }
    let particle = particles.get(node_id.index())?;
    let (x, y) = ctx
        .camera
        .world_to_screen((particle.x as f64, particle.y as f64), ctx.viewport);
    let radius = graph
        .get_node(node_id)
        .map(|node| ctx.camera.node_screen_radius(node.radius))
        .unwrap_or(0.0);
    *slot = CachedEdgeScreenNode {
        x,
        y,
        radius,
        present: true,
    };
    *transformed += 1;
    Some(*slot)
}

fn styled_edge_geometry(
    mut segment: LineSegment,
    style: &EdgeVisualStyle,
    target_radius: f64,
    width: f64,
) -> (LineSegment, Option<[LineSegment; 2]>) {
    if let Some(offset) = style.lateral_offset.filter(|value| value.is_finite()) {
        let dx = segment.x2 - segment.x1;
        let dy = segment.y2 - segment.y1;
        let length = (dx * dx + dy * dy).sqrt();
        if length > 1e-6 {
            let offset_x = -dy / length * f64::from(offset);
            let offset_y = dx / length * f64::from(offset);
            segment.x1 += offset_x;
            segment.y1 += offset_y;
            segment.x2 += offset_x;
            segment.y2 += offset_y;
        }
    }

    let arrowhead = if style.directed {
        let dx = segment.x2 - segment.x1;
        let dy = segment.y2 - segment.y1;
        let length = (dx * dx + dy * dy).sqrt();
        if length > 1e-6 {
            let ux = dx / length;
            let uy = dy / length;
            let target_offset = (target_radius + 2.0).min(length * 0.45);
            let tip_x = segment.x2 - ux * target_offset;
            let tip_y = segment.y2 - uy * target_offset;
            let arrow_length = 7.0 + width;
            let wing = arrow_length * 0.45;
            let back_x = tip_x - ux * arrow_length;
            let back_y = tip_y - uy * arrow_length;
            Some([
                LineSegment { x1: tip_x, y1: tip_y, x2: back_x - uy * wing, y2: back_y + ux * wing },
                LineSegment { x1: tip_x, y1: tip_y, x2: back_x + uy * wing, y2: back_y - ux * wing },
            ])
        } else {
            None
        }
    } else {
        None
    };
    (segment, arrowhead)
}

fn draw_styled_edge_batch(render: &mut dyn RenderContext, batch: &StyledEdgeBatch<'_>) {
    let dash = batch.style.dash();
    render.set_global_alpha(batch.style.alpha());
    render.set_line_dash(&dash);
    render.draw_line_batch(
        &batch.segments,
        &batch.style.color,
        batch.style.width(),
    );
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

    label_grid::select_labels(&candidates, ctx.viewport, ctx.camera.zoom, ctx.label_density, ctx.forced_labels, ctx.label_lod)
}

/// Draw every visible node as a circle (radius from `Camera2D::node_screen_radius`,
/// color from [`category_color`]), plus a selection/hover ring and
/// label-LOD labels (Wave 2.3 — `crate::label_grid`'s sigma `LabelGrid`
/// port, alpha boosted by degree). Returns node/label draw counts.
///
/// Honours `ctx.hidden` directly (graph-strengthening arc G1.4) — a node
/// hidden by cluster collapse/local-subgraph/filter is skipped even if
/// present in `ctx.visible`, mirroring [`draw_edges`]'s own
/// `ctx.hidden.contains(...)` check. Production always pre-filters
/// `ctx.visible` upstream (`GraphEngine::refresh_visible`), so this is
/// normally a no-op — but relying on the caller to have done that made
/// this function an API footgun (see `uzor-graph/CLAUDE.md`'s
/// graph-strengthening-arc entry): a caller building `visible` the
/// obvious way, without independently threading the exclusion set
/// through it too, got stacked circles at one pixel with `draw_edges`
/// silently disagreeing. Measured before choosing this over a debug
/// assert: an extra `HashSet::contains` per node costs ~0ns when `hidden`
/// is empty and ~5-7ns/node even at 50k nodes / 10k hidden (an order of
/// magnitude past this crate's own "large graph" scale) — negligible
/// against a 16.6ms frame budget, so consistency with `draw_edges` won
/// over a caller-pre-filtered contract.
///
/// Bracketed in `render.save()`/`render.restore()` (2D quality audit A5 /
/// graph-strengthening arc G1.3) — `draw_circle_batch` mutates persistent
/// fill-color state, and the ring/label passes below mutate stroke color/
/// width and font with no restore of their own; without the bracket that
/// state leaks past this call into whatever the caller paints next.
pub fn draw_nodes<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> NodeDrawStats {
    draw_nodes_impl(render, graph, particles, ctx, true)
}

/// Draw the same node geometry, semantic markers, and interaction rings
/// as [`draw_nodes`], but bypass all node-label candidate, LOD, degree
/// normalization, and text-rendering work.
pub fn draw_nodes_without_labels<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
) -> NodeDrawStats {
    draw_nodes_impl(render, graph, particles, ctx, false)
}

fn draw_nodes_impl<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
    labels_enabled: bool,
) -> NodeDrawStats {
    let collect_started = std::time::Instant::now();
    render.save();
    let mut by_color: HashMap<String, Vec<CircleBatch>> = HashMap::new();
    let mut dim: Vec<CircleBatch> = Vec::new();
    let mut styled: Vec<(NodeIndex, CircleBatch, bool)> = Vec::new();
    let mut nodes_drawn = 0usize;

    for &id in ctx.visible {
        if ctx.hidden.contains(&id) {
            continue;
        }
        let (Some(p), Some(node)) = (particles.get(id.index()), graph.get_node(id)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);
        let circle = CircleBatch { cx: sx, cy: sy, r };
        nodes_drawn += 1;
        let dimmed = ctx.focus.is_active() && !ctx.focus.is_selected(u64::from(id));
        if node.style.is_some() {
            styled.push((id, circle, dimmed));
        } else if dimmed {
            dim.push(circle);
        } else {
            by_color.entry(category_color(&node.category, &ctx.theme.category_palette)).or_default().push(circle);
        }
    }
    uzor::diagnostics::stage(
        "graph_2d_nodes",
        "collect_end",
        format_args!(
            "visible={} drawn={} colors={} dim={} styled={} duration_us={}",
            ctx.visible.len(),
            nodes_drawn,
            by_color.len(),
            dim.len(),
            styled.len(),
            collect_started.elapsed().as_micros(),
        ),
    );

    let base_emit_started = std::time::Instant::now();
    if !dim.is_empty() {
        render.set_global_alpha(ctx.theme.dim_alpha);
        render.draw_circle_batch(&dim, &ctx.theme.dim_node_fill);
        render.set_global_alpha(1.0);
    }
    // Deterministic paint order (graph-strengthening arc G1.3, five-leg
    // harness finding — the audits missed this, the harness caught it):
    // `HashMap` iteration order is NOT deterministic, and `by_color` is
    // rebuilt fresh every call, so the OLD `for (color, circles) in
    // &by_color` iterated colors in an order that could differ frame to
    // frame. Whenever two nodes of DIFFERENT categories overlap on
    // screen — routine in a force graph — which one painted on top
    // flickered between frames, and any pixel proof of an overlapping
    // scene was irreproducible. Fixed via `BTreeMap` (lexicographic hex
    // color order) instead of a per-node z-order: a real z-order (e.g.
    // largest-radius-first) would need one draw call per node, defeating
    // the whole point of this color-batching pass. Lexicographic order is
    // arbitrary but STABLE — every category's own batch always paints in
    // the same relative order, so overlap resolution is reproducible,
    // which is what the harness/pixel-proof gate actually needs; it is
    // not claimed to be a meaningful z-order (e.g. "important categories
    // on top").
    let by_color: BTreeMap<String, Vec<CircleBatch>> = by_color.into_iter().collect();
    for (color, circles) in &by_color {
        render.draw_circle_batch(circles, color);
    }
    for (id, circle, dimmed) in &styled {
        let node = graph.node(*id);
        let style = node.style.as_ref().expect("styled list only contains styled nodes");
        let fill = style.fill.as_deref().unwrap_or(if *dimmed {
            &ctx.theme.dim_node_fill
        } else {
            // Kept owned for the duration of this draw call below.
            ""
        });
        let category_fill;
        let fill = if fill.is_empty() {
            category_fill = category_color(&node.category, &ctx.theme.category_palette);
            category_fill.as_str()
        } else {
            fill
        };
        let alpha = style.alpha.unwrap_or(1.0).clamp(0.0, 1.0) as f64
            * if *dimmed { ctx.theme.dim_alpha } else { 1.0 };
        render.set_global_alpha(alpha);
        render.draw_circle_batch(&[*circle], fill);
        render.set_global_alpha(1.0);
    }
    uzor::diagnostics::stage(
        "graph_2d_nodes",
        "base_emit_end",
        format_args!(
            "colors={} styled={} duration_us={}",
            by_color.len(),
            styled.len(),
            base_emit_started.elapsed().as_micros(),
        ),
    );

    let label_set = labels_enabled.then(|| labels_to_draw(graph, particles, ctx));
    // Whole-graph max degree (not just the currently-visible subset) —
    // invariant across pan/zoom, so the same node always normalizes to
    // the same degree-boost regardless of what else happens to be on
    // screen this frame (Wave 2.3 determinism gate).
    let max_degree = if label_set.is_some() {
        graph.nodes().map(|(id, _)| graph.degree(id)).max().unwrap_or(0).max(1)
    } else {
        1
    };
    let mut labels_drawn = 0usize;

    let semantics_started = std::time::Instant::now();
    for &id in ctx.visible {
        if ctx.hidden.contains(&id) {
            continue;
        }
        let (Some(p), Some(node)) = (particles.get(id.index()), graph.get_node(id)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);

        if let Some(style) = &node.style {
            let dimmed = ctx.focus.is_active() && !ctx.focus.is_selected(u64::from(id));
            let category_fill;
            let fill = if let Some(fill) = style.fill.as_deref() {
                fill
            } else if dimmed {
                &ctx.theme.dim_node_fill
            } else {
                category_fill = category_color(&node.category, &ctx.theme.category_palette);
                category_fill.as_str()
            };
            draw_node_semantics(render, sx, sy, r, style, fill, dimmed, ctx.theme);
        }

        if ctx.selection.contains(&id) {
            render.set_stroke_color(&ctx.theme.selection_ring_color);
            render.set_stroke_width(ctx.theme.selection_ring_width);
            render.begin_path();
            render.arc(sx, sy, r + ctx.theme.selection_ring_offset_px, 0.0, std::f64::consts::TAU);
            render.stroke();
        } else if Some(id) == ctx.hovered {
            render.set_stroke_color(&ctx.theme.hover_ring_color);
            render.set_stroke_width(ctx.theme.hover_ring_width);
            render.begin_path();
            render.arc(sx, sy, r + ctx.theme.hover_ring_offset_px, 0.0, std::f64::consts::TAU);
            render.stroke();
        }

        let Some(label_set) = &label_set else {
            continue;
        };
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
            label_grid::label_alpha(ctx.camera.zoom, normalized_degree, ctx.label_lod)
        };

        if alpha > 0.01 {
            render.set_global_alpha(alpha);
            render.set_font(&ctx.theme.label_font);
            // Halo (owner defect report: thin edge strokes crossing node
            // label text made it unreadable) — a 4-direction offset-fill
            // in `ctx.label_halo` under the real `ctx.theme.label_fill`.
            fill_text_with_halo(
                render,
                &node.label,
                sx + r + ctx.theme.label_offset_x,
                sy + ctx.theme.label_offset_y,
                &ctx.theme.label_fill,
                ctx.label_halo,
            );
            render.set_global_alpha(1.0);
            labels_drawn += 1;
        }
    }
    uzor::diagnostics::stage(
        "graph_2d_nodes",
        "semantics_end",
        format_args!(
            "visible={} labels={} duration_us={}",
            ctx.visible.len(),
            labels_drawn,
            semantics_started.elapsed().as_micros(),
        ),
    );

    render.restore();
    NodeDrawStats { nodes_drawn, labels_drawn }
}

fn draw_node_semantics(
    render: &mut dyn RenderContext,
    sx: f64,
    sy: f64,
    radius: f64,
    style: &NodeVisualStyle,
    fill: &str,
    dimmed: bool,
    theme: &GraphTheme,
) {
    if style.outline.is_none() && style.marker.is_none() {
        return;
    }

    let color = style.outline.as_deref().unwrap_or(fill);
    let width = style.outline_width.unwrap_or(1.5).max(0.1) as f64;
    let alpha = style.alpha.unwrap_or(1.0).clamp(0.0, 1.0) as f64
        * if dimmed { theme.dim_alpha } else { 1.0 };
    render.set_global_alpha(alpha);
    render.set_stroke_color(color);
    render.set_stroke_width(width);
    render.set_line_dash(&[]);

    if style.outline.is_some() {
        render.begin_path();
        render.arc(sx, sy, radius + width * 0.5, 0.0, std::f64::consts::TAU);
        render.stroke();
    }

    match style.marker {
        Some(NodeMarker::DoubleRing) => {
            for offset in [3.0 + width, 6.0 + width * 2.0] {
                render.begin_path();
                render.arc(sx, sy, radius + offset, 0.0, std::f64::consts::TAU);
                render.stroke();
            }
        }
        Some(NodeMarker::Boundary) => {
            let extent = radius + 4.0 + width;
            render.stroke_rect(sx - extent, sy - extent, extent * 2.0, extent * 2.0);
        }
        Some(NodeMarker::Frontier) => {
            render.set_line_dash(&[4.0, 3.0]);
            render.begin_path();
            render.arc(sx, sy, radius + 4.0 + width, 0.0, std::f64::consts::TAU);
            render.stroke();
        }
        Some(NodeMarker::Warning) => {
            let extent = radius + 5.0 + width;
            render.begin_path();
            render.move_to(sx, sy - extent);
            render.line_to(sx + extent * 0.88, sy + extent * 0.62);
            render.line_to(sx - extent * 0.88, sy + extent * 0.62);
            render.close_path();
            render.stroke();
        }
        None => {}
    }

    render.set_line_dash(&[]);
    render.set_global_alpha(1.0);
}

/// Draw the aggregated cross-cluster edges for every collapsed cluster —
/// one synthetic line per outside neighbor (summed weight, thicker line
/// for a heavier aggregate), replacing the N raw overlapping lines
/// [`draw_edges`] already excludes via `ctx.hidden`. Returns the number
/// of synthetic edges drawn.
///
/// Bracketed in `render.save()`/`render.restore()` (2D quality audit A5 /
/// graph-strengthening arc G1.3) — `draw_line_batch` mutates persistent
/// stroke color/width state with no restore of its own.
pub fn draw_cluster_edges(
    render: &mut dyn RenderContext,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
    clusters: &ClusterRegistry,
) -> usize {
    render.save();
    let mut drawn = 0;
    for cluster in clusters.collapsed_clusters() {
        let Some(rep) = particles.get(cluster.representative.index()) else { continue };
        let (rx, ry) = ctx.camera.world_to_screen((rep.x as f64, rep.y as f64), ctx.viewport);
        for edge in cluster.aggregated_edges() {
            let Some(other) = particles.get(edge.outside.index()) else { continue };
            let (ox, oy) = ctx.camera.world_to_screen((other.x as f64, other.y as f64), ctx.viewport);
            let width = (1.0 + (edge.weight as f64).sqrt()).min(ctx.theme.cluster_edge_width_cap);
            render.draw_line_batch(&[LineSegment { x1: rx, y1: ry, x2: ox, y2: oy }], &ctx.theme.cluster_accent, width);
            drawn += 1;
        }
    }
    render.restore();
    drawn
}

/// Draw a distinct double-ring + member-count label over every collapsed
/// cluster's representative — the "this circle is actually N nodes"
/// affordance. Returns the number of super-nodes drawn.
///
/// The "×N" label paints through `fill_text_with_halo` (deferred tail
/// from the node-label halo pass — see `uzor-graph/CLAUDE.md`'s
/// divergence log): a thin edge stroke crossing this label at a similar
/// luminance is exactly the same defect the halo already fixes for
/// ordinary node labels, so this label gets the identical protection.
pub fn draw_cluster_supernodes<N, E>(
    render: &mut dyn RenderContext,
    graph: &Graph<N, E>,
    particles: &[Particle],
    ctx: &DrawContext<'_>,
    clusters: &ClusterRegistry,
) -> usize {
    render.save();
    let mut drawn = 0;
    for cluster in clusters.collapsed_clusters() {
        let rep = cluster.representative;
        let (Some(p), Some(node)) = (particles.get(rep.index()), graph.get_node(rep)) else { continue };
        let (sx, sy) = ctx.camera.world_to_screen((p.x as f64, p.y as f64), ctx.viewport);
        let r = ctx.camera.node_screen_radius(node.radius);

        render.set_stroke_color(&ctx.theme.cluster_accent);
        render.set_stroke_width(ctx.theme.cluster_ring_width);
        render.begin_path();
        render.arc(sx, sy, r + ctx.theme.cluster_ring_inner_offset_px, 0.0, std::f64::consts::TAU);
        render.stroke();
        render.begin_path();
        render.arc(sx, sy, r + ctx.theme.cluster_ring_outer_offset_px, 0.0, std::f64::consts::TAU);
        render.stroke();

        render.set_font(&ctx.theme.label_font);
        fill_text_with_halo(
            render,
            &format!("×{}", cluster.member_count()),
            sx + r + ctx.theme.cluster_label_offset_x,
            sy + ctx.theme.cluster_label_offset_y,
            &ctx.theme.cluster_label_color,
            ctx.label_halo,
        );
        drawn += 1;
    }
    render.restore();
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
/// `theme` picks the hover card's own `FigureTheme` (graph-strengthening
/// arc Wave G2 / 2D quality audit A3 — was an unconditional
/// `FigureTheme::dark()` literal with no override; see
/// [`crate::theme::GraphTheme::hover_card`]).
pub fn draw_hover_card(render: &mut dyn RenderContext, anchor_px: (f64, f64), info: &HoverCardInfo<'_>, bounds: Rect, theme: &FigureTheme) {
    // `draw_tooltip` itself already brackets its own state (font/text-
    // align/baseline) in `save()`/`restore()` — this bracket is
    // additionally applied here so every public entry point in this
    // module keeps the SAME guarantee independent of what its own
    // implementation happens to delegate to (2D quality audit A5 /
    // graph-strengthening arc G1.3).
    render.save();
    let lines = [
        ("label".to_owned(), info.label.to_owned()),
        ("category".to_owned(), info.category.to_owned()),
        ("degree".to_owned(), info.degree.to_string()),
        ("pinned".to_owned(), info.pinned.to_string()),
    ];
    draw_tooltip(render, theme, anchor_px, &lines, bounds);
    render.restore();
}

/// Live rubber-band overlay for an in-progress box-select drag (Wave 2.4
/// — oss doc §2.3: "a translucent rectangle overlay drawn from
/// `select[0..3]` each frame"). `rect` is already corner-normalized
/// screen-space (see [`crate::engine::GraphEngine::box_select_rect`]) —
/// this function is pure paint, no selection logic of its own. `theme`
/// supplies the fill/border colors (graph-strengthening arc Wave G2 — was
/// four hardcoded module constants; see [`crate::theme::GraphTheme::
/// box_select_fill`] and its sibling fields).
pub fn draw_box_select_rect(render: &mut dyn RenderContext, rect: Rect, theme: &GraphTheme) {
    // 2D quality audit A5 / graph-strengthening arc G1.3 — this function
    // leaves stroke color/width behind with no restore of its own
    // (`set_global_alpha` is the one property it already reset back to
    // `1.0` unprompted).
    render.save();
    render.set_global_alpha(theme.box_select_fill_alpha);
    render.set_fill_color(&theme.box_select_fill);
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);
    render.set_global_alpha(1.0);
    render.set_stroke_color(&theme.box_select_border);
    render.set_stroke_width(theme.box_select_border_width);
    render.stroke_rect(rect.x, rect.y, rect.width, rect.height);
    render.restore();
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
    // `StateTrackingContext`/`ColorOrderRecorder` below call `Painter`/
    // `TextRenderer` methods directly on a concrete type (not through
    // `&mut dyn RenderContext`, which the trait-object dispatch every
    // production call site uses) — the traits must be in scope for that.
    use uzor::render::{Painter, TextRenderer};

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
        let theme = GraphTheme::dark();
        let lod = LabelLodConfig::default();

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
            label_lod: &lod,
            theme: &theme,
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
        let theme = GraphTheme::dark();
        let lod = LabelLodConfig::default();
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
            label_lod: &lod,
            theme: &theme,
        };

        let first = labels_to_draw(&graph, &particles, &ctx);
        for _ in 0..5 {
            assert_eq!(labels_to_draw(&graph, &particles, &ctx), first, "identical state must yield an identical label set every call");
        }
    }

    // ── Graph-strengthening arc G1.3: no save()/restore() anywhere in the
    // render layer + nondeterministic node-color paint order ────────────
    //
    // `StateTrackingContext` is a REAL `save`/`restore` stack (unlike
    // `RecordingRenderContext`'s no-op pair in `engine3d.rs`'s test
    // module, which exists only to record `fill_text` calls) — its
    // `restore()` genuinely pops back to whatever `PaintState` was live at
    // the matching `save()`, so a bracketed draw fn that leaks nothing
    // leaves `state` byte-identical to what it was before the call, and a
    // fn missing its bracket (or missing a `restore()` on some path) does
    // not.

    #[derive(Clone, Debug, PartialEq)]
    struct PaintState {
        font: String,
        fill_color: String,
        stroke_color: String,
        stroke_width: f64,
        global_alpha: f64,
        text_align: uzor::render::TextAlign,
        text_baseline: uzor::render::TextBaseline,
        line_cap: String,
        line_join: String,
    }

    impl Default for PaintState {
        fn default() -> Self {
            Self {
                font: String::new(),
                fill_color: String::new(),
                stroke_color: String::new(),
                stroke_width: 1.0,
                global_alpha: 1.0,
                text_align: uzor::render::TextAlign::default(),
                text_baseline: uzor::render::TextBaseline::default(),
                line_cap: "butt".to_owned(),
                line_join: "miter".to_owned(),
            }
        }
    }

    struct StateTrackingContext {
        state: PaintState,
        stack: Vec<PaintState>,
    }

    impl StateTrackingContext {
        fn new() -> Self {
            Self { state: PaintState::default(), stack: Vec::new() }
        }
    }

    impl uzor::render::Painter for StateTrackingContext {
        fn save(&mut self) {
            self.stack.push(self.state.clone());
        }
        fn restore(&mut self) {
            if let Some(s) = self.stack.pop() {
                self.state = s;
            }
        }
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_fill_color(&mut self, color: &str) {
            self.state.fill_color = color.to_owned();
        }
        fn set_global_alpha(&mut self, alpha: f64) {
            self.state.global_alpha = alpha;
        }
        fn set_stroke_color(&mut self, color: &str) {
            self.state.stroke_color = color.to_owned();
        }
        fn set_stroke_width(&mut self, width: f64) {
            self.state.stroke_width = width;
        }
        fn set_line_dash(&mut self, _pattern: &[f64]) {}
        fn set_line_cap(&mut self, cap: &str) {
            self.state.line_cap = cap.to_owned();
        }
        fn set_line_join(&mut self, join: &str) {
            self.state.line_join = join.to_owned();
        }
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
    }
    impl uzor::render::TextRenderer for StateTrackingContext {
        fn set_font(&mut self, font: &str) {
            self.state.font = font.to_owned();
        }
        fn set_text_align(&mut self, align: uzor::render::TextAlign) {
            self.state.text_align = align;
        }
        fn set_text_baseline(&mut self, baseline: uzor::render::TextBaseline) {
            self.state.text_baseline = baseline;
        }
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    }
    impl uzor::render::TextMetrics for StateTrackingContext {
        fn measure_text(&self, _text: &str) -> f64 {
            0.0
        }
        fn text_bounds(&self, _text: &str, _font: &str) -> uzor::render::TextBounds {
            uzor::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl uzor::render::Masking for StateTrackingContext {
        fn clip(&mut self) {}
    }
    impl uzor::render::Effects for StateTrackingContext {}
    impl uzor::render::ShapeHelpers for StateTrackingContext {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    }
    impl uzor::render::GradientPainter for StateTrackingContext {}
    impl uzor::render::UiEffectHelpers for StateTrackingContext {}
    impl uzor::render::BatchPainter for StateTrackingContext {}
    impl uzor::render::RenderContext for StateTrackingContext {
        fn dpr(&self) -> f64 {
            1.0
        }
    }

    /// Triangle `a-b-c` (all overlapping at the same screen position — the
    /// exact "two nodes of different categories overlap" scenario fix G1.3
    /// targets) plus a 2-member cluster `{d, e}` collapsed, with `d`
    /// carrying one cross-cluster edge to `a` (so `draw_cluster_edges`/
    /// `draw_cluster_supernodes` both have something real to draw, not
    /// just an empty no-op loop).
    fn overlap_fixture() -> (G, Vec<Particle>, ClusterRegistry, crate::cluster::GroupId, NodeIndex, NodeIndex, NodeIndex) {
        let mut graph = G::new();
        let a = graph.push_node((), "a", "red", 6.0);
        let b = graph.push_node((), "b", "blue", 6.0);
        let c = graph.push_node((), "c", "green", 6.0);
        let d = graph.push_node((), "d", "amber", 4.0);
        let e = graph.push_node((), "e", "amber", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        graph.push_edge(d, a, 1.0, ());

        let mut particles = vec![Particle::default(); graph.node_count()];
        for p in particles.iter_mut().take(3) {
            *p = Particle::at(0.0, 0.0); // a, b, c all coincide on screen.
        }
        particles[d.index()] = Particle::at(200.0, 0.0);
        particles[e.index()] = Particle::at(220.0, 0.0);

        let mut clusters = ClusterRegistry::default();
        let cluster_id = clusters.define(&graph, vec![d, e]).expect("non-empty cluster");
        assert!(clusters.collapse(cluster_id, &mut graph, &mut particles));

        (graph, particles, clusters, cluster_id, a, b, c)
    }

    fn draw_ctx_for<'a>(
        camera: &'a Camera2D,
        viewport: Rect,
        visible: &'a [NodeIndex],
        focus: &'a FocusSet,
        selection: &'a BTreeSet<NodeIndex>,
        hovered: Option<NodeIndex>,
        hidden: &'a HashSet<NodeIndex>,
        forced: &'a HashSet<NodeIndex>,
        lod: &'a LabelLodConfig,
        theme: &'a GraphTheme,
    ) -> DrawContext<'a> {
        DrawContext {
            camera,
            viewport,
            visible,
            focus,
            selection,
            hovered,
            hidden,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
            label_halo: crate::engine::DEFAULT_LABEL_HALO,
            forced_labels: forced,
            label_lod: lod,
            theme,
        }
    }

    /// The state-leak gate itself: EVERY public draw fn in this module
    /// must leave `RenderContext`'s paint state (font/fill/stroke/alpha/
    /// text-align/baseline/line-cap/line-join) byte-identical to what it
    /// was immediately before the call — a draw-output test alone (the
    /// tests above) cannot catch this, since a state leak produces no
    /// wrong PIXEL in the SAME frame, only a corrupted starting state for
    /// whatever paints next.
    #[test]
    fn every_public_draw_fn_leaves_the_render_context_paint_state_unchanged() {
        let (graph, particles, clusters, _cluster_id, a, b, _c) = overlap_fixture();
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let focus = FocusSet::empty();
        let mut selection = BTreeSet::new();
        selection.insert(a);
        let hidden: HashSet<NodeIndex> = HashSet::new();
        let forced: HashSet<NodeIndex> = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, Some(b), &hidden, &forced, &lod, &theme);

        let mut render = StateTrackingContext::new();
        // A caller-set state distinct from every default this module's own
        // draw calls happen to use, so the test can't pass by coincidence
        // (a bracket-less leaked call would land on one of THIS module's
        // own literals, e.g. `"#ffffff"`/`"11px sans-serif"`, not silently
        // reproduce the caller's own prior state).
        render.set_font("20px serif");
        render.set_fill_color("#123456");
        render.set_stroke_color("#abcdef");
        render.set_stroke_width(9.0);
        render.set_global_alpha(0.42);
        render.set_text_align(uzor::render::TextAlign::Right);
        render.set_text_baseline(uzor::render::TextBaseline::Bottom);
        render.set_line_cap("square");
        render.set_line_join("bevel");
        let caller_state = render.state.clone();

        draw_edges(&mut render, &graph, &particles, &ctx);
        assert_eq!(render.state, caller_state, "draw_edges must not leak paint state past its own call");

        draw_nodes(&mut render, &graph, &particles, &ctx);
        assert_eq!(render.state, caller_state, "draw_nodes must not leak paint state past its own call");

        draw_cluster_edges(&mut render, &particles, &ctx, &clusters);
        assert_eq!(render.state, caller_state, "draw_cluster_edges must not leak paint state past its own call");

        draw_cluster_supernodes(&mut render, &graph, &particles, &ctx, &clusters);
        assert_eq!(render.state, caller_state, "draw_cluster_supernodes must not leak paint state past its own call");

        let info = HoverCardInfo { label: "b", category: "blue", degree: 1, pinned: false };
        draw_hover_card(&mut render, (400.0, 300.0), &info, viewport, &theme.hover_card);
        assert_eq!(render.state, caller_state, "draw_hover_card must not leak paint state past its own call");

        draw_box_select_rect(&mut render, Rect::new(10.0, 10.0, 50.0, 50.0), &theme);
        assert_eq!(render.state, caller_state, "draw_box_select_rect must not leak paint state past its own call");
    }

    /// Determinism gate for the fixed nondeterministic-paint-order defect
    /// (graph-strengthening arc G1.3): rendering the SAME overlapping-node
    /// scene twice (fresh `by_color: HashMap` rebuilt each call, per
    /// `draw_nodes`'s own doc comment) must issue the identical SEQUENCE
    /// of `draw_circle_batch` color batches both times — before the
    /// `BTreeMap` fix, `HashMap` iteration order was merely unspecified
    /// PER PROCESS RUN (stable within one run, since Rust's default hasher
    /// is seeded once at process start) — this test can't observe
    /// cross-run nondeterminism directly, so it instead asserts the
    /// STRUCTURAL property the fix actually guarantees: the batch color
    /// sequence is lexicographic (`BTreeMap` order), which is what makes
    /// repeated renders — and repeated PROCESS runs — reproducible.
    #[derive(Default)]
    struct ColorOrderRecorder {
        state: PaintState,
        stack: Vec<PaintState>,
        circle_batch_colors: Vec<String>,
        current_dash: Vec<f64>,
        line_batches: Vec<(usize, String, f64, Vec<f64>, f64)>,
        line_segments: Vec<Vec<LineSegment>>,
        alpha_changes: Vec<f64>,
        /// Every `set_stroke_color` call, in order — unlike `state.stroke_color`
        /// (which a `save()`/`restore()` bracket resets away by the time a
        /// caller can observe it), this survives past the call so a test can
        /// assert WHAT was painted mid-function, not just the final restored
        /// state (Wave G2 theme-swap test).
        stroke_colors: Vec<String>,
    }
    impl uzor::render::Painter for ColorOrderRecorder {
        fn save(&mut self) {
            self.stack.push(self.state.clone());
        }
        fn restore(&mut self) {
            if let Some(s) = self.stack.pop() {
                self.state = s;
            }
        }
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_fill_color(&mut self, color: &str) {
            self.state.fill_color = color.to_owned();
        }
        fn set_global_alpha(&mut self, alpha: f64) {
            self.state.global_alpha = alpha;
            self.alpha_changes.push(alpha);
        }
        fn set_stroke_color(&mut self, color: &str) {
            self.state.stroke_color = color.to_owned();
            self.stroke_colors.push(color.to_owned());
        }
        fn set_stroke_width(&mut self, width: f64) {
            self.state.stroke_width = width;
        }
        fn set_line_dash(&mut self, pattern: &[f64]) {
            self.current_dash = pattern.to_vec();
        }
        fn set_line_cap(&mut self, cap: &str) {
            self.state.line_cap = cap.to_owned();
        }
        fn set_line_join(&mut self, join: &str) {
            self.state.line_join = join.to_owned();
        }
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
    }
    impl uzor::render::TextRenderer for ColorOrderRecorder {
        fn set_font(&mut self, font: &str) {
            self.state.font = font.to_owned();
        }
        fn set_text_align(&mut self, align: uzor::render::TextAlign) {
            self.state.text_align = align;
        }
        fn set_text_baseline(&mut self, baseline: uzor::render::TextBaseline) {
            self.state.text_baseline = baseline;
        }
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    }
    impl uzor::render::TextMetrics for ColorOrderRecorder {
        fn measure_text(&self, _text: &str) -> f64 {
            0.0
        }
        fn text_bounds(&self, _text: &str, _font: &str) -> uzor::render::TextBounds {
            uzor::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl uzor::render::Masking for ColorOrderRecorder {
        fn clip(&mut self) {}
    }
    impl uzor::render::Effects for ColorOrderRecorder {}
    impl uzor::render::ShapeHelpers for ColorOrderRecorder {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    }
    impl uzor::render::GradientPainter for ColorOrderRecorder {}
    impl uzor::render::UiEffectHelpers for ColorOrderRecorder {}
    impl uzor::render::BatchPainter for ColorOrderRecorder {
        fn draw_circle_batch(&mut self, circles: &[uzor::render::CircleBatch], color: &str) {
            if circles.is_empty() {
                return;
            }
            self.circle_batch_colors.push(color.to_owned());
        }
        fn draw_line_batch(&mut self, lines: &[uzor::render::LineSegment], color: &str, width: f64) {
            if lines.is_empty() {
                return;
            }
            self.line_batches.push((lines.len(), color.to_owned(), width, self.current_dash.clone(), self.state.global_alpha));
            self.line_segments.push(lines.to_vec());
        }
    }
    impl uzor::render::RenderContext for ColorOrderRecorder {
        fn dpr(&self) -> f64 {
            1.0
        }
    }

    #[test]
    fn edge_prepare_reuses_screen_transforms_and_consecutive_style_groups() {
        let mut graph = G::new();
        let a = graph.push_node((), "a", "x", 3.0);
        let particles = vec![Particle::at(12.0, 34.0)];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible = vec![a];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(
            &camera,
            viewport,
            &visible,
            &focus,
            &selection,
            None,
            &hidden,
            &forced,
            &lod,
            &theme,
        );
        let mut screen_cache = vec![CachedEdgeScreenNode::missing(); graph.node_count()];
        let mut transformed = 0;
        let first = cached_edge_screen_node(
            &graph,
            &particles,
            &ctx,
            a,
            &mut screen_cache,
            &mut transformed,
        )
        .unwrap();
        let second = cached_edge_screen_node(
            &graph,
            &particles,
            &ctx,
            a,
            &mut screen_cache,
            &mut transformed,
        )
        .unwrap();
        assert_eq!(transformed, 1, "one node shared by many edges is transformed once per frame");
        assert_eq!((first.x, first.y, first.radius), (second.x, second.y, second.radius));

        let edge_style = EdgeVisualStyle {
            tint: Some("#55ccaa".into()),
            alpha: Some(0.7),
            width: Some(2.5),
            dash: Some(DashPattern::Pattern(vec![5.0, 3.0])),
            lateral_offset: None,
            directed: true,
        };
        let resolved = ResolvedEdgeStyle::new(&edge_style, false, &theme);
        let segment = LineSegment {
            x1: 0.0,
            y1: 1.0,
            x2: 2.0,
            y2: 3.0,
        };
        let mut body_batches = StyledEdgeBatchTable::with_capacity(2);
        let mut arrow_batches = StyledEdgeBatchTable::with_capacity(2);
        for _ in 0..64 {
            body_batches.push_segment(resolved, segment);
            arrow_batches.push_pair(resolved.with_solid_dash(), [segment, segment]);
        }
        assert_eq!(body_batches.batches.len(), 1);
        assert_eq!(body_batches.batches[0].segments.len(), 64);
        assert_eq!(body_batches.hash_lookups, 1);
        assert_eq!(body_batches.fast_hits, 63);
        assert_eq!(arrow_batches.batches.len(), 1);
        assert_eq!(arrow_batches.batches[0].segments.len(), 128);
        assert_eq!(arrow_batches.hash_lookups, 1);
        assert_eq!(arrow_batches.fast_hits, 63);
    }

    #[test]
    fn visible_edge_candidates_match_full_scan_order_and_reduce_scanned_edges() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 2.0);
        let b = graph.push_node((), "b", "x", 2.0);
        let c = graph.push_node((), "c", "x", 2.0);
        let d = graph.push_node((), "d", "x", 2.0);
        let noise: Vec<_> = (0..20)
            .map(|index| graph.push_node((), format!("noise-{index}"), "noise", 2.0))
            .collect();

        graph.push_edge(noise[0], noise[1], 1.0, ());
        let ab = graph.push_edge(a, b, 1.0, ());
        graph.push_edge(noise[1], noise[2], 1.0, ());
        let bc = graph.push_edge(b, c, 1.0, ());
        let cc = graph.push_edge(c, c, 1.0, ());
        let da = graph.push_edge(d, a, 1.0, ());
        for pair in noise[2..].windows(2) {
            graph.push_edge(pair[0], pair[1], 1.0, ());
        }

        // Deliberately out of graph order and with a repeated node. A
        // self-loop also appears twice in that node's adjacency list.
        let visible = vec![c, a, c];
        let scan = visible_edge_candidates(&graph, &visible);
        let visible_set: HashSet<_> = visible.iter().copied().collect();
        let full_scan_order: Vec<_> = graph
            .edges()
            .filter_map(|(edge_id, edge)| {
                (visible_set.contains(&edge.from) || visible_set.contains(&edge.to))
                    .then_some(edge_id)
            })
            .collect();

        assert_eq!(scan.edge_ids, full_scan_order);
        assert_eq!(scan.edge_ids, vec![ab, bc, cc, da]);
        assert_eq!(
            scan.candidates_scanned, 8,
            "only adjacency entries for the three visible-list entries are scanned",
        );
        assert!(!scan.used_full_scan);
        assert!(
            scan.candidates_scanned < graph.edge_count(),
            "the sparse visible set must inspect fewer candidates than a full edge scan",
        );

        let all_visible: Vec<_> = graph.nodes().map(|(node, _)| node).collect();
        let dense_scan = visible_edge_candidates(&graph, &all_visible);
        let all_edges: Vec<_> = graph.edges().map(|(edge, _)| edge).collect();
        assert!(dense_scan.used_full_scan);
        assert_eq!(dense_scan.candidates_scanned, graph.edge_count());
        assert_eq!(dense_scan.edge_ids, all_edges);

        let particles: Vec<_> = graph
            .nodes()
            .map(|(node, _)| Particle::at(node.index() as f32 * 10.0, node.index() as f32 * 3.0))
            .collect();
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);
        let expected_segments: Vec<_> = full_scan_order
            .iter()
            .map(|&edge_id| {
                let edge = graph.edge(edge_id);
                let from = particles[edge.from.index()];
                let to = particles[edge.to.index()];
                let (x1, y1) = camera.world_to_screen((from.x as f64, from.y as f64), viewport);
                let (x2, y2) = camera.world_to_screen((to.x as f64, to.y as f64), viewport);
                (x1, y1, x2, y2)
            })
            .collect();
        let mut render = ColorOrderRecorder::default();

        assert_eq!(draw_edges(&mut render, &graph, &particles, &ctx), expected_segments.len());
        let rendered_segments: Vec<_> = render.line_segments[0]
            .iter()
            .map(|segment| (segment.x1, segment.y1, segment.x2, segment.y2))
            .collect();
        assert_eq!(rendered_segments, expected_segments);
    }

    #[test]
    fn visible_edge_candidates_use_bitmap_order_for_wave_sized_sparse_adjacency() {
        let mut graph = G::new();
        let hub = graph.push_node((), "hub", "x", 2.0);
        let noise_a = graph.push_node((), "noise-a", "x", 2.0);
        let noise_b = graph.push_node((), "noise-b", "x", 2.0);
        let spokes: Vec<_> = (0..300)
            .map(|index| graph.push_node((), format!("spoke-{index}"), "x", 2.0))
            .collect();
        let mut expected = Vec::with_capacity(spokes.len());
        for spoke in spokes {
            graph.push_edge(noise_a, noise_b, 1.0, ());
            expected.push(graph.push_edge(hub, spoke, 1.0, ()));
            graph.push_edge(noise_b, noise_a, 1.0, ());
        }

        let scan = visible_edge_candidates(&graph, &[hub]);

        assert!(scan.used_bitmap_order);
        assert!(!scan.used_full_scan);
        assert_eq!(scan.candidates_scanned, 300);
        assert_eq!(scan.order_slots_scanned, 900);
        assert_eq!(
            scan.edge_ids, expected,
            "bitmap extraction must retain exact ascending EdgeIndex paint order",
        );
    }

    #[test]
    fn draw_nodes_paints_color_batches_in_deterministic_lexicographic_order_across_repeated_calls() {
        let (graph, particles, _clusters, _cluster_id, a, b, c) = overlap_fixture();
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        // Only the 3 coincident, differently-categorized nodes — the exact
        // "two nodes of DIFFERENT categories overlap on screen" scenario
        // the fix's own doc comment names.
        let visible: Vec<NodeIndex> = vec![a, b, c];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);

        let mut first_order: Option<Vec<String>> = None;
        for _ in 0..5 {
            let mut render = ColorOrderRecorder::default();
            draw_nodes(&mut render, &graph, &particles, &ctx);
            assert_eq!(render.circle_batch_colors.len(), 3, "3 distinct categories must paint 3 separate batches");
            let mut sorted = render.circle_batch_colors.clone();
            sorted.sort();
            assert_eq!(render.circle_batch_colors, sorted, "the batch color sequence must already be lexicographically ordered (BTreeMap, not HashMap)");
            match &first_order {
                None => first_order = Some(render.circle_batch_colors),
                Some(expected) => assert_eq!(&render.circle_batch_colors, expected, "repeated calls over unchanged state must paint colors in the identical order every time"),
            }
        }
    }

    #[test]
    fn unstyled_elements_preserve_existing_2d_category_and_global_edge_defaults() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "cat-a", 5.0);
        let b = graph.push_node((), "b", "cat-b", 5.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at(-20.0, 0.0), Particle::at(20.0, 0.0)];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible = vec![a, b];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);
        let mut render = ColorOrderRecorder::default();

        draw_edges(&mut render, &graph, &particles, &ctx);
        draw_nodes(&mut render, &graph, &particles, &ctx);

        assert_eq!(render.line_batches.len(), 1);
        assert_eq!(render.line_batches[0].1, theme.edge_color);
        assert_eq!(render.line_batches[0].2, theme.edge_width);
        assert!(render.line_batches[0].3.is_empty());
        let expected_from = camera.world_to_screen((-20.0, 0.0), viewport);
        let expected_to = camera.world_to_screen((20.0, 0.0), viewport);
        let rendered = render.line_segments[0][0];
        assert_eq!((rendered.x1, rendered.y1), expected_from, "the default style must not displace the from endpoint");
        assert_eq!((rendered.x2, rendered.y2), expected_to, "the default style must not displace the to endpoint");
        assert!(render.circle_batch_colors.contains(&category_color("cat-a", &theme.category_palette)));
        assert!(render.circle_batch_colors.contains(&category_color("cat-b", &theme.category_palette)));
    }

    #[test]
    fn draw_nodes_applies_per_node_fill_alpha_outline_and_double_ring_marker() {
        let mut graph: Graph<(), ()> = Graph::new();
        let node = graph.push_node((), "styled", "category", 5.0);
        graph.set_node_style(node, Some(NodeVisualStyle {
            fill: Some("#112233".into()),
            outline: Some("#fedcba".into()),
            alpha: Some(0.4),
            outline_width: Some(2.5),
            marker: Some(NodeMarker::DoubleRing),
        }));
        let particles = vec![Particle::at(0.0, 0.0)];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible = vec![node];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);
        let mut render = ColorOrderRecorder::default();

        draw_nodes(&mut render, &graph, &particles, &ctx);

        assert!(render.circle_batch_colors.contains(&"#112233".to_owned()));
        assert!(render.alpha_changes.iter().any(|alpha| (*alpha - 0.4).abs() < 1e-6));
        assert_eq!(render.stroke_colors.iter().filter(|color| color.as_str() == "#fedcba").count(), 1);
    }

    #[test]
    fn draw_edges_keeps_dashed_directed_bodies_and_solid_arrowheads_geometrically_distinct() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 2.0);
        let b = graph.push_node((), "b", "x", 2.0);
        let edge = graph.push_edge(a, b, 1.0, ());
        graph.set_edge_style(edge, Some(EdgeVisualStyle {
            tint: Some("#abcdef".into()),
            alpha: Some(0.35),
            width: Some(4.0),
            dash: Some(crate::style::DashPattern::Pattern(vec![6.0, 2.0])),
            lateral_offset: None,
            directed: true,
        }));
        let particles = vec![Particle::at(-20.0, 0.0), Particle::at(20.0, 0.0)];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible = vec![a, b];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);
        let mut render = ColorOrderRecorder::default();

        assert_eq!(draw_edges(&mut render, &graph, &particles, &ctx), 1);
        assert_eq!(render.line_batches.len(), 2, "main dashed segment plus one two-wing arrowhead batch");
        assert_eq!(render.line_batches[0].0, 1);
        assert_eq!(render.line_batches[0].1, "#abcdef");
        assert_eq!(render.line_batches[0].2, 4.0);
        assert_eq!(render.line_batches[0].3, vec![6.0, 2.0]);
        assert!((render.line_batches[0].4 - 0.35).abs() < 1e-6);
        assert_eq!(render.line_batches[1].0, 2);
        assert!(render.line_batches[1].3.is_empty(), "arrowhead wings stay solid");
        let body = render.line_segments[0][0];
        let arrow_left = render.line_segments[1][0];
        let arrow_right = render.line_segments[1][1];
        assert_ne!(
            (body.x1, body.y1, body.x2, body.y2),
            (arrow_left.x1, arrow_left.y1, arrow_left.x2, arrow_left.y2),
            "the arrowhead must retain its own geometry instead of duplicating the body",
        );
        assert_ne!(
            (arrow_left.x1, arrow_left.y1, arrow_left.x2, arrow_left.y2),
            (arrow_right.x1, arrow_right.y1, arrow_right.x2, arrow_right.y2),
            "the two arrowhead wings must remain distinct",
        );
        assert_eq!((arrow_left.x1, arrow_left.y1), (arrow_right.x1, arrow_right.y1), "both solid wings must share one computed tip");
    }

    #[test]
    fn draw_edges_batches_many_same_style_bodies_and_arrowheads_into_two_backend_calls() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 2.0);
        let b = graph.push_node((), "b", "x", 2.0);
        for _ in 0..64 {
            let edge = graph.push_edge(a, b, 1.0, ());
            graph.set_edge_style(edge, Some(EdgeVisualStyle {
                tint: Some("#55ccaa".into()),
                alpha: Some(0.7),
                width: Some(2.5),
                dash: Some(crate::style::DashPattern::Pattern(vec![5.0, 3.0])),
                lateral_offset: None,
                directed: true,
            }));
        }
        let particles = vec![Particle::at(-20.0, 0.0), Particle::at(20.0, 0.0)];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible = vec![a, b];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);
        let mut render = ColorOrderRecorder::default();

        assert_eq!(draw_edges(&mut render, &graph, &particles, &ctx), 64);
        assert_eq!(render.line_batches.len(), 2, "one body batch plus one solid arrowhead batch must replace 128 per-edge calls");
        assert_eq!(render.line_batches[0].0, 64);
        assert_eq!(render.line_batches[0].3, vec![5.0, 3.0]);
        assert_eq!(render.line_batches[1].0, 128);
        assert!(render.line_batches[1].3.is_empty());
    }

    #[test]
    fn draw_edges_offsets_parallel_segments_and_arrowheads_in_opposite_screen_directions() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 2.0);
        let b = graph.push_node((), "b", "x", 2.0);
        let positive = graph.push_edge(a, b, 1.0, ());
        let negative = graph.push_edge(a, b, 1.0, ());
        graph.set_edge_style(positive, Some(EdgeVisualStyle {
            lateral_offset: Some(6.0),
            directed: true,
            ..EdgeVisualStyle::default()
        }));
        graph.set_edge_style(negative, Some(EdgeVisualStyle {
            lateral_offset: Some(-6.0),
            directed: true,
            ..EdgeVisualStyle::default()
        }));
        let particles = vec![Particle::at(-20.0, 0.0), Particle::at(20.0, 0.0)];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible = vec![a, b];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden = HashSet::new();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);
        let mut render = ColorOrderRecorder::default();

        assert_eq!(draw_edges(&mut render, &graph, &particles, &ctx), 2);

        assert_eq!(render.line_segments.len(), 2, "same-style bodies and arrowheads collapse into one batch per geometry layer");
        let positive_segment = render.line_segments[0][0];
        let negative_segment = render.line_segments[0][1];
        let positive_arrow = render.line_segments[1][0];
        let negative_arrow = render.line_segments[1][2];
        assert!((positive_segment.y1 - negative_segment.y1 - 12.0).abs() < 1e-6);
        assert!((positive_segment.y2 - negative_segment.y2 - 12.0).abs() < 1e-6);
        assert!((positive_arrow.y1 - negative_arrow.y1 - 12.0).abs() < 1e-6, "arrowhead tip must move with its styled segment");
        assert_eq!(positive_segment.x1, negative_segment.x1);
        assert_eq!(positive_segment.x2, negative_segment.x2);
    }

    /// Graph-strengthening arc G1.4: `draw_nodes` must honor `ctx.hidden`
    /// directly, not merely rely on the caller having pre-filtered
    /// `ctx.visible` — a hidden node present in `visible` (the footgun
    /// shape the audit named) must draw neither a circle nor a label.
    #[test]
    fn draw_nodes_skips_a_node_present_in_visible_but_also_in_hidden() {
        let (graph, particles, _clusters, _cluster_id, a, b, _c) = overlap_fixture();
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        // `b` is in `visible` (the caller "forgot" to pre-filter it) AND
        // in `hidden` — `draw_nodes` must still skip it.
        let visible: Vec<NodeIndex> = vec![a, b];
        let focus = FocusSet::empty();
        let selection = BTreeSet::new();
        let hidden: HashSet<NodeIndex> = [b].into_iter().collect();
        let forced = HashSet::new();
        let lod = LabelLodConfig::default();
        let theme = GraphTheme::dark();
        let ctx = draw_ctx_for(&camera, viewport, &visible, &focus, &selection, None, &hidden, &forced, &lod, &theme);

        let mut render = ColorOrderRecorder::default();
        let stats = draw_nodes(&mut render, &graph, &particles, &ctx);

        assert_eq!(stats.nodes_drawn, 1, "the hidden node must not be counted as drawn");
        assert_eq!(render.circle_batch_colors.len(), 1, "the hidden node's category batch must never be issued");
    }

    // ── Graph-strengthening arc G2: `category_color`/`cull_visible`
    // configurability + theme-preset render smoke tests ─────────────────

    /// `category_color` against the crate's own default palette must
    /// reproduce the pre-Wave-G2 hardcoded hash -> palette-index mapping
    /// exactly (same FNV-1a hash, same 10-color set, same modulo).
    #[test]
    fn category_color_against_the_default_palette_matches_the_pre_existing_hash_mapping() {
        for category in ["alpha", "beta", "gamma", "delta", ""] {
            assert_eq!(category_color(category, &default_category_palette()), category_color_default(category));
        }
    }

    /// A caller-supplied categorical palette must actually be honoured —
    /// `category_color` must NOT silently fall back to the default
    /// palette when given a real, non-empty caller palette.
    #[test]
    fn category_color_honours_a_caller_supplied_palette() {
        let custom = CategoricalScale::new(vec!["#111111".to_owned(), "#222222".to_owned()]);
        for category in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"] {
            let got = category_color(category, &custom);
            assert!(got == "#111111" || got == "#222222", "category '{category}' resolved to {got}, outside the caller's 2-color palette");
        }
    }

    /// The Okabe-Ito colorblind-safe palette must be selectable through
    /// the exact same `category_color` entry point a caller already uses
    /// for the default palette.
    #[test]
    fn category_color_can_select_the_okabe_ito_palette() {
        let okabe_ito = CategoricalScale::default_palette();
        // Every value `category_color` can possibly return against this
        // palette must be one of okabe-ito's own 8 entries — never a
        // leftover default-palette color.
        for category in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa"] {
            let got = category_color(category, &okabe_ito);
            assert!((0..8).any(|i| okabe_ito.color_for(i) == got), "category '{category}' resolved to {got}, not one of okabe-ito's own 8 entries");
        }
    }

    /// `cull_visible`'s margin must be a real, caller-observable knob — a
    /// node just outside the viewport but within a widened margin must be
    /// culled at the default margin and NOT culled at a wider one.
    #[test]
    fn cull_visible_margin_is_a_real_override() {
        let mut graph = G::new();
        let far_node = graph.push_node((), "far", "x", 4.0);
        let mut particles = vec![Particle::default(); graph.node_count()];
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        // 100 world units past the right edge — outside the default 64.0
        // margin, inside a widened 200.0 margin.
        particles[far_node.index()] = Particle::at(900.0, 300.0);

        let default_visible = cull_visible(&graph, &particles, &camera, viewport, 64.0);
        let widened_visible = cull_visible(&graph, &particles, &camera, viewport, 200.0);
        assert!(!default_visible.contains(&far_node), "at the default margin this node must be culled");
        assert!(widened_visible.contains(&far_node), "at a widened caller-supplied margin this node must survive culling");
    }

    /// Every [`GraphTheme`] preset must render a full scene (edges, nodes,
    /// selection ring, hover ring, labels, cluster overlay, hover card,
    /// box-select rect) without panicking — the deliverable's own basic
    /// "does this actually work" gate, per each preset.
    #[test]
    fn every_theme_preset_renders_a_full_scene_without_panicking() {
        for theme in [GraphTheme::dark(), GraphTheme::light(), GraphTheme::high_contrast()] {
            let (graph, particles, clusters, _cluster_id, a, b, _c) = overlap_fixture();
            let camera = Camera2D::default();
            let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
            let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
            let focus = FocusSet::empty();
            let mut selection = BTreeSet::new();
            selection.insert(a);
            let hidden: HashSet<NodeIndex> = HashSet::new();
            let forced: HashSet<NodeIndex> = HashSet::new();
            let lod = LabelLodConfig::default();
            let ctx = DrawContext {
                camera: &camera,
                viewport,
                visible: &visible,
                focus: &focus,
                selection: &selection,
                hovered: Some(b),
                hidden: &hidden,
                label_density: 5.0,
                label_halo: "#0d0f14",
                forced_labels: &forced,
                label_lod: &lod,
                theme: &theme,
            };

            let mut render = ColorOrderRecorder::default();
            draw_edges(&mut render, &graph, &particles, &ctx);
            draw_nodes(&mut render, &graph, &particles, &ctx);
            draw_cluster_edges(&mut render, &particles, &ctx, &clusters);
            draw_cluster_supernodes(&mut render, &graph, &particles, &ctx, &clusters);
            let info = HoverCardInfo { label: "b", category: "blue", degree: 1, pinned: false };
            draw_hover_card(&mut render, (400.0, 300.0), &info, viewport, &theme.hover_card);
            draw_box_select_rect(&mut render, Rect::new(10.0, 10.0, 50.0, 50.0), &theme);
        }
    }

    /// [`GraphTheme::light`]'s own selection ring must actually paint a
    /// different color from [`GraphTheme::dark`]'s — proves a theme swap
    /// changes what `draw_nodes` actually issues to the renderer, not
    /// just that the struct fields differ.
    #[test]
    fn swapping_the_theme_changes_the_selection_ring_color_draw_nodes_issues() {
        let (graph, particles, _clusters, _cluster_id, a, _b, _c) = overlap_fixture();
        let camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let visible: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        let focus = FocusSet::empty();
        let mut selection = BTreeSet::new();
        selection.insert(a);
        let hidden: HashSet<NodeIndex> = HashSet::new();
        let forced: HashSet<NodeIndex> = HashSet::new();
        let lod = LabelLodConfig::default();

        // `draw_nodes` is bracketed in `save()`/`restore()` (G1.3) — the
        // final `render.state.stroke_color` is deliberately restored back
        // to whatever the caller had BEFORE the call, so it can't observe
        // what was painted mid-function. `ColorOrderRecorder::stroke_colors`
        // records every `set_stroke_color` call as it happens instead.
        let stroke_color_for = |theme: &GraphTheme| {
            let ctx = DrawContext {
                camera: &camera,
                viewport,
                visible: &visible,
                focus: &focus,
                selection: &selection,
                hovered: None,
                hidden: &hidden,
                label_density: 0.0,
                label_halo: "#0d0f14",
                forced_labels: &forced,
                label_lod: &lod,
                theme,
            };
            let mut render = ColorOrderRecorder::default();
            draw_nodes(&mut render, &graph, &particles, &ctx);
            render.stroke_colors.first().cloned().unwrap_or_default()
        };

        let dark_ring = stroke_color_for(&GraphTheme::dark());
        let light_ring = stroke_color_for(&GraphTheme::light());
        assert_eq!(dark_ring, "#ffffff");
        assert_eq!(light_ring, "#1a1a2e");
        assert_ne!(dark_ring, light_ring, "a caller-supplied theme swap must actually change what draw_nodes paints");
    }
}
