//! `SankeyFigure` — a staged flow diagram: weight-proportional ribbons
//! carrying value between nodes arranged in explicit `stage` columns.
//! Report figure #3 (Sankey / tiered flow, generalizing the case's
//! hand-drawn `money_flow.png`) — no case-specific vocabulary lives here,
//! only `stage`/`kind`/`label` the caller assigns meaning to (same idiom
//! [`crate::figure::TimelineFigure`] already follows for `lane`/`kind`).
//!
//! Phase C scope: classic layered Sankey over CALLER-SUPPLIED `stage`
//! columns — no automatic layering, no cycle-breaking. A caller that wants
//! those (e.g. deriving `stage` from a DAG's topological depth) does that
//! upstream; this figure only lays out and draws what it's given.
//!
//! [`layout_sankey`] is a pure, testable function separate from painting
//! (design law #1): it returns [`SankeyLayout`] (node rects + ribbon
//! geometry), and [`hit_test_node`] hit-tests through that SAME geometry —
//! `render_with`'s hover routing and a caller's own hover routing (if it
//! drives one externally) can never disagree.

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::figure::FigureOverlay;
use crate::guide::tooltip;
use crate::mark::text::{draw_label_left_aligned, draw_label_right_aligned};
use crate::scale::CategoricalScale;
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 12.0;
const MARGIN_RIGHT: f64 = 12.0;
const MARGIN_TOP: f64 = 8.0;
const MARGIN_BOTTOM: f64 = 8.0;
const TITLE_HEIGHT: f64 = 24.0;
/// Fixed node width (px) — within the task's ~12-16px band.
const NODE_WIDTH: f64 = 14.0;
/// Vertical gap (px) between two nodes stacked in the same column.
const NODE_GAP_PX: f64 = 6.0;
const NODE_CORNER_RADIUS: f64 = 3.0;
/// A node with zero measured in/out weight (an isolated node, or a column
/// where every node happens to have zero weight) still gets a visible
/// sliver — floored to this fraction of the figure's largest node value
/// (or `1.0` flat when EVERY node is zero-weight, so an all-isolated
/// fixture still lays out something rather than collapsing to zero height
/// everywhere).
const MIN_NODE_VALUE_FRACTION: f64 = 0.02;
const LABEL_GAP: f64 = 6.0;
const RIBBON_ALPHA: f64 = 0.5;
const RIBBON_HOVER_ALPHA: f64 = 0.85;
const RIBBON_DIM_ALPHA: f64 = 0.12;
const NODE_DIM_ALPHA: f64 = 0.3;
const HOVER_STROKE_WIDTH: f64 = 1.5;
const SELECTED_STROKE_WIDTH: f64 = 2.0;

/// One node: `stage` is an explicit column index — see the module docs
/// for why this figure doesn't derive it automatically.
#[derive(Debug, Clone)]
pub struct SankeyNode {
    pub id: String,
    pub label: String,
    pub stage: usize,
}

/// One weighted edge between two [`SankeyNode`] indices. `kind` indexes
/// [`FigureTheme::palette`] for this link's ribbon color, same convention
/// as [`crate::figure::TimelineEvent::kind`].
///
/// Conservation (sum of a node's in-weights equalling its out-weights) is
/// NOT enforced or assumed anywhere in this figure — real forensic flows
/// leak, and a node's ribbons on one side may not fully cover its own
/// height when its in/out totals differ (see [`layout_sankey`]'s docs).
#[derive(Debug, Clone)]
pub struct SankeyLink {
    pub from: usize,
    pub to: usize,
    pub weight: f64,
    pub kind: usize,
}

/// Which side of its own node a node's label is drawn on — see
/// [`node_label_side`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSide {
    Left,
    Right,
}

