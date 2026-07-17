//! Full-capability showcase fixture (test-only) — ONE deterministic PDF
//! (`out/typeset_showcase.pdf`) exercising every capability this crate and
//! its sibling engines (`uzor-text`, `uzor-figures`, `uzor-graph`,
//! `uzor-export`) ship: multi-page composition with header/footer/page
//! numbers, single- AND two-column body layout, the full figure family,
//! all four `uzor-graph` layout looks (embedded as real `ImageBlock`s),
//! ASCII-grid text rendering, glyph-level paragraph kinetics, anchored-
//! island wrap layout, and a padded table + bulleted list.
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

use std::path::PathBuf;

use uzor::fonts::FontFamily;
use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec};
use uzor_figures::{BarFigure, CurveFigure, HistogramFigure, SankeyFigure, SankeyLink, SankeyNode, TimeScale, TimelineEvent, TimelineFigure};
use uzor_graph::{ForceDirectedLayout, Graph, GraphEngine, HierarchicalLayout, HierarchicalParams, NodeIndex, RadialLayout, RadialParams};
use uzor_text::ascii::{build_ascii_grid, draw_ascii_grid, AsciiGridStyle};
use uzor_text::{build_morph, draw_paragraph, layout_text, sample_layout, BreakStrategy, CosmicShaper, FontSpec, Hyphenation, Paragraph, ParagraphAlign, StyledRun};

use crate::compose::{BreakControl, ComposeStyle};
use crate::export::pages_to_pdf;
use crate::master::{PageNumberFormat, PageNumberStyle};
use crate::scene::{
    AnchoredIsland, Block, BlockNode, BlockSizing, CellPadding, ColumnSpec, FigureBlock, ImageBlock, ImageFit, IslandAnchor, ListBlock, ListItem,
    MarkerStyle, TableBlock, TableCell, TableRow,
};
use crate::slice::{slice_pages, Margins, Page, PageMaster};
use crate::style::Theme;

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

/// Decode `bytes` (a `render_to_png` result) back into a straight RGBA8
/// buffer + its own dimensions — every render this fixture embeds as an
/// [`ImageBlock`] is fully opaque (every `ExportSpec::background` used
/// below is `Some([.., 255])` and nothing painted is ever translucent), so
/// straight vs. premultiplied never matters here.
fn decode_rgba8(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().expect("valid PNG header");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("decode PNG frame");
    let (w, h) = (info.width, info.height);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        other => panic!("showcase fixture: unexpected PNG color type {other:?} decoding an internal render"),
    };
    (rgba, w, h)
}

/// Render `draw` headlessly at `width`x`height` and return a straight RGBA8
/// buffer — the shared bridge every `uzor-graph`/ASCII/kinetics exhibit
/// below uses to become a real [`ImageBlock`] (there is no
/// `render_to_rgba` in `uzor-export` — only `render_to_png` — so this
/// decodes the PNG straight back via the `png` crate, dev-dep already,
/// matching this workspace's own existing decode-back-out convention, e.g.
/// `crate::render::tests::decoded_png_pixel`).
fn render_rgba(width: u32, height: u32, background: [u8; 4], draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let spec = ExportSpec { width_px: width, height_px: height, dpr: 1.0, background: Some(background) };
    let bytes = render_to_png(&spec, draw).unwrap_or_else(|e| panic!("showcase fixture rgba render failed: {e}"));
    let (rgba, w, h) = decode_rgba8(&bytes);
    assert_eq!((w, h), (width, height), "decoded PNG dims must match the requested render size");
    rgba
}

/// A deterministic, generated "photo" RGBA buffer — layered gradients +
/// two circles, no binary asset, no RNG/time (matches
/// `crate::render::tests::generated_photo_rgba` verbatim — this fixture
/// keeps its own copy per this workspace's own established convention of
/// each proof module owning its small render/decode helpers rather than
/// sharing test-only code across `#[cfg(test)]` boundaries).
fn generated_photo_rgba(width: u32, height: u32, seed: u32) -> Vec<u8> {
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

            let d1 = ((x as f64 - cx1).powi(2) + (y as f64 - cy1).powi(2)).sqrt();
            if d1 < r1 {
                pixel = [235, 205, 90, 255];
            }
            let d2 = ((x as f64 - cx2).powi(2) + (y as f64 - cy2).powi(2)).sqrt();
            if d2 < r2 {
                pixel = [60, 140, 95, 255];
            }

            let idx = ((y as usize) * (width as usize) + (x as usize)) * 4;
            buf[idx..idx + 4].copy_from_slice(&pixel);
        }
    }
    buf
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

