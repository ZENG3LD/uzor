//! Paint a composed [`Page`] onto a [`RenderContext`] — design law 6
//! (native measure/paint seams, no bespoke ones): paragraphs paint through
//! `uzor_text::draw_paragraph` unchanged, figures paint through their own
//! `TypesetFigure::render`, neither is a new drawing primitive this crate
//! invented.
//!
//! ## Image blocks — the `ImagePainter` seam IS reachable now (P1 gap
//! CLOSED)
//!
//! Design law 4 says images paint through `ImagePainter::draw_image_rgba`.
//! This crate's own P1 divergence log flagged that seam unreachable for
//! two reasons — BOTH now closed upstream, not in this crate:
//! 1. `RenderContext::image_painter()` is now a real capability-query
//!    accessor on the `RenderContext` supertrait itself
//!    (`uzor/src/core/render/context.rs`) — a `&mut dyn RenderContext`
//!    reaches `Option<&mut dyn ImagePainter>` directly, no downcast
//!    needed.
//! 2. `uzor-render-tiny-skia`'s own `draw_image_rgba` is now a REAL
//!    bilinear blit (`uzor-render-tiny-skia/src/context.rs`) — the
//!    caller's straight-alpha RGBA bytes are premultiplied and composited
//!    via `Pixmap::draw_pixmap`'s own Pattern-shader resampling, the same
//!    backend `uzor-export::render_to_png` uses.
//!
//! [`crate::scene::Block::Image`]/[`crate::scene::Block::Island`] both
//! paint real pixels through [`draw_image_content`]: `ctx.image_painter()`
//! queried first; `Some` composites the block's own RGBA at its already-
//! computed `ImageBlock::content_rect` (fit-mode geometry unchanged —
//! only the FINAL blit was ever the gap); `None` (a backend that genuinely
//! cannot composite images) falls back to the SAME deterministic dashed-
//! magenta placeholder this crate always painted, now a documented
//! fallback rather than the only path.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;
use uzor_figures::FigureTheme;
use uzor_text::{draw_decorations, draw_paragraph};

use crate::kinetics::FrameBlockState;
use crate::region::{ListPlacement, PlacedBlock, TablePlacement};
use crate::scene::Block;
use crate::slice::{Card, Page, PageNumberPlacement, Slide};
use crate::style::{ColorRole, FontRole, Theme};

/// Dashed magenta — the same debug-outline convention
/// `uzor_text::draw_paragraph`'s own `InlineBox` outline uses, so a
/// reserved-but-unpainted rect always reads as "placeholder," never as a
/// real content color. Fallback-only now (see this module's own "Image
/// blocks" doc comment) — painted only when `ctx.image_painter()` is
/// `None`.
const IMAGE_PLACEHOLDER_COLOR: &str = "#ff00ffff";

/// Which paint layers [`draw_page_layers`] includes — the "smallest clean
/// seam" Arc 4 Phase P5 (PDF assembly, `uzor-typeset::export::
/// pages_to_pdf`) needs: that adapter renders every page's NON-TEXT
/// content (figures/tables/gridlines/images/list markers/header-footer
/// chrome) as a raster background, then paints real, selectable/
/// searchable vector PDF text on top of it — so the raster pass must NOT
/// also ink paragraph glyphs (that would double-paint the same text once
/// as pixels, once as vector, and any tiny position mismatch between the
/// two would read as a visible ghosting artifact).
///
/// [`DrawLayers::default`] (`paragraph_ink: true`) is "every layer,
/// unchanged" — [`draw_page`] is a thin wrapper over [`draw_page_layers`]
/// at that default, so every pre-P5 caller's own visual output stays
/// byte-for-byte unchanged (every P0-P4 proof PNG this crate already
/// ships is unaffected).
#[derive(Debug, Clone, Copy)]
pub struct DrawLayers {
    /// Paint `Block::Paragraph` glyph ink — body text AND header/footer
    /// text (both are `Block::Paragraph`s, painted through the SAME
    /// `draw_placed_block` recursion), including nested paragraph content
    /// inside table cells/list items (the recursion threads this flag
    /// through unchanged). Table gridlines, list marker glyphs, figures/
    /// images, and the page-number text are NOT gated by this flag — see
    /// `uzor-typeset::export`'s own module docs for why those specific
    /// ad-hoc (non-`ParagraphLayout`, non-alphabetic-baseline) paint calls
    /// stay raster-only in this phase (reproducing their exact baseline
    /// position as PDF vector text would need real shaping-backend font
    /// metrics this crate's own `ParagraphLayout` doesn't carry for them).
    pub paragraph_ink: bool,
    /// Paint `Block::Paragraph` underline/strikethrough rects
    /// (`uzor_text::draw_decorations`, typography-gap WAVE 2) —
    /// independent of `paragraph_ink` so a caller can paint decoration
    /// rects into a REAL PDF vector content stream (real `fill_rect` ops)
    /// while the SAME pass suppresses raster glyph ink
    /// (`uzor-typeset::export::pdf_adapter`'s pass 1 does exactly this:
    /// `paragraph_ink: false, paragraph_decorations: true` — the glyphs
    /// themselves paint separately as real `Tj` text in pass 2, but there
    /// is no separate vector pass for decoration rects, so this flag lets
    /// pass 1 emit them once, into the real content stream, without also
    /// re-painting glyph ink a second time). `draw_placed_block`'s own
    /// Paragraph arm never double-paints: when `paragraph_ink` is `true`
    /// it paints ink+decorations together via `draw_paragraph` (which
    /// already calls `draw_decorations` internally); `paragraph_decorations`
    /// is only separately consulted when `paragraph_ink` is `false`.
    pub paragraph_decorations: bool,
}

impl Default for DrawLayers {
    fn default() -> Self {
        Self { paragraph_ink: true, paragraph_decorations: true }
    }
}

/// Paint every placed block of `page` onto `ctx`, each at its own
/// frame-relative rect origin (design law 1: one transform — every draw
/// derives its position from the SAME `PlacedBlock::rect`/
/// `paragraph_layout`/`table_placement`/`list_placement`
/// [`crate::compose::compose`] already computed, never a second ad hoc
/// position formula), THEN every block in `page.extra_frames` (columns
/// 2..N when [`crate::master::PageMaster::columns`] is `> 1` — see
/// `crate::slice::pages`'s own "Multi-column pages" doc comment), followed
/// by `page`'s own header/footer (P2 — see `crate::master::PageMaster`)
/// and page-number text, if present.
///
/// `theme` resolves EVERY paint color/font this function uses — the ink
/// color paragraphs/table gridlines/list markers fall back to
/// (`ColorRole::Ink`), the figure theme every `Block::Figure` renders
/// with (`Theme::figure_theme`), and the page-number's own caption
/// font/color — design doc §5's own closing paragraph: "a theme edit
/// propagates ... through the SAME master->layout->instance chain."
/// `draw_page`'s signature changed from P1's `(ctx, page, default_color,
/// figure_theme)` now that `Theme` exists for real (see this crate's
/// `CLAUDE.md` — the same "grow the signature to the type the phase
/// actually needs" pattern P0->P1 already used for this same function).
///
/// A thin wrapper over [`draw_page_layers`] at [`DrawLayers::default`]
/// (every layer painted) — P5's own new entry point.
pub fn draw_page(ctx: &mut dyn RenderContext, page: &Page<'_>, theme: &Theme) {
    draw_page_layers(ctx, page, theme, DrawLayers::default());
}

