//! Full-capability showcase fixture (test-only) — ONE deterministic PDF
//! (`out/typeset_showcase.pdf`) exercising every capability this crate and
//! its sibling engines (`uzor-text`, `uzor-figures`, `uzor-graph`,
//! `uzor-export`) ship: multi-page composition with header/footer/page
//! numbers, single- AND two-column body layout, the full figure family,
//! all four `uzor-graph` layout looks (drawn as real vector
//! [`crate::scene::TypesetFigure`]s straight into the page's own render
//! context, never rasterized into an `ImageBlock` — see [`GraphExhibit`]
//! below), ASCII-grid text rendering, glyph-level paragraph kinetics,
//! anchored-island wrap layout, and a padded table + bulleted list.
//!
//! ## The master/columns seam (report — the task's own explicit ask)
//!
//! [`crate::export::pages_to_pdf`] takes exactly ONE [`PageMaster`], but
//! only ever reads its `width`/`height` (confirmed by inspection of
//! `pdf_adapter.rs`'s own `pages_to_pdf` body — every other geometry
//! decision, including columns/header/footer/page-number, is already
//! BAKED into each composed [`Page`] by the time `pages_to_pdf` sees it).
//! Mixing a single-column opener + a two-column section + more
//! single-column pages in ONE document therefore does NOT need a change to
//! `pages_to_pdf`'s signature, nor a second `PdfBuilder` — the smallest
//! honest change is: **run `slice_pages` MULTIPLE times, once per section,
//! each against its own [`PageMaster`] (which may differ in `columns`, but
//! always shares the SAME physical `width`/`height`), then concatenate the
//! resulting `Vec<Page>`s and renumber `Page::index`/`Page::total`/
//! `Page::page_number` across the WHOLE concatenation** ([`renumber_pages`]
//! below) before the one, final `pages_to_pdf` call. Every section's
//! master shares byte-identical header/footer content and page-number
//! style, so the renumbering step only ever rewrites the page-number
//! STRING (`"n of total"`), never any painted geometry — the document
//! reads as one continuous, correctly-numbered artifact even though it was
//! assembled from three independent `slice_pages` calls.

use std::cell::RefCell;
use std::path::PathBuf;

use uzor::fonts::FontFamily;
use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec};
use uzor_figures::{
    Annotation, BarFigure, BoxplotFigure, CurveFigure, FigureTheme, HistogramFigure, KpiFigure, NumberFormat, PointRadius, SankeyFigure,
    SankeyLink, SankeyNode, ScatterFigure, ScatterPoint, TimeScale, TimelineEvent, TimelineFigure,
};
use uzor_graph::{ForceDirectedLayout, Graph, GraphEngine, HierarchicalLayout, HierarchicalParams, Layout, NodeIndex, RadialLayout, RadialParams};
use uzor_text::ascii::{build_ascii_grid, AsciiGrid, AsciiGridStyle};
use uzor_text::{
    build_morph, draw_paragraph, layout_text, sample_layout, BreakStrategy, CosmicShaper, FontSpec, Hyphenation, InlineBox, InlineBoxSlot,
    Paragraph, ParagraphAlign, ParagraphLayout, StyledRun, VerticalAlign,
};

use crate::caption::{attach_captions, resolve_caption_numbers, resolve_refs, CaptionStyle, RefSegment};
use crate::compose::{BreakControl, ComposeStyle};
use crate::export::pages_to_pdf;
use crate::master::{PageNumberFormat, PageNumberStyle};
use crate::scene::{
    AnchoredIsland, Block, BlockId, BlockNode, BlockSizing, CaptionKind, CellPadding, ColumnSpec, FigureBlock, Footnote, ImageBlock, ImageFit,
    IslandAnchor, ListBlock, ListItem, MarkerStyle, NumberScheme, TableBlock, TableCell, TableRow, TypesetFigure,
};
use crate::slice::{renumber_pages, slice_pages, Margins, Page, PageMaster};
use crate::style::Theme;
use crate::toc::{compose_document_with_toc, TocStyle};

const PAGE_W: f64 = 595.0;
const PAGE_H: f64 = 842.0;

const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 24.0, bold: true, italic: false };
const SUBTITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 15.0, bold: false, italic: true };
const HEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 17.0, bold: true, italic: false };
const SUBHEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 15.0, bold: true, italic: false };
const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };
const COLUMN_BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };
const CAPTION_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 11.0, bold: false, italic: true };
const CAPTION_SMALL_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 10.0, bold: false, italic: false };
const TABLE_HEADER_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 12.0, bold: true, italic: false };
const TABLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 12.0, bold: false, italic: false };

fn out_dir() -> PathBuf {
    PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
}

fn write_proof(name: &str, bytes: &[u8]) {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
    std::fs::write(dir.join(name), bytes).expect("write proof file");
}

/// How much denser than the placed logical size the generated "photo"
/// buffer below is rendered — the raster/XObject path's own proof fixture
/// (see [`generated_photo_rgba`]'s own doc comment) needs a source buffer
/// genuinely denser than its display rect, or no amount of downstream
/// embedding care can make it hold up at PDF zoom (`uzor-export`'s own
/// WAVE 1 convention: an image XObject embeds at the source data's own
/// native pixel resolution, never artificially upsampled). Raised `3x ->
/// 5x` (owner-reported still-blocky at 8x zoom) now that `uzor-export`'s
/// image XObject writer encodes an opaque source as JPEG (`/DCTDecode`)
/// instead of Flate-over-raw-RGB — the density budget that previously
/// capped this at `3x` (Flate cost scales with raw pixel count) no longer
/// applies at anywhere near the same rate.
const PHOTO_OVERSAMPLE: u32 = 5;

/// Alpha-blend `color` over `base` by `coverage` (`0.0` = `base`
/// untouched, `1.0` = fully `color`) — the one shared compositing step
/// [`generated_photo_rgba`]'s own anti-aliased circle edges use.
fn blend_over(base: [u8; 4], color: [u8; 3], coverage: f64) -> [u8; 4] {
    let coverage = coverage.clamp(0.0, 1.0);
    let inv = 1.0 - coverage;
    [
        (color[0] as f64 * coverage + base[0] as f64 * inv).round() as u8,
        (color[1] as f64 * coverage + base[1] as f64 * inv).round() as u8,
        (color[2] as f64 * coverage + base[2] as f64 * inv).round() as u8,
        255,
    ]
}

/// A deterministic, generated "photo" RGBA buffer — layered gradients +
/// two ANTI-ALIASED circles, no binary asset, no RNG/time. Diverges from
/// `crate::render::tests::generated_photo_rgba` on purpose now (this
/// fixture used to match it verbatim — see that function's own doc
/// comment, unchanged): the owner's own screenshot review found this
/// PDF-embedded copy specifically showing "a giant pixel staircase on a
/// yellow circle" at PDF zoom, traced to two causes fixed here — (a) a
/// binary `if d < r` hard cut with zero edge smoothing, now a real
/// coverage-based alpha blend (`blend_over`) falling off over ~1.5px of
/// this function's OWN buffer resolution, and (b) the buffer was
/// generated at exactly its placed display size (1x), so no embedding
/// discipline downstream could add detail that was never captured in the
/// first place — this function now renders at [`PHOTO_OVERSAMPLE`]x the
/// caller's LOGICAL `width`/`height` (the `ImageBlock`'s own display rect
/// is unaffected — callers pass the RETURNED, denser `(w, h)` as the
/// block's intrinsic size, keeping `BlockSizing`/`ImageFit` driven by the
/// original logical size). Returns `(rgba, actual_width, actual_height)`.
fn generated_photo_rgba(logical_width: u32, logical_height: u32, seed: u32) -> (Vec<u8>, u32, u32) {
    let width = logical_width.saturating_mul(PHOTO_OVERSAMPLE).max(1);
    let height = logical_height.saturating_mul(PHOTO_OVERSAMPLE).max(1);
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    let hue_shift = (seed % 3) as f64 * 40.0;
    let (cx1, cy1, r1) = (width as f64 * 0.35, height as f64 * 0.38, width.min(height) as f64 * 0.30);
    let (cx2, cy2, r2) = (width as f64 * 0.7, height as f64 * 0.65, width.min(height) as f64 * 0.22);

    for y in 0..height {
        for x in 0..width {
            let fx = x as f64 / (width.max(1) as f64);
            let fy = y as f64 / (height.max(1) as f64);
            let r = (30.0 + 150.0 * fx + hue_shift).clamp(0.0, 255.0) as u8;
            let g = (60.0 + 130.0 * fy).clamp(0.0, 255.0) as u8;
            let b = (150.0 + 90.0 * (1.0 - fx) - hue_shift).clamp(0.0, 255.0) as u8;
            let mut pixel = [r, g, b, 255u8];

            // Coverage falloff over ~1.5px of THIS buffer's own
            // resolution (denser than the display rect thanks to
            // `PHOTO_OVERSAMPLE`, so the edge reads smooth even after
            // downscaling back to the placed logical size).
            let d1 = ((x as f64 + 0.5 - cx1).powi(2) + (y as f64 + 0.5 - cy1).powi(2)).sqrt();
            let coverage1 = r1 - d1 + 0.75;
            if coverage1 > 0.0 {
                pixel = blend_over(pixel, [235, 205, 90], coverage1);
            }
            let d2 = ((x as f64 + 0.5 - cx2).powi(2) + (y as f64 + 0.5 - cy2).powi(2)).sqrt();
            let coverage2 = r2 - d2 + 0.75;
            if coverage2 > 0.0 {
                pixel = blend_over(pixel, [60, 140, 95], coverage2);
            }

            let idx = ((y as usize) * (width as usize) + (x as usize)) * 4;
            buf[idx..idx + 4].copy_from_slice(&pixel);
        }
    }
    (buf, width, height)
}

// ── uzor-graph fixtures (public API only — same shape as
// `uzor_graph::proof_tests`' own seeded fixtures, replicated here since
// those are private to that crate's own test module) ───────────────────

/// Deterministic 3-root tree/DAG (3 roots x 3 mid-nodes x 2 leaves = 30
/// nodes) — the hierarchical/radial looks' shared fixture.
fn build_tree_graph() -> Graph<(), ()> {
    let mut graph: Graph<(), ()> = Graph::new();
    for r in 0..3 {
        let root = graph.push_node((), format!("root-{r}"), format!("cluster-{r}"), 7.0);
        for c in 0..3 {
            let mid = graph.push_node((), format!("r{r}-c{c}"), format!("cluster-{r}"), 5.0);
            graph.push_edge(root, mid, 1.0, ());
            for l in 0..2 {
                let leaf = graph.push_node((), format!("r{r}-c{c}-l{l}"), format!("cluster-{r}"), 4.0);
                graph.push_edge(mid, leaf, 1.0, ());
            }
        }
    }
    graph
}