const GRAPH_BG: [u8; 4] = [13, 15, 20, 255];

fn force_directed_rgba(width: u32, height: u32) -> Vec<u8> {
    let (graph, _clusters) = build_cluster_graph();
    let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
    engine.seed_positions(&seed_cluster_positions());
    engine.set_canvas_rect(Rect::new(0.0, 0.0, width as f64, height as f64));
    for _ in 0..200 {
        engine.tick(1.0 / 60.0);
    }
    engine.fit_view();
    render_rgba(width, height, GRAPH_BG, |ctx| engine.draw(ctx))
}

fn hierarchical_rgba(width: u32, height: u32) -> Vec<u8> {
    let graph = build_tree_graph();
    let mut engine: GraphEngine<(), (), HierarchicalLayout> = GraphEngine::new(graph, HierarchicalLayout::new(HierarchicalParams::default()));
    engine.set_canvas_rect(Rect::new(0.0, 0.0, width as f64, height as f64));
    engine.tick(1.0 / 60.0);
    engine.fit_view();
    render_rgba(width, height, GRAPH_BG, |ctx| engine.draw(ctx))
}

fn radial_rgba(width: u32, height: u32) -> Vec<u8> {
    let graph = build_tree_graph();
    let mut engine: GraphEngine<(), (), RadialLayout> = GraphEngine::new(graph, RadialLayout::new(RadialParams::default()));
    engine.set_canvas_rect(Rect::new(0.0, 0.0, width as f64, height as f64));
    engine.tick(1.0 / 60.0);
    engine.fit_view();
    render_rgba(width, height, GRAPH_BG, |ctx| engine.draw(ctx))
}

fn collapsed_cluster_rgba(width: u32, height: u32) -> Vec<u8> {
    let (graph, clusters) = build_cluster_graph();
    let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
    engine.seed_positions(&seed_cluster_positions());

    let group_a = engine.define_cluster(clusters[0].clone());
    engine.define_cluster(clusters[1].clone());
    engine.define_cluster(clusters[2].clone());

    engine.set_canvas_rect(Rect::new(0.0, 0.0, width as f64, height as f64));
    for _ in 0..200 {
        engine.tick(1.0 / 60.0);
    }
    if let Some(group_a) = group_a {
        engine.collapse_cluster(group_a);
    }
    engine.fit_view();
    render_rgba(width, height, GRAPH_BG, |ctx| engine.draw(ctx))
}

// ── uzor-text ASCII + kinetics exhibits ─────────────────────────────────

/// Renders the ASCII-grid exhibit at a canvas sized TIGHTLY to the actual
/// stepped grid's own content (`grid_dims_for_layout`'s own `cols`/`rows` x
/// `style.cell_w`/`cell_h`, plus a fixed margin) rather than a fixed,
/// caller-guessed size — a fixed 620x260 canvas for this fixture's short
/// 3-line paragraph left ~85% of the canvas blank, which `ImageFit::
/// Contain` then faithfully preserved as a large empty gap once embedded
/// at a smaller display height (found + fixed during visual review of
/// `typeset_showcase_page8.png`). Returns the RGBA buffer plus its OWN
/// `(width, height)` — the caller uses these as the `ImageBlock`'s real
/// intrinsic dimensions, not a guessed constant.
fn ascii_exhibit_rgba(max_width: u32) -> (Vec<u8>, u32, u32) {
    const TEXT: &str = "This short seeded paragraph is rendered through the uzor-text ASCII cell-shader \
        bridge, sampling real glyph coverage onto a monospace character grid instead of painting glyph \
        outlines directly.";
    const MARGIN: f64 = 16.0;

    let font = FontSpec::new(FontFamily::Roboto, 15.0);
    let shaper = CosmicShaper::headless();
    let layout = layout_text(TEXT, &font, (max_width as f64) - 2.0 * MARGIN, &shaper);
    let style = AsciiGridStyle::square(8.0);
    let (cols, rows) = uzor_text::ascii::grid_dims_for_layout(&layout, style.cell_w, style.cell_h);
    let grid = build_ascii_grid(&layout, style);

    let width = ((cols as f64) * style.cell_w + 2.0 * MARGIN).round().max(1.0) as u32;
    let height = ((rows as f64) * style.cell_h + 2.0 * MARGIN).round().max(1.0) as u32;
    let rgba = render_rgba(width, height, [255, 255, 255, 255], |ctx| {
        draw_ascii_grid(ctx, (MARGIN, MARGIN), &grid, style);
    });
    (rgba, width, height)
}

