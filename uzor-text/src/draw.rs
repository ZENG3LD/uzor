//! [`draw_layout`]/[`draw_paragraph`] — paint a [`ParagraphLayout`] via
//! existing paint primitives (design law 6: no new drawing primitive for
//! text).

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::layout::ParagraphLayout;

/// Fill every glyph of `layout` at `origin`, translated by each glyph's own
/// already-measured `(x, y)` — one `fill_text` call per cluster, positions
/// come straight from `layout` (design law 1: one measure path, never a
/// second ad hoc width/position formula).
///
/// Forces `TextAlign::Left` + `TextBaseline::Alphabetic` on `ctx` before
/// painting (glyph `y` is already an alphabetic-baseline absolute
/// position — any other baseline state would double-offset it). Does
/// **not** call `ctx.set_font` — `layout`'s positions were measured
/// against a specific [`crate::model::FontSpec`]; the caller must have
/// already set that same font on `ctx` (Phase 1's `GlyphLayout` carries no
/// per-glyph font to read one back from — single font per paragraph).
pub fn draw_layout(ctx: &mut dyn RenderContext, origin: (f64, f64), layout: &ParagraphLayout, color: &str) {
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Alphabetic);
    ctx.set_fill_color(color);

    for glyph in &layout.glyphs {
        if glyph.cluster.is_empty() {
            continue;
        }
        ctx.fill_text(&glyph.cluster, origin.0 + glyph.x, origin.1 + glyph.y);
    }
}

/// Fill every glyph of a Phase 2 rich `layout` at `origin`, setting each
/// glyph's own run font ([`crate::model::GlyphLayout::font`]) and color
/// ([`GlyphLayout::color`], falling back to `default_color` when a run has
/// no override) before painting it — unlike [`draw_layout`], the caller
/// does not need to set the font/color itself.
///
/// [`crate::model::InlineBox`]es reserve space only; this never paints
/// their contents (the caller draws those). When `debug_outline_boxes` is
/// `true`, each [`crate::layout::PlacedInlineBox`] gets a stroked
/// debug-outline rect instead — a visual proof that the reservation
/// happened, per design law 8 (one screenshot is never proof).
pub fn draw_paragraph(
    ctx: &mut dyn RenderContext,
    origin: (f64, f64),
    layout: &ParagraphLayout,
    default_color: &str,
    debug_outline_boxes: bool,
) {
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Alphabetic);

    for glyph in &layout.glyphs {
        if glyph.cluster.is_empty() {
            continue;
        }
        ctx.set_font(&glyph.font.to_css_font());
        match glyph.color {
            Some(rgba) => ctx.set_fill_color(&rgba_hex(rgba)),
            None => ctx.set_fill_color(default_color),
        }
        ctx.fill_text(&glyph.cluster, origin.0 + glyph.x, origin.1 + glyph.y);
    }

    if debug_outline_boxes {
        ctx.set_stroke_color("#ff00ffff");
        ctx.set_stroke_width(1.0);
        for b in &layout.boxes {
            ctx.stroke_rect(origin.0 + b.x, origin.1 + b.y_top, b.width, b.height);
        }
    }
}

/// Format a packed `0xRRGGBBAA` color as the `#RRGGBBAA` hex string
/// `uzor::render::parse_color` accepts.
fn rgba_hex(color: u32) -> String {
    format!("#{color:08x}")
}

#[cfg(test)]
mod tests {
    //! Headless proof: render a wrapped paragraph (Left) and the same
    //! paragraph Center-aligned via `uzor-export`, at a fixed resolution
    //! with a deterministic (seeded, no RNG/time) sample paragraph, and
    //! write both to `uzor/out/` for a human to eyeball (design law 8 — one
    //! screenshot is never proof, hence the Left/Center pair).

    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_export::{render_to_png, ExportSpec};

    use super::{draw_layout, draw_paragraph};
    use crate::layout::{align_lines, layout_paragraph, layout_text, Align, ParagraphLayout};
    use crate::model::{FontSpec, InlineBox, InlineBoxSlot, Paragraph, ParagraphAlign, StyledRun};
    use crate::shape::CosmicShaper;

    const WIDTH: u32 = 500;
    const HEIGHT: u32 = 300;
    const MARGIN: f64 = 20.0;

    const WIDTH_P2: u32 = 600;
    const HEIGHT_P2: u32 = 400;

    /// Fixed seeded sample paragraph — no lorem-ipsum RNG (design law 8).
    const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog and \
        then keeps running further down the road without stopping, a fixed \
        seeded sample paragraph used for every uzor-text Phase 1 proof render.";