/// Deterministic 3-cluster x 8-node fixture plus one connecting hub — the
/// force-directed/collapsed-cluster looks' shared fixture.
fn build_cluster_graph() -> (Graph<(), ()>, [Vec<NodeIndex>; 3]) {
    let mut graph: Graph<(), ()> = Graph::new();
    let hub = graph.push_node((), "hub", "hub", 5.0);
    let mut clusters: [Vec<NodeIndex>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (c, cluster) in clusters.iter_mut().enumerate() {
        let mut members = Vec::new();
        for m in 0..8 {
            let id = graph.push_node((), format!("c{c}n{m}"), format!("cluster-{c}"), 4.0);
            members.push(id);
        }
        for i in 0..members.len() {
            graph.push_edge(members[i], members[(i + 1) % members.len()], 1.0, ());
        }
        graph.push_edge(hub, members[0], 1.0, ());
        graph.push_edge(members[1], hub, 0.6, ());
        *cluster = members;
    }
    (graph, clusters)
}

/// The same deterministic golden-angle-ish scatter `uzor_graph`'s own
/// `collapsed_cluster_renders_supernode_to_a_valid_png` proof uses.
fn seed_cluster_positions() -> Vec<(f32, f32)> {
    let mut positions = vec![(0.0f32, 0.0f32)];
    for c in 0..3 {
        let angle = c as f32 * 2.094_395;
        let cx = angle.cos() * 160.0;
        let cy = angle.sin() * 160.0;
        for m in 0..8 {
            let a = m as f32 / 8.0 * std::f32::consts::TAU;
            positions.push((cx + a.cos() * 40.0, cy + a.sin() * 40.0));
        }
    }
    positions
}

const GRAPH_BG_HEX: &str = "#0d0f14";

/// Adapter turning a fully-settled `uzor-graph` [`GraphEngine`] into a
/// [`TypesetFigure`] — the vector-PDF replacement for the former
/// render-to-RGBA-then-`ImageBlock` bridge every graph exhibit used below
/// (kills the "every uzor-graph exhibit rasterized at 1x, jagged diagonal
/// edges at PDF zoom" raster shakal the owner flagged: graph exhibits now
/// go through `Block::Figure` -> `PdfRenderContext`, the SAME real-vector
/// path every other figure in this document already uses, instead of
/// `Block::Image`'s raster/XObject path).
///
/// `TypesetFigure::render` only ever hands out `&self`, but
/// `GraphEngine::draw`/`set_canvas_rect`/`fit_view` are all `&mut self`
/// (they cache the culled `visible` set / re-fit the camera on every
/// call) — a `RefCell` is the smallest interior-mutability seam that
/// keeps this fixture's own graph-BUILDING code (seed positions, tick the
/// layout to settle it — done ONCE ahead of time, byte-identical to the
/// deleted `*_rgba` helpers this replaces) completely separate from the
/// PAINT call, which now happens directly against whatever `RenderContext`
/// the page itself is using (`PdfRenderContext` for the real PDF, plain
/// `tiny-skia` for the raster parity-PNG pass at the bottom of this file)
/// instead of an offscreen 1x pixmap.
///
/// No new "fit camera to rect" helper was needed in `uzor-graph` itself:
/// `GraphEngine::set_canvas_rect` + `GraphEngine::fit_view` together
/// already ARE that helper — `Camera2D::fit_view` computes pan/zoom
/// purely from the particle-position `Aabb` and whatever `viewport` rect
/// it's handed, never from any PREVIOUSLY set canvas_rect — so calling
/// both again at THIS figure's own placed `rect` re-fits the camera
/// exactly the way the old code fit it to a fixed 700x500 offscreen
/// buffer, just against the real placement rect instead.
struct GraphExhibit<L: Layout> {
    engine: RefCell<GraphEngine<(), (), L>>,
    background: &'static str,
}

impl<L: Layout> GraphExhibit<L> {
    fn new(engine: GraphEngine<(), (), L>, background: &'static str) -> Self {
        Self { engine: RefCell::new(engine), background }
    }
}

impl<L: Layout> TypesetFigure for GraphExhibit<L> {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        ctx.set_fill_color(self.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        let mut engine = self.engine.borrow_mut();
        engine.set_canvas_rect(rect);
        engine.fit_view();
        engine.draw(ctx);
    }
}

/// Settle a force-directed layout over the 3-cluster/hub fixture — same
/// 200-tick warm-up the deleted `force_directed_rgba` used, just never
/// touching `canvas_rect`/`fit_view` here (both are resolved lazily, at
/// paint time, against this figure's own real placed rect — see
/// [`GraphExhibit`]'s own doc comment for why that's exact, not an
/// approximation).
fn build_force_directed_exhibit() -> GraphExhibit<ForceDirectedLayout> {
    let (graph, _clusters) = build_cluster_graph();
    let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
    engine.seed_positions(&seed_cluster_positions());
    for _ in 0..200 {
        engine.tick(1.0 / 60.0);
    }
    GraphExhibit::new(engine, GRAPH_BG_HEX)
}

fn build_hierarchical_exhibit() -> GraphExhibit<HierarchicalLayout> {
    let graph = build_tree_graph();
    let mut engine: GraphEngine<(), (), HierarchicalLayout> = GraphEngine::new(graph, HierarchicalLayout::new(HierarchicalParams::default()));
    engine.tick(1.0 / 60.0);
    GraphExhibit::new(engine, GRAPH_BG_HEX)
}

fn build_radial_exhibit() -> GraphExhibit<RadialLayout> {
    let graph = build_tree_graph();
    let mut engine: GraphEngine<(), (), RadialLayout> = GraphEngine::new(graph, RadialLayout::new(RadialParams::default()));
    engine.tick(1.0 / 60.0);
    GraphExhibit::new(engine, GRAPH_BG_HEX)
}

fn build_collapsed_cluster_exhibit() -> GraphExhibit<ForceDirectedLayout> {
    let (graph, clusters) = build_cluster_graph();
    let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
    engine.seed_positions(&seed_cluster_positions());

    let group_a = engine.define_cluster(clusters[0].clone());
    engine.define_cluster(clusters[1].clone());
    engine.define_cluster(clusters[2].clone());

    for _ in 0..200 {
        engine.tick(1.0 / 60.0);
    }
    if let Some(group_a) = group_a {
        engine.collapse_cluster(group_a);
    }
    GraphExhibit::new(engine, GRAPH_BG_HEX)
}

// ── uzor-text ASCII + kinetics exhibits (vector cutover, 2026-07-22) ────
//
// Both exhibits used to render offscreen to an RGBA8 buffer and embed as a
// `Block::Image` — flagged by the owner as pixel mush at PDF zoom, same
// class of defect the graph-exhibit vector cutover already fixed for
// `uzor-graph` content (see the "Graph-exhibit vector cutover" doc
// comment above). Both `uzor_text::ascii::AsciiGrid::render` and
// `uzor_text::draw_paragraph` ALREADY paint through a plain `&mut dyn
// RenderContext` (real `fill_text`/`fill_rect` calls, not pixel writes) —
// contrary to this file's own now-stale WAVE-1-era claim that these two
// exhibits have "no live re-drivable RenderContext object": both are
// exactly that, they just used to be driven against an offscreen
// `tiny-skia` pixmap (`render_rgba`) instead of the page's own
// `PdfRenderContext`. The SAMPLING stays byte-identical in both cases
// (`build_ascii_grid`'s glyph-coverage math; `build_morph`/`sample_layout`'s
// resize-morph math) — only the paint target changes, via the SAME
// `TypesetFigure` seam `GraphExhibit` already established.

/// Adapter turning a stepped [`AsciiGrid`] into a [`TypesetFigure`] — the
/// vector-PDF replacement for the ASCII exhibit's former render-to-RGBA-
/// then-`ImageBlock` bridge. Paints its own small background panel (sized
/// tightly to the grid's own content, the same "no wasted blank space"
/// fix the deleted `ascii_exhibit_rgba` doc comment already established)
/// centered — well, left-flush, since this exhibit always spans the
/// caller's own `max_width` — inside whatever `rect` the page hands it.
///
/// Paint strategy (measured, not assumed — see [`Self::render`]'s own
/// body): this exhibit's shader (`uzor_text::ascii::ParagraphAsciiShader`)
/// always emits a flat, grid-wide-uniform `color` and a constant
/// `alpha: 1.0`/`scale: 1.0` for every cell (verified here by scanning
/// every cell, not hardcoded) — when that holds AND this exhibit's own
/// monospace font's REAL shaped glyph advance (measured through the SAME
/// shaper every paragraph in this document already goes through) lands
/// close to the grid's own `cell_w`, each row merges into one contiguous
/// same-color `fill_text` run (far fewer PDF ops than one call per
/// non-blank cell). Otherwise — never reached by this exhibit's own
/// shader today, but a real, always-correct fallback for a hypothetical
/// future variable-alpha/scale `AsciiGrid` — it degrades to one centered
/// `fill_text` per non-blank cell, mirroring `AsciiGrid::render`'s own
/// per-cell font-size-from-scale/alpha handling exactly.
struct AsciiExhibit {
    grid: AsciiGrid,
    style: AsciiGridStyle,
    margin: f64,
    background: &'static str,
}

impl AsciiExhibit {
    fn new(grid: AsciiGrid, style: AsciiGridStyle, margin: f64, background: &'static str) -> Self {
        Self { grid, style, margin, background }
    }
}

/// How close (as a fraction of `cell_w`) this exhibit's real measured
/// monospace glyph advance must land to `cell_w` before [`AsciiExhibit`]
/// merges a row into one `fill_text` run instead of one call per cell.
const ASCII_ROW_MERGE_TOLERANCE: f64 = 0.15;

fn rgb_css(color: [u8; 3]) -> String {
    format!("rgb({},{},{})", color[0], color[1], color[2])
}

impl TypesetFigure for AsciiExhibit {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let cols = self.grid.cols();
        let rows = self.grid.rows();
        let content_w = (cols as f64) * self.style.cell_w + 2.0 * self.margin;
        let content_h = (rows as f64) * self.style.cell_h + 2.0 * self.margin;
        ctx.set_fill_color(self.background);
        ctx.fill_rect(rect.x, rect.y, content_w, content_h);
        if cols == 0 || rows == 0 {
            return;
        }

        let cell_w = self.style.cell_w;
        let cell_h = self.style.cell_h;
        let base_fs = cell_h.max(4.0);
        let ox = rect.x + self.margin;
        let oy = rect.y + self.margin;

        let first = self.grid.cell(0, 0);
        let uniform_alpha_scale =
            (0..rows).all(|y| (0..cols).all(|x| { let c = self.grid.cell(x, y); c.alpha == first.alpha && c.scale == first.scale }));

        ctx.set_text_baseline(TextBaseline::Middle);

        let row_merge = if uniform_alpha_scale {
            let font_px = (base_fs * first.scale as f64).clamp(3.0, base_fs * 1.7).round().max(1.0);
            ctx.set_font(&format!("{font_px}px monospace"));
            ctx.set_global_alpha(first.alpha as f64);
            let glyph_advance = ctx.measure_text("M");
            glyph_advance > 0.0 && (glyph_advance - cell_w).abs() <= cell_w * ASCII_ROW_MERGE_TOLERANCE
        } else {
            false
        };

        if row_merge {
            ctx.set_text_align(TextAlign::Left);
            for y in 0..rows {
                let mut x = 0usize;
                while x < cols {
                    let color = self.grid.cell(x, y).color;
                    let start = x;
                    let mut run = String::with_capacity(cols - x);
                    let mut has_ink = false;
                    while x < cols {
                        let cell = self.grid.cell(x, y);
                        if cell.color != color {
                            break;
                        }
                        has_ink |= cell.ch != ' ';
                        run.push(cell.ch);
                        x += 1;
                    }
                    if has_ink {
                        ctx.set_fill_color(&rgb_css(color));
                        let run_x = ox + (start as f64) * cell_w;
                        let run_y = oy + (y as f64 + 0.5) * cell_h;
                        ctx.fill_text(&run, run_x, run_y);
                    }
                }
            }
            ctx.set_global_alpha(1.0);
            return;
        }

        // Per-cell fallback (never reached by this exhibit's own shader
        // today — see this struct's own doc comment).
        ctx.set_text_align(TextAlign::Center);
        let mut cur_fs = -1_i32;
        let mut cur_alpha = -1.0_f64;
        for y in 0..rows {
            for x in 0..cols {
                let cell = self.grid.cell(x, y);
                if cell.ch == ' ' || cell.alpha <= 0.01 || cell.scale <= 0.06 {
                    continue;
                }
                let fs = (base_fs * cell.scale as f64).clamp(3.0, base_fs * 1.7).round() as i32;
                if fs != cur_fs {
                    ctx.set_font(&format!("{fs}px monospace"));
                    cur_fs = fs;
                }
                let alpha = cell.alpha as f64;
                if (alpha - cur_alpha).abs() > 0.004 {
                    ctx.set_global_alpha(alpha);
                    cur_alpha = alpha;
                }
                ctx.set_fill_color(&rgb_css(cell.color));
                let cx = ox + (x as f64 + 0.5) * cell_w;
                let cy = oy + (y as f64 + 0.5) * cell_h;
                let mut buf = [0u8; 4];
                ctx.fill_text(cell.ch.encode_utf8(&mut buf), cx, cy);
            }
        }
        ctx.set_global_alpha(1.0);
    }
}

/// Build the ASCII-grid exhibit's [`TypesetFigure`] — the sampling
/// (`build_ascii_grid`'s own glyph-coverage math, unchanged) stays
/// byte-identical to the deleted raster `ascii_exhibit_rgba` this
/// replaces; only the paint target changes (see [`AsciiExhibit`]'s own
/// doc comment). Returns the figure plus its own exact content height
/// (`rows * cell_h + 2*margin`) so the caller can size a
/// `BlockSizing::FixedHeight` that leaves no letterboxing gap — the
/// figure paints its OWN background/grid filling exactly that much of
/// whatever rect it's handed (a `Block::Figure`'s placed rect always
/// spans the full region width, so there's no `ImageFit::Contain`
/// aspect-fitting concern left to solve at all).
fn build_ascii_exhibit(max_width: u32) -> (AsciiExhibit, f64) {
    const TEXT: &str = "This short seeded paragraph is rendered through the uzor-text ASCII cell-shader \
        bridge, sampling real glyph coverage onto a monospace character grid instead of painting glyph \
        outlines directly.";
    const MARGIN: f64 = 16.0;

    let font = FontSpec::new(FontFamily::Roboto, 15.0);
    let shaper = CosmicShaper::headless();
    let layout = layout_text(TEXT, &font, (max_width as f64) - 2.0 * MARGIN, &shaper);
    let style = AsciiGridStyle::square(8.0);
    let (_cols, rows) = uzor_text::ascii::grid_dims_for_layout(&layout, style.cell_w, style.cell_h);
    let grid = build_ascii_grid(&layout, style);

    let content_h = (rows as f64) * style.cell_h + 2.0 * MARGIN;
    (AsciiExhibit::new(grid, style, MARGIN, "#ffffff"), content_h)
}

