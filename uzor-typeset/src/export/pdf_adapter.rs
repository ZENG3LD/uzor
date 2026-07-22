//! [`pages_to_pdf`] — the adapter converting composed `uzor-typeset::Page`s
//! into a real PDF byte buffer via `uzor-export`'s neutral `pdf` module
//! (Arc 4 Phase P5, design doc §6). This is the ONE place a `Page`'s
//! `ParagraphLayout`/`Frame`/`PlacedBlock` shape ever gets walked to
//! produce PDF content — `uzor-export` itself never imports this crate
//! (§6.1's dependency-boundary law).
//!
//! ## Vector fidelity, in two passes per page (typography-gap WAVE 1)
//!
//! 1. **Figures/tables/page chrome — real PDF vector ops.** Every
//!    non-paragraph block (figures, tables, images, list-marker glyphs,
//!    header/footer chrome OTHER than its own paragraph text, the
//!    page-number) paints THROUGH a fresh per-page
//!    [`uzor_export::PdfRenderContext`], via the SAME
//!    [`crate::render::draw_page_layers`] with
//!    `DrawLayers { paragraph_ink: false, paragraph_decorations: true }`
//!    this adapter uses (real paragraph glyph ink is deliberately
//!    suppressed here so it isn't painted twice — see pass 2 below — but
//!    underline/strikethrough decoration RECTS paint here, as real vector
//!    `fill_rect` ops, typography-gap WAVE 2) — but now `ctx` IS the real
//!    PDF page's own content stream, not an offscreen raster pixmap. This
//!    is the WAVE 1 cutover this module's own history predates: the
//!    previous phase (P5) rendered this same `DrawLayers`-suppressed pass
//!    into a whole-page raster background at `2x` via
//!    `uzor_export::render_to_png`; that whole-page raster is GONE — a
//!    page with no [`crate::scene::Block::Image`]/[`crate::scene::
//!    Block::Island`] content now carries no raster at all.
//! 2. **Real vector text runs** — every `Block::Paragraph`'s own
//!    `ParagraphLayout` (body flow AND header/footer content — both are
//!    `Block::Paragraph`s, walked by the SAME recursive collector) is
//!    converted into [`uzor_export::PdfTextRun`]s: consecutive
//!    NON-WHITESPACE glyphs on one line sharing the same `(font, color)`
//!    are merged into ONE run PER WORD (per-word granularity — a run
//!    additionally breaks at every whitespace cluster, which is skipped
//!    entirely rather than emitted as its own run), each positioned at
//!    its own FIRST glyph's already-resolved `(x, y)` baseline
//!    (`GlyphLayout::{x,y}` — already absolute within the paragraph's own
//!    box, translated by the placed block's own `PlacedBlock::rect`
//!    origin, design law 1: one transform, never a second position
//!    formula). Per-word (not per-line) matters for `ParagraphAlign::
//!    Justify`: justified interword stretching lives in each glyph's own
//!    already-resolved `x`, so re-advancing PDF text naturally (as a
//!    single merged-line run would with `Tj`'s own font-metric advance)
//!    would silently discard it, and would also accumulate kerning drift
//!    over long lines since a single run's advance is never re-anchored
//!    against the shaper's own resolved positions mid-run.
//!
//! ## What stayed raster-only under P5 — now vector too (WAVE 1 closes it)
//!
//! Table gridlines, list-item MARKER glyphs (`•`, `1.`, ...), and the
//! page-number text used to bake into the P5 whole-page raster (a
//! documented v1 escape hatch — "reproducing their exact rendered
//! baseline as PDF vector text would need the SAME font-shaping-backend
//! metrics a real `ParagraphLayout` resolves but these ad-hoc
//! `TextBaseline::Top`/`TextBaseline::Middle` calls never compute or
//! expose"). That reasoning is now MOOT: [`uzor_export::
//! PdfRenderContext::fill_text`] resolves its OWN real font metrics
//! (ascent, glyph ids) at call time regardless of `TextBaseline` variant
//! — `crate::render::draw_list_placement`'s marker glyphs and
//! `crate::render::draw_page_number`'s own `fill_text` call now emit REAL
//! `Tj` text through the exact same call these figures/tables already go
//! through, no special-casing needed at either call site. `Block::
//! Paragraph` content remains the only content converted through the
//! SEPARATE per-word merged-run path (pass 2) rather than `fill_text`
//! directly — see that pass's own reasoning below (justified-text
//! interword stretch).
//!
//! ## Coordinate + unit convention
//!
//! `1pt = 1px` (v1 — no separate press-DPI unit conversion; a page's own
//! `PageMaster::{width,height}` in px becomes the PDF page's size in pt
//! directly). `uzor_export::pdf`'s own doc comment already documents the
//! bottom-left-vs-top-left PDF coordinate flip; this adapter never
//! reasons about it directly (`PdfTextRun`/`PdfPageSpec` are already
//! top-left, same as every rect in this crate — and so is
//! [`PdfRenderContext`]'s own accumulated op list, per that type's own
//! module doc).
//!
//! ## Font handling (design doc §6.1: "one embedded font resource per
//! unique `FontSpec` ... deduped, never re-embedded per page")
//!
//! [`PdfFontCache`] (moved to `uzor-export` this pass — see that type's
//! own doc comment for why) registers each distinct `(FontFamily, bold,
//! italic)` combination with the SAME `uzor_export::PdfBuilder` exactly
//! once across the WHOLE document (never per-page), shared between this
//! module's own paragraph-run resolution below AND every
//! [`PdfRenderContext::fill_text`] call a figure/table/chrome draw issues
//! — a figure's axis-label font and a paragraph's body font never
//! register the same logical font twice under two different `/Font`
//! resource names on the same page.

use uzor::render::{Painter, ShapeHelpers};
use uzor::types::Rect;
use uzor_export::{FontId, PdfBuilder, PdfFontCache, PdfLink, PdfMeta, PdfOutlineEntry, PdfPageSpec, PdfRenderContext, PdfTagRole, PdfTextRun, StructElemId};
use uzor_text::ParagraphLayout;

use crate::master::PageMaster;
use crate::region::{Frame, PlacedBlock};
use crate::render::{draw_footnote_separator, draw_page_layers, draw_page_number, draw_placed_block, DrawLayers};
use crate::scene::Block;
use crate::slice::Page;
use crate::style::{ColorRole, Theme};

/// One collected vector text run, owning its own merged text string until
/// [`pages_to_pdf`] hands a borrowed `&str` to `uzor_export::PdfTextRun`.
struct CollectedRun {
    font: FontId,
    size_pt: f64,
    x_pt: f64,
    y_pt: f64,
    rgb: u32,
    text: String,
    /// Real shaped per-glyph advance (px == pt), one entry per `char` in
    /// `text` — typography-gap WAVE 2 per-glyph PDF kerning. Fed straight
    /// into `uzor_export::PdfTextRun::glyph_advances_pt`.
    glyph_advances_pt: Vec<f64>,
    /// Typography wave 5 — moved verbatim into `uzor_export::PdfTextRun::
    /// struct_tag`. `None` for every run collected while tagging is off
    /// (or for header/footer/footnote/list content — see
    /// [`TagMode`]'s own doc comment for this module's tagging scope).
    struct_tag: Option<StructElemId>,
}

/// Export-time options for [`pages_to_pdf`] (typography wave 5 — tagged
/// PDF + `/Lang`). `Default` matches this wave's own "default ON" ask:
/// `tagged: true`, `lang: None`.
#[derive(Debug, Clone)]
pub struct PdfExportOptions {
    /// Emit a real Tagged-PDF structure tree (`/MarkInfo`, `/StructTreeRoot`,
    /// balanced `BDC`/`EMC` marked content, `/ParentTree`, `/StructParents`)
    /// — see `uzor_export::pdf`'s own module doc comment for the full
    /// engine-side mechanism, and this module's own "Typography wave 5"
    /// section below for the adapter-side tag-mapping decisions. `false`
    /// makes [`pages_to_pdf`] take the EXACT pre-wave-5 code path (a
    /// single whole-page [`draw_page_layers`] call + untagged text-run
    /// collection) — never merely a downstream no-op on the SAME code
    /// path — so the emitted bytes are byte-for-byte identical to the
    /// pre-wave-5 writer.
    pub tagged: bool,
    /// RFC 3066 language tag (e.g. `"ru"`) for the Catalog's own `/Lang`
    /// entry — independent of `tagged` (a document can declare its own
    /// language regardless of whether it is a Tagged PDF).
    pub lang: Option<String>,
    /// Document metadata (`/Info` dictionary + optional hand-rolled XMP
    /// `/Metadata` stream when `meta.write_xmp` is set) — forwarded to
    /// [`uzor_export::PdfMeta`] verbatim, except `lang`: [`Self::lang`]
    /// wins when both are set (one language source of truth at THIS
    /// options boundary; `meta.lang` fills in only when `Self::lang` is
    /// `None`).
    pub meta: Option<PdfMeta>,
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self { tagged: true, lang: None, meta: None }
    }
}

/// How [`collect_text_runs_from_placed`]'s `Block::Paragraph` arm resolves
/// a collected run's own [`StructElemId`] (typography wave 5). A plain,
/// `Copy` enum threaded alongside the existing recursion rather than
/// folded into a richer type — every variant is a small, self-contained
/// decision the CALLER already knows before recursing one level deeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagMode {
    /// Tagging is off, or this content is out of this wave's tagging
    /// scope (header/footer/footnote/list content — see this module's
    /// own "Typography wave 5" doc section for the scope-narrowing
    /// reasoning). No `PdfBuilder::register_struct_elem` call happens at
    /// all for a run collected under this mode.
    None,
    /// Top-level body-flow content: resolve EACH paragraph independently
    /// to `H1`/`H2` (if its own plain text matches a pending, same-page
    /// [`crate::slice::OutlineEntry::title`]) or `P` otherwise.
    AutoHeading,
    /// Use this EXACT, already-registered element for every run collected
    /// under this mode — a table cell's own paragraph content is tagged
    /// directly as its enclosing `TableCell` leaf, never a nested `P`
    /// inside a `TD` (this module's own "Typography wave 5" tag-mapping
    /// decision).
    Fixed(StructElemId),
}