    fn out_dir() -> PathBuf {
        // Fixed path — `uzor/out/` is the shared human-eyeball drop point
        // for every headless proof render in this workspace (matches
        // uzor-viz's `proof_tests::out_dir`).
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

    #[test]
    fn wrapped_paragraph_left_aligned_renders_to_a_valid_png() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = (WIDTH as f64) - 2.0 * MARGIN;

        let layout = layout_text(PARAGRAPH, &font, max_width, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            ctx.set_font(&font.to_css_font());
            draw_layout(ctx, (MARGIN, 40.0), &layout, "#111111");
        })
        .expect("left-aligned proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("text_p1_wrap_left.png", &bytes);
    }

    #[test]
    fn wrapped_paragraph_center_aligned_renders_to_a_valid_png() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = (WIDTH as f64) - 2.0 * MARGIN;

        let mut layout = layout_text(PARAGRAPH, &font, max_width, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        align_lines(&mut layout, max_width, Align::Center);

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            ctx.set_font(&font.to_css_font());
            draw_layout(ctx, (MARGIN, 40.0), &layout, "#111111");
        })
        .expect("center-aligned proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("text_p1_wrap_center.png", &bytes);
    }

    /// Fixed seeded Phase 2 sample: 3 runs (regular / bold-and-larger /
    /// colored) with one in-flow `InlineBox` spliced into the colored run,
    /// laid out at `align`.
    fn rich_paragraph_layout(align: ParagraphAlign) -> ParagraphLayout {
        let regular = FontSpec::new(FontFamily::Roboto, 16.0);
        let bold_larger = FontSpec::new(FontFamily::Roboto, 22.0).bold();
        let colored = FontSpec::new(FontFamily::Roboto, 16.0);

        let run3_text = " phrase continues, followed by a colored run with an inline ICONMARKER \
            placeholder sitting in the middle of this fixed seeded sample sentence used for \
            every uzor-text Phase 2 proof render.";
        let marker = "ICONMARKER";
        let marker_at = run3_text.find(marker).expect("fixture text must contain the marker") + marker.len();

        let runs = [
            StyledRun::new("Regular text opens the paragraph, then a ", regular),
            StyledRun::new("BOLD AND LARGER", bold_larger),
            StyledRun::new(run3_text, colored).with_color(0xcc2222ff),
        ];
        let icon = InlineBox::in_flow(42, 24.0, 24.0);
        let slots = [InlineBoxSlot::new(2, marker_at, icon)];

        let max_width = (WIDTH_P2 as f64) - 2.0 * MARGIN;
        let paragraph = Paragraph::new(&runs, max_width).with_align(align).with_inline_boxes(&slots);
        let shaper = CosmicShaper::headless();

        layout_paragraph(&paragraph, &shaper)
    }

    #[test]
    fn rich_paragraph_left_aligned_with_inline_box_renders_to_a_valid_png() {
        let layout = rich_paragraph_layout(ParagraphAlign::Left);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        assert_eq!(layout.boxes.len(), 1, "fixture must place exactly one inline box");

        let spec = ExportSpec { width_px: WIDTH_P2, height_px: HEIGHT_P2, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            draw_paragraph(ctx, (MARGIN, 40.0), &layout, "#111111", true);
        })
        .expect("rich left-aligned proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH_P2, HEIGHT_P2));
        write_proof_png("text_p2_rich.png", &bytes);
    }

    #[test]
    fn rich_paragraph_justified_renders_to_a_valid_png() {
        let layout = rich_paragraph_layout(ParagraphAlign::Justify);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        assert_eq!(layout.boxes.len(), 1, "fixture must place exactly one inline box");

        let spec = ExportSpec { width_px: WIDTH_P2, height_px: HEIGHT_P2, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            draw_paragraph(ctx, (MARGIN, 40.0), &layout, "#111111", true);
        })
        .expect("rich justified proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH_P2, HEIGHT_P2));
        write_proof_png("text_p2_justify.png", &bytes);
    }

    /// Phase 5 headless proof: the SAME fixed two-paragraph text, justified,
    /// laid out under `Greedy` (left column) vs. `KnuthPlass` (right column)
    /// side by side — doubles as the visual quality-delta demonstration and
    /// a regression/parity check (design law 8: a two-column comparison,
    /// never a single screenshot).
    #[test]
    fn greedy_vs_knuth_plass_justified_side_by_side_renders_to_a_valid_png() {
        use crate::linebreak::BreakStrategy;

        const COL_WIDTH: f64 = 420.0;
        const MARGIN: f64 = 20.0;
        const GAP: f64 = 24.0;
        const HEADER_H: f64 = 30.0;
        const PARA_GAP: f64 = 16.0;
        const HEIGHT: u32 = 620;

        let width = (MARGIN * 2.0 + COL_WIDTH * 2.0 + GAP).round() as u32;

        let body_font = FontSpec::new(FontFamily::Roboto, 16.0);
        let label_font = FontSpec::new(FontFamily::Roboto, 15.0).bold();

        // Fixed seeded two-paragraph fixture — no lorem-ipsum RNG (design
        // law 8), deliberately describing the very comparison this PNG
        // renders.
        const PARA_A: &str = "A disproportionately long word early in a paragraph \
            can force a first-fit greedy packer into an unevenly loose line, while \
            the very next line ends up abnormally tight by comparison, which is \
            precisely the raggedness Knuth-Plass distributes evenly instead.";
        const PARA_B: &str = "Business documents full of long compound words like \
            implementation, infrastructure, and accountability often expose this \
            unevenness, especially inside a narrow justified column where every \
            extra letter matters more than it first seems to.";

        let shaper = CosmicShaper::headless();
        let runs_a = [StyledRun::new(PARA_A, body_font)];
        let runs_b = [StyledRun::new(PARA_B, body_font)];

        let build_column = |strategy: BreakStrategy,
                             label: &str|
         -> (ParagraphLayout, ParagraphLayout, ParagraphLayout) {
            let label_runs = [StyledRun::new(label, label_font)];
            let label_paragraph = Paragraph::new(&label_runs, COL_WIDTH);
            let a = Paragraph::new(&runs_a, COL_WIDTH).with_align(ParagraphAlign::Justify).with_break_strategy(strategy);
            let b = Paragraph::new(&runs_b, COL_WIDTH).with_align(ParagraphAlign::Justify).with_break_strategy(strategy);
            (layout_paragraph(&label_paragraph, &shaper), layout_paragraph(&a, &shaper), layout_paragraph(&b, &shaper))
        };

        let (greedy_label, greedy_a, greedy_b) = build_column(BreakStrategy::Greedy, "GREEDY + JUSTIFY");
        let (kp_label, kp_a, kp_b) = build_column(BreakStrategy::KnuthPlass, "KNUTH-PLASS + JUSTIFY");
        assert!(greedy_a.lines.len() > 1 && kp_a.lines.len() > 1, "fixture must wrap to multiple lines");

        let spec = ExportSpec { width_px: width, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            let left_x = MARGIN;
            let right_x = MARGIN + COL_WIDTH + GAP;

            draw_paragraph(ctx, (left_x, 24.0), &greedy_label, "#111111", false);
            draw_paragraph(ctx, (left_x, 24.0 + HEADER_H), &greedy_a, "#111111", false);
            let greedy_b_y = 24.0 + HEADER_H + greedy_a.height + PARA_GAP;
            draw_paragraph(ctx, (left_x, greedy_b_y), &greedy_b, "#111111", false);

            draw_paragraph(ctx, (right_x, 24.0), &kp_label, "#111111", false);
            draw_paragraph(ctx, (right_x, 24.0 + HEADER_H), &kp_a, "#111111", false);
            let kp_b_y = 24.0 + HEADER_H + kp_a.height + PARA_GAP;
            draw_paragraph(ctx, (right_x, kp_b_y), &kp_b, "#111111", false);

            let divider_x = MARGIN + COL_WIDTH + GAP / 2.0;
            ctx.set_stroke_color("#cccccccc");
            ctx.set_stroke_width(1.0);
            ctx.begin_path();
            ctx.move_to(divider_x, 10.0);
            ctx.line_to(divider_x, HEIGHT as f64 - 10.0);
            ctx.stroke();
        })
        .expect("greedy-vs-kp side-by-side proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (width, HEIGHT));
        write_proof_png("text_p5_greedy_vs_kp.png", &bytes);
    }

    /// Phase 5 headless proof: a narrow column (English hyphenation +
    /// `KnuthPlass`) — the fixture is chosen so at least one long
    /// hyphenatable word actually breaks mid-word at this width.
    #[test]
    fn knuth_plass_hyphenation_narrow_column_renders_to_a_valid_png() {
        use crate::linebreak::{BreakStrategy, Hyphenation};

        const COL_WIDTH: f64 = 150.0;
        const MARGIN: f64 = 16.0;
        const HEIGHT: u32 = 280;

        let width = (COL_WIDTH + MARGIN * 2.0).round() as u32;
        let font = FontSpec::new(FontFamily::Roboto, 16.0);

        // Fixed seeded fixture, deliberately dense with this crate's own
        // Liang v1 test vocabulary (design law 8: deterministic, no RNG).
        const TEXT: &str = "An understanding of wonderful hyphenation helps a \
            beautiful narrow column of business text stay even instead of \
            ragged, even when running and happen show up.";
        let runs = [StyledRun::new(TEXT, font)];
        let paragraph = Paragraph::new(&runs, COL_WIDTH)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        assert!(
            layout.glyphs.iter().any(|g| g.cluster == "-"),
            "narrow column fixture must visibly hyphenate at least one word"
        );

        let spec = ExportSpec { width_px: width, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            draw_paragraph(ctx, (MARGIN, 24.0), &layout, "#111111", false);
        })
        .expect("kp hyphenation proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (width, HEIGHT));
        write_proof_png("text_p5_kp_hyphen.png", &bytes);
    }
}