const KINETICS_TEXT: &str = "A fixed seeded paragraph morphing between a narrow and a wide column, one glyph at a time.";

/// This exhibit's own fixed demo-box width — deliberately independent of
/// the page's own (much wider) body width, matching the compact box the
/// deleted raster `kinetics_frame_rgba` always rendered at (`460px` wide,
/// `15px` margins on every side, "wide" endpoint text wrapped at
/// `460 - 2*15 = 430px`) — kept byte-identical so the morph's own two
/// wrap endpoints (and therefore every sampled mid-flight frame) are
/// unchanged from the pre-cutover raster render.
const KINETICS_BOX_W: f64 = 460.0;
const KINETICS_MARGIN: f64 = 15.0;

/// Adapter turning one already-sampled [`ParagraphLayout`] frame (`t = 0.0
/// / 0.5 / 1.0`, from [`build_kinetics_exhibit`]) into a [`TypesetFigure`]
/// — `uzor_text::draw_paragraph` already paints through a plain `&mut dyn
/// RenderContext`, so this is a direct call, not a new drawing path.
/// Centers its own fixed [`KINETICS_BOX_W`]-wide panel horizontally inside
/// whatever (wider) `rect` the page hands it, so the compact demo box
/// keeps reading as a small inset figure rather than stretching to fill
/// the full body width.
struct KineticsExhibit {
    layout: ParagraphLayout,
    background: &'static str,
}

impl TypesetFigure for KineticsExhibit {
    fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, _theme: &FigureTheme) {
        let box_h = self.layout.height + 2.0 * KINETICS_MARGIN;
        let ox = rect.x + ((rect.width - KINETICS_BOX_W) / 2.0).max(0.0);
        let oy = rect.y;
        ctx.set_fill_color(self.background);
        ctx.fill_rect(ox, oy, KINETICS_BOX_W, box_h);
        draw_paragraph(ctx, (ox + KINETICS_MARGIN, oy + KINETICS_MARGIN), &self.layout, "#111111", false);
    }
}

/// Build one sampled kinetics-morph frame at `t` — the resize-morph
/// sampling (`build_morph`/`sample_layout`) stays byte-identical to the
/// deleted raster `kinetics_frame_rgba` this replaces; only the paint
/// target changes (see [`KineticsExhibit`]'s own doc comment). Returns
/// the figure plus its own exact content height (`layout.height +
/// 2*margin`) — each of the 3 sampled frames (t=0/0.5/1) gets its own
/// tightly-fit `BlockSizing::FixedHeight`, since a narrow-wrapped frame
/// and a wide-wrapped frame genuinely differ in line count/height.
fn build_kinetics_exhibit(t: f64) -> (KineticsExhibit, f64) {
    let font = FontSpec::new(FontFamily::Roboto, 14.0);
    let shaper = CosmicShaper::headless();
    let narrow = layout_text(KINETICS_TEXT, &font, 130.0, &shaper);
    let wide = layout_text(KINETICS_TEXT, &font, KINETICS_BOX_W - 2.0 * KINETICS_MARGIN, &shaper);
    let morph = build_morph(&narrow, &wide);
    let sampled = sample_layout(&morph, t, 0x111111_ff);
    let content_h = sampled.height + 2.0 * KINETICS_MARGIN;
    (KineticsExhibit { layout: sampled, background: "#ffffff" }, content_h)
}

/// Decode a PNG's dimensions only (used by the per-page parity render
/// loop's own sanity assertion).
fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
    let decoder = png::Decoder::new(bytes);
    let reader = decoder.read_info().expect("valid PNG header");
    let info = reader.info();
    (info.width, info.height)
}

