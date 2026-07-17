//! [`pages_to_pdf`] — the adapter converting composed `uzor-typeset::Page`s
//! into a real PDF byte buffer via `uzor-export`'s neutral `pdf` module
//! (Arc 4 Phase P5, design doc §6). This is the ONE place a `Page`'s
//! `ParagraphLayout`/`Frame`/`PlacedBlock` shape ever gets walked to
//! produce PDF content — `uzor-export` itself never imports this crate
//! (§6.1's dependency-boundary law).
//!
//! ## Hybrid fidelity, in two passes per page
//!
//! 1. **Raster background** — the WHOLE page is rendered at
//!    [`RASTER_SCALE`] via the existing, already-proven
//!    `uzor_export::render_to_png` offscreen path, through
//!    [`crate::render::draw_page_layers`] with
//!    `DrawLayers { paragraph_ink: false }` — every non-text block
//!    (figures, tables, images, list-marker glyphs, header/footer chrome
//!    OTHER than its own paragraph text) paints into this raster; real
//!    paragraph glyph ink is deliberately suppressed here so it isn't
//!    painted twice.
//! 2. **Real vector text runs** — every `Block::Paragraph`'s own
//!    `ParagraphLayout` (body flow AND header/footer content — both are
//!    `Block::Paragraph`s, walked by the SAME recursive collector) is
//!    converted into [`uzor_export::PdfTextRun`]s: consecutive glyphs on
//!    one line sharing the same `(font, color)` are merged into ONE run
//!    (per-line granularity — this phase's own chosen v1 fidelity level),
//!    positioned at the FIRST glyph's own already-resolved `(x, y)`
//!    baseline (`GlyphLayout::{x,y}` — already absolute within the
//!    paragraph's own box, translated by the placed block's own
//!    `PlacedBlock::rect` origin, design law 1: one transform, never a
//!    second position formula).
//!
//! ## What stays raster-only (report, not silent — task's own escape
//! hatch: "or raster v1 if the chrome path resists the layer split")
//!
//! Table gridlines, list-item MARKER glyphs (`•`, `1.`, ...), and the
//! page-number text are never converted to vector runs. All three paint
//! via ad-hoc `ctx.fill_text`/`ctx.stroke_rect` calls using
//! `TextBaseline::Top`/`TextBaseline::Middle` (`crate::render`'s own
//! `draw_list_placement`/`draw_page_number`) rather than a real, already-
//! shaped `ParagraphLayout` with `TextBaseline::Alphabetic` glyph
//! positions — reproducing their exact rendered baseline as PDF vector
//! text would need the SAME font-shaping-backend metrics
//! (ascent-at-this-size) `uzor-text`'s own `layout_paragraph` resolves for
//! a real paragraph but that these ad-hoc calls never compute or expose.
//! `Block::Paragraph` content is the only content this crate ever hands a
//! real, per-glyph-positioned `ParagraphLayout` — exactly the content this
//! adapter converts to vector text.
//!
//! ## Coordinate + unit convention
//!
//! `1pt = 1px` (v1 — no separate press-DPI unit conversion; a page's own
//! `PageMaster::{width,height}` in px becomes the PDF page's size in pt
//! directly). `uzor_export::pdf`'s own doc comment already documents the
//! bottom-left-vs-top-left PDF coordinate flip; this adapter never
//! reasons about it directly (`PdfTextRun`/`PdfPageSpec` are already
//! top-left, same as every rect in this crate).
//!
//! ## Font handling (design doc §6.1: "one embedded font resource per
//! unique `FontSpec` ... deduped, never re-embedded per page")
//!
//! [`FontCache`] registers each distinct `(FontFamily, bold, italic)`
//! combination with the SAME `uzor_export::PdfBuilder` exactly once
//! across the WHOLE document (never per-page) via `uzor::fonts::
//! font_bytes` — the SAME family->bytes resolution every render backend
//! in this workspace already shares (`uzor/src/ui/assets/fonts/fonts.rs`),
//! reused here rather than inventing a second one. A small `Vec` (not a
//! `HashMap`): `uzor::fonts::FontFamily` derives `PartialEq` but not
//! `Hash`, and a document realistically registers only a handful of
//! distinct fonts, so linear search is simpler and cheap enough.