/// [`draw_page`] with explicit layer control — see [`DrawLayers`]'s own
/// doc comment for what `layers.paragraph_ink` gates and why.
pub fn draw_page_layers(ctx: &mut dyn RenderContext, page: &Page<'_>, theme: &Theme, layers: DrawLayers) {
    let default_color = theme.color_hex(ColorRole::Ink);
    let figure_theme = theme.figure_theme();

    for placed in &page.frame.blocks {
        draw_placed_block(ctx, placed, &default_color, &figure_theme, layers);
    }
    for extra in &page.extra_frames {
        for placed in &extra.blocks {
            draw_placed_block(ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(header) = &page.header {
        for placed in &header.blocks {
            draw_placed_block(ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(footer) = &page.footer {
        for placed in &footer.blocks {
            draw_placed_block(ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(footnotes) = &page.footnotes {
        draw_footnote_separator(ctx, footnotes.frame.region.rect, &default_color);
        for placed in &footnotes.frame.blocks {
            draw_placed_block(ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(number) = &page.page_number {
        draw_page_number(ctx, number, theme);
    }
}

/// A short horizontal rule at the TOP of the footnote zone (typography-gap
/// WAVE 3) — the classic footnote-separator convention (a rule shorter than
/// the full text measure, never a full-width divider that would read as a
/// section break). Painted via the EXISTING `RenderContext::fill_rect`
/// primitive (design law 6 — no new drawing primitive), the SAME "paint a
/// decoration as a filled rect" convention `uzor_text::draw_decorations`
/// already uses for underline/strikethrough rules.
fn draw_footnote_separator(ctx: &mut dyn RenderContext, zone_rect: Rect, default_color: &str) {
    const RULE_HEIGHT: f64 = 1.0;
    const RULE_WIDTH_FRACTION: f64 = 0.35;
    if zone_rect.height <= 0.0 {
        return;
    }
    ctx.set_fill_color(default_color);
    ctx.fill_rect(zone_rect.x, zone_rect.y, zone_rect.width * RULE_WIDTH_FRACTION, RULE_HEIGHT);
}

/// Paint a composed [`Card`] (design doc §4.2's card/fluid mode) — the
/// same per-block recursion [`draw_page`] uses (design law 1); no
/// header/footer/page-number concept (those are `PageMaster`-specific;
/// a card is content-only). Unlike `draw_page` (which has always relied
/// on the caller's own `uzor-export::ExportSpec::background`), `draw_card`
/// paints its OWN background from `theme` explicitly, sized to the
/// card's own natural extent — this phase's own "slide/card bg from
/// theme" requirement, with no `PageMaster`-level equivalent to lean on.
pub fn draw_card(ctx: &mut dyn RenderContext, card: &Card<'_>, theme: &Theme) {
    let default_color = theme.color_hex(ColorRole::Ink);
    let figure_theme = theme.figure_theme();

    ctx.set_fill_color(&theme.color_hex(ColorRole::Background));
    ctx.fill_rect(0.0, 0.0, card.width, card.natural_height);

    for placed in &card.frame.blocks {
        draw_placed_block(ctx, placed, &default_color, &figure_theme, DrawLayers::default());
    }
}

/// Paint a composed [`Slide`] (design doc §4.2's fixed/PowerPoint mode).
/// The background always fills the FULL fixed viewport regardless of any
/// content shrink below — the slide's own physical size never changes,
/// only how much of it the (possibly shrunk) content occupies. When
/// [`Slide::shrink_scale`] is `Some(factor)` (a [`crate::slice::
/// SlideOverflow::Shrink`] result), the CONTENT paint pass (never the
/// background) is wrapped in a single `Painter::scale` transform — ONE
/// uniform factor for the whole slide, never a per-block rescale (design
/// doc §7 P3 risk note) — reusing the EXISTING `save`/`scale`/`restore`
/// primitive (`uzor::render::Painter`, part of the `RenderContext`
/// supertrait) rather than inventing a new geometry-rewrite pass over
/// every placed rect.
pub fn draw_slide(ctx: &mut dyn RenderContext, slide: &Slide<'_>, theme: &Theme) {
    let default_color = theme.color_hex(ColorRole::Ink);
    let figure_theme = theme.figure_theme();

    ctx.set_fill_color(&theme.color_hex(ColorRole::Background));
    ctx.fill_rect(0.0, 0.0, slide.width, slide.height);

    ctx.save();
    if let Some(factor) = slide.shrink_scale {
        ctx.scale(factor, factor);
    }
    for placed in &slide.frame.blocks {
        draw_placed_block(ctx, placed, &default_color, &figure_theme, DrawLayers::default());
    }
    ctx.restore();
}

/// Paint one [`FrameBlockState`] — [`crate::kinetics::FrameMorph::
/// sample`]'s own per-block drawable output (design doc §4.3's morph
/// sampling, Arc 4 Phase P4). Reuses [`draw_placed_block`] UNCHANGED
/// (design law 1: one paint path) by reconstructing the temporary
/// [`PlacedBlock`] shape it expects, wrapped in a single `Painter::
/// {save,set_global_alpha,restore}` bracket for `state.opacity` — the
/// SAME existing alpha primitive [`draw_slide`]'s own shrink-scale
/// wrapping already uses (never a new opacity-painting seam), confirmed
/// (by inspection of `uzor-render-tiny-skia/src/context.rs`) to reach
/// every block kind's own fill/stroke/text color through that backend's
/// `effective_fill_color`/`effective_stroke_color`, and to survive a
/// `Block::Figure`'s own internal `render_with` call under the default,
/// no-hover/no-focus `FigureOverlay` this crate's `TypesetFigure` blanket
/// impls always pass (the ONLY paths inside `uzor-figures` that reset
/// `set_global_alpha(1.0)` mid-draw are hover-highlight/focus-outline
/// branches, which `FigureOverlay::default()` never enters).
pub fn draw_frame_state(ctx: &mut dyn RenderContext, state: &FrameBlockState<'_>, theme: &Theme) {
    let default_color = theme.color_hex(ColorRole::Ink);
    let figure_theme = theme.figure_theme();

    let placed = PlacedBlock {
        id: state.id,
        rect: state.rect,
        kind: state.kind,
        paragraph_layout: state.paragraph_layout.clone(),
        table_placement: state.table_placement.clone(),
        list_placement: state.list_placement.clone(),
    };

    ctx.save();
    ctx.set_global_alpha(state.opacity as f64);
    draw_placed_block(ctx, &placed, &default_color, &figure_theme, DrawLayers::default());
    ctx.restore();
}

/// Paint a page's own page-number text (design doc §4.1's
/// `page_number_token`, "resolved per-page during slicing" — the STRING
/// and its RECT are both already resolved by [`crate::slice::slice_pages`]
/// at slice time; this function only paints, never re-derives either,
/// design law 1). Right-aligned + vertically centered within its own
/// reserved footer slice, in `FontRole::Caption`/`ColorRole::Muted` — the
/// same small, de-emphasized styling any real word processor gives a
/// page number.
fn draw_page_number(ctx: &mut dyn RenderContext, placement: &PageNumberPlacement, theme: &Theme) {
    ctx.set_font(&theme.font_spec(FontRole::Caption).to_css_font());
    ctx.set_fill_color(&theme.color_hex(ColorRole::Muted));
    ctx.set_text_align(TextAlign::Right);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&placement.text, placement.rect.x + placement.rect.width, placement.rect.y + placement.rect.height / 2.0);
}

/// Paint ONE placed block — factored out of [`draw_page`] so table/list
/// cell content (itself a `Vec<PlacedBlock>`) can recurse through the
/// exact same paint logic top-level flow blocks use. `layers` (P5) gates
/// only the `Block::Paragraph` arm — see [`DrawLayers`]'s own doc comment.
fn draw_placed_block(ctx: &mut dyn RenderContext, placed: &PlacedBlock<'_>, default_color: &str, figure_theme: &FigureTheme, layers: DrawLayers) {
    match placed.kind {
        Block::Paragraph(_) => {
            if let Some(layout) = &placed.paragraph_layout {
                if layers.paragraph_ink {
                    // `draw_paragraph` already paints decorations
                    // internally — never also call `draw_decorations`
                    // here, or a decoration rect would paint twice.
                    draw_paragraph(ctx, (placed.rect.x, placed.rect.y), layout, default_color, false);
                } else if layers.paragraph_decorations {
                    draw_decorations(ctx, (placed.rect.x, placed.rect.y), layout, default_color);
                }
            }
        }
        Block::Figure(figure_block) => {
            figure_block.figure.render(ctx, placed.rect, figure_theme);
        }
        Block::Image(image) => {
            let content_rect = image.content_rect(placed.rect.width, placed.rect.height);
            draw_image_content(ctx, image.rgba, image.intrinsic_width, image.intrinsic_height, content_rect, placed.rect);
        }
        Block::Island(island) => {
            let content_rect = island.image.content_rect(placed.rect.width, placed.rect.height);
            draw_image_content(ctx, island.image.rgba, island.image.intrinsic_width, island.image.intrinsic_height, content_rect, placed.rect);
        }
        Block::Table(_) => {
            if let Some(table) = &placed.table_placement {
                draw_table_placement(ctx, table, default_color, figure_theme, layers);
            }
        }
        Block::List(_) => {
            if let Some(list) = &placed.list_placement {
                draw_list_placement(ctx, list, default_color, figure_theme, layers);
            }
        }
        Block::Spacer(_) => {}
    }
}

/// Real image composite, shared by [`Block::Image`]/[`Block::Island`]
/// (design law 4/6: one image-blit seam, no bespoke second one for
/// islands). Queries [`RenderContext::image_painter`] — `Some` composites
/// `rgba` at `content_rect` (already-computed `ImageBlock::content_rect`
/// fit-mode geometry, origin-relative to `rect`, translated here into
/// absolute frame coordinates — design law 1, no new position math);
/// `None` (a backend that cannot composite raster images) falls back to
/// the SAME deterministic dashed-magenta placeholder this crate always
/// painted (this module's own documented fallback, not silently skipped).
fn draw_image_content(ctx: &mut dyn RenderContext, rgba: &[u8], intrinsic_width: u32, intrinsic_height: u32, content_rect: (f64, f64, f64, f64), rect: Rect) {
    let (cx, cy, cw, ch) = content_rect;
    match ctx.image_painter() {
        Some(painter) if cw > 0.0 && ch > 0.0 => {
            painter.draw_image_rgba(rgba, intrinsic_width, intrinsic_height, rect.x + cx, rect.y + cy, cw, ch);
        }
        _ => draw_image_placeholder(ctx, rect),
    }
}

fn draw_image_placeholder(ctx: &mut dyn RenderContext, rect: Rect) {
    ctx.set_stroke_color(IMAGE_PLACEHOLDER_COLOR);
    ctx.set_line_dash(&[4.0, 3.0]);
    ctx.set_stroke_width(1.0);
    ctx.stroke_rect(rect.x, rect.y, rect.width, rect.height);
    ctx.set_line_dash(&[]);
}

/// Draw a table's own row/column gridlines, then recurse into every
/// cell's already-placed content. Gridlines are never gated by `layers`
/// (see [`DrawLayers`]'s own doc comment) — only nested paragraph content
/// is.
fn draw_table_placement(ctx: &mut dyn RenderContext, table: &TablePlacement<'_>, default_color: &str, figure_theme: &FigureTheme, layers: DrawLayers) {
    ctx.set_stroke_color(default_color);
    ctx.set_stroke_width(1.0);
    for row in &table.rows {
        ctx.stroke_rect(row.rect.x, row.rect.y, row.rect.width, row.rect.height);
        for cell in &row.cells {
            ctx.stroke_rect(cell.rect.x, cell.rect.y, cell.rect.width, cell.rect.height);
            for inner in &cell.content {
                draw_placed_block(ctx, inner, default_color, figure_theme, layers);
            }
        }
    }
}

/// Draw every item's marker text, then recurse into its own already-placed
/// (already-indented) content. The marker glyph itself is never gated by
/// `layers` (see [`DrawLayers`]'s own doc comment) — only nested
/// paragraph content is.
fn draw_list_placement(ctx: &mut dyn RenderContext, list: &ListPlacement<'_>, default_color: &str, figure_theme: &FigureTheme, layers: DrawLayers) {
    for item in &list.items {
        if !item.marker_text.is_empty() {
            ctx.set_font(&item.marker_font.to_css_font());
            ctx.set_fill_color(default_color);
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            ctx.fill_text(&item.marker_text, item.marker_rect.x, item.marker_rect.y);
        }
        for inner in &item.content {
            draw_placed_block(ctx, inner, default_color, figure_theme, layers);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Headless proof: a fixed seeded 2-page report-shaped document —
    //! mixed alignments (Left/Center/Right) plus one Justify +
    //! `KnuthPlass` + `Hyphenation::English` paragraph (proves `uzor-text`
    //! is fully live inside `uzor-typeset`, not just re-exported), sliced
    //! at A4-ish 595x842 via `slice_pages` and painted through
    //! [`draw_page`] — written to `uzor/out/` for a human to eyeball
    //! (design law 8: one screenshot is never proof, hence the 2-page
    //! pair).

    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_export::{render_to_png, render_to_svg, ExportSpec};
    use uzor_text::{BreakStrategy, CosmicShaper, FontSpec, Hyphenation, Paragraph, ParagraphAlign, StyledRun};

    use super::{draw_card, draw_frame_state, draw_page, draw_slide};
    use crate::compose::ComposeStyle;
    use crate::scene::{Block, BlockNode};
    use crate::slice::{slice_pages, Margins, PageMaster};

    const PAGE_WIDTH: u32 = 595;
    const PAGE_HEIGHT: u32 = 842;

    fn out_dir() -> PathBuf {
        // Fixed path — `uzor/out/` is the shared human-eyeball drop point
        // for every headless proof render in this workspace (matches
        // `uzor-text`/`uzor-figures`'s own `out_dir` convention).
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn write_proof_text(name: &str, contents: &str) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), contents).expect("write proof SVG");
    }

    /// Minimal string-level SVG sanity check (no full XML parser
    /// dependency — `usvg` is heavy for a unit test): a single well-formed
    /// `<svg>` root, and every open tag has a matching close (or is
    /// self-closing) — a tiny stack scan, not a validating parser.
    /// Duplicated (not shared via a test-utils crate) in `uzor-figures`'s
    /// own equivalent proof test — a ~20-line helper isn't worth a new
    /// dev-dependency.
    fn assert_svg_is_well_formed(svg: &str) {
        let trimmed = svg.trim();
        assert!(trimmed.starts_with("<svg"), "SVG must start with the <svg root element");
        assert!(trimmed.ends_with("</svg>"), "SVG must end with a closed </svg> root");

        let mut stack: Vec<&str> = Vec::new();
        let mut idx = 0usize;
        while let Some(rel_start) = svg[idx..].find('<') {
            let start = idx + rel_start;
            let Some(rel_end) = svg[start..].find('>') else { break };
            let end = start + rel_end;
            let tag = &svg[start + 1..end];
            idx = end + 1;

            if let Some(name) = tag.strip_prefix('/') {
                let name = name.split_whitespace().next().unwrap_or("");
                let top = stack.pop().unwrap_or("");
                assert_eq!(top, name, "mismatched SVG closing tag </{name}>");
            } else if !tag.ends_with('/') {
                let name = tag.split_whitespace().next().unwrap_or("");
                stack.push(name);
            }
        }
        assert!(stack.is_empty(), "unbalanced SVG tags left open: {stack:?}");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    /// Decode `bytes` and return the RGBA pixel at `(x, y)` — used by the
    /// P3 deck proof to confirm `draw_card`/`draw_slide` genuinely PAINT
    /// their own background (opaque alpha) rather than leaving the
    /// `ExportSpec::background: None` canvas transparent underneath a
    /// viewer's own white matting.
    fn decoded_png_pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
        let decoder = png::Decoder::new(bytes);
        let mut reader = decoder.read_info().expect("valid PNG header");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decode PNG frame");
        let stride = info.width as usize * 4;
        let idx = y as usize * stride + x as usize * 4;
        [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
    }

    /// Fixed seeded fixture text (design law 8: no lorem-ipsum RNG). Long
    /// enough, repeated across several paragraphs, to overflow one A4-ish
    /// page body and split a paragraph across the page boundary.
    const FILLER_A: &str = "The confidence-matrix narrative for this section walks through every \
        traced hop in order, noting timestamps, counterparties, and the amount observed at each \
        step of the flow, so a reader can follow the chain of custody from the initial deposit all \
        the way through to the resting balance without needing to cross-reference a separate table.";
    const FILLER_B: &str = "Business documents full of long compound words like implementation, \
        infrastructure, counterparty, and accountability often expose uneven interword spacing in a \
        narrow justified column, especially once hyphenation is enabled and a break lands mid-word \
        near the right margin, which is exactly the case this fixed fixture paragraph is built to show.";

    fn seeded_flow() -> Vec<BlockNode<'static>> {
        // `'static` runs/text so the fixture can be built once and shared
        // by both the layout-count assertions and the PNG render closure
        // below without lifetime friction.
        static TITLE_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "Case Report — uzor-typeset P0 Proof", font: TITLE_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static LEFT_A_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static LEFT_B_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static CENTER_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "Section II — Centered Summary Heading", font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static RIGHT_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "— Exhibit reference, right-aligned —", font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static JUSTIFY_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static TAIL_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static EXTRA_A_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static EXTRA_B_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static EXTRA_C_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 16.0, bold: false, italic: false };

        let body_width = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0)).body_rect().width;

        vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&TITLE_RUN, body_width))),
            BlockNode::new(Block::Spacer(20.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&LEFT_A_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(
                Paragraph::new(&CENTER_RUN, body_width).with_align(ParagraphAlign::Center),
            )),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&RIGHT_RUN, body_width).with_align(ParagraphAlign::Right))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(
                Paragraph::new(&JUSTIFY_RUN, body_width)
                    .with_align(ParagraphAlign::Justify)
                    .with_break_strategy(BreakStrategy::KnuthPlass)
                    .with_hyphenation(Hyphenation::English),
            )),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&LEFT_B_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&TAIL_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&EXTRA_A_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&EXTRA_B_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&EXTRA_C_RUN, body_width))),
        ]
    }

    #[test]
    fn seeded_two_page_document_renders_to_two_valid_pngs_with_margins_and_a_split_paragraph() {
        let flow = seeded_flow();
        let master = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0));
        let style = ComposeStyle::new(14.0, FontSpec::new(FontFamily::Roboto, 16.0));
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2, "fixture is tuned to land on exactly 2 pages, got {}", pages.len());

        // A REAL mid-paragraph split (not just a whole-block deferral) —
        // page 1's last placed block and page 2's first placed block share
        // the same effective `BlockId` (same source paragraph) and each
        // carries only part of its full line count — the "split paragraph
        // continues cleanly on page 2" proof condition.
        assert!(pages[0].frame.overflow.is_some(), "fixture must force a mid-document split across the 2 pages");
        let page1_last = pages[0].frame.blocks.last().expect("page 1 must have content");
        let page2_first = pages[1].frame.blocks.first().expect("page 2 must have content");
        assert_eq!(page1_last.id, page2_first.id, "the split must continue the SAME source block onto page 2, not start a different one");
        let head_lines = page1_last.paragraph_layout.as_ref().expect("split block carries a layout").lines.len();
        let tail_lines = page2_first.paragraph_layout.as_ref().expect("split block carries a layout").lines.len();
        assert!(head_lines > 0 && tail_lines > 0, "both halves of the split must carry at least one whole line, never an empty placement");

        let spec = ExportSpec { width_px: PAGE_WIDTH, height_px: PAGE_HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let theme = crate::style::Theme::light_report();

        let page1_bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("page 1 proof render should succeed");
        assert_eq!(decoded_png_dims(&page1_bytes), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p0_page1.png", &page1_bytes);

        let page2_bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[1], &theme)).expect("page 2 proof render should succeed");
        assert_eq!(decoded_png_dims(&page2_bytes), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p0_page2.png", &page2_bytes);

        // Every placed block on both pages must respect the page margins
        // (never paint outside the 40px-inset body rect).
        let body = master.body_rect();
        for page in &pages {
            for placed in &page.frame.blocks {
                assert!(placed.rect.x >= body.x - 0.01, "block must not start left of the margin");
                assert!(placed.rect.x + placed.rect.width <= body.x + body.width + 0.01, "block must not extend past the right margin");
            }
        }
    }

    /// SVG sibling of the P0 PNG proof above — same seeded fixture, first
    /// page only, via `uzor_export::render_to_svg` instead of
    /// `render_to_png`. Validation is string-level (a lightweight
    /// balanced-tag scan — `usvg` is heavy for a unit test); the
    /// coordinator verifies visually by opening the written file.
    #[test]
    fn seeded_first_page_renders_to_a_well_formed_standalone_svg() {
        let flow = seeded_flow();
        let master = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0));
        let style = ComposeStyle::new(14.0, FontSpec::new(FontFamily::Roboto, 16.0));
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert!(!pages.is_empty(), "fixture must produce at least one page");

        let spec = ExportSpec { width_px: PAGE_WIDTH, height_px: PAGE_HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let theme = crate::style::Theme::light_report();

        let svg = render_to_svg(&spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("page proof SVG render should succeed");

        assert_svg_is_well_formed(&svg);
        let path_count = svg.matches("<path").count();
        assert!(path_count > 0, "expected at least one <path> element (paragraph glyph outlines), got 0");

        write_proof_text("typeset_page.svg", &svg);
    }

    /// P1 headless proof (design law 8 + this arc's own MUST report-figure
    /// #8 shape): a fixed seeded 3-page report — title, justified body
    /// paragraphs, a keep-with-next section heading, a `BarFigure` block, a
    /// `TimelineFigure` block, a 4-column x 3-row table (`Fixed`/`Auto`/
    /// `Fraction` columns all exercised), and a bulleted list (one item
    /// wrapping to multiple lines) — sliced at A4-ish 595x842 and painted
    /// through [`draw_page`] to `uzor/out/typeset_p1_page{1,2,3}.png`.
    #[test]
    fn seeded_three_page_report_with_figures_table_and_list_renders_to_three_valid_pngs() {
        use uzor_figures::{BarFigure, TimelineEvent, TimelineFigure};

        use crate::compose::BreakControl;
        use crate::scene::{BlockSizing, ColumnSpec, FigureBlock, ListBlock, ListItem, MarkerStyle, TableBlock, TableCell, TableRow};

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const HEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 17.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };
        const TABLE_HEADER_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: true, italic: false };
        const TABLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };

        let master = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0));
        let body_width = master.body_rect().width;

        // ── paragraphs (title / heading / reused filler runs) ──────────
        let title_run = [StyledRun::new("Case Report — uzor-typeset P1 Proof (Chain-of-Custody Excerpt)", TITLE_FONT)];
        let heading_a_run = [StyledRun::new("Section 1 — Transfer Narrative", HEADING_FONT)];
        let heading_b_run = [StyledRun::new("Section 2 — Observed Amounts", HEADING_FONT)];
        let filler_a_run = [StyledRun::new(FILLER_A, BODY_FONT)];
        let filler_b_run = [StyledRun::new(FILLER_B, BODY_FONT)];

        // ── figures (seeded, deterministic — no RNG/time) ──────────────
        let bar_figure = BarFigure::new(
            vec!["Hop 1".to_owned(), "Hop 2".to_owned(), "Hop 3".to_owned(), "Hop 4".to_owned(), "Hop 5".to_owned()],
            vec![128_500.0, 640_000.0, 2_600_000.0, 1_450_000.0, 300_000.0],
        )
        .with_title("Observed amount per hop (seeded)");

        const ANCHOR_2024_01_01: f64 = 1_704_067_200.0;
        const DAY_SECS: f64 = 86_400.0;
        let timeline_events = vec![
            TimelineEvent { ts: ANCHOR_2024_01_01, end_ts: None, lane: 0, label: "initial deposit".to_owned(), kind: 0 },
            TimelineEvent {
                ts: ANCHOR_2024_01_01 + 2.0 * DAY_SECS,
                end_ts: Some(ANCHOR_2024_01_01 + 5.0 * DAY_SECS),
                lane: 0,
                label: "relay window".to_owned(),
                kind: 1,
            },
            TimelineEvent { ts: ANCHOR_2024_01_01 + 6.0 * DAY_SECS, end_ts: None, lane: 1, label: "exchange deposit".to_owned(), kind: 2 },
            TimelineEvent { ts: ANCHOR_2024_01_01 + 9.0 * DAY_SECS, end_ts: None, lane: 1, label: "resting balance".to_owned(), kind: 0 },
        ];
        let timeline_figure =
            TimelineFigure::new(timeline_events, vec!["path-a".to_owned(), "path-b".to_owned()]).with_title("Timeline of transfers (seeded)");

        // ── table: 4 columns (Fixed/Auto/Auto/Fraction) x 3 rows ───────
        let c_hop_h = [StyledRun::new("Hop", TABLE_HEADER_FONT)];
        let n_hop_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_hop_h, f64::MAX)))];
        let c_from_h = [StyledRun::new("From", TABLE_HEADER_FONT)];
        let n_from_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_from_h, f64::MAX)))];
        let c_to_h = [StyledRun::new("To", TABLE_HEADER_FONT)];
        let n_to_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_to_h, f64::MAX)))];
        let c_amount_h = [StyledRun::new("Amount (USDT)", TABLE_HEADER_FONT)];
        let n_amount_h = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_amount_h, f64::MAX)))];

        let c_hop_1 = [StyledRun::new("1", TABLE_FONT)];
        let n_hop_1 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_hop_1, f64::MAX)))];
        let c_from_1 = [StyledRun::new("wallet-a19x...aa2", TABLE_FONT)];
        let n_from_1 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_from_1, f64::MAX)))];
        let c_to_1 = [StyledRun::new("exchange-hot-7", TABLE_FONT)];
        let n_to_1 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_to_1, f64::MAX)))];
        let c_amount_1 = [StyledRun::new("128,500.00", TABLE_FONT)];
        let n_amount_1 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_amount_1, f64::MAX)))];

        let c_hop_2 = [StyledRun::new("2", TABLE_FONT)];
        let n_hop_2 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_hop_2, f64::MAX)))];
        let c_from_2 = [StyledRun::new("exchange-hot-7", TABLE_FONT)];
        let n_from_2 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_from_2, f64::MAX)))];
        let c_to_2 = [StyledRun::new("cold-storage-vault-2", TABLE_FONT)];
        let n_to_2 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_to_2, f64::MAX)))];
        let c_amount_2 = [StyledRun::new("2,600,000.00", TABLE_FONT)];
        let n_amount_2 = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_amount_2, f64::MAX)))];

        let header_cells = [TableCell::new(&n_hop_h), TableCell::new(&n_from_h), TableCell::new(&n_to_h), TableCell::new(&n_amount_h)];
        let row1_cells = [TableCell::new(&n_hop_1), TableCell::new(&n_from_1), TableCell::new(&n_to_1), TableCell::new(&n_amount_1)];
        let row2_cells = [TableCell::new(&n_hop_2), TableCell::new(&n_from_2), TableCell::new(&n_to_2), TableCell::new(&n_amount_2)];
        let table_rows = [TableRow::new(&header_cells), TableRow::new(&row1_cells), TableRow::new(&row2_cells)];
        let table_columns = [ColumnSpec::Fixed(50.0), ColumnSpec::Auto, ColumnSpec::Auto, ColumnSpec::Fraction(1.0)];

        // ── bulleted list (one item wraps to several lines) ────────────
        let li1_run = [StyledRun::new("Confirmed direct transfer between wallet A and the exchange hot wallet.", BODY_FONT)];
        let li1_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li1_run, f64::MAX)))];
        let li2_run = [StyledRun::new(
            "Secondary relay observed roughly forty-eight hours later, consistent with layering behavior across \
            several intermediate wallets before consolidation, long enough that this single list item is expected \
            to wrap across more than one line once it is indented past the marker gutter at the page's own body width.",
            BODY_FONT,
        )];
        let li2_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li2_run, f64::MAX)))];
        let li3_run = [StyledRun::new("Resting balance identified at custodial cold storage, pending subpoena response.", BODY_FONT)];
        let li3_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li3_run, f64::MAX)))];
        let li4_run = [StyledRun::new("No further movement observed as of the report date.", BODY_FONT)];
        let li4_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li4_run, f64::MAX)))];
        let list_items =
            [ListItem::new(&li1_nodes), ListItem::new(&li2_nodes), ListItem::new(&li3_nodes), ListItem::new(&li4_nodes)];

        // ── assemble the flow: title, keep-with-next heading, figures,
        // table, list, and enough repeated filler paragraphs either side
        // to land on exactly 3 pages (design law 8: fixed, not tuned by
        // RNG/time — a deterministic paragraph COUNT, chosen by running
        // this fixture and observing the resulting page count).
        let mut flow: Vec<BlockNode<'_>> = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))),
            BlockNode::new(Block::Spacer(18.0)),
        ];
        for _ in 0..2 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_a_run, body_width).with_align(ParagraphAlign::Justify))));
            flow.push(BlockNode::new(Block::Spacer(14.0)));
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_b_run, body_width).with_align(ParagraphAlign::Justify))));
            flow.push(BlockNode::new(Block::Spacer(14.0)));
        }
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&heading_a_run, body_width))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(8.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_a_run, body_width).with_align(ParagraphAlign::Justify))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));
        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(180.0)))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));
        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&timeline_figure, BlockSizing::FixedHeight(180.0)))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));
        for _ in 0..3 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_b_run, body_width).with_align(ParagraphAlign::Justify))));
            flow.push(BlockNode::new(Block::Spacer(14.0)));
        }
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&heading_b_run, body_width))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(8.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_b_run, body_width).with_align(ParagraphAlign::Justify))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));
        flow.push(BlockNode::new(Block::Table(TableBlock::new(&table_columns, &table_rows))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));
        flow.push(BlockNode::new(Block::List(ListBlock::new(&list_items, MarkerStyle::Bullet('•'), 20.0))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));
        for _ in 0..2 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&filler_a_run, body_width).with_align(ParagraphAlign::Justify))));
            flow.push(BlockNode::new(Block::Spacer(14.0)));
        }

        let style = ComposeStyle::new(10.0, BODY_FONT);
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 3, "fixture is tuned to land on exactly 3 pages, got {}", pages.len());

        // Every table placement must show all 3 rows across however many
        // pages the table itself spans, never dropping or duplicating a
        // row (row-atomic splitting's own conservation law).
        let total_table_rows: usize = pages
            .iter()
            .flat_map(|p| p.frame.blocks.iter())
            .filter_map(|b| b.table_placement.as_ref())
            .map(|t| t.rows.len())
            .sum();
        assert_eq!(total_table_rows, 3, "every table row must appear exactly once across whatever pages the table spans");

        // The list must place all 4 items with real, non-zero indent.
        let list_placement = pages
            .iter()
            .flat_map(|p| p.frame.blocks.iter())
            .find_map(|b| b.list_placement.as_ref())
            .expect("the bulleted list must be placed on some page");
        assert_eq!(list_placement.items.len(), 4);
        for item in &list_placement.items {
            let content_block = item.content.first().expect("every item has content");
            assert!(content_block.rect.x > item.marker_rect.x, "item content must indent past its own marker");
        }
        let wrapped_item = &list_placement.items[1];
        let wrapped_lines = wrapped_item.content[0].paragraph_layout.as_ref().expect("list item paragraph carries a layout").lines.len();
        assert!(wrapped_lines > 1, "the second list item's fixture text is long enough that it must wrap to more than one line");

        // Neither keep-with-next heading is ever the LAST block on its own
        // page while its immediate follower starts the next one.
        for page in &pages {
            if let Some(last) = page.frame.blocks.last() {
                let is_heading = matches!(last.kind, Block::Paragraph(p) if p.runs[0].font.bold && p.runs[0].text.starts_with("Section"));
                assert!(!is_heading || page.frame.overflow.is_none(), "a keep-with-next heading must never be orphaned alone at a page bottom");
            }
        }

        let spec = ExportSpec { width_px: PAGE_WIDTH, height_px: PAGE_HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let theme = crate::style::Theme::light_report();

        for (i, page) in pages.iter().enumerate() {
            let bytes = render_to_png(&spec, |ctx| draw_page(ctx, page, &theme)).expect("page proof render should succeed");
            assert_eq!(decoded_png_dims(&bytes), (PAGE_WIDTH, PAGE_HEIGHT));
            write_proof_png(&format!("typeset_p1_page{}.png", i + 1), &bytes);
        }
    }

    /// P2 headless proof (design law 8 + this arc's own "themes visibly
    /// differ, geometry doesn't" promise, §7 P2 risk note: "theme never
    /// changes metrics"): the SAME seeded 2-page report, composed once,
    /// painted under TWO themes (`Theme::light_report`/
    /// `Theme::dark_deck`) — proves (a) every placed block's rect is
    /// bit-identical between the two themes (their `DesignTokens` font
    /// sizes are deliberately equal — a theme must never change layout
    /// geometry on its own), (b) colors visibly differ (asserted via
    /// `color_hex`, confirmed by eye in the PNGs), and (c) every page
    /// shows the master's header/footer + the correct `"n of total"`
    /// page number (design doc §4.1's `page_number_token`, "resolved
    /// per-page during slicing").
    #[test]
    fn same_scene_under_two_themes_shares_identical_geometry_but_differs_in_paint_with_a_footer_page_number_on_every_page() {
        use crate::master::{PageNumberFormat, PageNumberStyle};
        use crate::style::{ColorRole, Theme};

        const CAPTION_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 11.0, bold: false, italic: false };

        let body_width = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0)).body_rect().width;

        let header_run = [StyledRun::new("Case Report — Confidential (uzor-typeset P2 Proof)", CAPTION_FONT)];
        let header_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&header_run, body_width)))];
        let footer_run = [StyledRun::new("uzor-typeset — Arc 4 Phase P2", CAPTION_FONT)];
        let footer_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&footer_run, body_width)))];

        let flow = seeded_flow();
        let master = PageMaster::new(PAGE_WIDTH as f64, PAGE_HEIGHT as f64, Margins::uniform(40.0))
            .with_header(&header_flow)
            .with_footer(&footer_flow)
            .with_page_number(PageNumberStyle::new(PageNumberFormat::OfTotal, 1));

        let shaper = CosmicShaper::headless();
        let light = Theme::light_report();
        let dark = Theme::dark_deck();

        let light_style = ComposeStyle::from_theme(&light, 14.0);
        let dark_style = ComposeStyle::from_theme(&dark, 14.0);
        assert_eq!(
            light_style.default_font.size_px, dark_style.default_font.size_px,
            "the two proof themes share identical font sizes on purpose — only color may differ"
        );

        let light_pages = slice_pages(&flow, &master, &light_style, &shaper);
        let dark_pages = slice_pages(&flow, &master, &dark_style, &shaper);

        assert_eq!(light_pages.len(), 2, "fixture is tuned to land on exactly 2 pages, got {}", light_pages.len());
        assert_eq!(light_pages.len(), dark_pages.len(), "theme must never change how many pages the SAME content slices into");

        for (l, d) in light_pages.iter().zip(dark_pages.iter()) {
            assert_eq!(l.frame.blocks.len(), d.frame.blocks.len(), "theme must never change how many blocks land on a page");
            for (lb, db) in l.frame.blocks.iter().zip(d.frame.blocks.iter()) {
                assert_eq!(lb.rect, db.rect, "theme must never change a placed block's own geometry when font sizes are identical");
            }
        }

        // Every page (both themes) carries the master's header/footer
        // content and the correct "n of total" page number.
        for pages in [&light_pages, &dark_pages] {
            for (i, page) in pages.iter().enumerate() {
                assert!(page.header.as_ref().is_some_and(|h| !h.blocks.is_empty()), "every page must carry the master's header content");
                assert!(page.footer.as_ref().is_some_and(|f| !f.blocks.is_empty()), "every page must carry the master's footer content");
                let number = page.page_number.as_ref().expect("page-number token was configured on the master");
                assert_eq!(number.text, format!("{} of {}", i + 1, pages.len()));
            }
        }

        let light_bg = rgba_from_hex(light.color_rgb(ColorRole::Background));
        let dark_bg = rgba_from_hex(dark.color_rgb(ColorRole::Background));
        let spec_light = ExportSpec { width_px: PAGE_WIDTH, height_px: PAGE_HEIGHT, dpr: 1.0, background: Some(light_bg) };
        let spec_dark = ExportSpec { width_px: PAGE_WIDTH, height_px: PAGE_HEIGHT, dpr: 1.0, background: Some(dark_bg) };

        let light_page1 = render_to_png(&spec_light, |ctx| draw_page(ctx, &light_pages[0], &light)).expect("light page 1 proof render should succeed");
        assert_eq!(decoded_png_dims(&light_page1), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p2_light_page1.png", &light_page1);

        let light_page2 = render_to_png(&spec_light, |ctx| draw_page(ctx, &light_pages[1], &light)).expect("light page 2 proof render should succeed");
        assert_eq!(decoded_png_dims(&light_page2), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p2_light_page2.png", &light_page2);

        let dark_page1 = render_to_png(&spec_dark, |ctx| draw_page(ctx, &dark_pages[0], &dark)).expect("dark page 1 proof render should succeed");
        assert_eq!(decoded_png_dims(&dark_page1), (PAGE_WIDTH, PAGE_HEIGHT));
        write_proof_png("typeset_p2_dark_page1.png", &dark_page1);

        assert_ne!(
            light.color_hex(ColorRole::Background),
            dark.color_hex(ColorRole::Background),
            "the two themes must paint visibly different backgrounds"
        );
        assert_ne!(light.color_hex(ColorRole::Ink), dark.color_hex(ColorRole::Ink), "the two themes must paint visibly different ink");
    }

    /// `0xRRGGBB` -> straight, opaque RGBA bytes — the shape
    /// `uzor_export::ExportSpec::background` takes.
    fn rgba_from_hex(rgb: u32) -> [u8; 4] {
        [((rgb >> 16) & 0xff) as u8, ((rgb >> 8) & 0xff) as u8, (rgb & 0xff) as u8, 255]
    }

    /// P3 headless proof (design law 8 + design doc §4.2's own demo ask:
    /// "the SAME `SceneSpec` ... sliced three ways ... proving one
    /// content model / three targets"): a fixed seeded 3-block-group
    /// deck-ish scene — a title card, a bulleted-list card, and a
    /// `BarFigure` card — sliced BOTH ways from the exact SAME `flow`:
    /// (a) card mode (`slice_cards`, fluid, natural per-card height) to
    /// `uzor/out/typeset_p3_card{0,1,2}.png`, and (b) fixed mode
    /// (`slice_slides`, a 960x540 PowerPoint-shaped viewport) to
    /// `uzor/out/typeset_p3_slide{0,1,2}.png`. `ExportSpec::background:
    /// None` (fully transparent canvas) on every render deliberately
    /// isolates `draw_card`/`draw_slide`'s OWN theme-background painting
    /// — if either function forgot to paint its background, the PNG
    /// would show transparent pixels instead of the theme's chrome
    /// color, not a silent pass.
    ///
    /// This fixture is tuned so every card's natural height sits
    /// comfortably under 540px — nothing overflows the fixed viewport
    /// (`SlideOverflow::Report` never fires here); `slice::slides`'s own
    /// unit tests already cover `Report`/`Shrink` actually firing on
    /// content that doesn't fit.
    #[test]
    fn seeded_three_card_deck_slices_both_as_fluid_cards_and_as_fixed_slides() {
        use uzor_figures::BarFigure;

        use crate::compose::BreakControl;
        use crate::scene::{BlockSizing, FigureBlock, ListBlock, ListItem, MarkerStyle};
        use crate::slice::{slice_cards, slice_slides, SlideOverflow};

        const DECK_WIDTH: u32 = 960;
        const DECK_HEIGHT: u32 = 540;

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 32.0, bold: true, italic: false };
        const SUBTITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 18.0, bold: false, italic: false };
        const BULLET_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 18.0, bold: false, italic: false };

        let title_run = [StyledRun::new("Case Deck — uzor-typeset P3 Proof", TITLE_FONT)];
        let subtitle_run = [StyledRun::new("Card mode vs. fixed-slide mode, same content model", SUBTITLE_FONT)];

        let li1_run = [StyledRun::new("Confirmed direct transfer between wallet A and the exchange hot wallet.", BULLET_FONT)];
        let li1_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li1_run, f64::MAX)))];
        let li2_run = [StyledRun::new("Secondary relay observed roughly forty-eight hours later.", BULLET_FONT)];
        let li2_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li2_run, f64::MAX)))];
        let li3_run = [StyledRun::new("Resting balance identified at custodial cold storage.", BULLET_FONT)];
        let li3_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li3_run, f64::MAX)))];
        let list_items = [ListItem::new(&li1_nodes), ListItem::new(&li2_nodes), ListItem::new(&li3_nodes)];

        let bar_figure = BarFigure::new(
            vec!["Hop 1".to_owned(), "Hop 2".to_owned(), "Hop 3".to_owned(), "Hop 4".to_owned()],
            vec![128_500.0, 640_000.0, 2_600_000.0, 300_000.0],
        )
        .with_title("Observed amount per hop (seeded)");
        let figure_caption_run = [StyledRun::new("Figure — seeded bar chart, deterministic fixture.", SUBTITLE_FONT)];

        let body_width = DECK_WIDTH as f64 - 80.0; // 40px margin either side, this crate's own convention

        let flow = vec![
            // Card 0 — title.
            BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))),
            BlockNode::new(Block::Spacer(12.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&subtitle_run, body_width))),
            // Card 1 — bullets.
            BlockNode::new(Block::List(ListBlock::new(&list_items, MarkerStyle::Bullet('•'), 24.0)))
                .with_break_control(BreakControl::ForceBefore),
            // Card 2 — figure + caption.
            BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(320.0))))
                .with_break_control(BreakControl::ForceBefore),
            BlockNode::new(Block::Spacer(12.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&figure_caption_run, body_width))),
        ];

        let style = ComposeStyle::new(10.0, BULLET_FONT);
        let shaper = CosmicShaper::headless();

        let cards = slice_cards(&flow, body_width, &style, &shaper);
        assert_eq!(cards.len(), 3, "two ForceBefore markers must split the deck into exactly 3 cards");
        for card in &cards {
            assert!(
                card.natural_height < DECK_HEIGHT as f64,
                "fixture must be tuned so every card fits comfortably under the fixed deck height, got {}",
                card.natural_height
            );
        }

        let slides = slice_slides(&flow, DECK_WIDTH as f64, DECK_HEIGHT as f64, SlideOverflow::Report, &style, &shaper)
            .expect("fixture is tuned to fit the fixed 960x540 viewport with no overflow");
        assert_eq!(slides.len(), 3);
        for slide in &slides {
            assert!(slide.shrink_scale.is_none(), "fixture must fit without engaging a shrink transform");
        }

        // "Conversion first-class": every card's own block ids/order match
        // its slide counterpart's exactly (content fits both ways).
        for (card, slide) in cards.iter().zip(slides.iter()) {
            let card_ids: Vec<_> = card.frame.blocks.iter().map(|b| b.id).collect();
            let slide_ids: Vec<_> = slide.frame.blocks.iter().map(|b| b.id).collect();
            assert_eq!(card_ids, slide_ids, "card <-> slide conversion must preserve block ids and order");
        }

        let theme = crate::style::Theme::light_report();

        for (i, card) in cards.iter().enumerate() {
            let card_height = card.natural_height.ceil().max(1.0) as u32;
            let spec = ExportSpec { width_px: DECK_WIDTH, height_px: card_height, dpr: 1.0, background: None };
            let bytes = render_to_png(&spec, |ctx| draw_card(ctx, card, &theme)).expect("card proof render should succeed");
            assert_eq!(decoded_png_dims(&bytes), (DECK_WIDTH, card_height));
            // `background: None` means the CANVAS starts fully
            // transparent — a fully-opaque corner pixel proves
            // `draw_card` painted its OWN background from the theme
            // rather than relying on a caller-set export background.
            assert_eq!(decoded_png_pixel(&bytes, 2, 2)[3], 255, "draw_card must paint its own opaque background from the theme");
            write_proof_png(&format!("typeset_p3_card{i}.png"), &bytes);
        }

        let slide_spec = ExportSpec { width_px: DECK_WIDTH, height_px: DECK_HEIGHT, dpr: 1.0, background: None };
        for (i, slide) in slides.iter().enumerate() {
            let bytes = render_to_png(&slide_spec, |ctx| draw_slide(ctx, slide, &theme)).expect("slide proof render should succeed");
            assert_eq!(decoded_png_dims(&bytes), (DECK_WIDTH, DECK_HEIGHT));
            // Same background-ownership proof, checked far below any
            // placed content (every card fits well under the fixed
            // viewport height) — still opaque, so `draw_slide` paints the
            // FULL fixed viewport's background, not just the content's
            // own extent.
            assert_eq!(
                decoded_png_pixel(&bytes, DECK_WIDTH - 2, DECK_HEIGHT - 2)[3],
                255,
                "draw_slide must paint its own opaque background across the WHOLE fixed viewport"
            );
            write_proof_png(&format!("typeset_p3_slide{i}.png"), &bytes);
        }
    }

    /// P4 headless proof (design law 8 + this arc's own build-steps ask):
    /// a fixed seeded 2-build-step slide — a title (stays fixed the whole
    /// time), a `BarFigure` (present in both steps, but explicitly
    /// overridden to sit narrow-and-right in step 0, natural-and-left in
    /// step 1 — "moves right -> left"), and a bulleted list (present ONLY
    /// in step 1 -> fades in) — sliced via [`crate::slice::
    /// slice_build_steps`], morphed via [`crate::kinetics::
    /// build_frame_morph`], and sampled/painted at `t = 0.0 / 0.5 / 1.0` to
    /// `uzor/out/typeset_p4_morph_t{0,05,1}.png`.
    #[test]
    fn build_step_morph_proof_strip_title_stays_figure_slides_left_list_fades_in() {
        use uzor::types::Rect;
        use uzor_figures::BarFigure;

        use crate::kinetics::build_frame_morph;
        use crate::scene::{BlockId, BlockSizing, FigureBlock, ListBlock, ListItem, MarkerStyle};
        use crate::slice::{slice_build_steps, BlockOverride, BuildStep};

        const DECK_WIDTH: f64 = 960.0;
        const DECK_HEIGHT: f64 = 540.0;
        const MARGIN: f64 = 40.0;

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 30.0, bold: true, italic: false };
        const BULLET_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 18.0, bold: false, italic: false };

        let title_id = BlockId(1);
        let figure_id = BlockId(2);
        let list_id = BlockId(3);

        let title_run = [StyledRun::new("Build Steps — uzor-typeset P4 Proof", TITLE_FONT)];
        let bar_figure = BarFigure::new(
            vec!["Hop 1".to_owned(), "Hop 2".to_owned(), "Hop 3".to_owned()],
            vec![128_500.0, 640_000.0, 300_000.0],
        )
        .with_title("Observed amount per hop (seeded)");

        let li1_run = [StyledRun::new("First reveal bullet.", BULLET_FONT)];
        let li1_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li1_run, f64::MAX)))];
        let li2_run = [StyledRun::new("Second reveal bullet.", BULLET_FONT)];
        let li2_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&li2_run, f64::MAX)))];
        let list_items = [ListItem::new(&li1_nodes), ListItem::new(&li2_nodes)];

        // Compose into a MARGIN-inset "body" region (matching this crate's
        // own page-margin convention elsewhere) — painted translated by
        // `(MARGIN, MARGIN)` below, so every rect this fixture computes
        // (including the "right" override) is in body-LOCAL coordinates.
        let body_width = DECK_WIDTH - 2.0 * MARGIN;
        let body_height = DECK_HEIGHT - 2.0 * MARGIN;

        let flow = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))).with_id(title_id),
            BlockNode::new(Block::Spacer(20.0)),
            BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(220.0)))).with_id(figure_id),
            BlockNode::new(Block::Spacer(20.0)),
            BlockNode::new(Block::List(ListBlock::new(&list_items, MarkerStyle::Bullet('•'), 24.0))).with_id(list_id),
        ];

        let style = ComposeStyle::new(10.0, BULLET_FONT);
        let shaper = CosmicShaper::headless();

        // Probe: compose the WHOLE flow once (every block visible, no
        // overrides) purely to learn the figure's own NATURAL (left-
        // aligned) rect — the step-0 override below shifts it right of
        // that, never a hand-picked number independent of what
        // `compose()` actually produces.
        let probe_visible = [title_id, figure_id, list_id];
        let probe_step = BuildStep::new(&probe_visible, &[]);
        let probe = slice_build_steps(&flow, std::slice::from_ref(&probe_step), body_width, body_height, &style, &shaper);
        let natural_figure_rect =
            probe[0].blocks.iter().find(|b| b.id == figure_id).map(|b| b.rect).expect("probe must place the figure");

        let narrow_width = natural_figure_rect.width * 0.5;
        let right_rect = Rect::new(body_width - narrow_width, natural_figure_rect.y, narrow_width, natural_figure_rect.height);

        let step0_visible = [title_id, figure_id];
        let step0_overrides = [(figure_id, BlockOverride { rect: right_rect })];
        let step0 = BuildStep::new(&step0_visible, &step0_overrides);

        let step1_visible = [title_id, figure_id, list_id];
        let step1 = BuildStep::new(&step1_visible, &[]);

        let frames = slice_build_steps(&flow, &[step0, step1], body_width, body_height, &style, &shaper);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].blocks.len(), 2, "step 0 shows only the title + figure, list not yet visible");
        assert_eq!(frames[1].blocks.len(), 3, "step 1 shows all three blocks");
        let step0_figure_rect = frames[0].blocks.iter().find(|b| b.id == figure_id).expect("step 0 must place the figure").rect;
        assert_eq!(step0_figure_rect, right_rect, "step 0's override must place the figure narrow-and-right");

        let morph = build_frame_morph(&frames[0], &frames[1], &[]);
        assert_eq!(morph.matched_count(), 2, "title and figure match by id across the two steps");
        assert_eq!(morph.from_only_count(), 0, "nothing disappears between step 0 and step 1");
        assert_eq!(morph.to_only_count(), 1, "the bullet list is new in step 1 -> fades in, not matched");

        let theme = crate::style::Theme::light_report();
        let spec = ExportSpec { width_px: DECK_WIDTH as u32, height_px: DECK_HEIGHT as u32, dpr: 1.0, background: Some([255, 255, 255, 255]) };

        for (t, name) in [(0.0, "typeset_p4_morph_t0.png"), (0.5, "typeset_p4_morph_t05.png"), (1.0, "typeset_p4_morph_t1.png")] {
            let states = morph.sample(t, 0x111111);
            for state in &states {
                assert!(
                    state.rect.x.is_finite() && state.rect.y.is_finite() && state.rect.width.is_finite() && state.rect.height.is_finite(),
                    "no NaN/inf in any sampled rect at t={t}"
                );
                assert!((0.0..=1.0).contains(&state.opacity), "opacity must stay in [0,1] at t={t}, got {}", state.opacity);
            }

            // The figure must genuinely be mid-flight (strictly between
            // its two endpoint x positions) at t=0.5, and land exactly on
            // its own endpoint at t=0/t=1 -- the "moves right -> left"
            // claim, checked as data, not just eyeballed.
            let figure_state = states.iter().find(|s| s.id == figure_id).expect("figure state present at every t");
            if t == 0.0 {
                assert_eq!(figure_state.rect, right_rect);
            } else if t == 1.0 {
                assert_eq!(figure_state.rect, natural_figure_rect);
            } else {
                assert!(
                    figure_state.rect.x < right_rect.x && figure_state.rect.x > natural_figure_rect.x,
                    "the figure must sit strictly between its right-side start and its natural left-side end at t=0.5"
                );
            }

            let bytes = render_to_png(&spec, |ctx| {
                ctx.save();
                ctx.translate(MARGIN, MARGIN);
                for state in &states {
                    draw_frame_state(ctx, state, &theme);
                }
                ctx.restore();
            })
            .unwrap_or_else(|e| panic!("build-step morph proof render at t={t} should succeed: {e}"));
            assert_eq!(decoded_png_dims(&bytes), (DECK_WIDTH as u32, DECK_HEIGHT as u32));
            write_proof_png(name, &bytes);
        }
    }

    /// A deterministic, generated "photo" RGBA buffer — layered gradients
    /// + two circles so it reads as a real picture in the multi-column/
    /// island proof PNGs below (design law 8: no binary assets, no RNG —
    /// every pixel is a pure function of its own `(x, y)` and the
    /// caller's own `seed`, which only shifts the palette a little so two
    /// islands on the same page/demo don't look identical).
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

    /// Test 5 (task gate): a headless render of a `Block::Image` must
    /// paint REAL pixels — `RenderContext::image_painter()` is real on the
    /// `uzor-export::render_to_png` backend now (P1 gap CLOSED, see this
    /// module's own "Image blocks" doc comment) — proven as data: a pixel
    /// inside the image's own `content_rect` must be the image's OWN
    /// color (not the white export background), and a pixel along the
    /// image's own rect edge must NOT be the old dashed-magenta
    /// placeholder color (`0xff00ffff`) — the placeholder never fills,
    /// only strokes a border, so this distinguishes a real composite from
    /// the old fallback unambiguously.
    #[test]
    fn image_block_paints_real_pixels_not_the_placeholder() {
        use crate::scene::{BlockSizing, ImageBlock, ImageFit};

        const IMG_W: u32 = 40;
        const IMG_H: u32 = 40;
        // A solid, deliberately non-background, non-placeholder-magenta
        // color (opaque teal) — a real composite paints this at every
        // interior pixel.
        let mut rgba = vec![0u8; (IMG_W * IMG_H * 4) as usize];
        for px in rgba.chunks_mut(4) {
            px.copy_from_slice(&[20, 180, 200, 255]);
        }

        let image = ImageBlock::new(&rgba, IMG_W, IMG_H, BlockSizing::FixedHeight(120.0), ImageFit::Stretch);
        let flow = [BlockNode::new(Block::Image(image))];

        let master = PageMaster::new(300.0, 300.0, Margins::uniform(20.0));
        let style = ComposeStyle::new(0.0, FontSpec::new(FontFamily::Roboto, 14.0));
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        let placed = pages[0].frame.blocks.first().expect("image placed");

        let spec = ExportSpec { width_px: 300, height_px: 300, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let theme = crate::style::Theme::light_report();
        let bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("image proof render should succeed");
        write_proof_png("typeset_image_real_paint_proof.png", &bytes);

        let cx = (placed.rect.x + placed.rect.width / 2.0).round() as u32;
        let cy = (placed.rect.y + placed.rect.height / 2.0).round() as u32;
        let center = decoded_png_pixel(&bytes, cx, cy);
        assert_ne!(center, [255, 255, 255, 255], "a real image composite must paint OVER the white export background at the image's own center");
        assert!(
            center[2] > center[1] && center[0] < 100,
            "center pixel must read as the image's own teal-ish color (low red, blue > green), got {center:?}"
        );

        let edge_x = placed.rect.x.round() as u32;
        let edge_y = (placed.rect.y + 2.0).round() as u32;
        let edge = decoded_png_pixel(&bytes, edge_x, edge_y);
        assert_ne!(edge, [255, 0, 255, 255], "the old dashed-magenta placeholder must never appear once a real image composite is available");
    }

    /// Demo proof (multi-column layout): one A4 page, a full-width
    /// heading (painted via `PageMaster::with_header`'s own top margin
    /// band, sized generously) above body text flowing in 2 columns
    /// (justified, hyphenation on, ~18px column gap) —
    /// `out/typeset_layout_columns.png`.
    #[test]
    fn two_column_page_with_full_width_heading_demo() {
        const PAGE_W: u32 = 595;
        const PAGE_H: u32 = 842;
        const HEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 26.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };

        // A generous top margin band (90px) gives the full-width heading
        // real room, painted via the master's own header margin-box —
        // the SAME production `slice_pages`/`draw_page` pipeline every
        // other proof in this file already uses, not a bespoke one.
        let margins = crate::master::Margins::new(90.0, 40.0, 40.0, 40.0);
        let master = PageMaster::new(PAGE_W as f64, PAGE_H as f64, margins).with_columns(2, 18.0);
        let body_width = master.column_width();

        let heading_run = [StyledRun::new("Case Report — Two-Column Layout", HEADING_FONT)];
        let heading_flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&heading_run, master.body_rect().width)))];
        let master = master.with_header(&heading_flow);

        const FILLER_A: &str = "The confidence-matrix narrative for this section walks through every traced hop in \
            order, noting timestamps, counterparties, and the amount observed at each step of the flow, so a \
            reader can follow the chain of custody from the initial deposit all the way through to the resting \
            balance without needing a separate table for cross-reference.";
        const FILLER_B: &str = "Business documents full of long compound words like implementation, infrastructure, \
            counterparty, and accountability often expose uneven interword spacing in a narrow justified column, \
            especially once hyphenation is enabled and a break lands mid-word near the right margin, which is \
            exactly the case this fixture is built to show.";

        let filler_a_run = [StyledRun::new(FILLER_A, BODY_FONT)];
        let filler_b_run = [StyledRun::new(FILLER_B, BODY_FONT)];

        let mut flow: Vec<BlockNode<'_>> = Vec::new();
        for i in 0..6 {
            let run: &[StyledRun<'_>; 1] = if i % 2 == 0 { &filler_a_run } else { &filler_b_run };
            flow.push(BlockNode::new(Block::Paragraph(
                Paragraph::new(run, body_width)
                    .with_align(ParagraphAlign::Justify)
                    .with_break_strategy(BreakStrategy::KnuthPlass)
                    .with_hyphenation(Hyphenation::English),
            )));
            flow.push(BlockNode::new(Block::Spacer(12.0)));
        }

        let style = ComposeStyle::new(10.0, BODY_FONT);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "fixture must be tuned to fit ONE A4 page across 2 columns, got {}", pages.len());
        assert!(!pages[0].extra_frames.is_empty(), "content must actually spill from column 1 into column 2");

        // Column 1 and column 2 must never overlap horizontally, and
        // neither ever exceeds its own column's width.
        let col1_width = pages[0].frame.region.rect.width;
        let col2_rect = pages[0].extra_frames[0].region.rect;
        assert!(col2_rect.x >= pages[0].frame.region.rect.x + col1_width, "column 2 must sit to the right of column 1, past its own gap");
        for placed in &pages[0].frame.blocks {
            assert!(placed.rect.width <= col1_width + 0.01, "column 1 text must never exceed column 1's own width");
        }
        for placed in &pages[0].extra_frames[0].blocks {
            assert!(placed.rect.width <= col2_rect.width + 0.01, "column 2 text must never exceed column 2's own width");
        }

        let theme = crate::style::Theme::light_report();
        let spec = ExportSpec { width_px: PAGE_W, height_px: PAGE_H, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("columns demo render should succeed");
        assert_eq!(decoded_png_dims(&bytes), (PAGE_W, PAGE_H));
        write_proof_png("typeset_layout_columns.png", &bytes);
    }

    /// Demo proof (THE money shot): a centered image anchored in the flow
    /// via [`crate::scene::Block::Island`] — text flows down the LEFT
    /// strip beside it, then the RIGHT strip, then resumes at FULL body
    /// width below the image — `out/typeset_layout_island_center.png`.
    /// Island margin ~10px, matching the task's own professional-margins
    /// ask.
    #[test]
    fn island_center_anchor_text_wraps_both_strips_then_resumes_full_width_demo() {
        use uzor_text::layout_paragraph;

        use crate::scene::{AnchoredIsland, BlockSizing, ImageBlock, ImageFit, IslandAnchor};

        const PAGE_W: u32 = 595;
        const PAGE_H: u32 = 842;
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };
        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 24.0, bold: true, italic: false };
        const PARAGRAPH_SPACING: f64 = 10.0;

        let master = PageMaster::new(PAGE_W as f64, PAGE_H as f64, Margins::uniform(40.0));
        let body_width = master.body_rect().width;
        let shaper = CosmicShaper::headless();

        const ISLAND_W: f64 = 220.0;
        const ISLAND_MARGIN: f64 = 10.0;
        let strip_width = (body_width - ISLAND_W - 2.0 * ISLAND_MARGIN) / 2.0;

        const STRIP_A: &str = "Every wallet observed in this section routes through a single custodial exchange before resting.";
        const STRIP_B: &str = "Layering activity across several intermediate wallets is consistent with an attempt to obscure origin.";
        const STRIP_C: &str = "Two relay hops separated by roughly forty eight hours moved a comparable fraction of the total.";
        const STRIP_D: &str = "The final resting balance sits in cold storage pending a subpoena response from the custodian.";
        const BELOW: &str = "Once the centered figure's own vertical band ends, the remaining narrative resumes at the FULL \
            body width below it, exactly as an ordinary paragraph would — no strip constraint carries past the \
            image's own bottom edge, and every line here spans the entire page body from margin to margin.";

        let strip_a_run = [StyledRun::new(STRIP_A, BODY_FONT)];
        let strip_b_run = [StyledRun::new(STRIP_B, BODY_FONT)];
        let strip_c_run = [StyledRun::new(STRIP_C, BODY_FONT)];
        let strip_d_run = [StyledRun::new(STRIP_D, BODY_FONT)];

        // Size the island's own band so the LEFT strip holds (A + B) and
        // the RIGHT strip holds (C + D) almost exactly — measured via the
        // SAME `layout_paragraph` compose itself uses, never guessed —
        // leaving too little slack for the "below" paragraph to sneak
        // even one more line into either strip.
        let h_a = layout_paragraph(&Paragraph::new(&strip_a_run, strip_width), &shaper).height;
        let h_b = layout_paragraph(&Paragraph::new(&strip_b_run, strip_width), &shaper).height;
        let h_c = layout_paragraph(&Paragraph::new(&strip_c_run, strip_width), &shaper).height;
        let h_d = layout_paragraph(&Paragraph::new(&strip_d_run, strip_width), &shaper).height;
        let island_height = (h_a + PARAGRAPH_SPACING + h_b).max(h_c + PARAGRAPH_SPACING + h_d) + 2.0;

        let photo = generated_photo_rgba(220, island_height.round().max(1.0) as u32, 0);
        let island = AnchoredIsland::new(
            ImageBlock::new(&photo, 220, island_height.round().max(1.0) as u32, BlockSizing::FixedHeight(island_height), ImageFit::Cover),
            IslandAnchor::Center,
            ISLAND_W,
            ISLAND_MARGIN,
        );

        let title_run = [StyledRun::new("Centered Island — Text Wraps Both Sides", TITLE_FONT)];
        let below_run = [StyledRun::new(BELOW, BODY_FONT)];

        let flow = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))),
            BlockNode::new(Block::Spacer(16.0)),
            BlockNode::new(Block::Island(island)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&strip_a_run, body_width))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&strip_b_run, body_width))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&strip_c_run, body_width))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&strip_d_run, body_width))),
            BlockNode::new(Block::Spacer(16.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&below_run, body_width))),
        ];

        let style = ComposeStyle::new(PARAGRAPH_SPACING, BODY_FONT);
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "fixture must be tuned to fit on ONE page, got {}", pages.len());

        let island_placement = pages[0].frame.blocks.iter().find(|b| matches!(b.kind, Block::Island(_))).expect("island placed");
        let below_placement = pages[0]
            .frame
            .blocks
            .iter()
            .find(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == BELOW))
            .expect("below-the-island paragraph placed");
        assert!(
            below_placement.rect.y >= island_placement.rect.y + island_placement.rect.height - 0.5,
            "content must resume below the island's own bottom edge, got y={} vs island bottom={}",
            below_placement.rect.y,
            island_placement.rect.y + island_placement.rect.height
        );
        assert!((below_placement.rect.width - body_width).abs() < 1e-6, "content below the island must resume at full body width, got {}", below_placement.rect.width);

        // Neither strip paragraph ever overlaps the island's own rect.
        for text in [STRIP_A, STRIP_B, STRIP_C, STRIP_D] {
            if let Some(placed) = pages[0].frame.blocks.iter().find(|b| matches!(b.kind, Block::Paragraph(p) if p.runs[0].text == text)) {
                let inside_left = placed.rect.x + placed.rect.width <= island_placement.rect.x + 0.5;
                let inside_right = placed.rect.x >= island_placement.rect.x + island_placement.rect.width - 0.5;
                assert!(inside_left || inside_right, "strip text {text:?} must sit beside the island, never overlapping it (rect {:?})", placed.rect);
            }
        }

        let theme = crate::style::Theme::light_report();
        let spec = ExportSpec { width_px: PAGE_W, height_px: PAGE_H, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("island center demo render should succeed");
        assert_eq!(decoded_png_dims(&bytes), (PAGE_W, PAGE_H));
        write_proof_png("typeset_layout_island_center.png", &bytes);
    }

    /// Demo proof: one page, an image anchored LEFT (text runs in a
    /// strip to its right) followed further down the page by a second
    /// image anchored RIGHT (text runs in a strip to its left) —
    /// `out/typeset_layout_island_left_right.png`.
    #[test]
    fn island_left_and_right_anchor_demo() {
        use crate::scene::{AnchoredIsland, BlockSizing, ImageBlock, ImageFit, IslandAnchor};

        const PAGE_W: u32 = 595;
        const PAGE_H: u32 = 842;
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };
        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };

        let master = PageMaster::new(PAGE_W as f64, PAGE_H as f64, Margins::uniform(40.0));
        let body_width = master.body_rect().width;

        const ISLAND_W: f64 = 190.0;
        const ISLAND_MARGIN: f64 = 10.0;
        const ISLAND_H: f64 = 190.0;

        let photo_left = generated_photo_rgba(190, 190, 1);
        let photo_right = generated_photo_rgba(190, 190, 2);

        let island_left = AnchoredIsland::new(
            ImageBlock::new(&photo_left, 190, 190, BlockSizing::FixedHeight(ISLAND_H), ImageFit::Cover),
            IslandAnchor::Left,
            ISLAND_W,
            ISLAND_MARGIN,
        );
        let island_right = AnchoredIsland::new(
            ImageBlock::new(&photo_right, 190, 190, BlockSizing::FixedHeight(ISLAND_H), ImageFit::Cover),
            IslandAnchor::Right,
            ISLAND_W,
            ISLAND_MARGIN,
        );

        let title_run = [StyledRun::new("Left- and Right-Anchored Islands on One Page", TITLE_FONT)];
        const BESIDE_LEFT: &str = "Text belonging to this exhibit flows down the strip to the RIGHT of the left-anchored \
            image, wrapping naturally within its own narrower column while the image sits fixed at the page's own \
            left margin.";
        const AFTER_LEFT: &str = "Once the left-anchored image's own vertical band ends, the narrative resumes at the \
            full page width below it, continuing the exhibit's own description without any further column constraint.";
        const BESIDE_RIGHT: &str = "This second exhibit anchors its own image to the RIGHT margin instead, so the \
            accompanying text now flows down the strip to its LEFT, mirroring the layout above.";
        const AFTER_RIGHT: &str = "And, symmetrically, once the right-anchored image's own band ends, the remaining \
            narrative again resumes at the full page width below it.";

        let beside_left_run = [StyledRun::new(BESIDE_LEFT, BODY_FONT)];
        let after_left_run = [StyledRun::new(AFTER_LEFT, BODY_FONT)];
        let beside_right_run = [StyledRun::new(BESIDE_RIGHT, BODY_FONT)];
        let after_right_run = [StyledRun::new(AFTER_RIGHT, BODY_FONT)];

        let flow = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))),
            BlockNode::new(Block::Spacer(16.0)),
            BlockNode::new(Block::Island(island_left)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&beside_left_run, body_width))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&after_left_run, body_width))),
            BlockNode::new(Block::Spacer(28.0)),
            BlockNode::new(Block::Island(island_right)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&beside_right_run, body_width))),
            BlockNode::new(Block::Paragraph(Paragraph::new(&after_right_run, body_width))),
        ];

        let style = ComposeStyle::new(10.0, BODY_FONT);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "fixture must be tuned to fit on ONE page, got {}", pages.len());

        let islands: Vec<_> = pages[0].frame.blocks.iter().filter(|b| matches!(b.kind, Block::Island(_))).collect();
        assert_eq!(islands.len(), 2, "both islands must be placed on the SAME page");
        assert!(islands[1].rect.y > islands[0].rect.y, "the right-anchored island must sit further down the page than the left-anchored one");
        assert_eq!(islands[0].rect.x, master.body_rect().x, "the Left-anchored island must sit at the body's own left edge");
        assert!(
            (islands[1].rect.x + islands[1].rect.width - (master.body_rect().x + master.body_rect().width)).abs() < 1e-6,
            "the Right-anchored island must sit flush with the body's own right edge"
        );

        let theme = crate::style::Theme::light_report();
        let spec = ExportSpec { width_px: PAGE_W, height_px: PAGE_H, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("island left/right demo render should succeed");
        assert_eq!(decoded_png_dims(&bytes), (PAGE_W, PAGE_H));
        write_proof_png("typeset_layout_island_left_right.png", &bytes);
    }
}