/// One rendered ribbon's screen-pixel geometry — a cubic-bezier band from
/// the source node's right edge to the target node's left edge.
/// `link_index` indexes back into whatever `links` slice [`layout_sankey`]
/// was called with (e.g. [`SankeyFigure::links`]) — geometry is kept
/// separate from the link's own data (weight/kind) so this struct stays
/// pure position/size, painting reads the rest back through the index.
#[derive(Debug, Clone, Copy)]
pub struct RibbonGeom {
    pub link_index: usize,
    pub src_x: f64,
    pub src_y0: f64,
    pub src_y1: f64,
    pub dst_x: f64,
    pub dst_y0: f64,
    pub dst_y1: f64,
}

impl RibbonGeom {
    /// Ribbon thickness (px) at the source node's edge.
    pub fn src_thickness(&self) -> f64 {
        (self.src_y1 - self.src_y0).abs()
    }

    /// Ribbon thickness (px) at the target node's edge.
    pub fn dst_thickness(&self) -> f64 {
        (self.dst_y1 - self.dst_y0).abs()
    }
}

/// Pure layout geometry for a [`SankeyFigure`] — separate from painting
/// (design law #1) so a hover hit-test ([`hit_test_node`]) always agrees
/// pixel-for-pixel with what got drawn.
#[derive(Debug, Clone)]
pub struct SankeyLayout {
    /// One rect per input node, same index as the `nodes` slice passed to
    /// [`layout_sankey`]. A node with zero measured weight in an
    /// all-zero-weight input still gets a non-degenerate rect (see
    /// [`MIN_NODE_VALUE_FRACTION`]); the ONLY way to get a zero-size rect
    /// here is an empty `nodes` slice.
    pub node_rects: Vec<Rect>,
    /// One [`LabelSide`] per input node, same index as `node_rects`.
    pub label_sides: Vec<LabelSide>,
    /// One [`RibbonGeom`] per link with valid `from`/`to` indices — links
    /// referencing an out-of-range node index are silently dropped (a
    /// caller data bug, not a panic).
    pub ribbons: Vec<RibbonGeom>,
}

/// Guard a link weight against NaN/negative input. A caller passing a
/// malformed weight is a genuine data bug — loudly caught via
/// `debug_assert` in dev builds — but in release this degrades to an
/// invisible (`0.0`-thickness) ribbon rather than corrupting layout or
/// panicking; real forensic flows are allowed to be incomplete, but a
/// weight is never allowed to go negative or NaN.
fn clamped_weight(w: f64) -> f64 {
    debug_assert!(w.is_finite() && w >= 0.0, "SankeyLink weight must be finite and non-negative, got {w}");
    if w.is_finite() && w > 0.0 {
        w
    } else {
        0.0
    }
}