use uzor::fonts::FontFamily;
use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec, FontId, PdfBuilder, PdfPageSpec, PdfTextRun};
use uzor_text::ParagraphLayout;

use crate::master::PageMaster;
use crate::region::{Frame, PlacedBlock};
use crate::render::{draw_page_layers, DrawLayers};
use crate::scene::Block;
use crate::slice::Page;
use crate::style::{ColorRole, Theme};

/// Physical page raster scale (design doc §6.2: "rasterized once at print
/// DPI"). `2.0` is this phase's own chosen v1 value — sharp enough to read
/// as press-quality on a normal viewer zoom without producing an
/// unreasonably large file for the `nemo/uzor/out/` proof deliverables
/// this phase targets (no press-DPI unit system exists in this crate yet
/// — see this module's own "Coordinate + unit convention" doc comment).
const RASTER_SCALE: f64 = 2.0;

/// One dedup'd font registration, keyed by the SAME `(family, bold,
/// italic)` triple `uzor::fonts::font_bytes` itself takes.
struct FontCache(Vec<((FontFamily, bool, bool), FontId)>);

impl FontCache {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn id_for(&mut self, family: FontFamily, bold: bool, italic: bool, builder: &mut PdfBuilder) -> FontId {
        let key = (family, bold, italic);
        if let Some(&(_, id)) = self.0.iter().find(|&&(k, _)| k == key) {
            return id;
        }
        let bytes = uzor::fonts::font_bytes(family, bold, italic);
        let id = builder.register_font(bytes);
        self.0.push((key, id));
        id
    }
}

