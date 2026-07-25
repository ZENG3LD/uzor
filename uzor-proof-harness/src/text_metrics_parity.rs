//! Cross-backend `TextMetrics::measure_text` parity — the direct
//! regression floor for the "text measurement must not vary by render
//! backend" fix (owner investigation, 2026-07-25).
//!
//! Before that fix, three independent numeric families measured the
//! SAME string at the SAME font/size:
//! - `uzor-render-tiny-skia` — fontdue's own raw per-glyph
//!   `advance_width` sum (no GPOS kerning).
//! - `uzor-render-vello-{cpu,gpu,hybrid}` /
//!   `uzor-render-wgpu-instanced` — skrifa's own raw glyph-metrics
//!   advance sum (no GPOS kerning either — numerically IDENTICAL to
//!   fontdue's family for the SAME font file, since neither reads GPOS
//!   at all).
//! - `uzor-render-urx` / `uzor-export::PdfRenderContext` /
//!   `uzor-render-svg` — real shaped [`uzor::shaper`] (cosmic-text)
//!   advances, WITH GPOS kerning.
//!
//! Measured divergence (kern-heavy fixture, 18px Roboto): up to 1.54%
//! (`"AVWX"`, 0.765px) — sub-pixel for ordinary text, whole-pixel-plus
//! for kerning-dense strings, ALWAYS in the same direction (kerned
//! narrower than un-kerned). `uzor-figures::guide::labeler::place_labels`
//! feeds a backend's own `measure_text` result directly into a pass/fail
//! occupancy check, so this divergence could flip WHICH candidate slot a
//! label claims between two backends rendering the identical scene.
//!
//! Every backend's `TextMetrics::measure_text`/`text_bounds` now
//! delegates to [`uzor::shaper::measure_glyphs`] (the SAME function call,
//! not just the same algorithm) — this test proves the three backends
//! constructible without a live GPU device (tiny-skia, vello-cpu,
//! urx-cpu's own recording context) agree EXACTLY, not just within a
//! tolerance. `vello-gpu`/`vello-hybrid`/`wgpu-instanced`/`urx-gpu` all
//! share this same code path (see each crate's own `context.rs`), so
//! this 3-leg check is representative of all 8 render/export backends in
//! this workspace, not just the 3 tested directly here.

use uzor::render::{TextMetrics, TextRenderer};

/// A representative label set at the sizes `uzor-figures` actually uses
/// (11/13/18px axis/legend/timeline labels) — includes the kern-heavy
/// fixture (`"AVWX"`) that produced the LARGEST pre-fix divergence
/// (1.54%, 0.765px at 18px), a full sentence, digits, and punctuation —
/// covering every character class the pre-fix divergence measurement
/// exercised.
const SAMPLES: &[&str] = &["12", "Jan 24", "1,234.5", "Volume", "The quick brown fox", "AVWX", "gpqjy"];
const SIZES: &[f64] = &[11.0, 13.0, 18.0];

#[test]
fn measure_text_agrees_exactly_across_tiny_skia_vello_cpu_and_urx() {
    for &size in SIZES {
        for &text in SAMPLES {
            let font = format!("{size}px Roboto");

            let mut tiny_skia = uzor_render_tiny_skia::TinySkiaCpuRenderContext::new(200, 100, 1.0);
            tiny_skia.set_font(&font);
            let tiny_skia_w = tiny_skia.measure_text(text);

            let mut vello_cpu = uzor_render_vello_cpu::VelloCpuRenderContext::new(1.0);
            vello_cpu.begin_frame(200, 100);
            vello_cpu.set_font(&font);
            let vello_cpu_w = vello_cpu.measure_text(text);

            let mut urx = uzor_render_urx::UrxRenderContext::new(1.0);
            urx.set_font(&font);
            let urx_w = urx.measure_text(text);

            assert_eq!(
                tiny_skia_w.to_bits(),
                urx_w.to_bits(),
                "tiny-skia vs urx measure_text diverged for {text:?} @ {size}px: {tiny_skia_w} != {urx_w}"
            );
            assert_eq!(
                vello_cpu_w.to_bits(),
                urx_w.to_bits(),
                "vello-cpu vs urx measure_text diverged for {text:?} @ {size}px: {vello_cpu_w} != {urx_w}"
            );
        }
    }
}

/// Same parity claim for [`uzor::render::TextMetrics::text_bounds`]'s
/// own `w` field (the OTHER width-reporting entry point every backend
/// exposes — `uzor-figures::guide::axis::measure_y_axis_gutter` and
/// several others measure through `text_bounds`, not `measure_text`).
#[test]
fn text_bounds_width_agrees_exactly_across_tiny_skia_vello_cpu_and_urx() {
    for &size in SIZES {
        for &text in SAMPLES {
            let font = format!("{size}px Roboto");

            let tiny_skia = uzor_render_tiny_skia::TinySkiaCpuRenderContext::new(200, 100, 1.0);
            let tiny_skia_w = uzor::render::TextMetrics::text_bounds(&tiny_skia, text, &font).w;

            let mut vello_cpu = uzor_render_vello_cpu::VelloCpuRenderContext::new(1.0);
            vello_cpu.begin_frame(200, 100);
            let vello_cpu_w = uzor::render::TextMetrics::text_bounds(&vello_cpu, text, &font).w;

            let urx = uzor_render_urx::UrxRenderContext::new(1.0);
            let urx_w = uzor::render::TextMetrics::text_bounds(&urx, text, &font).w;

            assert_eq!(
                tiny_skia_w.to_bits(),
                urx_w.to_bits(),
                "tiny-skia vs urx text_bounds.w diverged for {text:?} @ {size}px: {tiny_skia_w} != {urx_w}"
            );
            assert_eq!(
                vello_cpu_w.to_bits(),
                urx_w.to_bits(),
                "vello-cpu vs urx text_bounds.w diverged for {text:?} @ {size}px: {vello_cpu_w} != {urx_w}"
            );
        }
    }
}