/// The full-capability showcase (design law 8 — the owner opens this file
/// directly): title/subtitle/justified+hyphenated intro (single column) ->
/// a two-column narrative section -> the full figure family (bar, curve
/// with a `TimeScale` X-axis, histogram, timeline with 3 lanes, sankey with
/// a dominant path) -> all four `uzor-graph` layout looks embedded as real
/// `ImageBlock`s -> an ASCII-grid text exhibit + a 3-frame kinetics morph
/// strip -> centered- and left/right-anchored island layout demos -> a
/// padded data table + bulleted list with keep-with-next headings ->
/// `out/typeset_showcase.pdf` + one parity PNG per page.
#[test]
fn full_capability_showcase_produces_the_pdf_and_a_parity_png_per_page() {
    let theme = Theme::light_report();
    let shaper = CosmicShaper::headless();

    // ── shared header/footer + page-number token (identical across every
    // section's own master, so renumbering only ever rewrites the
    // page-number STRING, never any painted chrome) ─────────────────────
    let header_run = [StyledRun::new("The uzor Engine Family — Full-Capability Showcase", CAPTION_SMALL_FONT)];
    let header_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&header_run, 400.0)))];
    let footer_run = [StyledRun::new("uzor-typeset showcase — deterministic fixture, uzor-text + uzor-figures + uzor-graph + uzor-export", CAPTION_SMALL_FONT)];
    let footer_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&footer_run, 400.0)))];
    let page_number_style = PageNumberStyle::new(PageNumberFormat::OfTotal, 1);

    let master_single = PageMaster::new(PAGE_W, PAGE_H, Margins::uniform(40.0))
        .with_header(&header_flow)
        .with_footer(&footer_flow)
        .with_page_number(page_number_style);
    let master_two_col = PageMaster::new(PAGE_W, PAGE_H, Margins::uniform(40.0))
        .with_columns(2, 18.0)
        .with_header(&header_flow)
        .with_footer(&footer_flow)
        .with_page_number(page_number_style);

    let body_width = master_single.body_rect().width;
    let column_width = master_two_col.column_width();
    let style_single = ComposeStyle::from_theme(&theme, 12.0);
    let style_two_col = ComposeStyle::from_theme(&theme, 10.0);

    // ── Section 1: opener (single column) ───────────────────────────────
    let title_run = [StyledRun::new("The uzor Engine Family — Full-Capability Showcase", TITLE_FONT)];
    let subtitle_run = [StyledRun::new(
        "A single deterministic document exercising composition, text, figure, graph, and export capability.",
        SUBTITLE_FONT,
    )];
    const INTRO_A: &str = "This document is produced entirely by uzor-typeset's own composition and slicing \
        pipeline, drawing on uzor-text for paragraph layout and kinetics, uzor-figures for chart rendering, \
        uzor-graph for force-directed and hierarchical graph visualization, and uzor-export for headless \
        rasterization and PDF assembly.";
    const INTRO_B: &str = "Every page that follows demonstrates one distinct capability against fixed, seeded, \
        deterministic data — nothing here depends on network access, wall-clock time, or random sampling, so \
        the same input always produces the identical output byte-for-byte, an important property for \
        reproducible documentation and automated verification alike.";
    // Cyrillic end-to-end proof (export SOTA pass — `uzor-export`'s Type0/
    // CID + `/ToUnicode` migration, see its own `CLAUDE.md`): a Russian
    // intro sentence flowing through the SAME `pages_to_pdf`/`PdfTextRun`
    // path as every other paragraph in this fixture, never a special-cased
    // text run — proves full-Unicode vector text end to end, not just at
    // the `uzor-export` unit-test level.
    const INTRO_RU: &str = "Этот показательный документ также подтверждает поддержку кириллического текста \
        в PDF-экспорте библиотеки uzor.";
    let intro_a_run = [StyledRun::new(INTRO_A, BODY_FONT)];
    let intro_ru_run = [StyledRun::new(INTRO_RU, BODY_FONT)];
    let intro_b_run = [StyledRun::new(INTRO_B, BODY_FONT)];

    let flow_opener: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&subtitle_run, body_width))),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(
            Paragraph::new(&intro_a_run, body_width)
                .with_align(ParagraphAlign::Justify)
                .with_break_strategy(BreakStrategy::KnuthPlass)
                .with_hyphenation(Hyphenation::English),
        )),
        BlockNode::new(Block::Spacer(12.0)),
        BlockNode::new(Block::Paragraph(
            Paragraph::new(&intro_ru_run, body_width)
                .with_align(ParagraphAlign::Justify)
                .with_break_strategy(BreakStrategy::KnuthPlass)
                .with_hyphenation(Hyphenation::Russian),
        )),
        BlockNode::new(Block::Spacer(12.0)),
        BlockNode::new(Block::Paragraph(
            Paragraph::new(&intro_b_run, body_width)
                .with_align(ParagraphAlign::Justify)
                .with_break_strategy(BreakStrategy::KnuthPlass)
                .with_hyphenation(Hyphenation::English),
        )),
    ];
    let pages_opener = slice_pages(&flow_opener, &master_single, &style_single, &shaper);

    // Typography quality wave: the Russian intro paragraph (the only
    // Cyrillic content in this fixture, so its own placed glyphs are
    // unambiguously identifiable) must show a REAL discretionary hyphen
    // break — checked directly against the composed `ParagraphLayout`
    // (a stronger, position-independent proof than PDF text extraction,
    // which legitimately no longer contains a hyphenated word as ONE
    // contiguous substring once it genuinely breaks across two lines).
    let russian_paragraph_hyphenates = pages_opener.iter().any(|page| {
        page.frame.blocks.iter().any(|b| {
            b.paragraph_layout.as_ref().is_some_and(|layout| {
                let is_cyrillic = layout.glyphs.iter().any(|g| g.cluster.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)));
                let has_hyphen = layout.glyphs.iter().any(|g| g.cluster == "-");
                is_cyrillic && has_hyphen
            })
        })
    });
    assert!(russian_paragraph_hyphenates, "the Russian intro paragraph must show a real discretionary hyphen break under Hyphenation::Russian");

    // ── Section 2: two-column narrative ─────────────────────────────────
    let two_col_heading_run = [StyledRun::new("Composition & Slicing", HEADING_FONT)];
    const TWO_COL_1: &str = "A scene in this workspace is a flat, borrowed tree of blocks — paragraphs, \
        figures, images, tables, lists, spacers, and anchored islands — that flows through a sequence of \
        regions supplied by whichever target is slicing it.";
    const TWO_COL_2: &str = "Pages, slides, and build-step frames are all the SAME underlying content model, \
        sliced three different ways: a page region shrinks as content fills it and resets on the next page, a \
        slide region is a fixed viewport that either fits its content or reports an overflow, and a build-step \
        frame filters one shared composition down to a visible subset per step.";
    const TWO_COL_3: &str = "A page master carries physical geometry — margins, an optional header and footer \
        living in their own margin bands, a page-number token, and an optional column count — while a theme \
        resolves every color and font through a small, three-tier token chain, so editing a theme's brand \
        colors never changes a single paragraph's line breaks or a table's column widths.";
    const TWO_COL_4: &str = "Keep-with-next and keep-together controls prevent a section heading from ever \
        being stranded alone at the bottom of a page, and a table or a bulleted list splits only between whole \
        rows or items, never mid-row or mid-item, even when it spans more than one region.";
    const TWO_COL_5: &str = "Long compound words like implementation, infrastructure, categorical, and \
        reproducibility often expose uneven interword spacing in a narrow justified column, especially once \
        hyphenation is enabled and a break lands mid-word near the right margin, which is exactly what a \
        narrow two-column layout is well suited to demonstrate.";
    const TWO_COL_6: &str = "Paragraph text throughout this workspace is laid out by uzor-text: a swappable \
        shaper resolves each styled run to positioned glyphs, a greedy or Knuth-Plass line breaker chooses \
        where each line ends, and an optional Liang-pattern hyphenator supplies a mid-word break when \
        justification alone would otherwise leave a line uncomfortably loose.";
    const TWO_COL_7: &str = "Charts throughout this document are rendered by uzor-figures, which owns scale and \
        mark computation for every figure kind — bar, curve, histogram, timeline, and flow diagram — behind one \
        shared theme, so a single figure family reads consistently no matter which report or dashboard embeds \
        it.";
    const TWO_COL_8: &str = "Node/edge graphs are a separate concern entirely, owned by uzor-graph: a \
        many-body force simulation with Barnes-Hut acceleration above a few hundred particles, one-shot \
        layering for hierarchical and radial looks, and a cluster registry that collapses a group of nodes into \
        a single super-node and expands it back to its exact prior positions.";
    const TWO_COL_9: &str = "Turning any of this into a delivered file is uzor-export's job: an offscreen CPU \
        rasterizer produces a PNG at a chosen resolution with no window and no GPU dependency, and a small PDF \
        builder assembles a hybrid document — one rasterized background per page plus real, searchable vector \
        text laid directly on top of it.";
    const TWO_COL_10: &str = "None of these four engines depends on any of the others reaching back into it — \
        uzor-typeset is the only place that ever bridges all of them together into one composed document, which \
        is exactly the arrangement this showcase itself is built to prove, one section at a time.";

    let two_col_1_run = [StyledRun::new(TWO_COL_1, COLUMN_BODY_FONT)];
    let two_col_2_run = [StyledRun::new(TWO_COL_2, COLUMN_BODY_FONT)];
    let two_col_3_run = [StyledRun::new(TWO_COL_3, COLUMN_BODY_FONT)];
    let two_col_4_run = [StyledRun::new(TWO_COL_4, COLUMN_BODY_FONT)];
    let two_col_5_run = [StyledRun::new(TWO_COL_5, COLUMN_BODY_FONT)];
    let two_col_6_run = [StyledRun::new(TWO_COL_6, COLUMN_BODY_FONT)];
    let two_col_7_run = [StyledRun::new(TWO_COL_7, COLUMN_BODY_FONT)];
    let two_col_8_run = [StyledRun::new(TWO_COL_8, COLUMN_BODY_FONT)];
    let two_col_9_run = [StyledRun::new(TWO_COL_9, COLUMN_BODY_FONT)];
    let two_col_10_run = [StyledRun::new(TWO_COL_10, COLUMN_BODY_FONT)];

    let flow_two_col: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_heading_run, column_width)))
            .with_break_control(BreakControl::AvoidAfter)
            // Document-navigation feature pass: this outline entry's own
            // TITLE carries a Cyrillic translation alongside the English
            // heading actually PAINTED on the page — proves the PDF
            // bookmark tree's own `/Title` round-trips Unicode (`TextStr`'s
            // UTF-16BE-with-BOM path), independent of the printed heading
            // text (an outline entry's title is always explicit, never
            // derived from the heading paragraph's own runs — see
            // `crate::scene::OutlineTag`'s own doc comment).
            .with_outline(1, "Composition & Slicing / Компоновка и разбиение"),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_1_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_2_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_3_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_4_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(
            Paragraph::new(&two_col_5_run, column_width)
                .with_align(ParagraphAlign::Justify)
                .with_break_strategy(BreakStrategy::KnuthPlass)
                .with_hyphenation(Hyphenation::English),
        )),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_6_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_7_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_8_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_9_run, column_width).with_align(ParagraphAlign::Justify))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_10_run, column_width).with_align(ParagraphAlign::Justify))),
    ];
    let pages_two_col = slice_pages(&flow_two_col, &master_two_col, &style_two_col, &shaper);
    assert!(!pages_two_col.is_empty());
    assert!(pages_two_col[0].extra_frames.iter().any(|f| !f.blocks.is_empty()) || pages_two_col.len() > 1, "the two-column section must actually use its second column (or spill to a second page)");

    // ── Section 3: the full figure family (5 figures, 2-per-page) ───────
    const FIGURE_HEIGHT: f64 = 230.0;

    let bar_figure = BarFigure::new(
        vec!["Category A".to_owned(), "Category B".to_owned(), "Category C".to_owned(), "Category D".to_owned(), "Category E".to_owned()],
        vec![42.0, 78.0, 65.0, 91.0, 54.0],
    )
    .with_title("Category comparison (seeded sample)");

    const ANCHOR: f64 = 1_704_067_200.0; // 2024-01-01T00:00:00Z
    const DAY: f64 = 86_400.0;
    let curve_points: Vec<(f64, f64)> = {
        let mut running = 0.0_f64;
        (0..90)
            .map(|i| {
                let step = ((i * 41 + 7) % 29) as f64 - 14.0;
                running += step;
                (ANCHOR + i as f64 * DAY, running)
            })
            .collect()
    };
    let curve_scale = TimeScale::new(curve_points[0].0, curve_points[curve_points.len() - 1].0);
    let curve_figure = CurveFigure::new(curve_points).with_title("Cumulative sample metric over 90 days (TimeScale X-axis)").with_x_scale(curve_scale);

    let histogram_samples: Vec<f64> = (0..500).map(|i| ((i * 97 + 13) % 1000) as f64 / 10.0).collect();
    let histogram_figure = HistogramFigure::new(histogram_samples, 20).with_title("Sample value distribution (20 bins)");

    let timeline_events = vec![
        // Fixed defect (found during visual review, two rounds): `TimelineFigure`'s
        // own interval-beside-label placement only ever checks collision
        // against the PLOT's own right edge (flipping left when it would
        // clip), never against another event's label — the first two
        // spacings tried here (0/3-7/5/10-16/20-day, then
        // 0/6-12/15/21-27/29-day) both still let "validation window"'s
        // interval sit close enough to the domain's own right edge that
        // its beside-right label clipped, flipped LEFT, and landed
        // directly on top of "checkpoint reached"'s own point label on
        // the SAME lane. Fixed by keeping every interval comfortably
        // clear of the domain's own right edge (so its beside-right label
        // never needs to flip at all) and giving every event sharing a
        // lane a wide day-gap — the SAME reasoning `export::pdf_adapter`'s
        // own "Fix D/E" comment documents for this exact failure mode,
        // applied here via event timing rather than a lane-spacing
        // constant.
        TimelineEvent { ts: ANCHOR, end_ts: None, lane: 0, label: "signal received".to_owned(), kind: 0 },
        TimelineEvent { ts: ANCHOR + 6.0 * DAY, end_ts: Some(ANCHOR + 12.0 * DAY), lane: 0, label: "processing window".to_owned(), kind: 1 },
        TimelineEvent { ts: ANCHOR + 4.0 * DAY, end_ts: Some(ANCHOR + 14.0 * DAY), lane: 1, label: "validation window".to_owned(), kind: 1 },
        TimelineEvent { ts: ANCHOR + 16.0 * DAY, end_ts: None, lane: 1, label: "checkpoint reached".to_owned(), kind: 2 },
        TimelineEvent { ts: ANCHOR + 26.0 * DAY, end_ts: None, lane: 2, label: "completion recorded".to_owned(), kind: 0 },
    ];
    let timeline_figure = TimelineFigure::new(timeline_events, vec!["Lane A".to_owned(), "Lane B".to_owned(), "Lane C".to_owned()])
        .with_title("Timeline of sample events across 3 lanes (seeded)");

    let sankey_nodes = vec![
        SankeyNode { id: "input-a".to_owned(), label: "Input A".to_owned(), stage: 0 },
        SankeyNode { id: "input-b".to_owned(), label: "Input B".to_owned(), stage: 0 },
        SankeyNode { id: "path-1".to_owned(), label: "Path 1".to_owned(), stage: 1 },
        SankeyNode { id: "path-2".to_owned(), label: "Path 2".to_owned(), stage: 1 },
        SankeyNode { id: "output-x".to_owned(), label: "Output X".to_owned(), stage: 2 },
        SankeyNode { id: "output-y".to_owned(), label: "Output Y".to_owned(), stage: 2 },
    ];
    let sankey_links = vec![
        SankeyLink { from: 0, to: 2, weight: 70.0, kind: 0 }, // Input A -> Path 1 (dominant path)
        SankeyLink { from: 0, to: 3, weight: 8.0, kind: 1 },
        SankeyLink { from: 1, to: 2, weight: 6.0, kind: 1 },
        SankeyLink { from: 1, to: 3, weight: 16.0, kind: 2 },
        SankeyLink { from: 2, to: 4, weight: 68.0, kind: 0 }, // Path 1 -> Output X (dominant path)
        SankeyLink { from: 2, to: 5, weight: 8.0, kind: 1 },
        SankeyLink { from: 3, to: 4, weight: 6.0, kind: 1 },
        SankeyLink { from: 3, to: 5, weight: 18.0, kind: 2 },
    ];
    let sankey_figure = SankeyFigure::new(sankey_nodes, sankey_links).with_title("Staged flow with one dominant path (seeded)");

    let fig_a_heading_run = [StyledRun::new("Figure Exhibits — Categorical & Time-Series", SUBHEADING_FONT)];
    let fig_b_heading_run = [StyledRun::new("Figure Exhibits — Distribution & Timeline", SUBHEADING_FONT)];
    let fig_c_heading_run = [StyledRun::new("Figure Exhibits — Staged Flow", SUBHEADING_FONT)];
    let fig_d_heading_run = [StyledRun::new("Figure Exhibits — Statistical & Business", SUBHEADING_FONT)];

    // Typography-gap WAVE 3: the bar figure's own caption is now the AUTO-
    // NUMBERED path (`.with_caption` + `attach_captions`, below) instead of
    // a hand-typed "Figure 1 — ..." paragraph — the task's own "replace at
    // least one hand-typed caption" gate. A real in-text cross-reference
    // (`RefSegment::Ref`) resolves "Figure 1" back from `BAR_FIGURE_ID` via
    // the SAME `resolve_caption_numbers`/`resolve_refs` pipeline this
    // crate's own unit tests already prove — a probe flow holding just the
    // tagged node is enough since this showcase's own construction order
    // guarantees no earlier `CaptionKind::Figure` caption exists anywhere
    // before it in the real document (`flow_figures` is the FIRST section
    // extended into `flow_rest` below).
    const BAR_FIGURE_ID: BlockId = BlockId(9_001);
    // Typography-gap WAVE 4: curve/histogram/timeline/sankey/scatter/
    // boxplot/kpi-row ALL moved onto the SAME auto-numbering path this
    // wave (previously only the bar figure did — the other four carried
    // hand-typed "Figure 2"/"3"/"4"/"5" strings). Adding 3 more real
    // auto-tagged figures makes a mixed hand/auto numbering scheme
    // unmaintainable (a hand-typed number has no way to know how many
    // auto-tagged captions land before it) — converting every figure
    // caption in this section to `.with_caption()` is the correct fix,
    // not a workaround: numbers are now GUARANTEED correct regardless of
    // future insertions, matching `crate::caption`'s own module doc
    // ("resolution depends only on document order").
    const CURVE_FIGURE_ID: BlockId = BlockId(9_002);
    const HISTOGRAM_FIGURE_ID: BlockId = BlockId(9_003);
    const TIMELINE_FIGURE_ID: BlockId = BlockId(9_004);
    const SANKEY_FIGURE_ID: BlockId = BlockId(9_005);
    const SCATTER_FIGURE_ID: BlockId = BlockId(9_006);
    const BOXPLOT_FIGURE_ID: BlockId = BlockId(9_007);
    const KPI_ROW_ID: BlockId = BlockId(9_008);
    let caption_style = CaptionStyle::default();
    let bar_probe = [BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
        .with_id(BAR_FIGURE_ID)
        .with_caption(CaptionKind::Figure, "a categorical bar chart over 5 seeded categories.")];
    let bar_numbers = resolve_caption_numbers(&bar_probe);
    let bar_ref_text = resolve_refs(
        &[
            RefSegment::Text("The chart below (see "),
            RefSegment::Ref(BAR_FIGURE_ID),
            RefSegment::Text(") demonstrates this engine's auto-numbering and in-text cross-reference machinery end to end."),
        ],
        &bar_numbers,
        &caption_style,
    );
    let bar_ref_text: &'static str = Box::leak(bar_ref_text.into_boxed_str());
    let bar_ref_run = [StyledRun::new(bar_ref_text, BODY_FONT)];

    // ── Statistical & Business figures (typography-gap WAVE 4) ──────────
    let scatter_points: Vec<ScatterPoint> = (0..180)
        .map(|i| {
            let fi = i as f64;
            let x = fi * 1.3 + ((i * 11) % 6) as f64 * 0.5;
            let y = 30.0 + (fi * 0.25) + ((i * 17) % 23) as f64 - 11.0;
            let value = 3.0 + ((i * 13) % 18) as f64;
            ScatterPoint::with_value(x, y, value)
        })
        .collect();
    let scatter_callout_point = scatter_points[140];
    let scatter_figure = ScatterFigure::new(scatter_points)
        .with_title("Sample metric vs. index (seeded, size-mapped)")
        .with_radius(PointRadius::ValueMapped { min_radius: 2.0, max_radius: 8.0 })
        .with_annotations(vec![
            Annotation::HBand { low: 30.0, high: 55.0, color: None, label: Some("target range".to_owned()) },
            Annotation::Callout { x: scatter_callout_point.x, y: scatter_callout_point.y, text: "notable reading".to_owned() },
        ]);

    let boxplot_categories: Vec<String> = ["group-a", "group-b", "group-c", "group-d"].iter().map(|s| (*s).to_owned()).collect();
    let boxplot_group_a: Vec<f64> = (0..30).map(|i| 40.0 + ((i * 7) % 25) as f64).collect();
    let boxplot_group_b: Vec<f64> = (0..25).map(|i| 55.0 + ((i * 11) % 30) as f64).collect();
    let boxplot_group_c: Vec<f64> = {
        let mut v: Vec<f64> = (0..28).map(|i| 35.0 + ((i * 5) % 20) as f64).collect();
        v.push(140.0); // deliberate high outlier
        v
    };
    let boxplot_group_d: Vec<f64> = (0..3).map(|i| 60.0 + i as f64 * 5.0).collect(); // n == 3 edge case
    let boxplot_figure = BoxplotFigure::new(boxplot_categories, vec![boxplot_group_a, boxplot_group_b, boxplot_group_c, boxplot_group_d])
        .with_title("Sample distributions by group (seeded)");

    const KPI_TILE_HEIGHT: f64 = 110.0;
    let kpi_sparkline: Vec<f64> = (0..24).map(|i| 100.0 + ((i * 7) % 22) as f64 - (i as f64) * 0.4).collect();
    let kpi_revenue = KpiFigure::new("Revenue", 128_430.0)
        .with_previous_value(110_000.0)
        .with_format(NumberFormat::Currency("$"))
        .with_sparkline(kpi_sparkline);
    // `NumberFormat::Percent` multiplies by 100 (it expects the underlying
    // FRACTION, per that variant's own docs) — a churn rate already
    // expressed as "4.8%"/"6.1%" is stored as 0.048/0.061.
    let kpi_churn = KpiFigure::new("Churn Rate", 0.048).with_previous_value(0.061).with_format(NumberFormat::Percent);
    let kpi_active_users = KpiFigure::new("Active Users", 48_213.0).with_previous_value(48_213.0).with_format(NumberFormat::Si);
    let kpi_revenue_nodes = [BlockNode::new(Block::Figure(FigureBlock::new(&kpi_revenue, BlockSizing::FixedHeight(KPI_TILE_HEIGHT))))];
    let kpi_churn_nodes = [BlockNode::new(Block::Figure(FigureBlock::new(&kpi_churn, BlockSizing::FixedHeight(KPI_TILE_HEIGHT))))];
    let kpi_active_nodes = [BlockNode::new(Block::Figure(FigureBlock::new(&kpi_active_users, BlockSizing::FixedHeight(KPI_TILE_HEIGHT))))];
    let kpi_cells = [TableCell::new(&kpi_revenue_nodes), TableCell::new(&kpi_churn_nodes), TableCell::new(&kpi_active_nodes)];
    let kpi_rows = [TableRow::new(&kpi_cells)];
    let kpi_columns = [ColumnSpec::Fraction(1.0), ColumnSpec::Fraction(1.0), ColumnSpec::Fraction(1.0)];
    let kpi_table = TableBlock::new(&kpi_columns, &kpi_rows);

    let graph_intro_heading_run = [StyledRun::new("Graph Engine Family", SUBHEADING_FONT)];
    const GRAPH_INTRO: &str = "The following pages draw real, live layouts from uzor-graph, the reusable \
        force-directed graph visualization engine, directly into this composed document as vector figures — \
        the SAME per-layout draw path uzor-graph's own headless proof tests use, painted straight into the \
        page's own render context (real paths and strokes in the PDF) rather than rasterized once and placed \
        as an image.";
    let graph_intro_run = [StyledRun::new(GRAPH_INTRO, BODY_FONT)];

    let flow_figures: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_a_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(1, "Figure Exhibits"),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&bar_ref_run, body_width))),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(BAR_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a categorical bar chart over 5 seeded categories."),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&curve_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(CURVE_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a cumulative curve plotted against a calendar TimeScale X-axis."),
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_b_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&histogram_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(HISTOGRAM_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a histogram of 500 seeded samples grouped into 20 bins."),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&timeline_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(TIMELINE_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a timeline of point and interval events across 3 lanes."),
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_c_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&sankey_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(SANKEY_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a staged flow diagram with one clearly dominant path."),
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_d_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&scatter_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(SCATTER_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a size-mapped scatter cloud with a target-range band and a callout."),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&boxplot_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_id(BOXPLOT_FIGURE_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a quartile summary across 4 groups, including a Tukey-fence outlier."),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Table(kpi_table))
            .with_id(KPI_ROW_ID)
            .with_break_control(BreakControl::AvoidAfter)
            .with_caption(CaptionKind::Figure, "a 3-tile KPI dashboard row — big number, colored delta, trailing sparkline."),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_intro_heading_run, body_width)))
            .with_break_control(BreakControl::AvoidAfter)
            .with_outline(1, "Graph Engine Family"),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_intro_run, body_width))),
    ];

    // ── Section 4: uzor-graph's 4 layout looks, real vector TypesetFigures
    // (killed the graph-exhibit raster shakal — see GraphExhibit's own
    // doc comment above for the mechanism) ──────────────────────────────
    const GRAPH_DISPLAY_H: f64 = 240.0;

    let force_directed_exhibit = build_force_directed_exhibit();
    let hierarchical_exhibit = build_hierarchical_exhibit();
    let radial_exhibit = build_radial_exhibit();
    let collapsed_exhibit = build_collapsed_cluster_exhibit();

    let force_directed_figure = FigureBlock::new(&force_directed_exhibit, BlockSizing::FixedHeight(GRAPH_DISPLAY_H));
    let hierarchical_figure = FigureBlock::new(&hierarchical_exhibit, BlockSizing::FixedHeight(GRAPH_DISPLAY_H));
    let radial_figure = FigureBlock::new(&radial_exhibit, BlockSizing::FixedHeight(GRAPH_DISPLAY_H));
    let collapsed_figure = FigureBlock::new(&collapsed_exhibit, BlockSizing::FixedHeight(GRAPH_DISPLAY_H));

    let graph_a_heading_run = [StyledRun::new("Graph Exhibits — Force-Directed & Hierarchical", SUBHEADING_FONT)];
    let graph_b_heading_run = [StyledRun::new("Graph Exhibits — Radial & Collapsed-Cluster", SUBHEADING_FONT)];
    let force_caption_run = [StyledRun::new("Exhibit 1 — force-directed layout (uzor-graph).", CAPTION_FONT)];
    let hierarchical_caption_run = [StyledRun::new("Exhibit 2 — hierarchical layout (uzor-graph).", CAPTION_FONT)];
    let radial_caption_run = [StyledRun::new("Exhibit 3 — radial layout (uzor-graph).", CAPTION_FONT)];
    let collapsed_caption_run = [StyledRun::new("Exhibit 4 — collapsed-cluster layout (uzor-graph).", CAPTION_FONT)];

    let flow_graphs: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_a_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(1, "Graph Exhibits"),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(force_directed_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&force_caption_run, body_width))),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(hierarchical_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&hierarchical_caption_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_b_heading_run, body_width))).with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(radial_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&radial_caption_run, body_width))),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(collapsed_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&collapsed_caption_run, body_width))),
    ];

    // ── Section 5: ASCII exhibit + kinetics morph strip ─────────────────
    // Both exhibits are now real `TypesetFigure`s (vector cutover — see
    // the "uzor-text ASCII + kinetics exhibits" doc comment above), each
    // sized TIGHTLY to its own content via its own returned height — no
    // `ImageFit::Contain` letterboxing/aspect-fitting concern left to
    // solve at all (a `Block::Figure`'s placed rect always spans the
    // full region width already).
    let (ascii_exhibit, ascii_content_h) = build_ascii_exhibit(body_width.round() as u32);
    let ascii_figure = FigureBlock::new(&ascii_exhibit, BlockSizing::FixedHeight(ascii_content_h));

    let (kinetics_t0_exhibit, kinetics_t0_h) = build_kinetics_exhibit(0.0);
    let (kinetics_t05_exhibit, kinetics_t05_h) = build_kinetics_exhibit(0.5);
    let (kinetics_t1_exhibit, kinetics_t1_h) = build_kinetics_exhibit(1.0);
    let kinetics_t0_figure = FigureBlock::new(&kinetics_t0_exhibit, BlockSizing::FixedHeight(kinetics_t0_h));
    let kinetics_t05_figure = FigureBlock::new(&kinetics_t05_exhibit, BlockSizing::FixedHeight(kinetics_t05_h));
    let kinetics_t1_figure = FigureBlock::new(&kinetics_t1_exhibit, BlockSizing::FixedHeight(kinetics_t1_h));

    let ascii_heading_run = [StyledRun::new("ASCII Text Mode", SUBHEADING_FONT)];
    let ascii_caption_run = [StyledRun::new("Exhibit 5 — ASCII-grid text rendering (uzor-text::ascii).", CAPTION_FONT)];
    let kinetics_heading_run = [StyledRun::new("Kinetics — Paragraph Morph", SUBHEADING_FONT)];
    let kinetics_t0_caption_run = [StyledRun::new("Kinetics exhibit — t = 0.0 (narrow).", CAPTION_FONT)];
    let kinetics_t05_caption_run = [StyledRun::new("Kinetics exhibit — t = 0.5 (mid-flight).", CAPTION_FONT)];
    let kinetics_t1_caption_run = [StyledRun::new("Kinetics exhibit — t = 1.0 (wide).", CAPTION_FONT)];

    let flow_ascii_kinetics: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&ascii_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(1, "ASCII Text Mode"),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(ascii_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&ascii_caption_run, body_width))),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_heading_run, body_width)))
            .with_break_control(BreakControl::AvoidAfter)
            .with_outline(2, "Kinetics — Paragraph Morph"),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Figure(kinetics_t0_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(4.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_t0_caption_run, body_width))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(kinetics_t05_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(4.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_t05_caption_run, body_width))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(kinetics_t1_figure)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(4.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_t1_caption_run, body_width))),
    ];

    // ── Section 6a: layout — centered anchored island, text both sides ──
    const ISLAND_CENTER_W: f64 = 200.0;
    const ISLAND_CENTER_MARGIN: f64 = 10.0;
    let strip_width = (body_width - ISLAND_CENTER_W - 2.0 * ISLAND_CENTER_MARGIN) / 2.0;

    const STRIP_A: &str = "Every exhibit in this section anchors an image directly in the reading flow.";
    const STRIP_B: &str = "Text immediately following the island wraps into a narrow strip beside it.";
    const STRIP_C: &str = "A centered island fills its left strip first, then its right strip.";
    const STRIP_D: &str = "Both strips share the same width, so a paragraph could span either cleanly.";
    const BELOW: &str = "Once the centered island's own vertical band ends, the remaining narrative resumes \
        at the FULL body width below it, exactly as an ordinary paragraph would — no strip constraint carries \
        past the image's own bottom edge, and this line spans the entire page body from margin to margin.";

    let strip_a_run = [StyledRun::new(STRIP_A, BODY_FONT)];
    let strip_b_run = [StyledRun::new(STRIP_B, BODY_FONT)];
    let strip_c_run = [StyledRun::new(STRIP_C, BODY_FONT)];
    let strip_d_run = [StyledRun::new(STRIP_D, BODY_FONT)];
    let below_run = [StyledRun::new(BELOW, BODY_FONT)];

    let h_a = uzor_text::layout_paragraph(&Paragraph::new(&strip_a_run, strip_width), &shaper).height;
    let h_b = uzor_text::layout_paragraph(&Paragraph::new(&strip_b_run, strip_width), &shaper).height;
    let h_c = uzor_text::layout_paragraph(&Paragraph::new(&strip_c_run, strip_width), &shaper).height;
    let h_d = uzor_text::layout_paragraph(&Paragraph::new(&strip_d_run, strip_width), &shaper).height;
    let island_center_height = (h_a + 10.0 + h_b).max(h_c + 10.0 + h_d) + 4.0;

    let (center_photo, center_photo_w, center_photo_h) =
        generated_photo_rgba(ISLAND_CENTER_W.round() as u32, island_center_height.round().max(1.0) as u32, 0);
    let center_island = AnchoredIsland::new(
        ImageBlock::new(&center_photo, center_photo_w, center_photo_h, BlockSizing::FixedHeight(island_center_height), ImageFit::Cover),
        IslandAnchor::Center,
        ISLAND_CENTER_W,
        ISLAND_CENTER_MARGIN,
    );

    let island_center_title_run = [StyledRun::new("Layout — Centered Island, Text Both Sides", SUBHEADING_FONT)];

    let flow_island_center: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&island_center_title_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(1, "Layout — Anchored Islands"),
        BlockNode::new(Block::Spacer(14.0)),
        BlockNode::new(Block::Island(center_island)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&strip_a_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&strip_b_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&strip_c_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&strip_d_run, body_width))),
        BlockNode::new(Block::Spacer(16.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&below_run, body_width))),
    ];

    // ── Section 6b: layout — left- and right-anchored islands ───────────
    const ISLAND_SIDE_W: f64 = 170.0;
    const ISLAND_SIDE_MARGIN: f64 = 10.0;
    const ISLAND_SIDE_H: f64 = 170.0;

    let (left_photo, left_photo_w, left_photo_h) = generated_photo_rgba(170, 170, 1);
    let (right_photo, right_photo_w, right_photo_h) = generated_photo_rgba(170, 170, 2);
    let island_left = AnchoredIsland::new(
        ImageBlock::new(&left_photo, left_photo_w, left_photo_h, BlockSizing::FixedHeight(ISLAND_SIDE_H), ImageFit::Cover),
        IslandAnchor::Left,
        ISLAND_SIDE_W,
        ISLAND_SIDE_MARGIN,
    );
    let island_right = AnchoredIsland::new(
        ImageBlock::new(&right_photo, right_photo_w, right_photo_h, BlockSizing::FixedHeight(ISLAND_SIDE_H), ImageFit::Cover),
        IslandAnchor::Right,
        ISLAND_SIDE_W,
        ISLAND_SIDE_MARGIN,
    );

    let island_side_title_run = [StyledRun::new("Layout — Left- and Right-Anchored Islands", SUBHEADING_FONT)];
    const BESIDE_LEFT: &str = "This exhibit's own image anchors at the page's left margin, with text wrapping into a strip on its right.";
    const AFTER_LEFT: &str = "Once the left-anchored image's own band ends, the narrative resumes at the full page width below it.";
    const BESIDE_RIGHT: &str = "This second exhibit anchors its image to the right margin instead, so text wraps into a strip on its left.";
    const AFTER_RIGHT: &str = "And, symmetrically, once the right-anchored image's own band ends, the narrative again resumes at full width.";

    let beside_left_run = [StyledRun::new(BESIDE_LEFT, BODY_FONT)];
    let after_left_run = [StyledRun::new(AFTER_LEFT, BODY_FONT)];
    let beside_right_run = [StyledRun::new(BESIDE_RIGHT, BODY_FONT)];
    let after_right_run = [StyledRun::new(AFTER_RIGHT, BODY_FONT)];

    let flow_island_sides: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&island_side_title_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(2, "Layout — Left- and Right-Anchored Islands"),
        BlockNode::new(Block::Spacer(14.0)),
        BlockNode::new(Block::Island(island_left)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&beside_left_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&after_left_run, body_width))),
        BlockNode::new(Block::Spacer(24.0)),
        BlockNode::new(Block::Island(island_right)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&beside_right_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&after_right_run, body_width))),
    ];

    // ── Section 7: data page — padded table + bulleted list ─────────────
    let table_heading_run = [StyledRun::new("Summary — Engine Family at a Glance", SUBHEADING_FONT)];
    let list_heading_run = [StyledRun::new("Capabilities Demonstrated in This Document", SUBHEADING_FONT)];

    let h_engine = [StyledRun::new("Engine", TABLE_HEADER_FONT)];
    let n_engine_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&h_engine, f64::MAX)))];
    let h_role = [StyledRun::new("Role", TABLE_HEADER_FONT)];
    let n_role_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&h_role, f64::MAX)))];
    let h_summary = [StyledRun::new("Summary", TABLE_HEADER_FONT)];
    let n_summary_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&h_summary, f64::MAX)))];
    let h_status = [StyledRun::new("Status", TABLE_HEADER_FONT)];
    let n_status_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&h_status, f64::MAX)))];

    let r1_engine = [StyledRun::new("uzor-typeset", TABLE_FONT)];
    let n_r1_engine = [BlockNode::new(Block::Paragraph(Paragraph::new(&r1_engine, f64::MAX)))];
    let r1_role = [StyledRun::new("Composition + slicing", TABLE_FONT)];
    let n_r1_role = [BlockNode::new(Block::Paragraph(Paragraph::new(&r1_role, f64::MAX)))];
    let r1_summary = [StyledRun::new("Flows blocks through regions, then slices frames into pages, slides, or build steps.", TABLE_FONT)];
    let n_r1_summary = [BlockNode::new(Block::Paragraph(Paragraph::new(&r1_summary, f64::MAX)))];
    let r1_status = [StyledRun::new("Active", TABLE_FONT)];
    let n_r1_status = [BlockNode::new(Block::Paragraph(Paragraph::new(&r1_status, f64::MAX)))];

    let r2_engine = [StyledRun::new("uzor-text", TABLE_FONT)];
    let n_r2_engine = [BlockNode::new(Block::Paragraph(Paragraph::new(&r2_engine, f64::MAX)))];
    let r2_role = [StyledRun::new("Paragraph layout", TABLE_FONT)];
    let n_r2_role = [BlockNode::new(Block::Paragraph(Paragraph::new(&r2_role, f64::MAX)))];
    let r2_summary = [StyledRun::new("Word-wrap, justification, hyphenation, Knuth-Plass breaking, ASCII rendering, glyph-level morphing.", TABLE_FONT)];
    let n_r2_summary = [BlockNode::new(Block::Paragraph(Paragraph::new(&r2_summary, f64::MAX)))];
    let r2_status = [StyledRun::new("Active", TABLE_FONT)];
    let n_r2_status = [BlockNode::new(Block::Paragraph(Paragraph::new(&r2_status, f64::MAX)))];

    let r3_engine = [StyledRun::new("uzor-figures", TABLE_FONT)];
    let n_r3_engine = [BlockNode::new(Block::Paragraph(Paragraph::new(&r3_engine, f64::MAX)))];
    let r3_role = [StyledRun::new("Chart rendering", TABLE_FONT)];
    let n_r3_role = [BlockNode::new(Block::Paragraph(Paragraph::new(&r3_role, f64::MAX)))];
    let r3_summary = [StyledRun::new("Bar, curve, histogram, timeline, and flow-diagram figures over a shared theme.", TABLE_FONT)];
    let n_r3_summary = [BlockNode::new(Block::Paragraph(Paragraph::new(&r3_summary, f64::MAX)))];
    let r3_status = [StyledRun::new("Active", TABLE_FONT)];
    let n_r3_status = [BlockNode::new(Block::Paragraph(Paragraph::new(&r3_status, f64::MAX)))];

    let r4_engine = [StyledRun::new("uzor-graph", TABLE_FONT)];
    let n_r4_engine = [BlockNode::new(Block::Paragraph(Paragraph::new(&r4_engine, f64::MAX)))];
    let r4_role = [StyledRun::new("Graph visualization", TABLE_FONT)];
    let n_r4_role = [BlockNode::new(Block::Paragraph(Paragraph::new(&r4_role, f64::MAX)))];
    let r4_summary = [StyledRun::new("Force-directed, hierarchical, radial, and cluster-collapsed layouts for node/edge graphs.", TABLE_FONT)];
    let n_r4_summary = [BlockNode::new(Block::Paragraph(Paragraph::new(&r4_summary, f64::MAX)))];
    let r4_status = [StyledRun::new("Active", TABLE_FONT)];
    let n_r4_status = [BlockNode::new(Block::Paragraph(Paragraph::new(&r4_status, f64::MAX)))];

    let r5_engine = [StyledRun::new("uzor-export", TABLE_FONT)];
    let n_r5_engine = [BlockNode::new(Block::Paragraph(Paragraph::new(&r5_engine, f64::MAX)))];
    let r5_role = [StyledRun::new("Headless render + export", TABLE_FONT)];
    let n_r5_role = [BlockNode::new(Block::Paragraph(Paragraph::new(&r5_role, f64::MAX)))];
    let r5_summary = [StyledRun::new("Offscreen PNG rasterization and hybrid-fidelity PDF assembly with real vector text.", TABLE_FONT)];
    let n_r5_summary = [BlockNode::new(Block::Paragraph(Paragraph::new(&r5_summary, f64::MAX)))];
    let r5_status = [StyledRun::new("Active", TABLE_FONT)];
    let n_r5_status = [BlockNode::new(Block::Paragraph(Paragraph::new(&r5_status, f64::MAX)))];

    let header_cells = [TableCell::new(&n_engine_h), TableCell::new(&n_role_h), TableCell::new(&n_summary_h), TableCell::new(&n_status_h)];
    let row1_cells = [TableCell::new(&n_r1_engine), TableCell::new(&n_r1_role), TableCell::new(&n_r1_summary), TableCell::new(&n_r1_status)];
    let row2_cells = [TableCell::new(&n_r2_engine), TableCell::new(&n_r2_role), TableCell::new(&n_r2_summary), TableCell::new(&n_r2_status)];
    let row3_cells = [TableCell::new(&n_r3_engine), TableCell::new(&n_r3_role), TableCell::new(&n_r3_summary), TableCell::new(&n_r3_status)];
    let row4_cells = [TableCell::new(&n_r4_engine), TableCell::new(&n_r4_role), TableCell::new(&n_r4_summary), TableCell::new(&n_r4_status)];
    let row5_cells = [TableCell::new(&n_r5_engine), TableCell::new(&n_r5_role), TableCell::new(&n_r5_summary), TableCell::new(&n_r5_status)];
    let table_rows = [
        TableRow::new(&header_cells),
        TableRow::new(&row1_cells),
        TableRow::new(&row2_cells),
        TableRow::new(&row3_cells),
        TableRow::new(&row4_cells),
        TableRow::new(&row5_cells),
    ];
    let table_columns = [ColumnSpec::Fixed(96.0), ColumnSpec::Fixed(122.0), ColumnSpec::Fraction(1.0), ColumnSpec::Fixed(60.0)];
    let table = TableBlock::new(&table_columns, &table_rows).with_cell_padding(CellPadding::new(10.0, 6.0));

    let li1_run = [StyledRun::new("Multi-page composition with a header, a footer, and running page numbers.", BODY_FONT)];
    let li1_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li1_run, f64::MAX)))];
    let li2_run = [StyledRun::new("Single- and multi-column body layout within one continuously numbered document.", BODY_FONT)];
    let li2_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li2_run, f64::MAX)))];
    let li3_run = [StyledRun::new("The full figure family: bar, time-series curve, histogram, timeline, and flow diagram.", BODY_FONT)];
    let li3_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li3_run, f64::MAX)))];
    let li4_run = [StyledRun::new("Force-directed, hierarchical, radial, and cluster-collapsed graph layouts.", BODY_FONT)];
    let li4_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li4_run, f64::MAX)))];
    let li5_run = [StyledRun::new("ASCII-grid text rendering and glyph-level paragraph kinetics.", BODY_FONT)];
    let li5_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li5_run, f64::MAX)))];
    let li6_run = [StyledRun::new("Images anchored in the flow with wrapping body text on either side.", BODY_FONT)];
    let li6_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li6_run, f64::MAX)))];

    // Typography-gap WAVE 3: a 7th top-level item whose OWN content is
    // ANOTHER list (roman-numbered) — nested lists (see `scene::list`'s own
    // module doc: this mechanism already worked structurally before this
    // wave, this item exists to SHOW it, per the task's own showcase gate).
    let li7_run = [StyledRun::new("Nested list numbering styles, demonstrated by this very sub-list:", BODY_FONT)];
    let nested_a_run = [StyledRun::new("Decimal, alpha, and roman schemes, selectable per list.", BODY_FONT)];
    let nested_a_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&nested_a_run, f64::MAX)))];
    let nested_b_run = [StyledRun::new("Indentation compounds automatically with nesting depth.", BODY_FONT)];
    let nested_b_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&nested_b_run, f64::MAX)))];
    let nested_items = [ListItem::new(&nested_a_nodes), ListItem::new(&nested_b_nodes)];
    let nested_list = ListBlock::new(&nested_items, MarkerStyle::Numbered { start: 1, scheme: NumberScheme::LowerRoman }, 20.0);
    let li7_nodes = [
        BlockNode::new(Block::Paragraph(Paragraph::new(&li7_run, f64::MAX))),
        BlockNode::new(Block::Spacer(4.0)),
        BlockNode::new(Block::List(nested_list)),
    ];

    let list_items = [
        ListItem::new(&li1_nodes),
        ListItem::new(&li2_nodes),
        ListItem::new(&li3_nodes),
        ListItem::new(&li4_nodes),
        ListItem::new(&li5_nodes),
        ListItem::new(&li6_nodes),
        ListItem::new(&li7_nodes),
    ];

    let flow_data: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&table_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(1, "Summary — Engine Family at a Glance"),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Table(table)).with_caption(CaptionKind::Table, "engine family summary — role, capability, and status per crate."),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&list_heading_run, body_width)))
            .with_break_control(BreakControl::AvoidAfter)
            .with_outline(2, "Capabilities Demonstrated in This Document"),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::List(ListBlock::new(&list_items, MarkerStyle::Bullet('•'), 20.0))),
    ];

    // ── Section: Footnotes (typography-gap WAVE 3) — own dedicated master
    // (`with_footnote_zone`, reserving bottom-of-page space on every page
    // this ONE section slices), sliced independently and concatenated
    // below, matching the SAME "separate master per section" pattern this
    // fixture already uses for the two-column section — never applied
    // globally to `master_single`, so every other section's own pagination
    // stays byte-identical to before this wave.
    let footnote_heading_run = [StyledRun::new("Footnotes", SUBHEADING_FONT)];
    const FOOTNOTE_INTRO: &str = "This engine reserves a fixed band at the bottom of every page for numbered \
        footnotes, separated from the body by a short rule, with numbering continuing across the whole document.";
    let footnote_intro_run = [StyledRun::new(FOOTNOTE_INTRO, BODY_FONT)];

    let fn1_body_run = [StyledRun::new("A seeded footnote proving the reserved bottom-of-page zone composes real content.", BODY_FONT)];
    let fn1_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&fn1_body_run, f64::MAX)))];
    let fn2_body_run = [StyledRun::new("A second footnote on the same paragraph, numbered continuously after the first.", BODY_FONT)];
    let fn2_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&fn2_body_run, f64::MAX)))];
    let footnotes_demo = [Footnote::new(&fn1_nodes), Footnote::new(&fn2_nodes)];

    let fn_body_a = StyledRun::new("Reserved bottom-of-page space for footnotes", BODY_FONT);
    let fn_marker_1 = StyledRun::new("1", CAPTION_FONT).with_vertical_align(VerticalAlign::Super);
    let fn_body_b = StyledRun::new(" is a real engine mechanism", BODY_FONT);
    let fn_marker_2 = StyledRun::new("2", CAPTION_FONT).with_vertical_align(VerticalAlign::Super);
    let fn_body_c = StyledRun::new(", not a hand-typed footer line.", BODY_FONT);
    let fn_runs = [fn_body_a, fn_marker_1, fn_body_b, fn_marker_2, fn_body_c];
    let fn_slots = [InlineBoxSlot::new(1, "1".len(), InlineBox::out_of_flow(0)), InlineBoxSlot::new(3, "2".len(), InlineBox::out_of_flow(1))];
    let fn_paragraph = Paragraph::new(&fn_runs, body_width).with_inline_boxes(&fn_slots);
    let fn_marked_node = BlockNode::new(Block::Paragraph(fn_paragraph)).with_footnotes(&footnotes_demo);

    let master_footnotes = PageMaster::new(PAGE_W, PAGE_H, Margins::uniform(40.0))
        .with_header(&header_flow)
        .with_footer(&footer_flow)
        .with_page_number(page_number_style)
        .with_footnote_zone(90.0);
    let flow_footnotes: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&footnote_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore)
            .with_outline(1, "Footnotes"),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&footnote_intro_run, body_width))),
        BlockNode::new(Block::Spacer(14.0)),
        fn_marked_node,
    ];
    let pages_footnotes = slice_pages(&flow_footnotes, &master_footnotes, &style_single, &shaper);
    assert!(pages_footnotes[0].footnotes.is_some(), "the footnote demo page must carry a real, non-empty footnote zone");

    // ── slice every single-column section as ONE flow (ForceBefore pins
    // each exhibit group to its own fresh page), then concatenate with the
    // separately-sliced two-column section and renumber the whole thing ──
    let mut flow_rest: Vec<BlockNode<'_>> = Vec::new();
    flow_rest.extend(flow_figures);
    flow_rest.extend(flow_graphs);
    flow_rest.extend(flow_ascii_kinetics);
    flow_rest.extend(flow_island_center);
    flow_rest.extend(flow_island_sides);
    flow_rest.extend(flow_data);
    // Typography-gap WAVE 3: resolve every `.with_caption()`-tagged node in
    // this concatenated flow into a real, auto-numbered caption paragraph
    // BEFORE composing — see `crate::caption`'s own module doc for why this
    // needs no fixpoint (a pure, single-pass function of flow order).
    let flow_rest = attach_captions(&flow_rest, &caption_style);
    let pages_rest = slice_pages(&flow_rest, &master_single, &style_single, &shaper);

    // ── Document navigation: a generated table of contents inserted
    // AFTER the opener (task's own explicit ask), covering every
    // `.with_outline()`-tagged heading in the two-column section + the
    // rest — never the opener itself (a TOC conventionally lists what
    // follows it, not the cover/intro page it sits on). `compose_document_
    // with_toc`'s own bounded 2-3-pass fixpoint (`crate::toc`) resolves
    // each heading's FINAL global page number (accounting for however
    // many pages the TOC itself ends up occupying) before the TOC's own
    // rows are ever painted.
    let mut body_after_opener: Vec<Page<'_>> = Vec::new();
    body_after_opener.extend(pages_two_col);
    body_after_opener.extend(pages_rest);
    body_after_opener.extend(pages_footnotes);

    let toc_style = TocStyle { font: BODY_FONT, dot_char: '.', level_indent_px: 16.0, row_gap_px: 10.0 };
    let with_toc = compose_document_with_toc(body_after_opener, &master_single, &toc_style, &shaper, Some(&page_number_style));

    let mut all_pages: Vec<Page<'_>> = Vec::new();
    all_pages.extend(pages_opener);
    all_pages.extend(with_toc);
    let all_pages = renumber_pages(all_pages, Some(&page_number_style));

    assert!(all_pages.len() >= 9, "the showcase must span at least the 9 distinct single-column exhibit groups plus the opener/two-column sections, got {} pages", all_pages.len());
    for page in &all_pages {
        assert!(page.header.as_ref().is_some_and(|h| !h.blocks.is_empty()), "every page must carry the shared header");
        assert!(page.footer.as_ref().is_some_and(|f| !f.blocks.is_empty()), "every page must carry the shared footer");
        assert!(page.page_number.is_some(), "every page must carry a page number");
    }
    for (i, page) in all_pages.iter().enumerate() {
        let expected = format!("{} of {}", i + 1, all_pages.len());
        assert_eq!(page.page_number.as_ref().expect("checked above").text, expected, "page numbers must be continuous across the concatenated sections");
    }

    // Every outline entry's own resolved page must match a REAL page in
    // the final document, and its 1-based number must be exactly what the
    // TOC's own generated row prints (proves the fixpoint actually
    // converged to the FINAL page positions, not a stale pre-TOC guess).
    let all_outline_entries: Vec<_> = all_pages.iter().flat_map(|p| p.outline.iter()).collect();
    assert!(all_outline_entries.len() >= 8, "the showcase must carry every one of its own tagged section headings, got {}", all_outline_entries.len());
    for entry in &all_outline_entries {
        assert!((entry.page_index as usize) < all_pages.len(), "every outline entry's own page_index must reference a real page in the final document");
    }

    // TOC page-number accuracy (task gate: "TOC page numbers match actual
    // pages") — verified as DATA, not by eyeballing: every generated TOC
    // row (a `Block::Paragraph` that is BOTH the source of a
    // `Page::links` entry AND carries a real `paragraph_layout`) must
    // PAINT the exact 1-based page number its own `LinkEntry::target_page`
    // resolves to in this FINAL, fully-renumbered document.
    let mut checked_toc_rows = 0usize;
    for page in &all_pages {
        for link in &page.links {
            let row_text: String = page
                .frame
                .blocks
                .iter()
                .find(|b| b.rect == link.rect)
                .and_then(|b| b.paragraph_layout.as_ref())
                .map(|layout| layout.glyphs.iter().map(|g| g.cluster.as_str()).collect::<String>())
                .expect("every link-tagged TOC row must have a matching placed paragraph with a real layout");
            let expected_number = (link.target_page + 1).to_string();
            assert!(
                row_text.ends_with(&expected_number) || row_text.contains(&format!("{expected_number} ")),
                "TOC row {row_text:?} must print its own target page's FINAL number {expected_number:?}"
            );
            checked_toc_rows += 1;
        }
    }
    assert_eq!(checked_toc_rows, all_outline_entries.len(), "every tagged heading must have a corresponding, correctly-numbered TOC row");

    let pdf_bytes = pages_to_pdf(&all_pages, &master_single, &theme);
    assert!(pdf_bytes.starts_with(b"%PDF-"));
    write_proof("typeset_showcase.pdf", &pdf_bytes);

    let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse the showcase's own PDF output");
    let lopdf_pages = doc.get_pages();
    assert_eq!(lopdf_pages.len(), all_pages.len(), "the PDF must carry every composed page");

    // Document-navigation feature pass: the PDF's own `/Outlines` tree is
    // present and structurally sound (every entry's own page target
    // resolves to a real page object) — the outline tree itself is built
    // entirely from `Page::outline`, wired automatically by `pages_to_pdf`
    // (no extra call needed at this call site).
    let catalog = doc.catalog().expect("catalog must be present");
    let outlines_ref = catalog.get(b"Outlines").and_then(lopdf::Object::as_reference).expect("the showcase's own tagged headings must produce a real /Outlines entry");
    let outlines_dict = doc.get_dictionary(outlines_ref).expect("must resolve the /Outlines dict");
    assert!(outlines_dict.get(b"First").is_ok(), "the /Outlines root must have at least one top-level item");

    // Internal links: every TOC row is a real `/Subtype /Link` annotation
    // with a `GoTo` `/Dest` pointing at a real page object (proves
    // `crate::toc::build_toc`'s own `.with_link_target()` tagging survives
    // all the way through `slice_pages`'s collection pass into the PDF).
    let mut total_link_annots = 0usize;
    for (_, page_id) in doc.get_pages() {
        let page_dict = doc.get_dictionary(page_id).expect("must resolve a page dict");
        if let Ok(annots) = page_dict.get(b"Annots").and_then(lopdf::Object::as_array) {
            for annot_ref in annots {
                let annot_ref = annot_ref.as_reference().expect("annotation must be an indirect reference");
                let annot = doc.get_dictionary(annot_ref).expect("must resolve the annotation dict");
                assert_eq!(annot.get(b"Subtype").and_then(|o| o.as_name()).expect("/Subtype must be present"), b"Link".as_slice());
                let action = annot.get(b"A").and_then(lopdf::Object::as_dict).expect("a Link annotation must carry an /A action dict");
                assert_eq!(action.get(b"S").and_then(|o| o.as_name()).expect("/S must be present"), b"GoTo".as_slice());
                let dest = action.get(b"D").and_then(lopdf::Object::as_array).expect("a GoTo action must carry a /D destination array");
                dest[0].as_reference().expect("the destination's first item must be a real page reference");
                total_link_annots += 1;
            }
        }
    }
    assert!(total_link_annots >= 8, "every TOC row must be a real clickable link annotation, got {total_link_annots}");

    let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
    let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
    for word in ["uzor-typeset", "uzor-graph", "uzor-figures", "Active", "Showcase"] {
        assert!(extracted.contains(word), "extracted PDF text must contain {word:?} — got a document of {} chars", extracted.len());
    }
    // A Cyrillic OUTLINE title (not just Cyrillic BODY text, already
    // proven below) round-trips through the PDF bookmark tree's own
    // `/Title` — the "incl. the Russian one" outline entry this fixture's
    // own two-column heading carries.
    let outline_titles_extracted = {
        fn collect_titles(doc: &lopdf::Document, dict: &lopdf::Dictionary, out: &mut Vec<String>) {
            let Ok(first_ref) = dict.get(b"First").and_then(lopdf::Object::as_reference) else { return };
            let mut current = Some(first_ref);
            while let Some(item_ref) = current {
                let Ok(item) = doc.get_dictionary(item_ref) else { break };
                if let Ok(bytes) = item.get(b"Title").and_then(|o| o.as_str()) {
                    let text = if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                        String::from_utf16_lossy(&units)
                    } else {
                        String::from_utf8_lossy(bytes).into_owned()
                    };
                    out.push(text);
                }
                collect_titles(doc, item, out);
                current = item.get(b"Next").and_then(lopdf::Object::as_reference).ok();
            }
        }
        let mut titles = Vec::new();
        collect_titles(&doc, outlines_dict, &mut titles);
        titles
    };
    assert!(
        outline_titles_extracted.iter().any(|t| t.contains("Компоновка и разбиение")),
        "a Cyrillic outline title must round-trip through the PDF bookmark tree, got {outline_titles_extracted:?}"
    );
    // Cyrillic end-to-end: the Russian intro sentence's own SHORTER words
    // (never hyphenated at this column width) must round-trip verbatim
    // (export SOTA pass — see this fixture's own `INTRO_RU` comment).
    // `"кириллического"` (the longest word in the sentence) is
    // deliberately NOT checked here anymore: since the typography quality
    // wave turned on real `Hyphenation::Russian` for this paragraph, that
    // word now genuinely breaks across two lines with a real hyphen glyph
    // between them (the whole point of the feature — see the
    // `russian_paragraph_hyphenates` assertion above, which checks that
    // directly against the composed layout) — it would no longer extract
    // as one contiguous PDF-text substring, which is CORRECT, not a
    // regression.
    for word in ["показательный", "документ", "экспорте"] {
        assert!(extracted.contains(word), "extracted PDF text must contain Cyrillic word {word:?} — got a document of {} chars", extracted.len());
    }

    // Raster parity reference (the task's own gate) — one PNG per page,
    // painted via the ordinary `draw_page` (every layer, unsuppressed) at
    // the SAME physical page size.
    let spec = ExportSpec { width_px: PAGE_W as u32, height_px: PAGE_H as u32, dpr: 1.0, background: Some([255, 255, 255, 255]) };
    for (i, page) in all_pages.iter().enumerate() {
        let bytes = render_to_png(&spec, |ctx| crate::render::draw_page(ctx, page, &theme)).expect("parity PNG render should succeed");
        assert_eq!(decoded_png_dims(&bytes), (PAGE_W as u32, PAGE_H as u32));
        write_proof(&format!("typeset_showcase_page{}.png", i + 1), &bytes);
    }
}

