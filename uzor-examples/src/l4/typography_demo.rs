//! # Typography reference demo — the text-calibration fixture
//!
//! URX text-gamma arc (2026-07-24 owner redirect): `l3-dashboard` is a
//! LEGACY interface, retired from the reference gate entirely (owner
//! verdict) — this demo replaces it as THE text-weight calibration
//! target. Built fresh rather than adopted from an existing bin: of
//! `uzor-examples`'s own demos, only `text-kinetics-demo` exercises
//! `RenderContext::fill_text` through a REAL paragraph-layout pipeline
//! (`uzor_text::draw_paragraph`), and even that one is a single fixed
//! paragraph at one font/size/color — it has no headings, no multiple
//! sizes, no numbers table, no muted-label row. `uzor-typeset`'s own
//! richer fixtures (`showcase.rs`/`yozarest_report.rs`) exist but are
//! either real Foxhound case content (inappropriate to repurpose for a
//! generic engine-calibration fixture) or a heavy multi-page PDF-export
//! pipeline (`pages_to_pdf`) not shaped for a quick `RenderContext`-
//! comparable PNG). Building `typography_page` directly gives exact
//! control over the content profile the owner specified: an H1 + H2
//! heading pair, two body paragraphs at 13-14px (via the REAL
//! `uzor_text::layout_text`/`draw_paragraph` pipeline — the same one
//! production dashboards use, `TextBaseline::Alphabetic` per line), an
//! 11px label row in `#a0a0b0` (the EXACT color `l3-dashboard`'s own
//! secondary radio labels used — the "reads too faint" symptom this
//! whole arc investigates), and a small numbers table — all routed
//! through `RenderContext::fill_text`, which now emits
//! `DrawCommand::GlyphRun` on the URX leg (`uzor 03be757`).
//!
//! Run:
//! ```sh
//! cargo run -p uzor-examples --bin typography-demo
//! ```

use uzor::fonts::FontFamily;
use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::layout::LayoutManager;
use uzor::platform::types::CornerStyle;
use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor_desktop::AppRun as _;
use uzor_text::{draw_paragraph, layout_text, CosmicShaper, FontSpec};

pub const TYPOGRAPHY_WIDTH: f64 = 1280.0;
pub const TYPOGRAPHY_HEIGHT: f64 = 800.0;

const BACKGROUND_COLOR: &str = "#0d0f14";
const HEADING_COLOR: &str = "#ffffff";
const BODY_COLOR: &str = "#d1d4dc";
/// The EXACT color `l3-dashboard`'s own unselected sidebar radio labels
/// used (`uzor/src/ui/widgets/composite/sidebar/input.rs::add_radio_group`)
/// — the color that read markedly fainter under real `GlyphRun`
/// rendering than under the old vector-outline path.
const LABEL_COLOR: &str = "#a0a0b0";
const NUMBER_COLOR: &str = "#e8e8e8";
const POSITIVE_COLOR: &str = "#4caf50";
const NEGATIVE_COLOR: &str = "#ef5350";

const MARGIN: f64 = 48.0;

const H1_TEXT: &str = "Typography Reference";
const H2_TEXT: &str = "Body copy, labels, and tabular figures";

/// Fixed, seeded prose — no RNG, deterministic across every run (same
/// discipline every other fixture in this workspace follows).
const PARA_1: &str = "The quick brown fox jumps over the lazy dog while the market consolidates near its prior session close, holding a narrow range through the opening hour before volume begins to build steadily across the major pairs.";
const PARA_2: &str = "Realized volatility remains compressed relative to the trailing thirty day average, and open interest continues to climb even as funding rates stay flat, a combination worth watching closely into the next scheduled catalyst.";

const LABEL_ROW: [&str; 4] = ["SYMBOL", "PRICE", "CHANGE", "VOLUME"];
const TABLE_ROWS: [(&str, &str, &str, &str); 5] = [
    ("BTC/USDT", "67,234.5", "+1.2%", "12.4M"),
    ("ETH/USDT", "3,421.8", "-0.8%", "8.2M"),
    ("SOL/USDT", "182.3", "+3.4%", "5.1M"),
    ("BNB/USDT", "612.7", "+0.5%", "2.8M"),
    ("ADA/USDT", "0.45", "-1.1%", "1.5M"),
];
const COL_X_OFFSETS: [f64; 4] = [0.0, 200.0, 340.0, 460.0];

