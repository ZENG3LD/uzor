//! Labelled side-by-side composite PNG writer.
//!
//! Stitches a [`ThreeLegRender`]'s three panels into ONE image — tiny-skia
//! | vello-cpu | urx-cpu, left to right, each with its own text header —
//! so a human eyeballs backend divergence in a single file instead of
//! three separate ones. The panel header itself is rendered through
//! `tiny-skia` (already this crate's own dependency, no extra font/text
//! library needed) — a fixed, deterministic label strip, not part of the
//! compared scene content.

use std::io;
use std::path::Path;

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::render::ThreeLegRender;

/// Height, in pixels, of the label strip painted above each panel.
const HEADER_HEIGHT: u32 = 28;
/// Horizontal gap, in pixels, between adjacent panels.
const PANEL_GAP: u32 = 8;
/// Composite background (also the header strip's own fill) — a plain
/// dark neutral, deliberately distinct from any theme this crate's
/// callers render with, so the panel boundary/gutter is unambiguous.
const COMPOSITE_BG: &str = "#14161c";
const HEADER_LABEL_COLOR: &str = "#e8e8ee";

/// Failure modes for [`write_composite_png`].
#[derive(Debug)]
pub enum CompositeError {
    Io(io::Error),
    Encode(String),
}

impl std::fmt::Display for CompositeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompositeError::Io(e) => write!(f, "I/O error writing composite PNG: {e}"),
            CompositeError::Encode(msg) => write!(f, "PNG encode error: {msg}"),
        }
    }
}

impl std::error::Error for CompositeError {}

impl From<io::Error> for CompositeError {
    fn from(e: io::Error) -> Self {
        CompositeError::Io(e)
    }
}

/// Render a `panel_w x HEADER_HEIGHT` label strip (dark background,
/// centered bold text) through `tiny-skia` — premultiplied RGBA8, same
/// pixel contract as every render leg.
fn render_header_label(panel_w: u32, label: &str) -> Vec<u8> {
    let mut ctx = uzor_render_tiny_skia::TinySkiaCpuRenderContext::new(panel_w, HEADER_HEIGHT, 1.0);
    let ctx_dyn: &mut dyn RenderContext = &mut ctx;
    ctx_dyn.set_fill_color(COMPOSITE_BG);
    ctx_dyn.fill_rect(0.0, 0.0, panel_w as f64, HEADER_HEIGHT as f64);
    ctx_dyn.set_fill_color(HEADER_LABEL_COLOR);
    ctx_dyn.set_font("bold 15px sans-serif");
    ctx_dyn.set_text_align(TextAlign::Center);
    ctx_dyn.set_text_baseline(TextBaseline::Middle);
    ctx_dyn.fill_text(label, panel_w as f64 / 2.0, HEADER_HEIGHT as f64 / 2.0);
    ctx.pixels().to_vec()
}

/// Copy a `src_w x src_h` premultiplied-RGBA8 block into `dst` (a
/// `dst_w`-wide canvas) at `(dst_x, dst_y)`.
fn blit(dst: &mut [u8], dst_w: u32, dst_x: u32, dst_y: u32, src: &[u8], src_w: u32, src_h: u32) {
    for row in 0..src_h {
        let src_start = (row * src_w * 4) as usize;
        let src_end = src_start + (src_w * 4) as usize;
        let dst_row_start = (((dst_y + row) * dst_w + dst_x) * 4) as usize;
        let dst_row_end = dst_row_start + (src_w * 4) as usize;
        dst[dst_row_start..dst_row_end].copy_from_slice(&src[src_start..src_end]);
    }
}

/// Un-premultiply a whole RGBA8 buffer in place — display-only
/// convenience (same conversion `uzor-examples/src/parity_harness.rs`'s
/// own `dump_png` uses) so a human eyeballing the composite doesn't see
/// it artificially darkened; no comparator in this crate ever touches
/// the un-premultiplied copy.
fn unpremultiply(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 || a == 255 {
            continue;
        }
        let unmul = |c: u8| -> u8 { ((c as u32 * 255 + (a as u32) / 2) / (a as u32)).min(255) as u8 };
        px[0] = unmul(px[0]);
        px[1] = unmul(px[1]);
        px[2] = unmul(px[2]);
    }
}

/// Write `render`'s three panels (tiny-skia | vello-cpu | urx-cpu),
/// each with its own text header, side by side into one PNG at `path`.
pub fn write_composite_png(render: &ThreeLegRender, path: &Path) -> Result<(), CompositeError> {
    let panel_w = render.width;
    let panel_h = render.height;
    let total_w = panel_w * 3 + PANEL_GAP * 2;
    let total_h = HEADER_HEIGHT + panel_h;

    let mut composite = vec![0u8; (total_w * total_h * 4) as usize];
    // Fill the gutters/background first (`COMPOSITE_BG`, straight RGB —
    // written directly since the whole canvas is opaque, no premultiply
    // step needed for a flat fill).
    let bg = parse_hex_rgb(COMPOSITE_BG);
    for px in composite.chunks_exact_mut(4) {
        px.copy_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }

    for (i, (label, pixels)) in render.legs().into_iter().enumerate() {
        let panel_x = i as u32 * (panel_w + PANEL_GAP);
        let header = render_header_label(panel_w, label);
        blit(&mut composite, total_w, panel_x, 0, &header, panel_w, HEADER_HEIGHT);
        blit(&mut composite, total_w, panel_x, HEADER_HEIGHT, pixels, panel_w, panel_h);
    }

    unpremultiply(&mut composite);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(io::BufWriter::new(file), total_w, total_h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| CompositeError::Encode(e.to_string()))?;
    writer.write_image_data(&composite).map_err(|e| CompositeError::Encode(e.to_string()))?;
    Ok(())
}

/// Parse a `"#rrggbb"` literal into `[r, g, b]` — this module's own
/// constants are always well-formed, so a malformed literal (never
/// reachable via this crate's public API) falls back to black rather
/// than panicking.
fn parse_hex_rgb(hex: &str) -> [u8; 3] {
    let h = hex.trim_start_matches('#');
    let byte = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    if h.len() >= 6 {
        [byte(&h[0..2]), byte(&h[2..4]), byte(&h[4..6])]
    } else {
        [0, 0, 0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ThreeLegRender;

    #[test]
    fn write_composite_png_produces_a_correctly_sized_valid_png() {
        let render = ThreeLegRender::capture(6, 6, |ctx| {
            ctx.set_fill_color("#3366ff");
            ctx.fill_rect(0.0, 0.0, 6.0, 6.0);
        });
        let dir = std::env::temp_dir().join("uzor-proof-harness-tests");
        let path = dir.join("composite_smoke.png");
        write_composite_png(&render, &path).expect("composite PNG should write");

        let bytes = std::fs::read(&path).expect("composite PNG should be readable back");
        let decoder = png::Decoder::new(bytes.as_slice());
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        assert_eq!(info.width, 6 * 3 + PANEL_GAP * 2);
        assert_eq!(info.height, HEADER_HEIGHT + 6);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_hex_rgb_parses_the_composite_background_constant() {
        assert_eq!(parse_hex_rgb(COMPOSITE_BG), [0x14, 0x16, 0x1c]);
    }
}