/// Typography quality wave — dedicated proof (task's own explicit ask,
/// separate from the full showcase above): one physical A4 page, a
/// GENUINELY narrow text column (asymmetric margins carve a ~205px body
/// out of the 595px-wide page — a real magazine-style narrow column, not
/// the showcase's own full-width or 2-column body), carrying an English
/// paragraph AND a Russian paragraph, both `Justify` + `KnuthPlass`, each
/// with its own real `hypher`-backed [`Hyphenation`] — proving both
/// languages' hyphenation is visibly breaking words at this narrow width,
/// not just type-checking.
#[test]
fn typography_wave_ru_and_en_hyphenation_in_a_narrow_justified_column() {
    let theme = Theme::light_report();
    let shaper = CosmicShaper::headless();

    // Asymmetric margins: a genuinely narrow (~205px) body column on an
    // otherwise full A4 (595x842) physical page.
    let margins = Margins::new(50.0, 350.0, 50.0, 40.0);
    let master = PageMaster::new(PAGE_W, PAGE_H, margins);
    let body_width = master.body_rect().width;
    assert!(body_width < 220.0, "fixture must be a genuinely narrow column");

    let heading_run = [StyledRun::new("Typography Quality Wave — Hyphenation Proof", HEADING_FONT)];

    const EN_TEXT: &str = "A narrow justified column of business presentation text quickly exposes uneven \
        interword spacing unless real hyphenation supplies a mid-word breakpoint near the right margin, \
        exactly the kind of long compound vocabulary — implementation, infrastructure, reproducibility, \
        categorical — a narrow column is well suited to demonstrate.";
    let en_run = [StyledRun::new(EN_TEXT, BODY_FONT)];

    const RU_TEXT: &str = "Показательный документ подтверждает поддержку кириллического текста, а узкая \
        колонка оправданного текста быстро показывает неравномерные промежутки между словами, если \
        настоящая расстановка переносов не подсказывает точку разрыва посреди длинного слова.";
    let ru_run = [StyledRun::new(RU_TEXT, BODY_FONT)];

    let style = ComposeStyle::from_theme(&theme, 10.0);
    let flow = [
        BlockNode::new(Block::Paragraph(Paragraph::new(&heading_run, body_width))),
        BlockNode::new(Block::Spacer(14.0)),
        BlockNode::new(Block::Paragraph(
            Paragraph::new(&en_run, body_width)
                .with_align(ParagraphAlign::Justify)
                .with_break_strategy(BreakStrategy::KnuthPlass)
                .with_hyphenation(Hyphenation::English),
        )),
        BlockNode::new(Block::Spacer(14.0)),
        BlockNode::new(Block::Paragraph(
            Paragraph::new(&ru_run, body_width)
                .with_align(ParagraphAlign::Justify)
                .with_break_strategy(BreakStrategy::KnuthPlass)
                .with_hyphenation(Hyphenation::Russian),
        )),
    ];

    let pages = slice_pages(&flow, &master, &style, &shaper);
    assert_eq!(pages.len(), 1, "this fixture's content must fit on ONE A4 page");
    let page = &pages[0];

    // Data-level proof (not just eyeballing): a real "-" glyph appears in
    // BOTH the English (pure-ASCII) paragraph and the Russian (Cyrillic)
    // paragraph's own placed layout — the Cyrillic check distinguishes the
    // two paragraphs unambiguously (only the Russian one has Cyrillic
    // glyphs at all).
    let mut ascii_hyphenates = false;
    let mut cyrillic_hyphenates = false;
    for block in &page.frame.blocks {
        if let Some(layout) = &block.paragraph_layout {
            let is_cyrillic = layout.glyphs.iter().any(|g| g.cluster.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c)));
            let has_hyphen = layout.glyphs.iter().any(|g| g.cluster == "-");
            if has_hyphen && is_cyrillic {
                cyrillic_hyphenates = true;
            } else if has_hyphen {
                ascii_hyphenates = true;
            }
        }
    }
    assert!(ascii_hyphenates, "the English paragraph must show a real discretionary hyphen break in this narrow column");
    assert!(cyrillic_hyphenates, "the Russian paragraph must show a real discretionary hyphen break in this narrow column");

    let spec = ExportSpec { width_px: PAGE_W as u32, height_px: PAGE_H as u32, dpr: 1.0, background: Some([255, 255, 255, 255]) };
    let bytes = render_to_png(&spec, |ctx| crate::render::draw_page(ctx, page, &theme)).expect("parity PNG render should succeed");
    assert_eq!(decoded_png_dims(&bytes), (PAGE_W as u32, PAGE_H as u32));
    write_proof("typography_wave.png", &bytes);
}

