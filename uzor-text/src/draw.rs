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
///
/// Also paints `layout.decorations` (typography-gap WAVE 2), if any, using
/// `color` as every span's own fallback (a span's own `color` override, if
/// set, wins — same convention [`draw_paragraph`] uses for glyph ink).
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

    draw_decorations(ctx, origin, layout, color);
}

/// Paint `layout.decorations` (typography-gap WAVE 2: underline/
/// strikethrough) as filled rects via the existing [`RenderContext::
/// fill_rect`] primitive (design law 6: no new drawing primitive) —
/// `(origin.0 + span.x_start, origin.1 + span.y, span.x_end - span.x_start,
/// span.thickness)`, one `fill_rect` call per span. A standalone entry
/// point (not folded into [`draw_paragraph`] alone) so a caller painting
/// glyph ink and decoration rects through TWO SEPARATE render-context
/// passes (`uzor-typeset::export::pdf_adapter`'s hybrid vector-text model —
/// glyph ink as real `Tj` runs, decoration rects as real vector-content
/// `fill_rect` ops, both against the SAME already-resolved layout) can
/// paint decorations without also re-painting glyph ink a second time.
/// [`draw_paragraph`]/[`draw_layout`] both already call this internally —
/// a caller using either of those never needs to call this separately.
pub fn draw_decorations(ctx: &mut dyn RenderContext, origin: (f64, f64), layout: &ParagraphLayout, default_color: &str) {
    for span in &layout.decorations {
        match span.color {
            Some(rgba) => ctx.set_fill_color(&rgba_hex(rgba)),
            None => ctx.set_fill_color(default_color),
        }
        ctx.fill_rect(origin.0 + span.x_start, origin.1 + span.y, span.x_end - span.x_start, span.thickness);
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

    draw_decorations(ctx, origin, layout, default_color);

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
        // uzor-figures's `proof_tests::out_dir`).
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

        const COL_WIDTH: f64 = 420.0;
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

    /// Typography-gap WAVE 2 headless proof: one paragraph exercising
    /// every new per-run attribute side by side — an underlined run, a
    /// strikethrough run, a wide-letter-spaced run, and `x` + superscript
    /// `2` + subscript `n` — confirmed by eye (design law 8).
    #[test]
    fn typography_wave_2_decorations_spacing_and_script_render_to_a_valid_png() {
        use crate::model::{TextDecoration, VerticalAlign};

        const WIDTH: u32 = 520;
        const HEIGHT: u32 = 160;

        let font = FontSpec::new(FontFamily::Roboto, 22.0);
        let runs = [
            StyledRun::new("Underlined", font).with_decoration(TextDecoration::underline()),
            StyledRun::new(" ", font),
            StyledRun::new("Struck", font).with_decoration(TextDecoration::strikethrough()),
            StyledRun::new(" ", font),
            StyledRun::new("Spaced", font).with_letter_spacing(6.0),
            StyledRun::new(" x", font),
            StyledRun::new("2", font).with_vertical_align(VerticalAlign::Super),
            StyledRun::new(" + a", font),
            StyledRun::new("n", font).with_vertical_align(VerticalAlign::Sub),
        ];
        let paragraph = Paragraph::new(&runs, (WIDTH as f64) - 40.0);
        let shaper = CosmicShaper::headless();
        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(!layout.decorations.is_empty(), "fixture must exercise at least one decoration span");

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            draw_paragraph(ctx, (20.0, 60.0), &layout, "#111111", false);
        })
        .expect("typography-wave-2 proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("text_wave2_decorations_spacing_script.png", &bytes);
    }

    /// Typography track T4, RE-PROVE (owner review, 2026-07-25): the
    /// justified-fixture regression floor this whole proof PNG depends on.
    /// [`protrusion_off_vs_on_labeled_three_panel_proof_renders_to_a_valid_png`]'s
    /// own fixture text/column width (`OVERSHOOT_FIXTURE_TEXT`/
    /// `OVERSHOOT_FIXTURE_COL_WIDTH`) was chosen SPECIFICALLY (measured,
    /// not eyeballed) so no justified line's advance-end exceeds the
    /// measure — this test locks that measurement in. See this module's
    /// own doc comment continuation on
    /// `knuth_plass_can_choose_a_locally_overfull_justified_line_a_known_pre_existing_limitation`
    /// below for the general (NOT universally true) case this fixture was
    /// deliberately chosen to avoid.
    #[test]
    fn justified_lines_never_exceed_the_measure_on_the_protrusion_proof_fixture() {
        use crate::linebreak::{BreakStrategy, Hyphenation};

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(OVERSHOOT_FIXTURE_TEXT, font)];
        let shaper = CosmicShaper::headless();
        let paragraph = Paragraph::new(&runs, OVERSHOOT_FIXTURE_COL_WIDTH)
            .with_align(ParagraphAlign::Justify)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English);
        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 3, "fixture must wrap to several lines");
        let last_index = layout.lines.len() - 1;

        let mut justified_line_count = 0;
        for line in &layout.lines {
            if line.line_index == last_index {
                continue; // the ragged last line is never justify-stretched — measuring it here is a different question, covered separately below
            }
            justified_line_count += 1;
            let Some(last_glyph) = layout.glyphs.iter().filter(|g| g.line_index == line.line_index).last() else { continue };
            let advance_end = last_glyph.x + last_glyph.advance;
            assert!(
                advance_end <= OVERSHOOT_FIXTURE_COL_WIDTH + 1e-6,
                "line {} ({:?}) must not exceed the measure {OVERSHOOT_FIXTURE_COL_WIDTH}, got advance_end={advance_end}",
                line.line_index,
                last_glyph.cluster
            );
        }
        assert!(justified_line_count >= 3, "fixture must exercise several justified (non-last) lines for this to be a meaningful regression floor");
    }

    /// Typography track T4, RE-PROVE (owner review, 2026-07-25): the
    /// GENERAL case, reported honestly rather than hidden by only ever
    /// testing a hand-picked clean fixture. [`crate::linebreak::knuth_plass`]
    /// is a deliberately SIMPLIFIED single-pass Knuth-Plass DP (this
    /// crate's own doc comment: "no looseness passes, no TeX fitness-class
    /// tiering") — unlike real TeX's `\tolerance` mechanism, it never
    /// treats "this candidate line's shrink NEED exceeds its render-time-
    /// capped shrink CAPACITY" as infeasible, only as increasingly
    /// EXPENSIVE (via `badness`'s own uncapped cubic growth). When every
    /// alternative breakpoint arrangement across the WHOLE paragraph
    /// scores worse in total demerits, the DP still picks this locally
    /// overfull line — a real "overfull hbox" outcome, the same
    /// PHENOMENON real TeX itself produces (and warns about) when no
    /// feasible breakpoint set exists under its own tolerance. This is
    /// PRE-EXISTING behavior (this module's own `CLAUDE.md` Phase 5
    /// section already documented the general shape of it — "a genuinely
    /// tight line may still render a few pixels past `max_width` in rare
    /// cases" — before this task ever started) and is NOT something
    /// typography track T4/T5 introduced: verified directly below by
    /// reproducing it with [`crate::linebreak::LineBreakParams::default`]
    /// (byte-identical pre-T5 constants) and WITH [`crate::linebreak::
    /// Hyphenation::English`] enabled (ruling out "just give it more break
    /// flexibility" as a one-line fix — this exact fixture was ALSO probed
    /// with hyphenation on and produced the identical overshoot).
    ///
    /// Numbers (probed 2026-07-25, `COL_WIDTH=320.0`, 16px Roboto,
    /// `Hyphenation::English`): line 1 ("well-set paragraph reads evenly,
    /// without ragged") has `natural_width=335.921875` against
    /// `max_width=320.0` — a `15.921875`px shrink NEED across 5 interword
    /// gaps (`~3.18`px/gap), but the render-time shrink CAP
    /// (`glue_shrink_ratio=1/3` of the narrowest gap's own `~3.97`px
    /// natural width, per [`crate::model::Paragraph::line_break_params`])
    /// only affords `~1.32`px/gap (`~6.61`px total) — leaving the line
    /// `9.307292`px over-full even after maximal render-time shrink. A
    /// full fix (make the DP treat "need > capacity" as infeasible, not
    /// merely expensive — closer to real TeX's own tolerance/overfull-hbox
    /// semantics) is a genuine cost-model redesign affecting every
    /// existing `KnuthPlass` caller, scoped OUT of this pass (a
    /// side-effect discovery while re-proving T4, not itself T4 or T5's
    /// own deliverable) — flagged here as a real, measured, follow-up-
    /// worthy finding rather than silently left undiscovered.
    #[test]
    fn knuth_plass_can_choose_a_locally_overfull_justified_line_a_known_pre_existing_limitation() {
        use crate::linebreak::{BreakStrategy, Hyphenation, LineBreakParams};

        const COL_WIDTH: f64 = 320.0;
        const TEXT: &str = "Good typography is invisible, or nearly so: a well-set \
            paragraph reads evenly, without ragged holes or crowded lines.";
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(TEXT, font)];
        let shaper = CosmicShaper::headless();

        let paragraph = Paragraph::new(&runs, COL_WIDTH)
            .with_align(ParagraphAlign::Justify)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English)
            .with_line_break_params(LineBreakParams::default()); // explicit: pre-T5 constants, unmodified
        let layout = layout_paragraph(&paragraph, &shaper);
        let last_index = layout.lines.len() - 1;

        let max_overshoot = layout
            .lines
            .iter()
            .filter(|line| line.line_index != last_index)
            .filter_map(|line| {
                let last_glyph = layout.glyphs.iter().filter(|g| g.line_index == line.line_index).last()?;
                Some((last_glyph.x + last_glyph.advance) - COL_WIDTH)
            })
            .fold(f64::MIN, f64::max);

        assert!(
            max_overshoot > 5.0,
            "this fixture is EXPECTED to reproduce the known overfull-hbox outcome (a real, pre-existing KnuthPlass property, not a T4/T5 regression) — got max_overshoot={max_overshoot}, expected > 5.0px; if this now reads ~0, the underlying cost model changed and this test (and its own doc comment) need revisiting"
        );
    }

    /// Fixed seeded fixture for the protrusion proof PNG + its own
    /// overshoot regression floor — chosen (MEASURED, not eyeballed) so
    /// every justified line lands at EXACTLY `max_width` (`0.0` overshoot)
    /// at `OVERSHOOT_FIXTURE_COL_WIDTH` with `Hyphenation::English`, and so
    /// most (3 of 4) justified lines end in a character T4's own default
    /// protrusion table covers (comma, hyphen×2) — a REAL consequence of
    /// this text's own clause lengths at this column width, not a forced/
    /// faked line break.
    const OVERSHOOT_FIXTURE_TEXT: &str = "Good margins read clean, calm, and quiet, line after line, \
        comma by comma, period by period, pause by careful pause, \
        letting each column breathe, evenly, without visible strain, \
        so hanging commas, quotes, and hyphens settle softly, always.";
    const OVERSHOOT_FIXTURE_COL_WIDTH: f64 = 395.0;

    /// Typography track T4 headless proof, RE-BUILT (owner review,
    /// 2026-07-25 — the first version was judged unjudgeable: only 2 of 7
    /// lines ended in tabled punctuation, the effect was barely
    /// perceptible at real table values, and the measure rule sat directly
    /// on top of glyph ink, inviting exactly the "is that overshoot or
    /// antialiasing" confusion the owner caught). Three panels, same
    /// justified Knuth-Plass paragraph
    /// ([`OVERSHOOT_FIXTURE_TEXT`]/[`OVERSHOOT_FIXTURE_COL_WIDTH`], MEASURED
    /// to have ZERO justified-line overshoot —
    /// `justified_lines_never_exceed_the_measure_on_the_protrusion_proof_fixture`
    /// above is this PNG's own direct regression guard):
    ///
    /// 1. **OFF** — protrusion disabled.
    /// 2. **ON (real)** — [`crate::model::ProtrusionTable::default_punctuation`],
    ///    the actual shipped values.
    /// 3. **ON (3x EXAGGERATED)** — every real factor multiplied by 3,
    ///    labeled as such — exists ONLY to prove the mechanism is wired and
    ///    show the direction of the effect, never presented as realistic.
    ///
    /// Each column gets a short TICK mark at the true `max_width` (in the
    /// header strip, above any glyph row — never overlapping ink) plus a
    /// thin DASHED rule OFFSET 4px outside the measure (never drawn
    /// through the text itself, so it can never be confused with glyph
    /// ink touching/crossing it — the owner's own specific complaint about
    /// the prior version). A second row crops+zooms (4x, via `ctx.scale`
    /// after `ctx.clip_rect`, painting the SAME already-resolved layout a
    /// second time — no new measurement, no new drawing primitive) the
    /// right-margin region of line 0 (ends in `","` in all three columns —
    /// protrusion never changes line breaks, only edge-glyph paint
    /// position, so the SAME line is comparable across all three) for a
    /// sub-pixel-to-few-pixel effect that the un-zoomed row alone cannot
    /// make legible.
    #[test]
    fn protrusion_off_vs_on_labeled_three_panel_proof_renders_to_a_valid_png() {
        use crate::linebreak::{BreakStrategy, Hyphenation};
        use crate::model::{ProtrusionFactors, ProtrusionTable};

        const MARGIN: f64 = 24.0;
        const COL_GAP: f64 = 30.0;
        const HEADER_H: f64 = 22.0;
        const ZOOM: f64 = 4.0;
        const ZOOM_LABEL_H: f64 = 18.0;
        const CROP_BEFORE_MEASURE: f64 = 40.0; // how far LEFT of max_width the crop window starts
        const CROP_AFTER_MEASURE: f64 = 18.0; // how far RIGHT of max_width the crop window ends
        const CROP_PAD_Y: f64 = 4.0;
        const TICK_H: f64 = 8.0;
        const RULE_OFFSET: f64 = 4.0; // the dashed reference rule sits this far OUTSIDE (right of) the true measure

        let body_font = FontSpec::new(FontFamily::Roboto, 16.0);
        let label_font = FontSpec::new(FontFamily::Roboto, 13.0).bold();
        let tag_font = FontSpec::new(FontFamily::Roboto, 10.0);

        let real_table = ProtrusionTable::default_punctuation();
        // Every real factor times 3 — same characters, same directional
        // shape (end_only/start_only/symmetric), purely scaled.
        let exaggerated_table = ProtrusionTable::new()
            .with_entry('.', ProtrusionFactors::end_only(3.0))
            .with_entry(',', ProtrusionFactors::end_only(3.0))
            .with_entry('-', ProtrusionFactors::symmetric(1.5))
            .with_entry(':', ProtrusionFactors::end_only(0.6))
            .with_entry(';', ProtrusionFactors::end_only(0.6))
            .with_entry('\u{0027}', ProtrusionFactors::symmetric(1.5))
            .with_entry('\u{0022}', ProtrusionFactors::symmetric(1.5))
            .with_entry('\u{2018}', ProtrusionFactors::start_only(1.5))
            .with_entry('\u{2019}', ProtrusionFactors::end_only(1.5))
            .with_entry('\u{201C}', ProtrusionFactors::start_only(1.5))
            .with_entry('\u{201D}', ProtrusionFactors::end_only(1.5));

        let shaper = CosmicShaper::headless();
        let runs = [StyledRun::new(OVERSHOOT_FIXTURE_TEXT, body_font)];
        let col_width = OVERSHOOT_FIXTURE_COL_WIDTH;

        let base = || {
            Paragraph::new(&runs, col_width).with_align(ParagraphAlign::Justify).with_break_strategy(BreakStrategy::KnuthPlass).with_hyphenation(Hyphenation::English)
        };
        let off_layout = layout_paragraph(&base(), &shaper);
        let real_layout = layout_paragraph(&base().with_protrusion(&real_table), &shaper);
        let exaggerated_layout = layout_paragraph(&base().with_protrusion(&exaggerated_table), &shaper);

        assert!(off_layout.lines.len() > 3, "fixture must wrap to several lines at this column width");
        for other in [&real_layout, &exaggerated_layout] {
            assert_eq!(off_layout.lines.len(), other.lines.len(), "protrusion must never change the line count, only edge-glyph paint position");
        }

        // RE-PROVE (owner review, 2026-07-25): the paragraph's own ragged
        // FINAL line must be byte-identical across all three variants —
        // the exact defect an earlier version of this test caught by eye
        // ("line after line ." with a stray gap before the period).
        let last_line_index = off_layout.lines.len() - 1;
        let off_last: Vec<&crate::layout::GlyphLayout> = off_layout.glyphs.iter().filter(|g| g.line_index == last_line_index).collect();
        for other in [&real_layout, &exaggerated_layout] {
            let other_last: Vec<&crate::layout::GlyphLayout> = other.glyphs.iter().filter(|g| g.line_index == last_line_index).collect();
            assert_eq!(off_last.len(), other_last.len());
            for (a, b) in off_last.iter().zip(other_last.iter()) {
                assert_eq!(a.cluster, b.cluster);
                assert_eq!(a.x, b.x, "final line must be byte-identical across every protrusion variant — {:?} moved from {} to {}", a.cluster, a.x, b.x);
            }
        }

        // The zoom target: line 0, which ends in "," in every variant
        // (protrusion never changes WHICH glyph is last on a line).
        let zoom_line_index = 0usize;
        let zoom_line = off_layout.lines[zoom_line_index];
        assert_eq!(
            off_layout.glyphs.iter().filter(|g| g.line_index == zoom_line_index).last().map(|g| g.cluster.as_str()),
            Some(","),
            "regression floor: the zoom panel targets line 0 assuming it ends in a comma"
        );

        let panels: [(&str, &ParagraphLayout, &str); 3] =
            [("OFF", &off_layout, "#111111"), ("ON \u{2014} real values", &real_layout, "#0a5c2a"), ("ON \u{2014} 3\u{d7} EXAGGERATED (mechanism proof only)", &exaggerated_layout, "#8a2a00")];

        let row1_h = HEADER_H + off_layout.height + 10.0;
        let crop_w = CROP_BEFORE_MEASURE + CROP_AFTER_MEASURE;
        let crop_h = zoom_line.height + 2.0 * CROP_PAD_Y;
        let zoom_panel_w = crop_w * ZOOM;
        let zoom_panel_h = crop_h * ZOOM;
        let row2_label_y = MARGIN + row1_h + 14.0;
        let row2_panel_y = row2_label_y + ZOOM_LABEL_H;

        let main_row_width = MARGIN * 2.0 + col_width * 3.0 + COL_GAP * 2.0 + 60.0; // +60: room for the trailing "measure" tag label past the last column's own rule
        let zoom_row_width = MARGIN * 2.0 + zoom_panel_w * 3.0 + COL_GAP * 2.0;
        let width = main_row_width.max(zoom_row_width).round() as u32;
        let height = (row2_panel_y + zoom_panel_h + MARGIN).round() as u32;

        let spec = ExportSpec { width_px: width, height_px: height, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            for (i, (label, layout, color)) in panels.iter().enumerate() {
                let col_x = MARGIN + (col_width + COL_GAP) * i as f64;
                let body_y = MARGIN + HEADER_H;

                let label_runs = [StyledRun::new(*label, label_font)];
                let label_layout = layout_paragraph(&Paragraph::new(&label_runs, col_width), &shaper);
                draw_paragraph(ctx, (col_x, MARGIN), &label_layout, "#111111", false);
                draw_paragraph(ctx, (col_x, body_y), layout, color, false);

                // Short tick at the TRUE measure, in the header strip only
                // (never crossing a glyph row) + a thin DASHED rule offset
                // outside the measure — cannot be confused with glyph ink
                // touching/crossing it, per the owner's own review.
                let measure_x = col_x + col_width;
                ctx.set_stroke_color("#666666ff");
                ctx.set_stroke_width(1.0);
                ctx.begin_path();
                ctx.move_to(measure_x, MARGIN - TICK_H);
                ctx.line_to(measure_x, MARGIN);
                ctx.stroke();

                ctx.set_stroke_color("#cc333399");
                ctx.set_stroke_width(0.75);
                ctx.set_line_dash(&[3.0, 3.0]);
                ctx.begin_path();
                ctx.move_to(measure_x + RULE_OFFSET, body_y - 4.0);
                ctx.line_to(measure_x + RULE_OFFSET, body_y + layout.height + 4.0);
                ctx.stroke();
                ctx.set_line_dash(&[]);

                let tag_runs = [StyledRun::new("measure", tag_font)];
                let tag_layout = layout_paragraph(&Paragraph::new(&tag_runs, 200.0), &shaper);
                draw_paragraph(ctx, (measure_x + RULE_OFFSET + 4.0, MARGIN - TICK_H - 2.0), &tag_layout, "#666666", false);

                // Zoom row: crop [measure_x - CROP_BEFORE_MEASURE, measure_x
                // + CROP_AFTER_MEASURE] x [line top .. line bottom] of THIS
                // SAME already-resolved layout, scaled ZOOM x — computed
                // MANUALLY per selected glyph (screen_x/screen_y below),
                // never via `ctx.scale`/`ctx.clip_rect`: this render
                // backend's own clip does not reliably bound a scaled
                // transform (verified empirically — an earlier version
                // using `save`/`clip_rect`/`translate`/`scale` rendered
                // the WHOLE paragraph, unclipped, bleeding across every
                // panel). Only glyphs whose own x-range overlaps the crop
                // window are drawn — the SAME positions/advances/fonts
                // `layout` already computed, just re-projected into the
                // zoom panel's own local pixel space (design law 6: no new
                // measurement, still only `set_font`+`fill_text`).
                let crop_x0 = measure_x - CROP_BEFORE_MEASURE;
                let crop_x1 = crop_x0 + crop_w;
                let crop_y0 = body_y + zoom_line.y_top - CROP_PAD_Y;
                let zoom_panel_x = MARGIN + (zoom_panel_w + COL_GAP) * i as f64;

                let zoom_label_runs = [StyledRun::new("ZOOM 4\u{d7}", tag_font)];
                let zoom_label_layout = layout_paragraph(&Paragraph::new(&zoom_label_runs, 200.0), &shaper);
                draw_paragraph(ctx, (zoom_panel_x, row2_label_y), &zoom_label_layout, "#666666", false);

                ctx.set_stroke_color("#00000033");
                ctx.set_stroke_width(1.0);
                ctx.stroke_rect(zoom_panel_x, row2_panel_y, zoom_panel_w, zoom_panel_h);

                ctx.set_text_align(uzor::render::TextAlign::Left);
                ctx.set_text_baseline(uzor::render::TextBaseline::Alphabetic);
                for g in layout.glyphs.iter().filter(|g| g.line_index == zoom_line_index) {
                    if g.cluster.is_empty() {
                        continue;
                    }
                    let abs_x = col_x + g.x;
                    let center = abs_x + g.advance / 2.0;
                    if center < crop_x0 || center > crop_x1 {
                        continue; // only glyphs MOSTLY inside the crop window — never a half-glyph overflowing the panel's own edge
                    }
                    let screen_x = zoom_panel_x + (abs_x - crop_x0) * ZOOM;
                    let screen_y = row2_panel_y + (body_y + g.y - crop_y0) * ZOOM;
                    let scaled_font = FontSpec { size_px: g.font.size_px * ZOOM, ..g.font };
                    ctx.set_font(&scaled_font.to_css_font());
                    ctx.set_fill_color(color);
                    ctx.fill_text(&g.cluster, screen_x, screen_y);
                }

                let rule_screen_x = zoom_panel_x + (measure_x + RULE_OFFSET - crop_x0) * ZOOM;
                ctx.set_stroke_color("#cc333399");
                ctx.set_stroke_width(0.75);
                ctx.set_line_dash(&[3.0, 3.0]);
                ctx.begin_path();
                ctx.move_to(rule_screen_x, row2_panel_y);
                ctx.line_to(rule_screen_x, row2_panel_y + zoom_panel_h);
                ctx.stroke();
                ctx.set_line_dash(&[]);

                let tick_screen_x = zoom_panel_x + (measure_x - crop_x0) * ZOOM;
                ctx.set_stroke_color("#666666ff");
                ctx.set_stroke_width(1.0);
                ctx.begin_path();
                ctx.move_to(tick_screen_x, row2_panel_y);
                ctx.line_to(tick_screen_x, row2_panel_y + zoom_panel_h);
                ctx.stroke();
            }
        })
        .expect("protrusion three-panel proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (width, height));
        write_proof_png("text_t4_protrusion_off_vs_on.png", &bytes);
    }
}