/// Classic layered Sankey layout over EXPLICIT `node.stage` columns — no
/// automatic layering or cycle-breaking (Phase C scope, see the module
/// docs).
///
/// Node height ∝ `max(in-weight-sum, out-weight-sum)`. Every column shares
/// ONE pixels-per-weight-unit scale (`ppu`), chosen so the column with the
/// most stacked content exactly fills `rect`'s height — other columns'
/// (shorter) stacks are centered vertically within it. Ribbon thickness at
/// BOTH its source and target edge uses this SAME `ppu`, so a ribbon's
/// thickness is directly comparable to node height everywhere in the
/// figure, not merely proportional within one node's own edge.
///
/// A node's outgoing links stack top-to-bottom on its right edge in
/// TARGET-row order (sorted by the target node's own vertical position);
/// its incoming links stack on its left edge in SOURCE-row order —
/// keeping each node's own edge crossing-free even though the figure does
/// no whole-diagram crossing minimization.
pub fn layout_sankey(nodes: &[SankeyNode], links: &[SankeyLink], rect: Rect) -> SankeyLayout {
    if nodes.is_empty() {
        return SankeyLayout { node_rects: Vec::new(), label_sides: Vec::new(), ribbons: Vec::new() };
    }

    // Only links whose `from`/`to` are real node indices participate —
    // malformed input degrades to "this link doesn't exist" rather than
    // an out-of-bounds panic anywhere below.
    let valid_links: Vec<(usize, &SankeyLink)> =
        links.iter().enumerate().filter(|(_, l)| l.from < nodes.len() && l.to < nodes.len()).collect();

    let num_columns = nodes.iter().map(|n| n.stage).max().unwrap_or(0) + 1;
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); num_columns];
    for (i, n) in nodes.iter().enumerate() {
        columns[n.stage].push(i);
    }

    let mut in_sum = vec![0.0_f64; nodes.len()];
    let mut out_sum = vec![0.0_f64; nodes.len()];
    for &(_, link) in &valid_links {
        let w = clamped_weight(link.weight);
        out_sum[link.from] += w;
        in_sum[link.to] += w;
    }
    let raw_values: Vec<f64> = (0..nodes.len()).map(|i| in_sum[i].max(out_sum[i])).collect();
    let global_max = raw_values.iter().copied().fold(0.0_f64, f64::max);
    let value_floor = if global_max > 0.0 { global_max * MIN_NODE_VALUE_FRACTION } else { 1.0 };
    let layout_values: Vec<f64> = raw_values.iter().map(|&v| v.max(value_floor)).collect();

    // The tightest column (least available px-per-unit) sets the ONE
    // scale every column and every ribbon draws through.
    let available_h = rect.height.max(0.0);
    let mut ppu = f64::INFINITY;
    for col in &columns {
        if col.is_empty() {
            continue;
        }
        let total: f64 = col.iter().map(|&i| layout_values[i]).sum();
        if total <= 0.0 {
            continue;
        }
        let gap_h = NODE_GAP_PX * (col.len() - 1) as f64;
        let usable = (available_h - gap_h).max(1.0);
        ppu = ppu.min(usable / total);
    }
    if !ppu.is_finite() {
        ppu = 1.0; // defensive only — every non-empty column carries a floored value > 0 above
    }

    let column_x = |c: usize| -> f64 {
        if num_columns <= 1 {
            rect.x
        } else {
            rect.x + c as f64 * (rect.width - NODE_WIDTH).max(0.0) / (num_columns - 1) as f64
        }
    };

    // Stack each column's nodes top-down (input order within the column),
    // centered vertically within the available height.
    let mut node_rects = vec![Rect::default(); nodes.len()];
    for (c, col) in columns.iter().enumerate() {
        if col.is_empty() {
            continue;
        }
        let total: f64 = col.iter().map(|&i| layout_values[i]).sum();
        let gap_h = NODE_GAP_PX * (col.len() - 1) as f64;
        let content_h = ppu * total + gap_h;
        let mut y = rect.y + ((available_h - content_h) / 2.0).max(0.0);
        let x = column_x(c);
        for &i in col {
            let h = (ppu * layout_values[i]).max(0.0);
            node_rects[i] = Rect::new(x, y, NODE_WIDTH, h);
            y += h + NODE_GAP_PX;
        }
    }

    let label_sides: Vec<LabelSide> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| node_label_side(i, n.stage, num_columns, &node_rects, &columns))
        .collect();

    // Per-node stacking order: outgoing links sorted by their target's
    // row position, incoming links sorted by their source's row position.
    let mut out_order: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut in_order: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for &(li, link) in &valid_links {
        out_order[link.from].push(li);
        in_order[link.to].push(li);
    }
    for order in out_order.iter_mut() {
        order.sort_by(|&a, &b| {
            node_rects[links[a].to].y.partial_cmp(&node_rects[links[b].to].y).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
        });
    }
    for order in in_order.iter_mut() {
        order.sort_by(|&a, &b| {
            node_rects[links[a].from].y.partial_cmp(&node_rects[links[b].from].y).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
        });
    }

    let mut src_range = vec![(0.0_f64, 0.0_f64); links.len()];
    let mut dst_range = vec![(0.0_f64, 0.0_f64); links.len()];
    for (node_idx, order) in out_order.iter().enumerate() {
        let mut offset = 0.0_f64;
        for &li in order {
            let thickness = ppu * clamped_weight(links[li].weight);
            let y0 = node_rects[node_idx].y + offset;
            src_range[li] = (y0, y0 + thickness);
            offset += thickness;
        }
    }
    for (node_idx, order) in in_order.iter().enumerate() {
        let mut offset = 0.0_f64;
        for &li in order {
            let thickness = ppu * clamped_weight(links[li].weight);
            let y0 = node_rects[node_idx].y + offset;
            dst_range[li] = (y0, y0 + thickness);
            offset += thickness;
        }
    }

    let ribbons: Vec<RibbonGeom> = valid_links
        .iter()
        .map(|&(li, link)| {
            let (src_y0, src_y1) = src_range[li];
            let (dst_y0, dst_y1) = dst_range[li];
            RibbonGeom {
                link_index: li,
                src_x: node_rects[link.from].right(),
                src_y0,
                src_y1,
                dst_x: node_rects[link.to].x,
                dst_y0,
                dst_y1,
            }
        })
        .collect();

    SankeyLayout { node_rects, label_sides, ribbons }
}