/// Draw the fixed typography reference page. Pure function of `(w, h)`
/// -- no retained state, deterministic every call (same "record once,
/// replay to every backend" contract every other app-level parity
/// fixture in this workspace already follows).
pub fn typography_page(ctx: &mut dyn RenderContext, w: f64, h: f64) {
    ctx.set_fill_color(BACKGROUND_COLOR);
    ctx.fill_rect(0.0, 0.0, w, h);

    let shaper = CosmicShaper::headless();
    let body_width = (w - MARGIN * 2.0).max(100.0);

    // H1 + H2 -- single-line headings, plain `fill_text` (no wrap needed).
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Alphabetic);

    ctx.set_fill_color(HEADING_COLOR);
    ctx.set_font("bold 28px Roboto");
    let h1_baseline_y = MARGIN + 28.0;
    ctx.fill_text(H1_TEXT, MARGIN, h1_baseline_y);

    ctx.set_font("18px Roboto");
    let h2_baseline_y = h1_baseline_y + 34.0;
    ctx.fill_text(H2_TEXT, MARGIN, h2_baseline_y);

    // Two body paragraphs at 13/14px via the REAL paragraph-layout
    // pipeline (`layout_text`/`draw_paragraph`) -- `TextBaseline::Alphabetic`
    // per line, exactly like `uzor-text::draw.rs::draw_paragraph`'s own
    // production convention.
    let font_13 = FontSpec::new(FontFamily::Roboto, 13.0);
    let font_14 = FontSpec::new(FontFamily::Roboto, 14.0);

    let para1_top = h2_baseline_y + 34.0;
    let para1_layout = layout_text(PARA_1, &font_13, body_width, &shaper);
    draw_paragraph(ctx, (MARGIN, para1_top), &para1_layout, BODY_COLOR, false);

    let para2_top = para1_top + para1_layout.height + 20.0;
    let para2_layout = layout_text(PARA_2, &font_14, body_width, &shaper);
    draw_paragraph(ctx, (MARGIN, para2_top), &para2_layout, BODY_COLOR, false);

    // Label row -- 11px, the exact muted-grey color this whole arc is
    // investigating.
    let label_baseline_y = para2_top + para2_layout.height + 32.0;
    ctx.set_fill_color(LABEL_COLOR);
    ctx.set_font("11px Roboto");
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Alphabetic);
    for (label, x_off) in LABEL_ROW.iter().zip(COL_X_OFFSETS) {
        ctx.fill_text(label, MARGIN + x_off, label_baseline_y);
    }

    // Numbers table -- small numeric figures, positive/negative accent
    // colors (same convention `l3-dashboard`'s own watchlist table used).
    ctx.set_font("13px Roboto");
    let mut row_baseline_y = label_baseline_y + 26.0;
    for (symbol, price, change, volume) in TABLE_ROWS {
        ctx.set_fill_color(NUMBER_COLOR);
        ctx.fill_text(symbol, MARGIN + COL_X_OFFSETS[0], row_baseline_y);
        ctx.fill_text(price, MARGIN + COL_X_OFFSETS[1], row_baseline_y);
        ctx.set_fill_color(if change.starts_with('+') { POSITIVE_COLOR } else { NEGATIVE_COLOR });
        ctx.fill_text(change, MARGIN + COL_X_OFFSETS[2], row_baseline_y);
        ctx.set_fill_color(NUMBER_COLOR);
        ctx.fill_text(volume, MARGIN + COL_X_OFFSETS[3], row_baseline_y);
        row_baseline_y += 24.0;
    }
}

// ── App (minimal -- static content, no interaction state needed) ──────

struct DemoApp;

