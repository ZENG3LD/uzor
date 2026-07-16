//! Paint a composed [`Page`] onto a [`RenderContext`] — design law 6
//! (native measure/paint seams, no bespoke ones): paragraphs paint through
//! `uzor_text::draw_paragraph` unchanged, figures paint through their own
//! `TypesetFigure::render`, neither is a new drawing primitive this crate
//! invented.
//!
//! ## Image blocks — the `ImagePainter` seam is not reachable here (report)
//!
//! Design law 4 says images paint through `ImagePainter::draw_image_rgba`.
//! Two independent gaps make that seam unreachable from THIS crate's own
//! headless proof path:
//! 1. `ImagePainter` is declared opt-in, NOT part of the `RenderContext`
//!    supertrait (`uzor/src/core/render/context.rs`) — a `&mut dyn
//!    RenderContext` alone cannot reach it, and this crate has no `Any`-
//!    style downcast primitive to recover it (none exists anywhere in
//!    `uzor` core yet — confirmed by inspection, not assumed).
//! 2. Even the ONE backend that also implements `ImagePainter` in this
//!    crate's own dependency chain (`uzor-render-tiny-skia`, the backend
//!    `uzor-export::render_to_png` uses) declares `draw_image_rgba` a
//!    documented no-op (`uzor-render-tiny-skia/src/context.rs:1617-1645`:
//!    "tiny-skia does not load URL-based images... no wgpu texture
//!    support in the CPU path") — so even a hypothetical downcast would
//!    paint nothing.
//!
//! [`draw_page`] therefore paints a deterministic, visibly-labeled
//! placeholder rect for [`crate::scene::Block::Image`] (never silently
//! skipped) instead of pretending to composite pixels no reachable
//! backend can actually draw. `crate::scene::image_block`'s own fit-rect
//! geometry (`ImageBlock::content_rect`) is real, tested, backend-
//! independent math — only the FINAL pixel blit is the gap.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;
use uzor_figures::FigureTheme;
use uzor_text::draw_paragraph;

use crate::region::{ListPlacement, PlacedBlock, TablePlacement};
use crate::scene::Block;
use crate::slice::{Card, Page, PageNumberPlacement, Slide};
use crate::style::{ColorRole, FontRole, Theme};

/// Dashed magenta — the same debug-outline convention
/// `uzor_text::draw_paragraph`'s own `InlineBox` outline uses, so a
/// reserved-but-unpainted rect always reads as "placeholder," never as a
/// real content color.
const IMAGE_PLACEHOLDER_COLOR: &str = "#ff00ffff";

/// Paint every placed block of `page` onto `ctx`, each at its own
/// frame-relative rect origin (design law 1: one transform — every draw
/// derives its position from the SAME `PlacedBlock::rect`/
/// `paragraph_layout`/`table_placement`/`list_placement`
/// [`crate::compose::compose`] already computed, never a second ad hoc
/// position formula), followed by `page`'s own header/footer (P2 — see
/// `crate::master::PageMaster`) and page-number text, if present.
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
pub fn draw_page(ctx: &mut dyn RenderContext, page: &Page<'_>, theme: &Theme) {
    let default_color = theme.color_hex(ColorRole::Ink);
    let figure_theme = theme.figure_theme();

    for placed in &page.frame.blocks {
        draw_placed_block(ctx, placed, &default_color, &figure_theme);
    }
    if let Some(header) = &page.header {
        for placed in &header.blocks {
            draw_placed_block(ctx, placed, &default_color, &figure_theme);
        }
    }
    if let Some(footer) = &page.footer {
        for placed in &footer.blocks {
            draw_placed_block(ctx, placed, &default_color, &figure_theme);
        }
    }
    if let Some(number) = &page.page_number {
        draw_page_number(ctx, number, theme);
    }
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
        draw_placed_block(ctx, placed, &default_color, &figure_theme);
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
        draw_placed_block(ctx, placed, &default_color, &figure_theme);
    }
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
/// exact same paint logic top-level flow blocks use.
fn draw_placed_block(ctx: &mut dyn RenderContext, placed: &PlacedBlock<'_>, default_color: &str, figure_theme: &FigureTheme) {
    match placed.kind {
        Block::Paragraph(_) => {
            if let Some(layout) = &placed.paragraph_layout {
                draw_paragraph(ctx, (placed.rect.x, placed.rect.y), layout, default_color, false);
            }
        }
        Block::Figure(figure_block) => {
            figure_block.figure.render(ctx, placed.rect, figure_theme);
        }
        Block::Image(_) => {
            draw_image_placeholder(ctx, placed.rect);
        }
        Block::Table(_) => {
            if let Some(table) = &placed.table_placement {
                draw_table_placement(ctx, table, default_color, figure_theme);
            }
        }
        Block::List(_) => {
            if let Some(list) = &placed.list_placement {
                draw_list_placement(ctx, list, default_color, figure_theme);
            }
        }
        Block::Spacer(_) => {}
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
/// cell's already-placed content.
fn draw_table_placement(ctx: &mut dyn RenderContext, table: &TablePlacement<'_>, default_color: &str, figure_theme: &FigureTheme) {
    ctx.set_stroke_color(default_color);
    ctx.set_stroke_width(1.0);
    for row in &table.rows {
        ctx.stroke_rect(row.rect.x, row.rect.y, row.rect.width, row.rect.height);
        for cell in &row.cells {
            ctx.stroke_rect(cell.rect.x, cell.rect.y, cell.rect.width, cell.rect.height);
            for inner in &cell.content {
                draw_placed_block(ctx, inner, default_color, figure_theme);
            }
        }
    }
}

/// Draw every item's marker text, then recurse into its own already-placed
/// (already-indented) content.
fn draw_list_placement(ctx: &mut dyn RenderContext, list: &ListPlacement<'_>, default_color: &str, figure_theme: &FigureTheme) {
    for item in &list.items {
        if !item.marker_text.is_empty() {
            ctx.set_font(&item.marker_font.to_css_font());
            ctx.set_fill_color(default_color);
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            ctx.fill_text(&item.marker_text, item.marker_rect.x, item.marker_rect.y);
        }
        for inner in &item.content {
            draw_placed_block(ctx, inner, default_color, figure_theme);
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
    use uzor_export::{render_to_png, ExportSpec};
    use uzor_text::{BreakStrategy, CosmicShaper, FontSpec, Hyphenation, Paragraph, ParagraphAlign, StyledRun};

    use super::{draw_card, draw_page, draw_slide};
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
            [StyledRun { text: "Case Report — uzor-typeset P0 Proof", font: TITLE_FONT, color: None }];
        static LEFT_A_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None }];
        static LEFT_B_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];
        static CENTER_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "Section II — Centered Summary Heading", font: BODY_FONT, color: None }];
        static RIGHT_RUN: [StyledRun<'static>; 1] =
            [StyledRun { text: "— Exhibit reference, right-aligned —", font: BODY_FONT, color: None }];
        static JUSTIFY_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];
        static TAIL_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None }];
        static EXTRA_A_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];
        static EXTRA_B_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_A, font: BODY_FONT, color: None }];
        static EXTRA_C_RUN: [StyledRun<'static>; 1] = [StyledRun { text: FILLER_B, font: BODY_FONT, color: None }];

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
}