/// Raster-shakal gate (task's own explicit ask): a page carrying ONLY a
/// [`GraphExhibit`] must show NO `/XObject` at all (the old
/// `render_rgba`-into-`ImageBlock` bridge every graph exhibit used is
/// gone — `Block::Figure` never touches the raster/XObject path) AND its
/// content stream must carry real vector path/stroke/fill operators
/// (`GraphExhibit::render` paints edges/nodes straight through
/// `PdfRenderContext`'s own `Painter`/`BatchPainter` primitives — edges
/// stroke ("S"), node circles fill ("f") via bezier-approximated arcs
/// ("c")).
#[test]
fn graph_exhibit_page_carries_real_vector_ops_and_no_image_xobject() {
    let theme = Theme::light_report();
    let shaper = CosmicShaper::headless();
    let master = PageMaster::new(PAGE_W, PAGE_H, Margins::uniform(40.0));
    let body_width = master.body_rect().width;

    let exhibit = build_force_directed_exhibit();
    let figure = FigureBlock::new(&exhibit, BlockSizing::FixedHeight(240.0));
    let flow = [BlockNode::new(Block::Figure(figure))];

    let style = ComposeStyle::from_theme(&theme, 12.0);
    let pages = slice_pages(&flow, &master, &style, &shaper);
    assert_eq!(pages.len(), 1, "a single graph exhibit must land on exactly 1 page");
    assert!(body_width > 0.0, "sanity: the page must have a real body width");

    let pdf_bytes = pages_to_pdf(&pages, &master, &theme);
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");
    let lopdf_pages = doc.get_pages();
    assert_eq!(lopdf_pages.len(), 1);
    let page_id = *lopdf_pages.values().next().expect("exactly one page");

    let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources should succeed");
    let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
    assert!(!has_xobjects, "a graph-exhibit-only page must carry NO image XObject — the render_rgba-into-ImageBlock bridge is gone for graph exhibits");

    let content_bytes = doc.get_page_content(page_id);
    let content_str = String::from_utf8_lossy(&content_bytes);
    let toks: Vec<&str> = content_str.split_whitespace().collect();
    assert!(toks.iter().any(|&t| t == "S"), "the graph's own edges must be real stroke 'S' operators, got: {content_str}");
    assert!(toks.iter().any(|&t| t == "f"), "the graph's own node circles must be real fill 'f' operators, got: {content_str}");
    assert!(toks.iter().any(|&t| t == "c"), "the graph's own node circles are bezier-approximated arcs, must carry a 'c' curve operator, got: {content_str}");
}