/// One collected vector text run, owning its own merged text string until
/// [`pages_to_pdf`] hands a borrowed `&str` to `uzor_export::PdfTextRun`.
struct CollectedRun {
    font: FontId,
    size_pt: f64,
    x_pt: f64,
    y_pt: f64,
    rgb: u32,
    text: String,
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
pub fn pages_to_pdf(pages: &[Page<'_>], master: &PageMaster<'_>, theme: &Theme) -> Vec<u8> {
    let mut builder = PdfBuilder::new();
    let mut fonts = FontCache::new();

    for page in pages {
        let width_px = (master.width * RASTER_SCALE).round().max(1.0) as u32;
        let height_px = (master.height * RASTER_SCALE).round().max(1.0) as u32;

        let raster_spec = ExportSpec { width_px, height_px, dpr: 1.0, background: Some(background_rgba(theme)) };
        let png_bytes = render_to_png(&raster_spec, |ctx| {
            ctx.scale(RASTER_SCALE, RASTER_SCALE);
            draw_page_layers(ctx, page, theme, DrawLayers { paragraph_ink: false });
        })
        .unwrap_or_else(|e| {
            panic!(
                "pages_to_pdf: rendering page {}'s raster background failed unexpectedly \
                 (master dimensions are always positive here, so this indicates a real \
                 adapter bug, not caller input): {e}",
                page.index
            )
        });

        let mut collected = Vec::new();
        collect_text_runs_from_frame(&page.frame, theme, &mut fonts, &mut builder, &mut collected);
        if let Some(header) = &page.header {
            collect_text_runs_from_frame(header, theme, &mut fonts, &mut builder, &mut collected);
        }
        if let Some(footer) = &page.footer {
            collect_text_runs_from_frame(footer, theme, &mut fonts, &mut builder, &mut collected);
        }

        let text_runs: Vec<PdfTextRun<'_>> = collected
            .iter()
            .map(|run| PdfTextRun { font: run.font, size_pt: run.size_pt, x_pt: run.x_pt, y_pt: run.y_pt, rgb: run.rgb, text: run.text.as_str() })
            .collect();

        builder
            .add_page(PdfPageSpec { width_pt: master.width, height_pt: master.height, raster: Some(&png_bytes), raster_px: (width_px, height_px), text_runs })
            .unwrap_or_else(|e| {
                panic!(
                    "pages_to_pdf: assembling page {} failed unexpectedly (raster_px is \
                     computed from the SAME render_to_png call above, so a dimension \
                     mismatch here is an adapter bug, not caller input): {e}",
                    page.index
                )
            });
    }

    builder.finish()
}

/// `ColorRole::Background` as straight, opaque RGBA bytes — the shape
/// `uzor_export::ExportSpec::background` takes (matches this crate's own
/// `render.rs` test-module convention for the same conversion).
fn background_rgba(theme: &Theme) -> [u8; 4] {
    let rgb = theme.color_rgb(ColorRole::Background);
    [((rgb >> 16) & 0xFF) as u8, ((rgb >> 8) & 0xFF) as u8, (rgb & 0xFF) as u8, 255]
}

fn collect_text_runs_from_frame(frame: &Frame<'_>, theme: &Theme, fonts: &mut FontCache, builder: &mut PdfBuilder, out: &mut Vec<CollectedRun>) {
    for placed in &frame.blocks {
        collect_text_runs_from_placed(placed, theme, fonts, builder, out);
    }
}

fn collect_text_runs_from_placed(placed: &PlacedBlock<'_>, theme: &Theme, fonts: &mut FontCache, builder: &mut PdfBuilder, out: &mut Vec<CollectedRun>) {
    match placed.kind {
        Block::Paragraph(_) => {
            if let Some(layout) = &placed.paragraph_layout {
                collect_from_layout(layout, placed.rect, theme, fonts, builder, out);
            }
        }
        Block::Table(_) => {
            if let Some(table) = &placed.table_placement {
                for row in &table.rows {
                    for cell in &row.cells {
                        for inner in &cell.content {
                            collect_text_runs_from_placed(inner, theme, fonts, builder, out);
                        }
                    }
                }
            }
        }
        Block::List(_) => {
            if let Some(list) = &placed.list_placement {
                for item in &list.items {
                    for inner in &item.content {
                        collect_text_runs_from_placed(inner, theme, fonts, builder, out);
                    }
                }
            }
        }
        // Figures/images paint their own raster content only (design doc
        // §6.2 — figures stay raster this phase); a spacer paints nothing.
        Block::Figure(_) | Block::Image(_) | Block::Spacer(_) => {}
    }
}

/// Merge `layout`'s glyphs into `PdfTextRun`-shaped runs — one per maximal
/// run of consecutive, non-empty-cluster glyphs sharing the same
/// `(line_index, font, color)` (this phase's own "per-line granularity is
/// fine v1" choice), positioned at the run's own FIRST glyph baseline
/// (`rect.{x,y} + glyph.{x,y}` — `rect` is the enclosing `PlacedBlock`'s
/// own frame-relative origin, `glyph.{x,y}` already absolute within the
/// paragraph's own box, mirroring `uzor_text::draw_paragraph`'s own
/// `origin + glyph.{x,y}` convention exactly, design law 1).
fn collect_from_layout(layout: &ParagraphLayout, rect: Rect, theme: &Theme, fonts: &mut FontCache, builder: &mut PdfBuilder, out: &mut Vec<CollectedRun>) {
    let glyphs = &layout.glyphs;
    let mut i = 0;
    while i < glyphs.len() {
        if glyphs[i].cluster.is_empty() {
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
        let mut j = i;
        while j < glyphs.len() {
            let g = &glyphs[j];
            if g.cluster.is_empty() {
                j += 1;
                continue;
            }
            if g.line_index != line_index || g.font != font || g.color != color {
                break;
            }
            text.push_str(&g.cluster);
            j += 1;
        }

        if !text.is_empty() {
            let font_id = fonts.id_for(font.family, font.bold, font.italic, builder);
            let rgb = color.map(|rgba| (rgba >> 8) & 0x00FF_FFFF).unwrap_or_else(|| theme.color_rgb(ColorRole::Ink));
            out.push(CollectedRun { font: font_id, size_pt: font.size_px, x_pt: rect.x + start_x, y_pt: rect.y + start_y, rgb, text });
        }

        i = j.max(i + 1);
    }
}

#[cfg(test)]
mod tests {
    //! Adapter proof (design law 8 + this phase's own gate): a 2-page P0-
    //! style fixture -> `pages_to_pdf` -> `lopdf` (dev-dep): 2 pages,
    //! extractable text contains a known fixture word (`lopdf::Document::
    //! extract_text` — this crate's own text IS real WinAnsi-encoded PDF
    //! text operators against an embedded, subset-free TrueType font, so a
    //! standard PDF text-extraction path recovers it verbatim, proving the
    //! text is genuinely vector/selectable, not a raster). Also the real
    //! deliverable: the P1 report-page fixture (figures/table/list) ->
    //! `out/typeset_p5_report.pdf` + a raster parity PNG.

    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_text::{CosmicShaper, FontSpec, Paragraph, StyledRun};

    use super::pages_to_pdf;
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
        static TITLE_RUN: [StyledRun<'static>; 1] = [StyledRun { text: "PDF Adapter Proof", font: TITLE_FONT, color: None }];
        static MARKER_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "The fixture's own distinctive word is Zephyrine.", font: BODY_FONT, color: None }];
        static FILLER_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER, font: BODY_FONT, color: None }];

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

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme);
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
    }

    /// The real deliverable this phase exists to produce (design law 8 —
    /// the owner opens this file directly): the P1 report-page fixture
    /// (title + filler paragraphs + a `BarFigure` + a `TimelineFigure` + a
    /// table + a bulleted list, spanning 3 pages) rendered through
    /// `pages_to_pdf` to `out/typeset_p5_report.pdf`, plus a same-content
    /// `draw_page` PNG (`out/typeset_p5_report_page1.png`) as the raster
    /// parity reference the task's own gate asks for.
    #[test]
    fn p1_report_fixture_produces_the_p5_pdf_deliverable_and_a_parity_png() {
        use uzor_figures::{BarFigure, TimelineEvent, TimelineFigure};
        use uzor_export::{render_to_png as export_render_to_png, ExportSpec};

        use crate::compose::BreakControl;
        use crate::scene::{BlockSizing, ColumnSpec, FigureBlock, ListBlock, ListItem, MarkerStyle, TableBlock, TableCell, TableRow};
        use uzor_text::ParagraphAlign;

        const TITLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 22.0, bold: true, italic: false };
        const HEADING_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 17.0, bold: true, italic: false };
        const BODY_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 14.0, bold: false, italic: false };
        const TABLE_HEADER_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: true, italic: false };
        const TABLE_FONT: FontSpec = FontSpec { family: FontFamily::Roboto, size_px: 13.0, bold: false, italic: false };

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

        let theme = Theme::light_report();
        let style = ComposeStyle::from_theme(&theme, 10.0);
        let shaper = CosmicShaper::headless();

        let pages = slice_pages(&flow, &master, &style, &shaper);
        assert_eq!(pages.len(), 3, "fixture is tuned to land on exactly 3 pages, got {}", pages.len());

        let pdf_bytes = pages_to_pdf(&pages, &master, &theme);
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        write_proof("typeset_p5_report.pdf", &pdf_bytes);

        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf must parse the report deliverable");
        let lopdf_pages = doc.get_pages();
        assert_eq!(lopdf_pages.len(), 3, "the report PDF must carry all 3 composed pages");

        // The table header/body cells and the list items are all
        // `Block::Paragraph`s (per this crate's own P1 fixture shape) —
        // real vector text, extractable verbatim, not baked into the
        // raster background.
        let page_numbers: Vec<u32> = lopdf_pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        for word in ["Amount (USDT)", "wallet-a19x", "Confirmed direct transfer", "Section 1", "Section 2"] {
            assert!(extracted.contains(word), "extracted PDF text must contain {word:?} verbatim — got: {extracted:?}");
        }

        // Raster parity reference (the task's own gate): the SAME page 1,
        // painted via the ordinary `draw_page` (every layer, unsuppressed)
        // at the SAME physical page size — a human can diff this against
        // the PDF's own rendered page 1 to confirm the hybrid model's
        // raster background matches what full rendering produces.
        let png_spec = ExportSpec { width_px: PAGE_WIDTH as u32, height_px: PAGE_HEIGHT as u32, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let png_bytes = export_render_to_png(&png_spec, |ctx| draw_page(ctx, &pages[0], &theme)).expect("parity PNG render should succeed");
        write_proof("typeset_p5_report_page1.png", &png_bytes);
    }
}