impl App<NoPanel> for DemoApp {
    fn init(&mut self, _key: &WindowKey, _layout: &mut LayoutManager<NoPanel>) {}

    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        let w = win.rect.width.max(1.0);
        let h = win.rect.height.max(1.0);
        typography_page(win.render, w, h);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DemoApp)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor — typography reference")
                .size(TYPOGRAPHY_WIDTH as u32, TYPOGRAPHY_HEIGHT as u32)
                .min_size(720, 480)
                .decorations(false)
                .background(0xFF_0d_0f_14)
                .corner_style(CornerStyle::Rounded)
                .border_color(0x00_4d_90_fe),
        )
        .icon_from_png(include_bytes!("../../assets/icon.png"))?
        .run()?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use uzor_export::{render_to_png, ExportSpec};

    use super::*;

    /// Proves `typography_page` renders headlessly end-to-end (no
    /// window, no GPU) -- the same function the live app uses.
    #[test]
    fn typography_page_renders_headlessly_to_a_valid_png() {
        let spec = ExportSpec { width_px: TYPOGRAPHY_WIDTH as u32, height_px: TYPOGRAPHY_HEIGHT as u32, dpr: 1.0, background: None };
        let bytes = render_to_png(&spec, |ctx| typography_page(ctx, TYPOGRAPHY_WIDTH, TYPOGRAPHY_HEIGHT))
            .expect("typography_page should render headlessly");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}

// ── App-level screenshot-diff harness fixture ──────────────────────────
//
// Same "record once, replay to every backend" contract as
// `figures_demo`/`force_graph_demo`'s own fixtures.

#[cfg(test)]
mod screenshot_diff {
    use uzor_examples::parity_harness::{
        compare_tight, dump_comparison_pngs, record_via_urx_ctx, render_via_urx_cpu, render_via_urx_native,
        render_via_vello_cpu, ChannelTolerance,
    };

    use super::*;

    const WIDTH: u32 = TYPOGRAPHY_WIDTH as u32;
    const HEIGHT: u32 = TYPOGRAPHY_HEIGHT as u32;

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn typography_page_frame_native_matches_cpu_within_the_base_tier() {
        let scene = record_via_urx_ctx(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64));
        let cpu = render_via_urx_cpu(&scene, WIDTH, HEIGHT);
        let Some(native) = render_via_urx_native(&scene, WIDTH, HEIGHT) else {
            eprintln!("typography_page_frame_native_matches_cpu_within_the_base_tier: no GPU/software adapter available; skipping");
            return;
        };
        let tol = ChannelTolerance::default();
        let report = compare_tight(&cpu, &native, tol);
        println!(
            "typography_page_frame urx-cpu-vs-native: {:.3}% differing (edge {}), budget {:.2}%, max_channel_diff {}",
            report.differing_fraction * 100.0,
            tol.edge,
            tol.max_differing_fraction * 100.0,
            report.max_channel_diff,
        );
        if !report.within_budget {
            dump_comparison_pngs("typography_page_urx_cpu_vs_native", WIDTH, HEIGHT, &cpu, &native);
        }
        assert!(
            report.within_budget,
            "typography_page frame exceeded the base tolerance tier: {:.3}% differing (budget {:.2}%), max_channel_diff {}",
            report.differing_fraction * 100.0,
            tol.max_differing_fraction * 100.0,
            report.max_channel_diff,
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; dumps PNGs for human review, not a hard gate"]
    fn typography_page_frame_native_vs_vello_visual_dump() {
        let vello = render_via_vello_cpu(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64));
        let scene = record_via_urx_ctx(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64));
        let Some(native) = render_via_urx_native(&scene, WIDTH, HEIGHT) else {
            eprintln!("typography_page_frame_native_vs_vello_visual_dump: no GPU/software adapter available; skipping");
            return;
        };
        dump_comparison_pngs("typography_page_urx_native_vs_vello", WIDTH, HEIGHT, &native, &vello);
    }
}

// ── Vertical-position check ──────────────────────────────────────────
//
// Owner ask (2026-07-24 redirect): "add an explicit assertion/
// measurement comparing TEXT LINE Y-POSITIONS between the urx leg and
// the vello leg." Both legs render CPU-only (`render_via_urx_cpu`/
// `render_via_vello_cpu`) -- no GPU adapter needed, so this runs on
// every `cargo test` pass, not just an `--ignored` hardware run.

#[cfg(test)]
mod row_ink_profile {
    use uzor_examples::parity_harness::{record_via_urx_ctx, render_via_urx_cpu, render_via_vello_cpu};

    use super::*;

    const WIDTH: u32 = TYPOGRAPHY_WIDTH as u32;
    const HEIGHT: u32 = TYPOGRAPHY_HEIGHT as u32;

    /// `#0d0f14` -- [`BACKGROUND_COLOR`] as an `(r, g, b)` triple.
    const BACKGROUND_RGB: (i32, i32, i32) = (0x0d, 0x0f, 0x14);