/// Raster-shakal gate, ASCII exhibit half (owner's own follow-up report:
/// the ASCII-grid exhibit was still a sampled `Block::Image` — pixel mush
/// at PDF zoom — even after the graph-exhibit vector cutover above). A
/// page carrying ONLY an [`AsciiExhibit`] must show NO `/XObject` at all
/// and its content stream must carry a real `Tj` text-showing operator
/// (every non-blank grid cell/row paints via `PdfRenderContext::
/// fill_text`, never a pixel write).
#[test]
fn ascii_exhibit_page_carries_real_text_ops_and_no_image_xobject() {
    let theme = Theme::light_report();
    let shaper = CosmicShaper::headless();
    let master = PageMaster::new(PAGE_W, PAGE_H, Margins::uniform(40.0));
    let body_width = master.body_rect().width;

    let (exhibit, content_h) = build_ascii_exhibit(body_width.round() as u32);
    let figure = FigureBlock::new(&exhibit, BlockSizing::FixedHeight(content_h));
    let flow = [BlockNode::new(Block::Figure(figure))];

    let style = ComposeStyle::from_theme(&theme, 12.0);
    let pages = slice_pages(&flow, &master, &style, &shaper);
    assert_eq!(pages.len(), 1, "a single ASCII exhibit must land on exactly 1 page");

    let pdf_bytes = pages_to_pdf(&pages, &master, &theme);
    let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");
    let lopdf_pages = doc.get_pages();
    assert_eq!(lopdf_pages.len(), 1);
    let page_id = *lopdf_pages.values().next().expect("exactly one page");

    let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources should succeed");
    let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
    assert!(!has_xobjects, "an ASCII-exhibit-only page must carry NO image XObject — the sampled-ImageBlock bridge is gone");

    let content_bytes = doc.get_page_content(page_id);
    let content_str = String::from_utf8_lossy(&content_bytes);
    assert!(content_str.contains("Tj"), "the ASCII grid's own cells must paint as real 'Tj' text-showing operators, got: {content_str}");
}