/// Deterministic label-side rule, purely a function of column geometry
/// (no text measurement — a caller's paint pass measures the string
/// itself if it wants to react further):
/// - First column (`stage == 0`): always [`LabelSide::Right`] (verbatim
///   per the figure's brief).
/// - Last column: always [`LabelSide::Left`].
/// - A middle column picks whichever NEIGHBOR column has no node whose
///   vertical span overlaps this node's own — i.e. "whichever side has
///   room" measured as "no other node rect sits behind the label" —
///   defaulting to [`LabelSide::Right`] when both neighbors are crowded.
fn node_label_side(node_idx: usize, stage: usize, num_columns: usize, node_rects: &[Rect], columns: &[Vec<usize>]) -> LabelSide {
    if num_columns <= 1 || stage == 0 {
        return LabelSide::Right;
    }
    if stage == num_columns - 1 {
        return LabelSide::Left;
    }
    let own = node_rects[node_idx];
    let overlaps_column = |col: &[usize]| {
        col.iter().any(|&j| {
            let r = node_rects[j];
            r.y < own.bottom() && r.bottom() > own.y
        })
    };
    if !overlaps_column(&columns[stage + 1]) {
        LabelSide::Right
    } else if !overlaps_column(&columns[stage - 1]) {
        LabelSide::Left
    } else {
        LabelSide::Right
    }
}

/// Index of the node whose rect contains `(px, py)` — hit-tests through
/// the EXACT same `node_rects` geometry [`layout_sankey`] just produced
/// (design law #1), so a caller's hover routing can never disagree with
/// what got drawn. `None` when no node rect contains the point.
pub fn hit_test_node(layout: &SankeyLayout, px: f64, py: f64) -> Option<usize> {
    layout.node_rects.iter().position(|r| r.contains(px, py))
}

/// `palette.color_for(kind)` when an explicit [`CategoricalScale`] override
/// is given; otherwise `theme.palette[kind % palette.len()]`, guarded
/// against a degenerate empty custom theme — same convention as
/// [`crate::figure::timeline::palette_color`]. This figure's own
/// `kind`-indexed ribbon/accent color resolver — see
/// [`SankeyFigure::with_category_palette`]'s own doc comment for why
/// `kind` (not node identity) is this figure's real "categorical" axis.
fn palette_color<'a>(theme: &'a FigureTheme, palette: Option<&'a CategoricalScale>, kind: usize) -> &'a str {
    if let Some(p) = palette {
        return p.color_for(kind);
    }
    if theme.palette.is_empty() {
        return &theme.axis_color;
    }
    &theme.palette[kind % theme.palette.len()]
}

/// A staged flow diagram: [`SankeyNode`]s arranged in explicit `stage`
/// columns, connected by weight-proportional [`SankeyLink`] ribbons.
pub struct SankeyFigure {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
    pub title: String,
    /// This figure's own per-[`SankeyLink::kind`] ribbon color source —
    /// see [`SankeyFigure::with_category_palette`]. Default (unset)
    /// reproduces this figure's pre-existing `theme.palette[kind %
    /// theme.palette.len()]` ribbon-color indexing byte-for-byte.
    category_palette: Option<CategoricalScale>,
}