/// Convert `pages` (already composed + sliced via [`crate::slice::
/// slice_pages`]) into one complete PDF byte buffer.
///
/// **Divergence from this task's own literal signature** (report, not a
/// silent deviation): no `shaper` parameter. Every other `slice_*`/
/// `compose` function in this crate takes one because it measures text
/// for the FIRST time; `pages` here are already fully composed — every
/// `Block::Paragraph`'s `ParagraphLayout` (glyph positions, baselines,
/// line boxes) is already baked into its own `PlacedBlock`, and this
/// function never re-measures or re-shapes anything, only walks already-
/// resolved data. Carrying an unread `shaper` parameter to match the
/// brief's own literal text would be dead-weight this crate's own
/// established convention doesn't keep (see this crate's `CLAUDE.md` —
/// every prior phase's own divergence log reports a brief/doc mismatch
/// like this rather than carrying an unused parameter).
///
/// ## Document navigation (bookmarks + internal links)
///
/// No new parameter here either — `pages`' own already-attached
/// [`crate::slice::Page::outline`]/[`crate::slice::Page::links`] (filled
/// by `slice_pages`/`crate::toc::compose_document_with_toc` from
/// `.with_outline()`/`.with_link_target()`-tagged blocks) carry
/// everything this function needs: every page's own `outline` entries are
/// concatenated in document order and handed to
/// `uzor_export::PdfBuilder::set_outline` (skipped entirely when no page
/// carries any — additive, zero-cost for every pre-existing caller);
/// every page's own `links` become that SAME page's
/// `uzor_export::PdfPageSpec::links`. Both convert 1:1 (this module's own
/// "1pt = 1px" convention — a link/outline-destination rect lives in the
/// SAME page-pt coordinate space [`PdfTextRun`] positions already do).
///
/// ## Typography wave 5 — tagged PDF tag-mapping decisions
///
/// `options.tagged` (default `true`) drives a REAL Tagged-PDF structure
/// tree via `uzor_export::pdf`'s own new mechanism (see that crate's own
/// `CLAUDE.md`). This adapter is the ONLY place that ever decides a run's
/// own [`PdfTagRole`] — `uzor-export` itself holds no `Page`/
/// `ParagraphLayout`/outline knowledge (design law: engine-agnostic
/// boundary).
///
/// - **`P`/`H1`/`H2`** — every TOP-LEVEL `Block::Paragraph` (this page's
///   own `frame`/`extra_frames`, never header/footer/footnotes) is tagged
///   `H1`/`H2` when its own PLAIN TEXT (glyph clusters concatenated
///   verbatim) matches a PENDING, same-page [`crate::slice::OutlineEntry::
///   title`] at level 1/2 — the SAME title an outline-tagged heading's
///   own `.with_outline(level, title)` call supplied. A matched entry is
///   consumed (never reused for a second paragraph on the same page).
///   Everything else falls back to `P`, including: a heading whose own
///   OUTLINE title deliberately differs from its PAINTED text (this
///   crate's own showcase fixture intentionally does this once, for a
///   bilingual outline title — see that fixture's own comment), a level
///   3+ heading (out of this wave's own P/H1/H2 scope), and every
///   ordinary body paragraph. This is a DOCUMENTED, honest text-matching
///   heuristic (report, not silent): `PlacedBlock` carries no back-
///   reference to the `BlockNode`/`OutlineTag` that produced it (only
///   `Page::outline`, a separate, already-page-resolved list with no
///   block-level correlation), so exact-text matching is the only signal
///   available at this module's own recursion depth without a deeper,
///   out-of-scope change to `compose`/`region` (this task's own scope is
///   "pdf_adapter wiring").
/// - **`Figure`** — every top-level `Block::Figure`/`Block::Image` wraps
///   its OWN painted ops (via [`PdfRenderContext::begin_tag`]/
///   [`PdfRenderContext::end_tag`]) in one marked-content sequence, with
///   `/Alt` resolved from the immediately-FOLLOWING sibling block (SAME
///   frame, next index) when it is a `Block::Paragraph` — this crate's
///   own `caption::attach_captions` convention always splices a
///   captioned block's resolved caption paragraph directly after it, so
///   this is a real, honest signal, not a guess (see
///   [`resolve_figure_alt`]'s own doc comment for exactly how far this
///   heuristic goes and why). A figure with no following paragraph gets
///   NO `/Alt` at all (an honest absence, never a fabricated placeholder
///   string) — `uzor_export::pdf`'s own engine-side convention already
///   never invents one either.
/// - **`Table`/`TableRow`/`TableCell`** — real, full nesting, not a
///   whole-table `Figure` fallback: [`crate::region::PlacedTableCell`]/
///   [`crate::region::PlacedTableRow`] already expose clean cell/row
///   geometry (`rect`, `column_index`) unconditionally on every
///   `TablePlacement`, so the brief's own "only if cell geometry is
///   exposed cleanly, else whole-table Figure" condition is ALWAYS
///   satisfied in this codebase's current architecture — logged here per
///   the brief's own explicit "log the decision either way" instruction,
///   no Figure-fallback branch exists because none was ever needed. Each
///   cell's own paragraph content is tagged directly with its enclosing
///   `TableCell`'s [`StructElemId`] (`TagMode::Fixed`) — never a nested
///   `P` inside the `TD` (one fewer, purely cosmetic tree level; a
///   screen reader still reads the SAME cell text either way).
/// - **Out of scope this wave (documented, not silent)**: list markers/
///   items (`Block::List`, including the footnote zone's own numbered
///   list), header/footer running chrome, and the page-number text are
///   never struct-tagged — the brief's own explicit tag-mapping list
///   names only P/H1/H2/Figure/Table(+TR+TD); this content still paints
///   (untagged — a screen reader receives it as ordinary, un-tagged
///   content, tolerated by every reader as "artifact-like," never
///   dropped or hidden).
pub fn pages_to_pdf(pages: &[Page<'_>], master: &PageMaster<'_>, theme: &Theme, options: PdfExportOptions) -> Vec<u8> {
    let mut builder = PdfBuilder::new();
    builder.set_tagged(options.tagged);
    let lang = options.lang.clone().or_else(|| options.meta.as_ref().and_then(|m| m.lang.clone()));
    match (options.meta.clone(), lang) {
        (Some(meta), lang) => builder.set_meta(PdfMeta { lang, ..meta }),
        (None, Some(lang)) => builder.set_meta(PdfMeta { lang: Some(lang), ..Default::default() }),
        (None, None) => {}
    }
    let mut fonts = PdfFontCache::new();

    let outline_entries: Vec<PdfOutlineEntry> = pages
        .iter()
        .flat_map(|page| page.outline.iter())
        .map(|entry| PdfOutlineEntry { level: entry.level, title: entry.title.clone(), page_index: entry.page_index })
        .collect();
    if !outline_entries.is_empty() {
        builder.set_outline(outline_entries);
    }

    for page in pages {
        // Figures/tables/images/list markers/header-footer chrome/page
        // number — everything `DrawLayers { paragraph_ink: false }`
        // still paints — now render THROUGH a real PDF content stream
        // (typography-gap WAVE 1), not an offscreen raster pixmap. The
        // page's own opaque background is painted explicitly first (the
        // whole-page raster this used to ride along with is gone).
        //
        // Typography wave 5: `options.tagged == false` takes the EXACT
        // pre-wave-5 branch below, unchanged — no new code path (struct-
        // elem registration, `begin_tag`/`end_tag`) ever executes, not
        // merely "has no effect" (see `PdfExportOptions::tagged`'s own
        // doc comment).
        let content = if options.tagged {
            build_tagged_content(page, master, theme, &mut builder, &mut fonts)
        } else {
            let mut pdf_ctx = PdfRenderContext::new(1.0, &mut builder, &mut fonts);
            pdf_ctx.set_fill_color(&theme.color_hex(ColorRole::Background));
            pdf_ctx.fill_rect(0.0, 0.0, master.width, master.height);
            draw_page_layers(&mut pdf_ctx, page, theme, DrawLayers { paragraph_ink: false, paragraph_decorations: true });
            pdf_ctx.finish()
        };

        // Typography wave 5: this page's own PENDING (level, title) pairs
        // — consumed one at a time by `collect_text_runs_from_placed`'s
        // `Block::Paragraph` arm under `TagMode::AutoHeading` as it walks
        // `page.frame`/`page.extra_frames` in placement order. Empty (and
        // therefore inert) whenever `!options.tagged`.
        let mut pending_headings: Vec<(u8, String)> = if options.tagged { page.outline.iter().map(|e| (e.level, e.title.clone())).collect() } else { Vec::new() };
        let body_tag_mode = if options.tagged { TagMode::AutoHeading } else { TagMode::None };

        let mut collected = Vec::new();
        collect_text_runs_from_frame(&page.frame, theme, &mut fonts, &mut builder, &mut collected, body_tag_mode, &mut pending_headings);
        for extra in &page.extra_frames {
            collect_text_runs_from_frame(extra, theme, &mut fonts, &mut builder, &mut collected, body_tag_mode, &mut pending_headings);
        }
        if let Some(header) = &page.header {
            collect_text_runs_from_frame(header, theme, &mut fonts, &mut builder, &mut collected, TagMode::None, &mut pending_headings);
        }
        if let Some(footer) = &page.footer {
            collect_text_runs_from_frame(footer, theme, &mut fonts, &mut builder, &mut collected, TagMode::None, &mut pending_headings);
        }
        if let Some(footnotes) = &page.footnotes {
            // The footnote zone's own content is a real `Block::List`
            // (typography-gap WAVE 3) — walked by the SAME
            // `collect_text_runs_from_frame`/`collect_text_runs_from_placed`
            // recursion every other list in this crate already goes
            // through (design law 1); its numbered marker glyphs paint via
            // the page's own `PdfRenderContext` pass above (`draw_page_layers`
            // already walks `Page::footnotes` there), never a second path.
            // `TagMode::None` — list content is out of this wave's own
            // tagging scope (this module's own "Typography wave 5" doc
            // section above).
            collect_text_runs_from_frame(&footnotes.frame, theme, &mut fonts, &mut builder, &mut collected, TagMode::None, &mut pending_headings);
        }

        let text_runs: Vec<PdfTextRun<'_>> = collected
            .iter()
            .map(|run| PdfTextRun {
                font: run.font,
                size_pt: run.size_pt,
                x_pt: run.x_pt,
                y_pt: run.y_pt,
                rgb: run.rgb,
                text: run.text.as_str(),
                glyph_advances_pt: Some(run.glyph_advances_pt.clone()),
                struct_tag: run.struct_tag,
            })
            .collect();
        let links: Vec<PdfLink> = page
            .links
            .iter()
            .map(|link| PdfLink { x_pt: link.rect.x, y_pt: link.rect.y, width_pt: link.rect.width, height_pt: link.rect.height, target_page: link.target_page })
            .collect();

        builder
            .add_page(PdfPageSpec { width_pt: master.width, height_pt: master.height, raster: None, raster_px: (0, 0), text_runs, links, content })
            .unwrap_or_else(|e| {
                panic!(
                    "pages_to_pdf: assembling page {} failed unexpectedly (this adapter never \
                     supplies PdfPageSpec::raster, so the only fallible path in add_page — a \
                     raster dimension mismatch — can never trigger here; a failure indicates a \
                     real adapter bug, not caller input): {e}",
                    page.index
                )
            });
    }

    builder.finish()
}

/// The `options.tagged == true` content-stream pass — mirrors
/// [`draw_page_layers`]'s own body exactly (background paint, then
/// `frame`/`extra_frames`/`header`/`footer`/`footnotes`/`page_number`, in
/// the SAME order) with ONE difference: every top-level `Block::Figure`/
/// `Block::Image` in `frame`/`extra_frames` is individually bracketed in
/// its own `BDC`/`EMC` marked-content sequence (this module's own
/// "Typography wave 5" doc section above). Every struct element this
/// pass registers is allocated BEFORE [`PdfRenderContext`] takes its own
/// exclusive `&mut PdfBuilder` borrow for the whole painting pass below —
/// `PdfBuilder::register_struct_elem` needs `&mut PdfBuilder` too, which
/// a live `PdfRenderContext` would otherwise make unreachable.
fn build_tagged_content(page: &Page<'_>, master: &PageMaster<'_>, theme: &Theme, builder: &mut PdfBuilder, fonts: &mut PdfFontCache) -> uzor_export::PdfContentStream {
    let default_color = theme.color_hex(ColorRole::Ink);
    let figure_theme = theme.figure_theme();
    let layers = DrawLayers { paragraph_ink: false, paragraph_decorations: true };

    let frame_tags = register_figure_tags(&page.frame.blocks, builder);
    let extra_tags: Vec<Vec<Option<StructElemId>>> = page.extra_frames.iter().map(|extra| register_figure_tags(&extra.blocks, builder)).collect();

    let mut pdf_ctx = PdfRenderContext::new(1.0, builder, fonts);
    pdf_ctx.set_fill_color(&theme.color_hex(ColorRole::Background));
    pdf_ctx.fill_rect(0.0, 0.0, master.width, master.height);

    paint_blocks_tagged(&mut pdf_ctx, &page.frame.blocks, &frame_tags, &default_color, &figure_theme, layers);
    for (extra, tags) in page.extra_frames.iter().zip(extra_tags.iter()) {
        paint_blocks_tagged(&mut pdf_ctx, &extra.blocks, tags, &default_color, &figure_theme, layers);
    }
    if let Some(header) = &page.header {
        for placed in &header.blocks {
            draw_placed_block(&mut pdf_ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(footer) = &page.footer {
        for placed in &footer.blocks {
            draw_placed_block(&mut pdf_ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(footnotes) = &page.footnotes {
        draw_footnote_separator(&mut pdf_ctx, footnotes.frame.region.rect, &default_color);
        for placed in &footnotes.frame.blocks {
            draw_placed_block(&mut pdf_ctx, placed, &default_color, &figure_theme, layers);
        }
    }
    if let Some(page_number) = &page.page_number {
        draw_page_number(&mut pdf_ctx, page_number, theme);
    }

    pdf_ctx.finish()
}

/// Register a `Figure` struct element (with `/Alt` resolved via
/// [`resolve_figure_alt`]) for every top-level `Block::Figure`/
/// `Block::Image` in `blocks`, `None` for every other block kind — the
/// returned `Vec` is index-parallel to `blocks` itself, read back by
/// [`paint_blocks_tagged`].
fn register_figure_tags(blocks: &[PlacedBlock<'_>], builder: &mut PdfBuilder) -> Vec<Option<StructElemId>> {
    blocks
        .iter()
        .enumerate()
        .map(|(i, placed)| match placed.kind {
            Block::Figure(_) | Block::Image(_) => {
                let alt = resolve_figure_alt(blocks, i);
                Some(builder.register_struct_elem(PdfTagRole::Figure, None, alt))
            }
            _ => None,
        })
        .collect()
}

/// A figure/image's own `/Alt` text: the immediately-FOLLOWING sibling
/// block's plain text, when it is a `Block::Paragraph` with a real
/// layout — this crate's own `caption::attach_captions` convention
/// ALWAYS splices a captioned figure/table's resolved caption paragraph
/// directly after it (and upgrades the figure's own `break_control` to
/// `AvoidAfter` so the pair never separates across a page break), so
/// "the next sibling in this SAME frame" is a real, honest signal — not
/// a guessed heuristic invented for this wave. **Documented scope limit
/// (report, not silent)**: `pages_to_pdf` only ever sees the already-
/// composed `Page`/`PlacedBlock` tree, never the original flow's own
/// `BlockId`-keyed caption map (`crate::caption::resolve_caption_numbers`)
/// — there is no way at this recursion depth to distinguish "the
/// following paragraph really is this figure's caption" from "the
/// following paragraph merely happens to be the next thing in the flow"
/// with certainty; a figure followed by ordinary body text (rare — this
/// crate's own fixtures always pair a figure with its caption
/// immediately) would get that body text's own first words as its
/// `/Alt` instead of nothing, which is still marginally more useful than
/// a placeholder to an assistive-technology reader, never actively
/// wrong/fabricated data. A figure with NO following paragraph at all
/// (the last block in its frame) gets NO `/Alt` — an honest absence,
/// never a synthesized generic string.
fn resolve_figure_alt(blocks: &[PlacedBlock<'_>], index: usize) -> Option<String> {
    let next = blocks.get(index + 1)?;
    let Block::Paragraph(_) = next.kind else { return None };
    let layout = next.paragraph_layout.as_ref()?;
    let text = paragraph_plain_text(layout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Paint every block in `blocks` (one frame's worth), bracketing a
/// `Block::Figure`/`Block::Image`'s own ops in `begin_tag`/`end_tag`
/// whenever `tags[i]` is `Some` — every other block kind (including a
/// `Block::Table`, whose own cell TEXT is tagged separately via the
/// text-run collection pass, never here) paints exactly like
/// [`draw_page_layers`]'s own untagged loop.
fn paint_blocks_tagged(ctx: &mut PdfRenderContext<'_>, blocks: &[PlacedBlock<'_>], tags: &[Option<StructElemId>], default_color: &str, figure_theme: &uzor_figures::FigureTheme, layers: DrawLayers) {
    for (placed, tag) in blocks.iter().zip(tags.iter()) {
        match tag {
            Some(id) => {
                ctx.begin_tag(*id);
                draw_placed_block(ctx, placed, default_color, figure_theme, layers);
                ctx.end_tag();
            }
            None => draw_placed_block(ctx, placed, default_color, figure_theme, layers),
        }
    }
}

/// A paragraph's own visible text — every glyph cluster concatenated
/// verbatim, in glyph order (whitespace clusters included, ligature-
/// continuation empty clusters contribute nothing) — used ONLY to
/// correlate a placed paragraph against a pending [`crate::slice::
/// OutlineEntry::title`] for `H1`/`H2` tagging (typography wave 5); NOT a
/// general-purpose text-extraction utility (no line-break/hyphenation
/// awareness is needed for a single-paragraph heading match).
fn paragraph_plain_text(layout: &ParagraphLayout) -> String {
    layout.glyphs.iter().map(|g| g.cluster.as_str()).collect()
}

fn collect_text_runs_from_frame(
    frame: &Frame<'_>,
    theme: &Theme,
    fonts: &mut PdfFontCache,
    builder: &mut PdfBuilder,
    out: &mut Vec<CollectedRun>,
    tag_mode: TagMode,
    pending_headings: &mut Vec<(u8, String)>,
) {
    for placed in &frame.blocks {
        collect_text_runs_from_placed(placed, theme, fonts, builder, out, tag_mode, pending_headings);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_text_runs_from_placed(
    placed: &PlacedBlock<'_>,
    theme: &Theme,
    fonts: &mut PdfFontCache,
    builder: &mut PdfBuilder,
    out: &mut Vec<CollectedRun>,
    tag_mode: TagMode,
    pending_headings: &mut Vec<(u8, String)>,
) {
    match placed.kind {
        Block::Paragraph(_) => {
            if let Some(layout) = &placed.paragraph_layout {
                let struct_tag = resolve_paragraph_tag(layout, tag_mode, builder, pending_headings);
                collect_from_layout(layout, placed.rect, theme, fonts, builder, out, struct_tag);
            }
        }
        Block::Table(_) => {
            if let Some(table) = &placed.table_placement {
                // Typography wave 5 — real Table/TableRow/TableCell
                // nesting (this module's own "Typography wave 5" doc
                // section on `pages_to_pdf` has the full tag-mapping
                // decision + why no Figure-fallback branch was needed).
                // Registered ONLY when tagging is active at all (`tag_mode
                // != TagMode::None`) — header/footer/footnote content
                // never reaches this arm with anything else, but a table
                // is never placed there in practice either way.
                let table_id = if tag_mode != TagMode::None { Some(builder.register_struct_elem(PdfTagRole::Table, None, None)) } else { None };
                for row in &table.rows {
                    let tr_id = table_id.map(|t| builder.register_struct_elem(PdfTagRole::TableRow, Some(t), None));
                    for cell in &row.cells {
                        let cell_tag_mode = match tr_id {
                            Some(tr) => TagMode::Fixed(builder.register_struct_elem(PdfTagRole::TableCell, Some(tr), None)),
                            None => TagMode::None,
                        };
                        for inner in &cell.content {
                            collect_text_runs_from_placed(inner, theme, fonts, builder, out, cell_tag_mode, pending_headings);
                        }
                    }
                }
            }
        }
        Block::List(_) => {
            if let Some(list) = &placed.list_placement {
                // Out of this wave's own tagging scope — see this
                // module's own "Typography wave 5" doc section above.
                for item in &list.items {
                    for inner in &item.content {
                        collect_text_runs_from_placed(inner, theme, fonts, builder, out, TagMode::None, pending_headings);
                    }
                }
            }
        }
        // Figures/images/islands contribute NO paragraph-style text runs:
        // their ink (including figure axis/label text) is emitted by the
        // page's own `PdfRenderContext` pass in `add_document` — vector
        // ops in the content stream since WAVE 1 (Figure/Image tagging
        // itself happens in `build_tagged_content`'s own ops-level pass,
        // not here). A spacer paints nothing.
        Block::Figure(_) | Block::Image(_) | Block::Island(_) | Block::Spacer(_) => {}
    }
}

/// Resolve a top-level paragraph's own [`StructElemId`] under `tag_mode`
/// (typography wave 5) — see [`TagMode`]'s own doc comment for what each
/// variant means; `AutoHeading` is the only variant that actually reads
/// `layout`'s own text.
fn resolve_paragraph_tag(layout: &ParagraphLayout, tag_mode: TagMode, builder: &mut PdfBuilder, pending_headings: &mut Vec<(u8, String)>) -> Option<StructElemId> {
    match tag_mode {
        TagMode::None => None,
        TagMode::Fixed(id) => Some(id),
        TagMode::AutoHeading => {
            let text = paragraph_plain_text(layout);
            let matched_level = pending_headings
                .iter()
                .position(|(_, title)| title.trim() == text.trim())
                .map(|idx| pending_headings.remove(idx).0);
            let role = match matched_level {
                Some(1) => PdfTagRole::H1,
                Some(2) => PdfTagRole::H2,
                // A matched level outside 1/2 (never produced by this
                // crate's own `.with_outline()` convention today, but a
                // defensive, honest fallback rather than a panic) and
                // "no match at all" (an ordinary body paragraph, OR a
                // heading whose own outline title deliberately differs
                // from its painted text) both degrade to a plain `P`.
                _ => PdfTagRole::P,
            };
            Some(builder.register_struct_elem(role, None, None))
        }
    }
}

/// Merge `layout`'s glyphs into `PdfTextRun`-shaped runs — one per maximal
/// run of consecutive, non-empty-cluster, NON-WHITESPACE glyphs sharing
/// the same `(line_index, font, color)` (per-word granularity: a run also
/// breaks at every whitespace cluster, so each WORD becomes its own
/// `CollectedRun`), positioned at the run's own FIRST glyph baseline
/// (`rect.{x,y} + glyph.{x,y}` — `rect` is the enclosing `PlacedBlock`'s
/// own frame-relative origin, `glyph.{x,y}` already absolute within the
/// paragraph's own box, mirroring `uzor_text::draw_paragraph`'s own
/// `origin + glyph.{x,y}` convention exactly, design law 1). Whitespace
/// clusters are skipped entirely — never emitted as their own run — since
/// every word is already positioned absolutely from its own resolved
/// glyph `x`, so the PDF viewer never needs to re-advance a space to
/// place the next word: this is exactly what preserves `ParagraphAlign::
/// Justify`'s interword stretch (which lives in the resolved `x` values
/// themselves, not in any run's own natural advance).
///
/// **Typography-gap WAVE 2 (per-glyph PDF kerning)**: every collected
/// run's own `glyph_advances_pt` records `GlyphLayout::advance` — the
/// shaper's OWN real per-glyph advance, not the embedded font's static
/// declared width — one entry per `char` in the merged `text`, matching
/// `uzor_export::PdfTextRun::glyph_advances_pt`'s own per-char contract.
/// `uzor_export::pdf::show_run` then emits a real `TJ` array (never a
/// plain `Tj`) whenever any glyph pair's real advance genuinely differs
/// from the font's static width by more than its own documented threshold
/// — this is what prevents kerning drift from accumulating across a
/// multi-glyph word's worth of merged glyphs (a plain `Tj` alone would
/// silently re-advance every glyph after the first using the STATIC
/// width, which is exactly what this pass fixes). A cluster spanning MORE
/// than one `char` (a ligature — rare in the fonts this workspace embeds)
/// assigns its own full real advance to the cluster's FIRST char and `0.0`
/// to every subsequent char in that same cluster — the existing, pre-WAVE-
/// 2 `gid_for_char`-per-char resolution already treats a ligature as
/// several independent glyphs (a documented, pre-existing simplification
/// this pass doesn't newly introduce), so a `0.0` continuation advance is
/// the correct value to keep those glyphs visually coincident rather than
/// spread across the ligature's own combined width.
#[allow(clippy::too_many_arguments)]
fn collect_from_layout(
    layout: &ParagraphLayout,
    rect: Rect,
    theme: &Theme,
    fonts: &mut PdfFontCache,
    builder: &mut PdfBuilder,
    out: &mut Vec<CollectedRun>,
    struct_tag: Option<StructElemId>,
) {
    let glyphs = &layout.glyphs;
    let mut i = 0;
    while i < glyphs.len() {
        if glyphs[i].cluster.is_empty() || glyphs[i].cluster.trim().is_empty() {
            // Empty cluster (ligature continuation) or whitespace: never
            // starts a run of its own.
            i += 1;
            continue;
        }
        let first = &glyphs[i];
        let line_index = first.line_index;
        let font = first.font;
        let color = first.color;
        let start_x = first.x;
        let start_y = first.y;

        let mut text = String::new();
        let mut glyph_advances_pt: Vec<f64> = Vec::new();
        let mut j = i;
        while j < glyphs.len() {
            let g = &glyphs[j];
            if g.cluster.is_empty() {
                j += 1;
                continue;
            }
            if g.cluster.trim().is_empty() {
                // Whitespace ends the current word run; the space itself
                // is never emitted (the next word is positioned
                // absolutely from its own resolved glyph x).
                break;
            }
            if g.line_index != line_index || g.font != font || g.color != color {
                break;
            }
            text.push_str(&g.cluster);
            let mut chars = g.cluster.chars();
            if chars.next().is_some() {
                glyph_advances_pt.push(g.advance);
            }
            glyph_advances_pt.extend(chars.map(|_| 0.0));
            j += 1;
        }

        if !text.is_empty() {
            let font_id = fonts.id_for(font.family, font.bold, font.italic, builder);
            let rgb = color.map(|rgba| (rgba >> 8) & 0x00FF_FFFF).unwrap_or_else(|| theme.color_rgb(ColorRole::Ink));
            out.push(CollectedRun { font: font_id, size_pt: font.size_px, x_pt: rect.x + start_x, y_pt: rect.y + start_y, rgb, text, glyph_advances_pt, struct_tag });
        }

        i = j.max(i + 1);
    }
}

#[cfg(test)]
mod tests {
    //! Adapter proof (design law 8 + this phase's own gate): a 2-page P0-
    //! style fixture -> `pages_to_pdf` -> `lopdf` (dev-dep): 2 pages,
    //! extractable text contains a known fixture word (`lopdf::Document::
    //! extract_text` — this crate's own text IS real Type0/CID (Identity-H)
    //! PDF text operators against an embedded, subsetted TrueType font
    //! (`uzor-export`'s export SOTA pass — see its own `CLAUDE.md`/
    //! `pdf/mod.rs` doc comment), with a `/ToUnicode` CMap driving
    //! extraction, so a standard PDF text-extraction path recovers it
    //! verbatim, proving the text is genuinely vector/selectable, not a
    //! raster). Also the real deliverable: the P1 report-page fixture
    //! (figures/table/list) ->
    //! `out/typeset_p5_report.pdf` + a raster parity PNG.

    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_text::{CosmicShaper, FontSpec, Paragraph, StyledRun};

    use super::{pages_to_pdf, PdfExportOptions};
    use crate::compose::ComposeStyle;
    use crate::render::draw_page;
    use crate::scene::{Block, BlockNode};
    use crate::slice::{slice_pages, Margins, PageMaster};
    use crate::style::Theme;

    const PAGE_WIDTH: f64 = 595.0;
    const PAGE_HEIGHT: f64 = 842.0;

    fn out_dir() -> PathBuf {
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof file");
    }

    /// A fixed seeded 2-page-ish fixture — a title, several filler
    /// paragraphs, and one paragraph containing the known word
    /// `"Zephyrine"` (a deliberately distinctive fixture word, never
    /// occurring by accident) this test's own extraction assertion looks
    /// for.
    fn two_page_fixture(body_width: f64) -> Vec<BlockNode<'static>> {
        static TITLE_RUN: [StyledRun<'static>; 1] = [StyledRun { text: "PDF Adapter Proof", font: TITLE_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static MARKER_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "The fixture's own distinctive word is Zephyrine.", font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];
        static FILLER_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER, font: BODY_FONT, color: None, decoration: uzor_text::TextDecoration::NONE, letter_spacing: 0.0, vertical_align: uzor_text::VerticalAlign::Baseline }];

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 16.0, bold: false, italic: false };
        const FILLER: &str = "This fixed seeded sentence exists only to consume vertical space \
            for the PDF adapter test, repeated enough times to overflow comfortably past a \
            single page so the fixture genuinely spans two pages, not one.";

        let mut flow = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&TITLE_RUN, body_width))),
            BlockNode::new(Block::Spacer(20.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&MARKER_RUN, body_width))),
            BlockNode::new(Block::Spacer(14.0)),
        ];
        for _ in 0..10 {
            flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&FILLER_RUN, body_width))));
            flow.push(BlockNode::new(Block::Spacer(12.0)));
        }
        flow
    }

    #[test]
    fn two_page_fixture_converts_to_a_two_page_pdf_with_extractable_fixture_text() {
        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;
        let flow = two_page_fixture(body_width);

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2, "fixture must be tuned to land on exactly 2 pages, got {}", pages.len());

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        write_proof("typeset_p5_adapter_proof.pdf", &pdf_bytes);

        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse the adapter's own PDF output");
        let lopdf_pages = doc.get_pages();
        assert_eq!(lopdf_pages.len(), 2, "lopdf must see exactly the 2 pages this fixture composed");

        let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(
            extracted.contains("Zephyrine"),
            "extracted PDF text must contain the fixture's own distinctive word verbatim \
             (proves real, searchable vector text, not a raster) — got: {extracted:?}"
        );

        // Typography-gap WAVE 1: this fixture places no figure/table/
        // image at all — its own page background is now painted as a
        // real vector `fill_rect` through `PdfRenderContext`, so neither
        // page may carry an `/XObject` entry at all (no whole-page
        // raster left to inherit one from).
        for (page_num, page_id) in &lopdf_pages {
            let (resources, _) = doc.get_page_resources(*page_id).expect("get_page_resources should succeed");
            let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
            assert!(!has_xobjects, "page {page_num} must carry NO XObject on a fixture with no raster background left");
        }
    }

    /// Fix B proof: `collect_from_layout` breaks a `Justify`-aligned
    /// paragraph into one `CollectedRun` PER WORD, each positioned at
    /// EXACTLY its own first glyph's already-resolved (justified) `(x,
    /// y)` — never a natural-advance cumulative position that would
    /// silently discard the justification stretch baked into
    /// `ParagraphLayout::glyphs` by `layout_paragraph`.
    #[test]
    fn justified_paragraph_words_position_at_their_own_resolved_glyph_xy() {
        use uzor_text::{layout_paragraph, ParagraphAlign};

        use super::collect_from_layout;
        use uzor_export::PdfFontCache;

        let font = FontSpec { family: FontFamily::Roboto, size_px: 16.0, bold: false, italic: false };
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let runs = [StyledRun::new(text, font)];
        let paragraph = Paragraph::new(&runs, 260.0).with_align(ParagraphAlign::Justify);
        let shaper = CosmicShaper::headless();
        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to at least 2 lines to exercise justify stretch");

        let theme = Theme::light_report();
        let mut builder = uzor_export::PdfBuilder::new();
        let mut fonts = PdfFontCache::new();
        let mut collected = Vec::new();
        let rect = uzor::types::Rect { x: 10.0, y: 20.0, width: 260.0, height: 400.0 };
        collect_from_layout(&layout, rect, &theme, &mut fonts, &mut builder, &mut collected, None);

        // No collected run ever spans a whitespace boundary (per-word, not
        // per-line, granularity).
        for run in &collected {
            assert!(
                !run.text.contains(char::is_whitespace),
                "a run must never span a whitespace boundary, got {:?}",
                run.text
            );
        }

        // Reconstruct the SAME word-boundary grouping directly over
        // `layout.glyphs` (independent of `collect_from_layout`'s own
        // internals) and confirm every collected run's (x, y) matches
        // its word's own FIRST glyph exactly (mod the rect translation) —
        // proves positions come from the shaper's own resolved (justified)
        // glyph coordinates, not a re-derived natural-advance sum.
        let mut expected: Vec<(f64, f64, String)> = Vec::new();
        let mut prev_was_word = false;
        let mut prev_font: Option<FontSpec> = None;
        let mut prev_color: Option<Option<u32>> = None;
        let mut prev_line: Option<usize> = None;
        for g in &layout.glyphs {
            if g.cluster.is_empty() || g.cluster.trim().is_empty() {
                prev_was_word = false;
                continue;
            }
            let starts_new = !prev_was_word || prev_font != Some(g.font) || prev_color != Some(g.color) || prev_line != Some(g.line_index);
            if starts_new {
                expected.push((g.x, g.y, String::new()));
            }
            if let Some(last) = expected.last_mut() {
                last.2.push_str(&g.cluster);
            }
            prev_was_word = true;
            prev_font = Some(g.font);
            prev_color = Some(g.color);
            prev_line = Some(g.line_index);
        }

        assert_eq!(expected.len(), collected.len(), "expected {} word runs, collected {}", expected.len(), collected.len());
        for (exp, got) in expected.iter().zip(collected.iter()) {
            assert!((got.x_pt - (rect.x + exp.0)).abs() < 1e-6, "run {:?} x_pt {} != expected {}", got.text, got.x_pt, rect.x + exp.0);
            assert!((got.y_pt - (rect.y + exp.1)).abs() < 1e-6, "run {:?} y_pt {} != expected {}", got.text, got.y_pt, rect.y + exp.1);
            assert_eq!(&got.text, &exp.2);
        }
    }

    /// Typography-gap WAVE 2 (per-glyph PDF kerning): `collect_from_layout`
    /// must record each collected word's own REAL shaped per-glyph
    /// advances (`GlyphLayout::advance`), not the embedded font's static
    /// declared widths — proven directly by comparing `CollectedRun::
    /// glyph_advances_pt` against the source `ParagraphLayout::glyphs`
    /// verbatim, independent of `PdfBuilder`'s own internal TJ-vs-Tj
    /// decision (that decision is `uzor-export`'s own gate, see
    /// `pdf::tests::a_run_whose_real_advances_differ_from_static_widths_
    /// emits_a_tj_array_with_adjustments`).
    #[test]
    fn collect_from_layout_records_the_real_shaped_advance_per_glyph() {
        use super::collect_from_layout;
        use uzor_export::PdfFontCache;
        use uzor_text::layout_paragraph;

        let font = FontSpec { family: FontFamily::Roboto, size_px: 20.0, bold: false, italic: false };
        let runs = [StyledRun::new("Word", font)];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();
        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.lines.len(), 1);

        let theme = Theme::light_report();
        let mut builder = uzor_export::PdfBuilder::new();
        let mut fonts = PdfFontCache::new();
        let mut collected = Vec::new();
        let rect = uzor::types::Rect { x: 0.0, y: 0.0, width: 1000.0, height: 100.0 };
        collect_from_layout(&layout, rect, &theme, &mut fonts, &mut builder, &mut collected, None);
        assert_eq!(collected.len(), 1, "single unbroken word must collect into exactly one run");

        let word_glyph_advances: Vec<f64> = layout.glyphs.iter().filter(|g| !g.cluster.trim().is_empty()).map(|g| g.advance).collect();
        assert_eq!(
            collected[0].glyph_advances_pt, word_glyph_advances,
            "collected glyph_advances_pt must equal GlyphLayout::advance verbatim, in order"
        );
    }

    /// Typography-gap WAVE 2 end-to-end proof: a letter-spaced word's real
    /// shaped advances are GUARANTEED to differ from the embedded font's
    /// own static declared widths by exactly `letter_spacing` per glyph
    /// (deterministic, unlike relying on Roboto's own real kerning-table
    /// contents for an ordinary word) — `pages_to_pdf` must therefore emit
    /// a real `TJ` array (never a plain `Tj`) for that run, AND the word
    /// must still round-trip verbatim through `/ToUnicode` text
    /// extraction (position-adjusted text is still real, searchable text).
    #[test]
    fn a_letter_spaced_word_round_trips_through_a_real_tj_array() {
        let font = FontSpec { family: FontFamily::Roboto, size_px: 24.0, bold: false, italic: false };
        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;

        let spaced_run = [StyledRun::new("Spacing", font).with_letter_spacing(5.0)];
        let flow = vec![BlockNode::new(Block::Paragraph(Paragraph::new(&spaced_run, body_width)))];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");
        let lopdf_pages = doc.get_pages();
        let page_id = *lopdf_pages.values().next().expect("one page");

        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        assert!(
            content_str.contains("TJ"),
            "a letter-spaced word's real advances must differ enough from static widths to force a real TJ array, got: {content_str}"
        );

        let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains("Spacing"), "TJ-positioned, letter-spaced text must still extract verbatim — got: {extracted:?}");
    }

    /// Typography-gap WAVE 2: an underlined paragraph's decoration rect
    /// paints as a REAL vector `re`/`f` fill operator in the PDF content
    /// stream (through `PdfRenderContext`'s pass-1 pipeline, gated by
    /// `DrawLayers::paragraph_decorations`), never baked into a raster —
    /// this fixture places no `Block::Image`/`Block::Island`, so NO page
    /// may carry an `/XObject` at all (WAVE 1's own promise, still true
    /// here).
    #[test]
    fn underlined_paragraph_decoration_rect_is_real_vector_content_not_raster() {
        use uzor_text::TextDecoration;

        let font = FontSpec { family: FontFamily::Roboto, size_px: 18.0, bold: false, italic: false };
        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;

        let underlined_run = [StyledRun::new("Underlined heading", font).with_decoration(TextDecoration::underline())];
        let flow = vec![BlockNode::new(Block::Paragraph(Paragraph::new(&underlined_run, body_width)))];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");
        let lopdf_pages = doc.get_pages();
        let page_id = *lopdf_pages.values().next().expect("one page");

        let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources should succeed");
        let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
        assert!(!has_xobjects, "an underline-only, image-free page must carry NO XObject at all");

        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        let toks: Vec<&str> = content_str.split_whitespace().collect();
        assert!(toks.contains(&"f") || toks.contains(&"re"), "the underline rect must be a real fill operator, got: {content_str}");

        let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains("Underlined"), "the underlined text itself must still extract verbatim — got: {extracted:?}");
    }

    /// The real deliverable this phase exists to produce (design law 8 —
    /// the owner opens this file directly): the FULL figure-family
    /// showcase fixture (title + filler paragraphs + a `BarFigure` + a
    /// `CurveFigure` with a `TimeScale` X-axis + a `HistogramFigure` + a
    /// `TimelineFigure` + a `SankeyFigure` — each followed by its own
    /// caption paragraph — + a table + a bulleted list, spanning several
    /// pages) rendered through `pages_to_pdf` to `out/typeset_p5_report.pdf`,
    /// plus a same-content `draw_page` PNG PER PAGE
    /// (`out/typeset_p5_report_page{N}.png`) as the raster parity
    /// reference the task's own gate asks for.
    #[test]
    fn p1_report_fixture_produces_the_p5_pdf_deliverable_and_a_parity_png() {
        use uzor_figures::{BarFigure, CurveFigure, HistogramFigure, SankeyFigure, SankeyLink, SankeyNode, TimeScale, TimelineEvent, TimelineFigure};
        use uzor_export::{render_to_png as export_render_to_png, ExportSpec};

        use crate::compose::BreakControl;
        use crate::scene::{BlockSizing, ColumnSpec, FigureBlock, ListBlock, ListItem, MarkerStyle, TableBlock, TableCell, TableRow};
        use uzor_text::ParagraphAlign;

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const HEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 17.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };
        const CAPTION_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 11.0, bold: false, italic: true };
        const TABLE_HEADER_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: true, italic: false };
        const TABLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };

        // Fix E: every figure block gets a legible 210px height (task's
        // own 200-220 sensible-height ask) — was 180.0 pre-fix, tall
        // enough that axis tick labels never crowd the plot.
        const FIGURE_HEIGHT: f64 = 210.0;

        const FILLER_A: &str = "The confidence-matrix narrative for this section walks through every \
            traced hop in order, noting timestamps, counterparties, and the amount observed at each \
            step of the flow, so a reader can follow the chain of custody from the initial deposit all \
            the way through to the resting balance without needing to cross-reference a separate table.";
        const FILLER_B: &str = "Business documents full of long compound words like implementation, \
            infrastructure, counterparty, and accountability often expose uneven interword spacing in a \
            narrow justified column, especially once hyphenation is enabled and a break lands mid-word \
            near the right margin, which is exactly the case this fixed fixture paragraph is built to show.";

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;

        let title_run = [StyledRun::new("Case Report — uzor-typeset P5 PDF Deliverable", TITLE_FONT)];
        let heading_a_run = [StyledRun::new("Section 1 — Transfer Narrative", HEADING_FONT)];
        let heading_b_run = [StyledRun::new("Section 2 — Observed Amounts", HEADING_FONT)];
        let filler_a_run = [StyledRun::new(FILLER_A, BODY_FONT)];
        let filler_b_run = [StyledRun::new(FILLER_B, BODY_FONT)];

        let bar_caption_run = [StyledRun::new(
            "Figure 1 — observed amount transferred at each hop of the traced chain (seeded, deterministic).",
            CAPTION_FONT,
        )];
        let curve_caption_run = [StyledRun::new(
            "Figure 2 — cumulative daily running balance over a 90-day observation window, plotted against a \
            calendar TimeScale X-axis (seeded, deterministic).",
            CAPTION_FONT,
        )];
        let histogram_caption_run = [StyledRun::new(
            "Figure 3 — distribution of observed transfer amounts across the seeded sample set, grouped into bins.",
            CAPTION_FONT,
        )];
        let timeline_caption_run = [StyledRun::new(
            "Figure 4 — chronological timeline of transfers across two paths, including a multi-day relay window \
            interval and a point event near the report's own resting-balance conclusion (seeded).",
            CAPTION_FONT,
        )];
        let sankey_caption_run = [StyledRun::new(
            "Figure 5 — staged flow from source wallets through intermediate mixers to their eventual sinks \
            (seeded; includes a deliberate conservation leak at one mixer, matching real observed flows).",
            CAPTION_FONT,
        )];

        let bar_figure = BarFigure::new(
            vec!["Hop 1".to_owned(), "Hop 2".to_owned(), "Hop 3".to_owned(), "Hop 4".to_owned(), "Hop 5".to_owned()],
            vec![128_500.0, 640_000.0, 2_600_000.0, 1_450_000.0, 300_000.0],
        )
        .with_title("Observed amount per hop (seeded)");

        const ANCHOR_2024_01_01: f64 = 1_704_067_200.0;
        const DAY_SECS: f64 = 86_400.0;

        // Deterministic 90-daily-point running-balance series (fixed
        // pseudo-sequence, no RNG/time) — same seeding convention
        // `uzor-figures`'s own TimeScale proof fixture uses.
        let curve_points: Vec<(f64, f64)> = {
            let mut running = 0.0_f64;
            (0..90)
                .map(|i| {
                    let step = ((i * 41 + 7) % 29) as f64 - 14.0;
                    running += step;
                    (ANCHOR_2024_01_01 + i as f64 * DAY_SECS, running)
                })
                .collect()
        };
        let curve_x_min = curve_points[0].0;
        let curve_x_max = curve_points[curve_points.len() - 1].0;
        let curve_time_scale = TimeScale::new(curve_x_min, curve_x_max);
        let curve_figure = CurveFigure::new(curve_points)
            .with_title("Cumulative daily balance over 90 days (TimeScale X-axis, seeded)")
            .with_x_scale(curve_time_scale);

        // Deterministic sample set (a fixed pseudo-sequence spread over
        // `[0, 100)`) — same seeding convention `uzor-figures`'s own
        // histogram proof fixture uses.
        let histogram_samples: Vec<f64> = (0..500).map(|i| ((i * 97 + 13) % 1000) as f64 / 10.0).collect();
        let histogram_figure = HistogramFigure::new(histogram_samples, 20).with_title("Observed amount distribution (seeded)");

        // Fix E/D showcase: lane spacing is deliberately widened (vs. the
        // original 0/2/5/6/9-day fixture) so the seeded 30-day tail event
        // ("resting balance") sits genuinely near the plot's own right
        // edge — its natural, right-of-marker label would clip past it
        // and Fix D flips it left instead — while every other lane-0/
        // lane-1 point/interval label pair stays comfortably clear of its
        // own neighbor (verified visually: `_debug_timeline_zoom.png`
        // during development, no longer part of this test).
        let timeline_events = vec![
            TimelineEvent { ts: ANCHOR_2024_01_01, end_ts: None, lane: 0, label: "initial deposit".to_owned(), kind: 0 },
            TimelineEvent {
                ts: ANCHOR_2024_01_01 + 8.0 * DAY_SECS,
                end_ts: Some(ANCHOR_2024_01_01 + 11.0 * DAY_SECS),
                lane: 0,
                label: "relay window".to_owned(),
                kind: 1,
            },
            TimelineEvent { ts: ANCHOR_2024_01_01 + 13.0 * DAY_SECS, end_ts: None, lane: 1, label: "exchange deposit".to_owned(), kind: 2 },
            TimelineEvent { ts: ANCHOR_2024_01_01 + 30.0 * DAY_SECS, end_ts: None, lane: 1, label: "resting balance".to_owned(), kind: 0 },
        ];
        let timeline_figure =
            TimelineFigure::new(timeline_events, vec!["path-a".to_owned(), "path-b".to_owned()]).with_title("Timeline of transfers (seeded)");

        // Deterministic 7-node/8-link staged flow (fixed weights, no RNG)
        // — same shape as `uzor-figures`'s own seeded Sankey proof
        // fixture, renamed onto this report's own wallet/mixer/exchange
        // vocabulary, one deliberate conservation leak preserved
        // (mixer-1's 59 total in vs. 57 total out).
        let sankey_nodes = vec![
            SankeyNode { id: "wallet-a".to_owned(), label: "wallet-a".to_owned(), stage: 0 },
            SankeyNode { id: "wallet-b".to_owned(), label: "wallet-b".to_owned(), stage: 0 },
            SankeyNode { id: "mixer-1".to_owned(), label: "mixer-1".to_owned(), stage: 1 },
            SankeyNode { id: "mixer-2".to_owned(), label: "mixer-2".to_owned(), stage: 1 },
            SankeyNode { id: "mixer-3".to_owned(), label: "mixer-3".to_owned(), stage: 1 },
            SankeyNode { id: "exchange-hot".to_owned(), label: "exchange-hot".to_owned(), stage: 2 },
            SankeyNode { id: "cold-storage".to_owned(), label: "cold-storage".to_owned(), stage: 2 },
        ];
        let sankey_links = vec![
            SankeyLink { from: 0, to: 2, weight: 50.0, kind: 0 }, // wallet-a -> mixer-1 (dominant path)
            SankeyLink { from: 0, to: 3, weight: 6.0, kind: 1 },  // wallet-a -> mixer-2
            SankeyLink { from: 1, to: 2, weight: 9.0, kind: 0 },  // wallet-b -> mixer-1
            SankeyLink { from: 1, to: 4, weight: 14.0, kind: 2 }, // wallet-b -> mixer-3
            SankeyLink { from: 2, to: 5, weight: 48.0, kind: 0 }, // mixer-1 -> exchange-hot
            SankeyLink { from: 2, to: 6, weight: 9.0, kind: 0 },  // mixer-1 -> cold-storage (leaks 2 vs its 59 in)
            SankeyLink { from: 3, to: 6, weight: 6.0, kind: 1 },  // mixer-2 -> cold-storage
            SankeyLink { from: 4, to: 5, weight: 14.0, kind: 2 }, // mixer-3 -> exchange-hot
        ];
        let sankey_figure = SankeyFigure::new(sankey_nodes, sankey_links).with_title("Staged flow: wallets -> mixers -> sinks (seeded)");

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
        let list_items = [ListItem::new(&li1_nodes), ListItem::new(&li2_nodes), ListItem::new(&li3_nodes), ListItem::new(&li4_nodes)];

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

        // Fix E: the FULL figure family — each figure immediately followed
        // by its own caption paragraph (`BreakControl::AvoidAfter` so a
        // caption never gets stranded alone at the top of the next page,
        // away from the figure it describes).
        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT)))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(6.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&bar_caption_run, body_width))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));

        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&curve_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT)))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(6.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&curve_caption_run, body_width))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));

        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&histogram_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT)))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(6.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&histogram_caption_run, body_width))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));

        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&timeline_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT)))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(6.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&timeline_caption_run, body_width))));
        flow.push(BlockNode::new(Block::Spacer(18.0)));

        flow.push(BlockNode::new(Block::Figure(FigureBlock::new(&sankey_figure, BlockSizing::FixedHeight(FIGURE_HEIGHT)))).with_break_control(BreakControl::AvoidAfter));
        flow.push(BlockNode::new(Block::Spacer(6.0)));
        flow.push(BlockNode::new(Block::Paragraph(Paragraph::new(&sankey_caption_run, body_width))));
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

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 10.0);
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        // Fix E: the fixture is tuned to land on exactly 5 pages now that
        // it carries the FULL figure family (5 figures + captions), not
        // just bars/timeline/table/list — within the task's own "~4-5
        // pages" showcase range.
        assert_eq!(pages.len(), 5, "showcase fixture is tuned to land on exactly 5 pages, got {}", pages.len());

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        write_proof("typeset_p5_report.pdf", &pdf_bytes);

        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse the report deliverable");
        let lopdf_pages = doc.get_pages();
        assert_eq!(lopdf_pages.len(), pages.len(), "the report PDF must carry every composed page");

        // The table header/body cells and the list items are all
        // `Block::Paragraph`s (per this crate's own P1 fixture shape) —
        // real vector text, extractable verbatim, not baked into the
        // raster background.
        //
        // Fix B (per-word runs, not per-line): each word is now its own
        // `Tj` positioned absolutely, with no `TJ`-array negative-offset
        // gap or `T*`/`ET` line break between adjacent words on the same
        // line — `lopdf::Document::extract_text` therefore glues adjacent
        // words on one line together with no inserted separator (a
        // documented lopdf extraction quirk, not a bug in this adapter:
        // the PDF's own real, on-screen glyph positions are still exactly
        // right — see `justified_paragraph_words_position_at_their_own_
        // resolved_glyph_xy` above, which asserts the actual positions
        // directly). This assertion therefore checks each WORD is present
        // individually rather than a verbatim multi-word phrase.
        let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        for word in ["Amount", "USDT", "wallet-a19x", "Confirmed", "direct", "transfer"] {
            assert!(extracted.contains(word), "extracted PDF text must contain {word:?} — got: {extracted:?}");
        }
        assert_eq!(
            extracted.matches("Section").count(),
            2,
            "both 'Section 1' and 'Section 2' headings must be extractable — got: {extracted:?}"
        );

        // Typography-gap WAVE 1 proof: figure axis/category labels are now
        // REAL vector text too (painted through `PdfRenderContext`, not
        // baked into a whole-page raster background as they were pre-
        // WAVE-1) — "Hop 1"/"Hop 5" are the BarFigure's own category axis
        // labels, extractable verbatim exactly like a paragraph word.
        for word in ["Hop 1", "Hop 5"] {
            assert!(extracted.contains(word), "figure axis label {word:?} must be extractable as real vector text — got: {extracted:?}");
        }

        // This fixture places no `Block::Image`/`Block::Island` at all —
        // WAVE 1's own promise ("the 2x raster background layer remains
        // ONLY for ImageBlocks ... or disappears entirely on pages
        // without images") means NOT ONE page's `/Resources` dict may
        // carry an `/XObject` entry: every figure/table/chrome pixel this
        // fixture paints is now real vector content-stream ops, not a
        // raster image XObject.
        for (page_num, page_id) in &lopdf_pages {
            let (resources, _) = doc.get_page_resources(*page_id).expect("get_page_resources should succeed");
            let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
            assert!(!has_xobjects, "page {page_num} must carry NO XObject (no whole-page raster, no ImageBlock on this fixture)");
        }

        // Every page's own content stream must contain REAL vector path
        // ops (figures draw curves/bars/axis lines, tables draw
        // gridlines) — a `c` (cubic bezier) token proves at least the
        // curve/timeline/sankey figures' own arcs/lines reached the
        // content stream as real PDF path geometry, not raster pixels.
        for (_, page_id) in &lopdf_pages {
            let content_bytes = doc.get_page_content(*page_id);
            let content_str = String::from_utf8_lossy(&content_bytes);
            let has_vector_path_op = content_str.split_whitespace().any(|tok| tok == "c" || tok == "l" || tok == "S");
            assert!(has_vector_path_op, "every page must carry at least one real vector path/stroke operator, got: {content_str}");
        }

        // Raster parity reference (the task's own gate) — EVERY page, not
        // just page 1: the SAME content, painted via the ordinary
        // `draw_page` (every layer, unsuppressed) at the SAME physical
        // page size, so a human can diff each one against the PDF's own
        // rendered page to confirm the hybrid model's raster background
        // matches what full rendering produces, across the whole document.
        let png_spec = ExportSpec { width_px: PAGE_WIDTH as u32, height_px: PAGE_HEIGHT as u32, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        for (i, page) in pages.iter().enumerate() {
            let png_bytes = export_render_to_png(&png_spec, |ctx| draw_page(ctx, page, &theme)).expect("parity PNG render should succeed");
            write_proof(&format!("typeset_p5_report_page{}.png", i + 1), &png_bytes);
        }
    }

    /// Typography-gap WAVE 1's own "page chrome" promise: a page-number
    /// footer (`crate::render::draw_page_number`) and a table's own
    /// gridlines (`crate::render::draw_table_placement`) both used to
    /// paint into the P5 whole-page raster background — neither reproduced
    /// its exact baseline as PDF vector text, and neither was individually
    /// checkable via `lopdf`. Both now paint through `PdfRenderContext`
    /// directly: the page number is real, extractable `Tj` text, and the
    /// gridlines are real `re`/`S` vector stroke ops — proven directly
    /// here, independent of the bigger multi-figure fixture above.
    #[test]
    fn page_number_footer_and_table_gridlines_are_real_vector_content_not_raster() {
        use crate::master::{PageNumberFormat, PageNumberStyle};
        use crate::scene::{ColumnSpec, TableBlock, TableCell, TableRow};

        const CELL_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0)).with_page_number(PageNumberStyle::new(PageNumberFormat::OfTotal, 1));

        let c_a = [StyledRun::new("Alpha", CELL_FONT)];
        let n_a = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_a, f64::MAX)))];
        let c_b = [StyledRun::new("Beta", CELL_FONT)];
        let n_b = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_b, f64::MAX)))];
        let cells = [TableCell::new(&n_a), TableCell::new(&n_b)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Auto, ColumnSpec::Fraction(1.0)];

        let flow = vec![BlockNode::new(Block::Table(TableBlock::new(&columns, &rows)))];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "single small table must land on exactly 1 page");

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");
        let lopdf_pages = doc.get_pages();
        let page_id = *lopdf_pages.values().next().expect("one page");

        let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources should succeed");
        let has_xobjects = resources.and_then(|r| r.get(b"XObject").and_then(lopdf::Object::as_dict).ok()).is_some_and(|d| !d.is_empty());
        assert!(!has_xobjects, "a table-only, image-free page must carry NO XObject at all");

        let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains("1 of 1"), "the page-number footer must be real, extractable vector text — got: {extracted:?}");

        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        let toks: Vec<&str> = content_str.split_whitespace().collect();
        assert!(toks.contains(&"S"), "the table's own gridlines must be a real stroke 'S' operator, got: {content_str}");
    }

    // -----------------------------------------------------------------
    // Typography wave 5 — adapter-side tag-mapping decisions.
    // -----------------------------------------------------------------

    /// Decode a PDF text-string's raw bytes the same convention
    /// `pdf_writer::TextStr` itself writes (bare ASCII, or a `U+FEFF`
    /// byte-order-mark followed by UTF-16BE code units) — this test
    /// module's own copy of the identical helper `uzor-export`'s own
    /// `pdf::tests` module already carries (test-only, no production
    /// code ever needs to decode a `TextStr` it just wrote).
    fn decode_text_str(bytes: &[u8]) -> String {
        if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        }
    }

    /// Count every real `/StructElem` object's own `/S` role across the
    /// whole finished PDF — a cheap, direct way to prove "exactly N
    /// elements of role X exist" without walking the tree by hand.
    fn count_struct_roles(doc: &lopdf::Document) -> std::collections::HashMap<Vec<u8>, u32> {
        let mut counts = std::collections::HashMap::new();
        for obj in doc.objects.values() {
            let lopdf::Object::Dictionary(dict) = obj else { continue };
            if dict.get(b"Type").and_then(|o| o.as_name()).ok() != Some(b"StructElem".as_slice()) {
                continue;
            }
            if let Ok(role) = dict.get(b"S").and_then(|o| o.as_name()) {
                *counts.entry(role.to_vec()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Heading paragraphs whose own PAINTED text matches a same-page
    /// `.with_outline(level, title)` entry resolve to `H1`/`H2`; every
    /// other paragraph (no match at all) falls back to `P` — this
    /// module's own "Typography wave 5" tag-mapping decision, proven end
    /// to end through `slice_pages` -> `pages_to_pdf` -> a real parsed
    /// struct tree.
    #[test]
    fn heading_paragraphs_resolve_to_h1_h2_tags_matching_the_outline_levels() {
        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;

        let title_run = [StyledRun::new("Intro", TITLE_FONT)];
        let sub_run = [StyledRun::new("Sub", TITLE_FONT)];
        let body_run = [StyledRun::new("Just a body paragraph.", BODY_FONT)];

        let flow = vec![
            BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, body_width))).with_outline(1, "Intro"),
            BlockNode::new(Block::Spacer(10.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&sub_run, body_width))).with_outline(2, "Sub"),
            BlockNode::new(Block::Spacer(10.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, body_width))),
        ];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1, "this small fixture must fit on one page");

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");

        let roles = count_struct_roles(&doc);
        assert_eq!(roles.get(b"H1".as_slice()).copied().unwrap_or(0), 1, "exactly one H1-tagged heading, got {roles:?}");
        assert_eq!(roles.get(b"H2".as_slice()).copied().unwrap_or(0), 1, "exactly one H2-tagged heading, got {roles:?}");
        assert_eq!(roles.get(b"P".as_slice()).copied().unwrap_or(0), 1, "exactly one plain-P body paragraph, got {roles:?}");
    }

    /// A figure's own `/Alt` resolves from its immediately-following
    /// sibling caption paragraph (this module's own "Typography wave 5"
    /// adjacent-caption heuristic — see `resolve_figure_alt`'s own doc
    /// comment).
    #[test]
    fn figure_alt_text_is_resolved_from_its_own_following_caption_paragraph() {
        use uzor_figures::BarFigure;

        use crate::scene::{BlockSizing, FigureBlock};

        const CAPTION_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 11.0, bold: false, italic: true };

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;
        let caption_run = [StyledRun::new("Figure 1 - a seeded bar chart of observed amounts.", CAPTION_FONT)];

        let bar_figure = BarFigure::new(vec!["A".to_owned(), "B".to_owned()], vec![10.0, 20.0]);
        let figure_block = FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(150.0));

        let flow = vec![BlockNode::new(Block::Figure(figure_block)), BlockNode::new(Block::Paragraph(Paragraph::new(&caption_run, body_width)))];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");

        let alt = doc
            .objects
            .values()
            .find_map(|obj| {
                let lopdf::Object::Dictionary(dict) = obj else { return None };
                if dict.get(b"S").and_then(|o| o.as_name()).ok() != Some(b"Figure".as_slice()) {
                    return None;
                }
                dict.get(b"Alt").and_then(|o| o.as_str()).ok().map(decode_text_str)
            })
            .expect("the Figure struct element must carry a resolved /Alt from its own following caption");
        assert!(alt.contains("bar chart of observed amounts"), "unexpected alt text: {alt}");
    }

    /// A figure with NO following paragraph gets no `/Alt` at all — an
    /// honest absence, never a fabricated placeholder string (this
    /// module's own explicit divergence from a blanket generic alt).
    #[test]
    fn a_figure_with_no_following_paragraph_gets_no_alt_at_all() {
        use uzor_figures::BarFigure;

        use crate::scene::{BlockSizing, FigureBlock};

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let bar_figure = BarFigure::new(vec!["A".to_owned(), "B".to_owned()], vec![10.0, 20.0]);
        let figure_block = FigureBlock::new(&bar_figure, BlockSizing::FixedHeight(150.0));
        let flow = vec![BlockNode::new(Block::Figure(figure_block))];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");

        let figure_elem = doc
            .objects
            .values()
            .find_map(|obj| {
                let lopdf::Object::Dictionary(dict) = obj else { return None };
                (dict.get(b"S").and_then(|o| o.as_name()).ok() == Some(b"Figure".as_slice())).then_some(dict)
            })
            .expect("a Figure struct element must exist");
        assert!(figure_elem.get(b"Alt").is_err(), "no following paragraph exists, so no /Alt must be written");
    }

    /// Table tagging: real `Table`/`TableRow`/`TableCell` nesting (this
    /// module's own "cell geometry is always exposed cleanly, no Figure
    /// fallback needed" finding — see `pages_to_pdf`'s own "Typography
    /// wave 5" doc section) — one `Table`, one `TableRow`, one `TableCell`
    /// PER cell, and every cell's own text is still extractable verbatim.
    #[test]
    fn table_cells_are_tagged_table_tr_td_and_their_own_text_stays_extractable() {
        use crate::scene::{ColumnSpec, TableBlock, TableCell, TableRow};

        const CELL_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));

        let c_a = [StyledRun::new("Alpha", CELL_FONT)];
        let n_a = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_a, f64::MAX)))];
        let c_b = [StyledRun::new("Beta", CELL_FONT)];
        let n_b = [BlockNode::new(Block::Paragraph(Paragraph::new(&c_b, f64::MAX)))];
        let cells = [TableCell::new(&n_a), TableCell::new(&n_b)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Auto, ColumnSpec::Fraction(1.0)];
        let flow = vec![BlockNode::new(Block::Table(TableBlock::new(&columns, &rows)))];

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 1);

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions::default());
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse this fixture's own PDF output");

        let roles = count_struct_roles(&doc);
        assert_eq!(roles.get(b"Table".as_slice()).copied().unwrap_or(0), 1, "exactly one Table container, got {roles:?}");
        assert_eq!(roles.get(b"TR".as_slice()).copied().unwrap_or(0), 1, "exactly one TableRow container, got {roles:?}");
        assert_eq!(roles.get(b"TD".as_slice()).copied().unwrap_or(0), 2, "one TableCell leaf per real cell, got {roles:?}");

        let page_numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains("Alpha") && extracted.contains("Beta"), "cell text must still extract verbatim under real Table/TR/TD tagging, got: {extracted:?}");
    }

    /// `tagged: false` must never write any tagging marker, AND must
    /// never perturb the SEPARATE raster paint path (`crate::render::
    /// draw_page`) either build variant drives afterward — parity PNGs
    /// captured immediately before and after both `pages_to_pdf` calls
    /// must be byte-identical (structurally guaranteed by `pages: &[Page]`
    /// being an immutable borrow throughout, verified here as data, not
    /// just by inspection).
    #[test]
    fn tagged_false_writes_no_tagging_markers_and_never_perturbs_the_raster_paint_path() {
        use uzor_export::{render_to_png as export_render_to_png, ExportSpec};

        use crate::slice::Page;

        let master = PageMaster::new(PAGE_WIDTH, PAGE_HEIGHT, Margins::uniform(40.0));
        let body_width = master.body_rect().width;
        let flow = two_page_fixture(body_width);

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 12.0);
        let shaper = CosmicShaper::headless();
        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 2);

        let png_spec = ExportSpec { width_px: PAGE_WIDTH as u32, height_px: PAGE_HEIGHT as u32, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let render_all = |pages: &[Page<'_>]| -> Vec<Vec<u8>> {
            pages.iter().map(|p| export_render_to_png(&png_spec, |ctx| draw_page(ctx, p, &theme)).expect("parity render should succeed")).collect()
        };

        let png_before = render_all(&pages);
        let untagged_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions { tagged: false, lang: None, meta: None });
        let tagged_bytes = pages_to_pdf(&pages, &master, &theme, PdfExportOptions { tagged: true, lang: None, meta: None });
        let png_after = render_all(&pages);

        assert_eq!(png_before, png_after, "generating either PDF variant must never perturb the separate raster paint path");

        let untagged_text = String::from_utf8_lossy(&untagged_bytes);
        assert!(!untagged_text.contains("/MarkInfo") && !untagged_text.contains("/StructTreeRoot"), "tagged:false must never write /MarkInfo or /StructTreeRoot");

        let doc_untagged = lopdf::Document::load_mem(&untagged_bytes).expect("lopdf must parse the untagged PDF");
        for (_, page_id) in doc_untagged.get_pages() {
            let content_str = String::from_utf8_lossy(&doc_untagged.get_page_content(page_id)).into_owned();
            assert!(!content_str.contains("BDC") && !content_str.contains("EMC"), "tagged:false must never emit BDC/EMC on any page");
        }

        // Positive control: the SAME fixture with tagging on really does
        // carry the expected additions (proves the ABSENCE above is a
        // real signal, not an artifact of a broken assertion).
        let tagged_text = String::from_utf8_lossy(&tagged_bytes);
        assert!(tagged_text.contains("/MarkInfo") && tagged_text.contains("/StructTreeRoot"), "tagged:true must carry the expected additions");
    }
}