    /// A pixel counts as "ink" when any channel departs from
    /// [`BACKGROUND_RGB`] by more than this -- generous enough to skip
    /// sub-pixel AA fringe noise, tight enough to catch every real glyph
    /// stroke.
    const INK_THRESHOLD: i32 = 10;

    /// Adjacent ink rows separated by this many (or fewer) blank rows
    /// merge into the same band -- collapses a single line's own
    /// ascender/x-height/descender gaps (a few px of blank space inside
    /// one line of text) without merging two DIFFERENT lines together
    /// (which sit much further apart -- the smallest inter-line gap in
    /// [`typography_page`] is a wrapped-paragraph line-height, always
    /// comfortably wider than this).
    const BAND_MERGE_GAP: u32 = 2;

    /// For a rendered `width x height` premultiplied-RGBA8 buffer, the
    /// sorted list of contiguous "ink bands" (`(first_row, last_row)`,
    /// inclusive) -- one band per visually distinct row of text ink.
    /// Two renders of the SAME content by two DIFFERENT rasterisers
    /// produce matching band counts/positions (within a few px of
    /// AA/hinting slop) exactly when they agree on where each text line
    /// sits vertically; a real baseline-offset bug shifts every band
    /// by the bug's own per-element pixel delta instead.
    fn ink_row_bands(pixels: &[u8], width: u32, height: u32) -> Vec<(u32, u32)> {
        let mut bands: Vec<(u32, u32)> = Vec::new();
        for y in 0..height {
            let row_start = (y * width * 4) as usize;
            let row_end = row_start + (width * 4) as usize;
            let row = &pixels[row_start..row_end];
            let has_ink = row.chunks_exact(4).any(|px| {
                (px[0] as i32 - BACKGROUND_RGB.0).abs() > INK_THRESHOLD
                    || (px[1] as i32 - BACKGROUND_RGB.1).abs() > INK_THRESHOLD
                    || (px[2] as i32 - BACKGROUND_RGB.2).abs() > INK_THRESHOLD
            });
            if !has_ink {
                continue;
            }
            match bands.last_mut() {
                Some((_, end)) if y <= *end + BAND_MERGE_GAP => *end = y,
                _ => bands.push((y, y)),
            }
        }
        bands
    }

    /// The actual "vertical squashing" check: urx-cpu and vello-cpu run
    /// the SAME `typography_page` closure and must agree on how many
    /// text-line ink bands exist and where each one starts, within a
    /// small AA/hinting tolerance. This is the fixture that would have
    /// caught the `TextBaseline::Alphabetic` bug directly (see
    /// `uzor-render-urx`/`uzor-render-vello-cpu`'s own `fill_text` doc
    /// comments) -- a leg with the bug shifts every band down by that
    /// element's own `size * 0.35`, desyncing band counts/positions from
    /// a leg without it.
    #[test]
    fn typography_page_urx_cpu_and_vello_cpu_agree_on_text_line_y_positions() {
        let scene = record_via_urx_ctx(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64));
        let urx = render_via_urx_cpu(&scene, WIDTH, HEIGHT);
        let vello = render_via_vello_cpu(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64));

        let urx_bands = ink_row_bands(&urx, WIDTH, HEIGHT);
        let vello_bands = ink_row_bands(&vello, WIDTH, HEIGHT);

        println!("urx-cpu ink bands   ({}): {:?}", urx_bands.len(), urx_bands);
        println!("vello-cpu ink bands ({}): {:?}", vello_bands.len(), vello_bands);

        assert_eq!(
            urx_bands.len(),
            vello_bands.len(),
            "urx-cpu and vello-cpu must find the same number of text-line ink bands -- a mismatch means one leg is \
             merging/splitting lines differently, itself a vertical-position symptom"
        );

        let mut max_start_offset = 0u32;
        for (&(u_start, _), &(v_start, _)) in urx_bands.iter().zip(vello_bands.iter()) {
            let offset = u_start.abs_diff(v_start);
            max_start_offset = max_start_offset.max(offset);
        }
        println!("max per-band start-row offset (urx vs vello): {max_start_offset}px");

        assert!(
            max_start_offset <= 3,
            "urx-cpu vs vello-cpu text line y-positions diverge by {max_start_offset}px (budget 3px) -- \
             re-check TextBaseline handling in both fill_text implementations"
        );
    }
}