/// Raster-shakal gate, kinetics-morph exhibit half (same owner report as
/// the ASCII exhibit above). A page carrying ONLY a [`KineticsExhibit`]
/// must show NO `/XObject` at all and must carry a real `Tj` text-showing
/// operator (`uzor_text::draw_paragraph` paints every glyph via
/// `fill_text`).
#[test]
fn kinetics_exhibit_page_carries_real_text_ops_and_no_image_xobject() {
    let theme = Theme::light_report();
    let shaper = CosmicShaper::headless();
    let master = PageMaster::new(PAGE_W, PAGE_H, Margins::uniform(40.0));

    let (exhibit, content_h) = build_kinetics_exhibit(0.5);
    let figure = FigureBlock::new(&exhibit, BlockSizing::FixedHeight(content_h));
    let flow = [BlockNode::new(Block::Figure(figure))];

    let style = ComposeStyle::from_theme(&theme, 12.0);
    let pages = slice_pages(&flow, &master, &style, &shaper);
    assert_eq!(pages.len(), 1, "a single kinetics exhibit must land on exactly 1 page");

    let pdf_bytes = pages_to_pdf(&pages, &master, &theme);
    let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");
    let lopdf_pages = doc.get_pages();
    assert_eq!(lopdf_pages.len(), 1);
    let page_id = *lopdf_pages.values().next().expect("exactly one page");

    let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources should succeed");
    let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
    assert!(!has_xobjects, "a kinetics-exhibit-only page must carry NO image XObject — the sampled-ImageBlock bridge is gone");

    let content_bytes = doc.get_page_content(page_id);
    let content_str = String::from_utf8_lossy(&content_bytes);
    assert!(content_str.contains("Tj"), "the kinetics frame's own glyphs must paint as real 'Tj' text-showing operators, got: {content_str}");
}