const KINETICS_TEXT: &str = "A fixed seeded paragraph morphing between a narrow and a wide column, one glyph at a time.";

fn kinetics_frame_rgba(width: u32, height: u32, t: f64) -> Vec<u8> {
    let font = FontSpec::new(FontFamily::Roboto, 14.0);
    let shaper = CosmicShaper::headless();
    let narrow = layout_text(KINETICS_TEXT, &font, 130.0, &shaper);
    let wide = layout_text(KINETICS_TEXT, &font, (width as f64) - 30.0, &shaper);
    let morph = build_morph(&narrow, &wide);
    let sampled = sample_layout(&morph, t, 0x111111_ff);
    render_rgba(width, height, [255, 255, 255, 255], |ctx| {
        draw_paragraph(ctx, (15.0, 15.0), &sampled, "#111111", false);
    })
}

/// Renumber a concatenation of several independent `slice_pages` runs into
/// one continuous document — see this module's own "master/columns seam"
/// doc comment above for why this, not a `pages_to_pdf` signature change,
/// is the correct seam. Only `Page::index`/`Page::total`/
/// `Page::page_number`'s own TEXT are rewritten; every other already-baked
/// field (frame/extra_frames/header/footer geometry, the page-number's own
/// RECT) is untouched.
fn renumber_pages<'a>(mut pages: Vec<Page<'a>>, style: &PageNumberStyle) -> Vec<Page<'a>> {
    let total = pages.len() as u32;
    for (i, page) in pages.iter_mut().enumerate() {
        page.index = i as u32;
        page.total = total;
        if let Some(number) = &mut page.page_number {
            number.text = style.format_for(page.index, total);
        }
    }
    pages
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
        BlockNode::new(Block::Paragraph(Paragraph::new(&two_col_heading_run, column_width))).with_break_control(BreakControl::AvoidAfter),
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
    let bar_caption_run = [StyledRun::new("Figure 1 — a categorical bar chart over 5 seeded categories.", CAPTION_FONT)];
    let curve_caption_run = [StyledRun::new("Figure 2 — a cumulative curve plotted against a calendar TimeScale X-axis.", CAPTION_FONT)];
    let histogram_caption_run = [StyledRun::new("Figure 3 — a histogram of 500 seeded samples grouped into 20 bins.", CAPTION_FONT)];
    let timeline_caption_run = [StyledRun::new("Figure 4 — a timeline of point and interval events across 3 lanes.", CAPTION_FONT)];
    let sankey_caption_run = [StyledRun::new("Figure 5 — a staged flow diagram with one clearly dominant path.", CAPTION_FONT)];

    let graph_intro_heading_run = [StyledRun::new("Graph Engine Family", SUBHEADING_FONT)];
    const GRAPH_INTRO: &str = "The following pages embed real headless renders from uzor-graph, the reusable \
        force-directed graph visualization engine, as ordinary images inside this composed document — the \
        SAME per-layout draw path uzor-graph's own headless proof tests use, only rasterized once and \
        placed as an ImageBlock rather than left as a standalone PNG.";
    let graph_intro_run = [StyledRun::new(GRAPH_INTRO, BODY_FONT)];

    let flow_figures: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_a_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&bar_caption_run, body_width))),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&curve_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&curve_caption_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_b_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&histogram_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&histogram_caption_run, body_width))),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&timeline_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&timeline_caption_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&fig_c_heading_run, body_width)))
            .with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Figure(FigureBlock::new(&sankey_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT))))
            .with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&sankey_caption_run, body_width))),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_intro_heading_run, body_width))).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_intro_run, body_width))),
    ];

    // ── Section 4: uzor-graph's 4 layout looks, real ImageBlocks ────────
    const GRAPH_RENDER_W: u32 = 700;
    const GRAPH_RENDER_H: u32 = 500;
    const GRAPH_DISPLAY_H: f64 = 240.0;

    let force_directed_pixels = force_directed_rgba(GRAPH_RENDER_W, GRAPH_RENDER_H);
    let hierarchical_pixels = hierarchical_rgba(GRAPH_RENDER_W, GRAPH_RENDER_H);
    let radial_pixels = radial_rgba(GRAPH_RENDER_W, GRAPH_RENDER_H);
    let collapsed_pixels = collapsed_cluster_rgba(GRAPH_RENDER_W, GRAPH_RENDER_H);

    let force_directed_image = ImageBlock::new(&force_directed_pixels, GRAPH_RENDER_W, GRAPH_RENDER_H, BlockSizing::FixedHeight(GRAPH_DISPLAY_H), ImageFit::Contain);
    let hierarchical_image = ImageBlock::new(&hierarchical_pixels, GRAPH_RENDER_W, GRAPH_RENDER_H, BlockSizing::FixedHeight(GRAPH_DISPLAY_H), ImageFit::Contain);
    let radial_image = ImageBlock::new(&radial_pixels, GRAPH_RENDER_W, GRAPH_RENDER_H, BlockSizing::FixedHeight(GRAPH_DISPLAY_H), ImageFit::Contain);
    let collapsed_image = ImageBlock::new(&collapsed_pixels, GRAPH_RENDER_W, GRAPH_RENDER_H, BlockSizing::FixedHeight(GRAPH_DISPLAY_H), ImageFit::Contain);

    let graph_a_heading_run = [StyledRun::new("Graph Exhibits — Force-Directed & Hierarchical", SUBHEADING_FONT)];
    let graph_b_heading_run = [StyledRun::new("Graph Exhibits — Radial & Collapsed-Cluster", SUBHEADING_FONT)];
    let force_caption_run = [StyledRun::new("Exhibit 1 — force-directed layout (uzor-graph).", CAPTION_FONT)];
    let hierarchical_caption_run = [StyledRun::new("Exhibit 2 — hierarchical layout (uzor-graph).", CAPTION_FONT)];
    let radial_caption_run = [StyledRun::new("Exhibit 3 — radial layout (uzor-graph).", CAPTION_FONT)];
    let collapsed_caption_run = [StyledRun::new("Exhibit 4 — collapsed-cluster layout (uzor-graph).", CAPTION_FONT)];

    let flow_graphs: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_a_heading_run, body_width))).with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Image(force_directed_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&force_caption_run, body_width))),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Image(hierarchical_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&hierarchical_caption_run, body_width))),
        BlockNode::new(Block::Paragraph(Paragraph::new(&graph_b_heading_run, body_width))).with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Image(radial_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&radial_caption_run, body_width))),
        BlockNode::new(Block::Spacer(18.0)),
        BlockNode::new(Block::Image(collapsed_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&collapsed_caption_run, body_width))),
    ];

    // ── Section 5: ASCII exhibit + kinetics morph strip ─────────────────
    // The ASCII exhibit's own render is sized TIGHTLY to its content (see
    // `ascii_exhibit_rgba`'s own doc comment), so its display height is
    // derived from its OWN intrinsic aspect at full body width — an exact
    // fit, never a guessed box that leaves `ImageFit::Contain` letterboxing
    // blank space above/below.
    let (ascii_pixels, ascii_w, ascii_h) = ascii_exhibit_rgba(body_width.round() as u32);
    let ascii_display_h = (ascii_h as f64 / ascii_w as f64) * body_width;
    let ascii_image = ImageBlock::new(&ascii_pixels, ascii_w, ascii_h, BlockSizing::FixedHeight(ascii_display_h), ImageFit::Contain);

    const KINETICS_RENDER_W: u32 = 460;
    const KINETICS_RENDER_H: u32 = 110;
    const KINETICS_DISPLAY_H: f64 = 85.0;
    let kinetics_t0_pixels = kinetics_frame_rgba(KINETICS_RENDER_W, KINETICS_RENDER_H, 0.0);
    let kinetics_t05_pixels = kinetics_frame_rgba(KINETICS_RENDER_W, KINETICS_RENDER_H, 0.5);
    let kinetics_t1_pixels = kinetics_frame_rgba(KINETICS_RENDER_W, KINETICS_RENDER_H, 1.0);
    let kinetics_t0_image = ImageBlock::new(&kinetics_t0_pixels, KINETICS_RENDER_W, KINETICS_RENDER_H, BlockSizing::FixedHeight(KINETICS_DISPLAY_H), ImageFit::Contain);
    let kinetics_t05_image = ImageBlock::new(&kinetics_t05_pixels, KINETICS_RENDER_W, KINETICS_RENDER_H, BlockSizing::FixedHeight(KINETICS_DISPLAY_H), ImageFit::Contain);
    let kinetics_t1_image = ImageBlock::new(&kinetics_t1_pixels, KINETICS_RENDER_W, KINETICS_RENDER_H, BlockSizing::FixedHeight(KINETICS_DISPLAY_H), ImageFit::Contain);

    let ascii_heading_run = [StyledRun::new("ASCII Text Mode", SUBHEADING_FONT)];
    let ascii_caption_run = [StyledRun::new("Exhibit 5 — ASCII-grid text rendering (uzor-text::ascii).", CAPTION_FONT)];
    let kinetics_heading_run = [StyledRun::new("Kinetics — Paragraph Morph", SUBHEADING_FONT)];
    let kinetics_t0_caption_run = [StyledRun::new("Kinetics exhibit — t = 0.0 (narrow).", CAPTION_FONT)];
    let kinetics_t05_caption_run = [StyledRun::new("Kinetics exhibit — t = 0.5 (mid-flight).", CAPTION_FONT)];
    let kinetics_t1_caption_run = [StyledRun::new("Kinetics exhibit — t = 1.0 (wide).", CAPTION_FONT)];

    let flow_ascii_kinetics: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&ascii_heading_run, body_width))).with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Image(ascii_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(6.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&ascii_caption_run, body_width))),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_heading_run, body_width))).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Image(kinetics_t0_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(4.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_t0_caption_run, body_width))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Image(kinetics_t05_image)).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(4.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&kinetics_t05_caption_run, body_width))),
        BlockNode::new(Block::Spacer(10.0)),
        BlockNode::new(Block::Image(kinetics_t1_image)).with_break_control(BreakControl::AvoidAfter),
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

    let center_photo = generated_photo_rgba(ISLAND_CENTER_W.round() as u32, island_center_height.round().max(1.0) as u32, 0);
    let center_island = AnchoredIsland::new(
        ImageBlock::new(
            &center_photo,
            ISLAND_CENTER_W.round() as u32,
            island_center_height.round().max(1.0) as u32,
            BlockSizing::FixedHeight(island_center_height),
            ImageFit::Cover,
        ),
        IslandAnchor::Center,
        ISLAND_CENTER_W,
        ISLAND_CENTER_MARGIN,
    );

    let island_center_title_run = [StyledRun::new("Layout — Centered Island, Text Both Sides", SUBHEADING_FONT)];

    let flow_island_center: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&island_center_title_run, body_width))).with_break_control(BreakControl::ForceBefore),
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

    let left_photo = generated_photo_rgba(170, 170, 1);
    let right_photo = generated_photo_rgba(170, 170, 2);
    let island_left = AnchoredIsland::new(
        ImageBlock::new(&left_photo, 170, 170, BlockSizing::FixedHeight(ISLAND_SIDE_H), ImageFit::Cover),
        IslandAnchor::Left,
        ISLAND_SIDE_W,
        ISLAND_SIDE_MARGIN,
    );
    let island_right = AnchoredIsland::new(
        ImageBlock::new(&right_photo, 170, 170, BlockSizing::FixedHeight(ISLAND_SIDE_H), ImageFit::Cover),
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
        BlockNode::new(Block::Paragraph(Paragraph::new(&island_side_title_run, body_width))).with_break_control(BreakControl::ForceBefore),
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
    let list_items = [
        ListItem::new(&li1_nodes),
        ListItem::new(&li2_nodes),
        ListItem::new(&li3_nodes),
        ListItem::new(&li4_nodes),
        ListItem::new(&li5_nodes),
        ListItem::new(&li6_nodes),
    ];

    let flow_data: Vec<BlockNode<'_>> = vec![
        BlockNode::new(Block::Paragraph(Paragraph::new(&table_heading_run, body_width))).with_break_control(BreakControl::ForceBefore),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::Table(table)),
        BlockNode::new(Block::Spacer(20.0)),
        BlockNode::new(Block::Paragraph(Paragraph::new(&list_heading_run, body_width))).with_break_control(BreakControl::AvoidAfter),
        BlockNode::new(Block::Spacer(8.0)),
        BlockNode::new(Block::List(ListBlock::new(&list_items, MarkerStyle::Bullet('•'), 20.0))),
    ];

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
    let pages_rest = slice_pages(&flow_rest, &master_single, &style_single, &shaper);

    let mut all_pages: Vec<Page<'_>> = Vec::new();
    all_pages.extend(pages_opener);
    all_pages.extend(pages_two_col);
    all_pages.extend(pages_rest);
    let all_pages = renumber_pages(all_pages, &page_number_style);

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

    let pdf_bytes = pages_to_pdf(&all_pages, &master_single, &theme);
    assert!(pdf_bytes.starts_with(b"%PDF-"));
    write_proof("typeset_showcase.pdf", &pdf_bytes);

    let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse the showcase's own PDF output");
    let lopdf_pages = doc.get_pages();
    assert_eq!(lopdf_pages.len(), all_pages.len(), "the PDF must carry every composed page");

    let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
    let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
    for word in ["uzor-typeset", "uzor-graph", "uzor-figures", "Active", "Showcase"] {
        assert!(extracted.contains(word), "extracted PDF text must contain {word:?} — got a document of {} chars", extracted.len());
    }
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