// ── Text-gamma calibration (re-run against real typography content) ──
//
// Owner ask (2026-07-24 redirect): re-run the URX text-gamma sweep
// (`uzor-urx-core/src/text_gamma.rs`) against real typographic content
// now that `UrxRenderContext::fill_text` reaches `DrawCommand::GlyphRun`
// (`uzor 03be757`) -- l3-dashboard's own scene had ZERO GlyphRun
// commands at calibration time (see `TEXT_GAMMA_CURVE`'s own doc
// comment); this fixture's `typography_page` is built specifically to
// have real ones. Same two-pass sweep shape as
// `l3::dashboard::text_gamma_calibration` (coarse step 0.1, refine step
// 0.02, argmin-prefer-smaller-gamma tiebreak), generalized from that
// module's solid-black/white "R byte is coverage" trick to a busy,
// multi-colored dark-theme page via a text-affected pixel mask.

#[cfg(test)]
mod text_gamma_calibration {
    use uzor_examples::parity_harness::{dump_comparison_pngs, record_via_urx_ctx, render_via_vello_cpu};
    use uzor_urx_core::math::Brush;
    use uzor_urx_core::scene::{DrawCommand, Scene};
    use uzor_urx_core::text_gamma::{build_text_gamma_lut, luma_bin, TextGammaLut, TEXT_GAMMA_BINS};
    use uzor_urx_cpu::{CpuBackend, Pixmap};

    use super::*;

    const WIDTH: u32 = TYPOGRAPHY_WIDTH as u32;
    const HEIGHT: u32 = TYPOGRAPHY_HEIGHT as u32;

    /// Byte-tight "differing" threshold, matching `ChannelTolerance::
    /// default().edge` (24) -- reused here as a bare constant since
    /// `compare_masked` scopes to a boolean mask the shared
    /// `compare_tight` comparator has no notion of.
    const DIFFERING_EDGE: i32 = 24;