impl SankeyFigure {
    pub fn new(nodes: Vec<SankeyNode>, links: Vec<SankeyLink>) -> Self {
        Self { nodes, links, title: String::new(), category_palette: None }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Override this figure's per-[`SankeyLink::kind`] ribbon color source
    /// — see [`crate::scale::CategoricalScale`]'s own doc comment. Default
    /// (unset) is `None`, byte-identical to this figure's pre-existing
    /// `theme.palette[kind % theme.palette.len()]` indexing. This figure's
    /// NODES are always drawn in the theme's neutral `label_color` (never
    /// category-colored, by this figure's own pre-existing design); `kind`
    /// (the ribbon/link classification) is this figure's real
    /// "categorical" axis, so that's what this builder overrides.
    pub fn with_category_palette(mut self, palette: CategoricalScale) -> Self {
        self.category_palette = Some(palette);
        self
    }

    fn plot_rect(&self, rect: Rect) -> Rect {
        let title_h = if self.title.is_empty() { 0.0 } else { TITLE_HEIGHT };
        Rect::new(
            rect.x + MARGIN_LEFT,
            rect.y + title_h + MARGIN_TOP,
            (rect.width - MARGIN_LEFT - MARGIN_RIGHT).max(0.0),
            (rect.height - title_h - MARGIN_TOP - MARGIN_BOTTOM).max(0.0),
        )
    }

    /// This figure's plot rect for `rect` — exposed for the same reason as
    /// every other figure's `plot_area`/`plot_rect` accessor: a caller
    /// driving hover routing from outside needs the EXACT rect
    /// [`SankeyFigure::layout`] was computed against.
    pub fn plot_rect_for(&self, rect: Rect) -> Rect {
        self.plot_rect(rect)
    }

    /// This figure's own layout geometry for `rect` — exposed for the same
    /// reason as [`SankeyFigure::plot_rect_for`].
    pub fn layout(&self, rect: Rect) -> SankeyLayout {
        layout_sankey(&self.nodes, &self.links, self.plot_rect(rect))
    }

    /// Total in-weight and out-weight for node `idx` — `(0.0, 0.0)` for an
    /// out-of-range index (never panics).
    fn node_totals(&self, idx: usize) -> (f64, f64) {
        let mut in_total = 0.0;
        let mut out_total = 0.0;
        for link in &self.links {
            if link.to == idx {
                in_total += clamped_weight(link.weight);
            }
            if link.from == idx {
                out_total += clamped_weight(link.weight);
            }
        }
        (in_total, out_total)
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a node
    /// highlights that node + every ribbon touching it (dimming
    /// everything else) and shows a label/in/out tooltip;
    /// `overlay.focus`-selected nodes (keyed by index into
    /// [`SankeyFigure::nodes`]) get a persistent accent outline, the same
    /// convention [`crate::figure::BarFigure`]/[`crate::figure::TimelineFigure`]
    /// use keyed by their own row index. `overlay.brush` is not consumed
    /// by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let plot_rect = self.plot_rect(rect);
        let layout = layout_sankey(&self.nodes, &self.links, plot_rect);

        let hovered = overlay.hover_px.and_then(|(hx, hy)| hit_test_node(&layout, hx, hy));
        let connected = |i: usize| -> bool {
            match hovered {
                None => true,
                Some(h) => i == h || self.links.iter().any(|l| (l.from == h && l.to == i) || (l.to == h && l.from == i)),
            }
        };

        self.draw_ribbons(ctx, &layout, theme, hovered);
        self.draw_nodes(ctx, &layout, theme, overlay, hovered, &connected);
        self.draw_labels(ctx, &layout, theme, &connected);

        if let (Some(h), Some((hx, hy))) = (hovered, overlay.hover_px) {
            let (in_total, out_total) = self.node_totals(h);
            let label = self.nodes.get(h).map(|n| n.label.clone()).unwrap_or_default();
            let lines = vec![
                ("node".to_owned(), label),
                ("in".to_owned(), format!("{in_total:.2}")),
                ("out".to_owned(), format!("{out_total:.2}")),
            ];
            tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, plot_rect);
        }

        if !self.title.is_empty() {
            crate::figure::draw_title(ctx, rect, &self.title, theme);
        }
    }

    fn draw_ribbons(&self, ctx: &mut dyn RenderContext, layout: &SankeyLayout, theme: &FigureTheme, hovered: Option<usize>) {
        for ribbon in &layout.ribbons {
            let Some(link) = self.links.get(ribbon.link_index) else { continue };
            if ribbon.src_thickness() <= 0.0 && ribbon.dst_thickness() <= 0.0 {
                continue;
            }
            let touches_hovered = hovered.is_some_and(|h| link.from == h || link.to == h);
            let alpha = match hovered {
                None => RIBBON_ALPHA,
                Some(_) if touches_hovered => RIBBON_HOVER_ALPHA,
                Some(_) => RIBBON_DIM_ALPHA,
            };
            ctx.set_fill_color(palette_color(theme, self.category_palette.as_ref(), link.kind));
            ctx.set_global_alpha(alpha);
            draw_ribbon_path(ctx, ribbon);
            ctx.fill();
        }
        ctx.set_global_alpha(1.0);
    }

    fn draw_nodes(
        &self,
        ctx: &mut dyn RenderContext,
        layout: &SankeyLayout,
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
            ctx.set_fill_color(&theme.label_color);
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
                    ctx.set_stroke_color(palette_color(theme, self.category_palette.as_ref(), 1));
                    ctx.set_stroke_width(SELECTED_STROKE_WIDTH);
                    ctx.stroke_rounded_rect(r.x, r.y, r.width, r.height, NODE_CORNER_RADIUS);
                }
            }
        }
    }

    fn draw_labels(&self, ctx: &mut dyn RenderContext, layout: &SankeyLayout, theme: &FigureTheme, connected: &dyn Fn(usize) -> bool) {
        ctx.set_font(&theme.label_font);
        for (i, node) in self.nodes.iter().enumerate() {
            if node.label.is_empty() {
                continue;
            }
            let Some(r) = layout.node_rects.get(i) else { continue };
            if r.width <= 0.0 {
                continue;
            }
            let cy = r.y + r.height / 2.0;
            let alpha = if connected(i) { 1.0 } else { NODE_DIM_ALPHA };
            ctx.set_global_alpha(alpha);
            match layout.label_sides.get(i) {
                Some(LabelSide::Right) => {
                    draw_label_left_aligned(ctx, &node.label, r.right() + LABEL_GAP, cy, &theme.label_color, &theme.label_font)
                }
                _ => draw_label_right_aligned(ctx, &node.label, r.x - LABEL_GAP, cy, &theme.label_color, &theme.label_font),
            }
            ctx.set_global_alpha(1.0);
        }
    }
}