    fn record_scene() -> Scene {
        record_via_urx_ctx(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64))
    }

    /// Render `scene` through `uzor-urx-cpu` with an EXPLICIT candidate
    /// LUT -- via `CpuBackend::render_with_gamma_lut_for_test` (the
    /// `#[doc(hidden)]` calibration-tooling entry point Commit 1 added
    /// specifically so a sweep can try an arbitrary curve without
    /// touching the production `TEXT_GAMMA_CURVE` constant or
    /// `UrxConfig::text_gamma_enabled`).
    fn render_via_urx_cpu_with_gamma(scene: &Scene, lut: &TextGammaLut) -> Vec<u8> {
        let mut pixmap = Pixmap::new(WIDTH, HEIGHT);
        let backend = CpuBackend::new();
        backend
            .render_with_gamma_lut_for_test(scene, &mut pixmap, Some(lut))
            .expect("CpuBackend::render must not error on the typography_page scene");
        pixmap.pixels().to_vec()
    }

    /// `GlyphRun` count + brush luma-bin census. Proves this fixture has
    /// real text on the `GlyphRun` path (unlike l3-dashboard's own scene
    /// at calibration time, which had zero -- see `TEXT_GAMMA_CURVE`'s
    /// own doc comment), and empirically answers "does real content ever
    /// reach text-gamma's UNEXPLORED dark bin (bin 0)?" instead of only
    /// the light bin (bin 1) the earlier synthetic-scene sweep covered.
    fn print_scene_census(scene: &Scene) -> usize {
        let mut glyph_run_count = 0usize;
        let mut fill_path_count = 0usize;
        let mut bin_counts = [0usize; TEXT_GAMMA_BINS];
        for cmd in &scene.commands {
            match cmd {
                DrawCommand::GlyphRun { brush, .. } => {
                    glyph_run_count += 1;
                    if let Brush::Solid(color) = brush {
                        let rgba = color.to_rgba8();
                        bin_counts[luma_bin([rgba.r, rgba.g, rgba.b, rgba.a]) as usize] += 1;
                    }
                }
                DrawCommand::FillPath { .. } => fill_path_count += 1,
                _ => {}
            }
        }
        println!("scene census: {glyph_run_count} GlyphRun commands, {fill_path_count} FillPath commands");
        println!(
            "GlyphRun brush luma-bin distribution: bin0(dark)={} bin1(light)={}",
            bin_counts[0], bin_counts[1]
        );
        glyph_run_count
    }

    /// Diff a gamma=1.0 render against a deliberately extreme gamma=3.0
    /// render of the SAME scene -- any pixel that moves is, by
    /// construction, exactly one gamma CAN affect (bin 0 is pinned to
    /// `1.0` regardless of the swept value, `build_text_gamma_lut`'s own
    /// row-0 convention, so a mask pixel implies bin-1 text ink).
    /// Generalizes `l3::dashboard::text_gamma_calibration`'s solid
    /// black/white "R byte is coverage" trick to this busy, multi-
    /// colored real page.
    fn text_affected_mask(scene: &Scene) -> Vec<bool> {
        let identity = build_text_gamma_lut(&[1.0, 1.0]);
        let extreme = build_text_gamma_lut(&[1.0, 3.0]);
        let a = render_via_urx_cpu_with_gamma(scene, &identity);
        let b = render_via_urx_cpu_with_gamma(scene, &extreme);
        a.chunks_exact(4).zip(b.chunks_exact(4)).map(|(pa, pb)| pa != pb).collect()
    }

    /// Average per-pixel Rec.601 luma restricted to `mask` -- the masked
    /// generalization of the l3-era sweep's whole-canvas `average_ink`
    /// (that trick relied on a solid black/white canvas where a pixel's
    /// own R byte WAS its coverage; this busy real scene has no such
    /// shortcut, so the same "average brightness" idea is restricted to
    /// only the pixels gamma can actually move).
    fn masked_avg_luma(pixels: &[u8], mask: &[bool]) -> f64 {
        let mut sum = 0u64;
        let mut n = 0u64;
        for (px, &m) in pixels.chunks_exact(4).zip(mask.iter()) {
            if !m {
                continue;
            }
            let luma = (299 * px[0] as u32 + 587 * px[1] as u32 + 114 * px[2] as u32) / 1000;
            sum += luma as u64;
            n += 1;
        }
        if n == 0 { 0.0 } else { sum as f64 / n as f64 / 255.0 }
    }

    /// `compare_tight`-style diff restricted to `mask` -- same shape as
    /// the shared harness's whole-image comparator (`DIFFERING_EDGE`
    /// matches its own default `edge`), scoped to the text-affected
    /// region only.
    fn compare_masked(a: &[u8], b: &[u8], mask: &[bool]) -> (f64, i32) {
        let mut differing = 0usize;
        let mut total = 0usize;
        let mut max_diff = 0i32;
        for ((pa, pb), &m) in a.chunks_exact(4).zip(b.chunks_exact(4)).zip(mask.iter()) {
            if !m {
                continue;
            }
            total += 1;
            let d = pa.iter().zip(pb.iter()).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0);
            max_diff = max_diff.max(d);
            if d > DIFFERING_EDGE {
                differing += 1;
            }
        }
        let frac = if total == 0 { 0.0 } else { differing as f64 / total as f64 };
        (frac, max_diff)
    }

    struct SweepRow {
        gamma: f32,
        text_region_differing_fraction: f64,
        text_region_max_channel_diff: i32,
        urx_masked_luma: f64,
        ink_gap: f64,
    }

    fn sweep_row(scene: &Scene, vello: &[u8], vello_masked_luma: f64, mask: &[bool], gamma: f32) -> SweepRow {
        let lut = build_text_gamma_lut(&[1.0, gamma]);
        let urx = render_via_urx_cpu_with_gamma(scene, &lut);
        let (text_region_differing_fraction, text_region_max_channel_diff) = compare_masked(&urx, vello, mask);
        let urx_masked_luma = masked_avg_luma(&urx, mask);
        SweepRow {
            gamma,
            text_region_differing_fraction,
            text_region_max_channel_diff,
            urx_masked_luma,
            ink_gap: (urx_masked_luma - vello_masked_luma).abs(),
        }
    }

    fn print_row(row: &SweepRow, vello_masked_luma: f64) {
        println!(
            "gamma={:.2}  text_region_differing_fraction={:.4}  max_channel_diff={}  urx_masked_luma={:.4}  \
             vello_masked_luma={:.4}  ink_gap={:.4}",
            row.gamma,
            row.text_region_differing_fraction,
            row.text_region_max_channel_diff,
            row.urx_masked_luma,
            vello_masked_luma,
            row.ink_gap,
        );
    }

    /// argmin by `ink_gap`; ties break toward the SMALLER gamma -- same
    /// convention as `l3::dashboard::text_gamma_calibration`'s own
    /// `argmin_prefer_smaller_gamma` (assumes `rows` is pre-sorted by
    /// ascending `gamma`, true for both sweep passes below).
    fn argmin_prefer_smaller_gamma(rows: &[SweepRow]) -> f32 {
        rows.iter()
            .min_by(|a, b| a.ink_gap.partial_cmp(&b.ink_gap).expect("ink_gap is never NaN"))
            .expect("rows must be non-empty")
            .gamma
    }

    /// Never runs in normal CI (`#[ignore]`) -- a manual, human-reviewed
    /// tool: prints one row per candidate gamma, the human reads the
    /// table (+ eyeballs the dumped PNGs) before touching
    /// `TEXT_GAMMA_CURVE`.
    #[test]
    #[ignore = "calibration sweep, run manually, prints a table"]
    fn text_gamma_calibration_sweep_real_typography_content() {
        let scene = record_scene();
        let glyph_run_count = print_scene_census(&scene);
        assert!(glyph_run_count > 0, "typography_page must reach the GlyphRun path for this sweep to mean anything");

        let mask = text_affected_mask(&scene);
        let mask_pixel_count = mask.iter().filter(|&&m| m).count();
        println!(
            "text-affected mask: {mask_pixel_count} of {} pixels ({:.3}%)",
            mask.len(),
            mask_pixel_count as f64 / mask.len() as f64 * 100.0,
        );
        assert!(mask_pixel_count > 0, "gamma must move at least one pixel on this fixture, or the sweep has nothing to measure");

        let vello = render_via_vello_cpu(WIDTH, HEIGHT, |ctx| typography_page(ctx, WIDTH as f64, HEIGHT as f64));
        let vello_masked_luma = masked_avg_luma(&vello, &mask);
        println!("vello masked-region luma: {vello_masked_luma:.4}");

        println!("=== typography-page text-gamma calibration: pass 1 (coarse, step 0.1) ===");
        let mut coarse = Vec::new();
        let mut g = 1.00_f32;
        while g <= 2.2001 {
            let row = sweep_row(&scene, &vello, vello_masked_luma, &mask, g);
            print_row(&row, vello_masked_luma);
            coarse.push(row);
            g += 0.1;
        }
        let coarse_best = argmin_prefer_smaller_gamma(&coarse);
        println!("pass 1 minimum: gamma={coarse_best:.2}");

        println!("=== typography-page text-gamma calibration: pass 2 (refine, step 0.02 around {coarse_best:.2}) ===");
        let lo = (coarse_best - 0.1).max(1.0);
        let hi = coarse_best + 0.1;
        let mut fine = Vec::new();
        let mut g = lo;
        while g <= hi + 0.0001 {
            let row = sweep_row(&scene, &vello, vello_masked_luma, &mask, g);
            print_row(&row, vello_masked_luma);
            fine.push(row);
            g += 0.02;
        }
        let fine_best = argmin_prefer_smaller_gamma(&fine);
        println!("pass 2 (final) minimum: gamma={fine_best:.3}");

        if (fine_best - 1.0).abs() < 0.021 {
            println!(
                "STOP: argmin is gamma=1.0 (within tolerance) -- dumping evidence PNGs at gamma=1.0/1.4/1.8, NOT hardcoding \
                 a new TEXT_GAMMA_CURVE value"
            );
            for g in [1.0_f32, 1.4, 1.8] {
                let lut = build_text_gamma_lut(&[1.0, g]);
                let urx = render_via_urx_cpu_with_gamma(&scene, &lut);
                let tag = format!("typography_page_gamma_stop_evidence_g{}", format!("{g:.1}").replace('.', "_"));
                dump_comparison_pngs(&tag, WIDTH, HEIGHT, &urx, &vello);
                println!("dumped {tag}_{{a,b,diff}}.png");
            }
            return;
        }

        println!("winning candidate gamma={fine_best:.3} -- dumping evidence PNGs");
        let winning_lut = build_text_gamma_lut(&[1.0, fine_best]);
        let winning_urx = render_via_urx_cpu_with_gamma(&scene, &winning_lut);
        dump_comparison_pngs("typography_page_gamma_winning_candidate", WIDTH, HEIGHT, &winning_urx, &vello);
        println!(
            "winning candidate PNGs dumped to target/parity-app/typography_page_gamma_winning_candidate_{{a,b,diff}}.png"
        );
    }
}