/// Append (but don't fill/stroke) one ribbon's closed cubic-bezier path:
/// a top edge curving `src_y0 -> dst_y0`, a bottom edge curving
/// `dst_y1 -> src_y1`, straight verticals closing both ends — the classic
/// Sankey band shape. Control points sit at the horizontal midpoint
/// between the two edges (a symmetric S-curve), the usual Sankey-ribbon
/// idiom.
fn draw_ribbon_path(ctx: &mut dyn RenderContext, ribbon: &RibbonGeom) {
    let mid_x = (ribbon.src_x + ribbon.dst_x) / 2.0;
    ctx.begin_path();
    ctx.move_to(ribbon.src_x, ribbon.src_y0);
    ctx.bezier_curve_to(mid_x, ribbon.src_y0, mid_x, ribbon.dst_y0, ribbon.dst_x, ribbon.dst_y0);
    ctx.line_to(ribbon.dst_x, ribbon.dst_y1);
    ctx.bezier_curve_to(mid_x, ribbon.dst_y1, mid_x, ribbon.src_y1, ribbon.src_x, ribbon.src_y1);
    ctx.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, stage: usize) -> SankeyNode {
        SankeyNode { id: id.to_owned(), label: id.to_owned(), stage }
    }

    fn link(from: usize, to: usize, weight: f64, kind: usize) -> SankeyLink {
        SankeyLink { from, to, weight, kind }
    }

    // ── Node height proportional to weight (requirement 1) ────────────────

    #[test]
    fn node_heights_are_proportional_to_their_weight_within_one_column() {
        let nodes = vec![node("a", 0), node("b", 0), node("sink", 1)];
        // a carries weight 2, b carries weight 1 — both stack into column 0.
        let links = vec![link(0, 2, 2.0, 0), link(1, 2, 1.0, 0)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let layout = layout_sankey(&nodes, &links, rect);

        let ha = layout.node_rects[0].height;
        let hb = layout.node_rects[1].height;
        assert!(ha > 0.0 && hb > 0.0);
        assert!(((ha / hb) - 2.0).abs() < 1e-6, "heights must carry the 2:1 weight ratio, got {ha}/{hb}");
    }

    // ── Column x positions evenly spaced (requirement 1) ───────────────────

    #[test]
    fn column_x_positions_are_evenly_spaced() {
        let nodes = vec![node("a", 0), node("b", 1), node("c", 2), node("d", 3)];
        let links: Vec<SankeyLink> = Vec::new();
        let rect = Rect::new(10.0, 0.0, 300.0, 200.0);
        let layout = layout_sankey(&nodes, &links, rect);

        let xs: Vec<f64> = layout.node_rects.iter().map(|r| r.x).collect();
        let step_a = xs[1] - xs[0];
        let step_b = xs[2] - xs[1];
        let step_c = xs[3] - xs[2];
        assert!((step_a - step_b).abs() < 1e-9);
        assert!((step_b - step_c).abs() < 1e-9);
    }

    // ── Ribbon end thickness matches weight share (requirement 1) ─────────

    #[test]
    fn ribbon_thickness_matches_link_weight_at_both_ends_for_a_single_link() {
        let nodes = vec![node("a", 0), node("b", 1)];
        let links = vec![link(0, 1, 4.0, 0)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let layout = layout_sankey(&nodes, &links, rect);

        assert_eq!(layout.ribbons.len(), 1);
        let ribbon = &layout.ribbons[0];
        // A single link fully occupies both its source's and target's
        // edge — with only one link, node height IS that link's weight
        // in pixels, at both ends.
        let node_h = layout.node_rects[0].height;
        assert!((ribbon.src_thickness() - node_h).abs() < 1e-6);
        assert!((ribbon.dst_thickness() - node_h).abs() < 1e-6);
    }

    #[test]
    fn ribbon_thickness_share_is_proportional_to_weight_when_a_node_fans_out() {
        let nodes = vec![node("a", 0), node("b", 1), node("c", 1)];
        // a fans out 2:1 into b and c.
        let links = vec![link(0, 1, 2.0, 0), link(0, 2, 1.0, 0)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let layout = layout_sankey(&nodes, &links, rect);

        let t0 = layout.ribbons[0].src_thickness();
        let t1 = layout.ribbons[1].src_thickness();
        assert!(t0 > 0.0 && t1 > 0.0);
        assert!(((t0 / t1) - 2.0).abs() < 1e-6, "fan-out thickness must carry the 2:1 weight ratio, got {t0}/{t1}");
    }

    // ── Stacked ribbons within a node abut, never overlap (requirement 1) ─

    #[test]
    fn stacked_outgoing_ribbons_abut_without_overlapping() {
        let nodes = vec![node("a", 0), node("b", 1), node("c", 1), node("d", 1)];
        let links = vec![link(0, 1, 3.0, 0), link(0, 2, 5.0, 0), link(0, 3, 2.0, 0)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let layout = layout_sankey(&nodes, &links, rect);

        let mut segments: Vec<(f64, f64)> = layout.ribbons.iter().map(|r| (r.src_y0, r.src_y1)).collect();
        segments.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let node_top = layout.node_rects[0].y;
        let node_bottom = layout.node_rects[0].bottom();
        assert!((segments[0].0 - node_top).abs() < 1e-6, "first stacked segment must start at the node's own top edge");
        for w in segments.windows(2) {
            assert!((w[0].1 - w[1].0).abs() < 1e-6, "adjacent segments must abut exactly — no gap, no overlap");
        }
        assert!(segments.last().unwrap().1 <= node_bottom + 1e-6, "stacked segments must fit inside the node's own height");
    }

    // ── Empty / single node / zero-weight link — no panic (requirement 2) ─

    #[test]
    fn empty_nodes_produce_an_empty_layout_without_panicking() {
        let layout = layout_sankey(&[], &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert!(layout.node_rects.is_empty());
        assert!(layout.ribbons.is_empty());
    }

    #[test]
    fn single_node_with_no_links_lays_out_without_panicking() {
        let nodes = vec![node("solo", 0)];
        let layout = layout_sankey(&nodes, &[], Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.node_rects.len(), 1);
        assert!(layout.node_rects[0].height > 0.0);
    }

    #[test]
    fn zero_weight_link_lays_out_without_panicking() {
        let nodes = vec![node("a", 0), node("b", 1)];
        let links = vec![link(0, 1, 0.0, 0)];
        let layout = layout_sankey(&nodes, &links, Rect::new(0.0, 0.0, 400.0, 300.0));
        assert_eq!(layout.ribbons.len(), 1);
        assert!(layout.ribbons[0].src_thickness().abs() < 1e-9);
    }

    #[test]
    fn out_of_range_link_endpoints_are_dropped_without_panicking() {
        let nodes = vec![node("a", 0), node("b", 1)];
        let links = vec![link(0, 99, 5.0, 0), link(99, 1, 3.0, 0)];
        let layout = layout_sankey(&nodes, &links, Rect::new(0.0, 0.0, 400.0, 300.0));
        assert!(layout.ribbons.is_empty(), "malformed link endpoints must be dropped, not panic");
    }

    #[test]
    fn empty_figure_renders_without_panicking() {
        let figure = SankeyFigure::new(Vec::new(), Vec::new());
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 120.0), &theme);
        });
        assert!(result.is_ok(), "empty sankey figure must render without panicking");
    }

    // ── Hit-test (requirement 3) ────────────────────────────────────────────

    #[test]
    fn hit_test_node_resolves_a_point_inside_its_own_rect() {
        let nodes = vec![node("a", 0), node("b", 1)];
        let links = vec![link(0, 1, 5.0, 0)];
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let layout = layout_sankey(&nodes, &links, rect);

        let r = layout.node_rects[1];
        let cx = r.x + r.width / 2.0;
        let cy = r.y + r.height / 2.0;
        assert_eq!(hit_test_node(&layout, cx, cy), Some(1));
        assert_eq!(hit_test_node(&layout, -1000.0, -1000.0), None);
    }

    // ── category_palette (Wave 4a) ──────────────────────────────────────

    #[test]
    fn default_category_palette_is_unset_and_ribbon_color_uses_theme_palette() {
        let theme = FigureTheme::dark();
        assert_eq!(palette_color(&theme, None, 2), theme.palette[2 % theme.palette.len()]);
    }

    #[test]
    fn with_category_palette_routes_ribbon_color_through_the_explicit_palette() {
        let theme = FigureTheme::dark();
        let palette = CategoricalScale::default_palette();
        assert_eq!(palette_color(&theme, Some(&palette), 2), palette.color_for(2));
        assert_ne!(palette_color(&theme, Some(&palette), 2), theme.palette[2 % theme.palette.len()]);
    }

    #[test]
    fn with_category_palette_renders_without_panicking() {
        let nodes = vec![node("a", 0), node("b", 1)];
        let links = vec![link(0, 1, 5.0, 0)];
        let figure = SankeyFigure::new(nodes, links).with_category_palette(CategoricalScale::default_palette());
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 120.0), &theme);
        });
        assert!(result.is_ok());
    }
}
