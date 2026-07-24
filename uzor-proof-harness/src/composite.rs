//! Labelled composite PNG writer.
//!
//! Stitches a [`MultiLegRender`]'s available panels into ONE image, each
//! with its own text header, so a human eyeballs backend divergence in a
//! single file instead of five separate ones. Family-grouped 3-column x
//! 2-row grid (col0 = tiny-skia, col1 = vello family, col2 = urx family;
//! row0 = CPU, row1 = GPU) rather than one wide 5-panel row — this puts
//! each family's CPU/GPU pair in directly adjacent panels, the layout
//! that best serves the NEW within-family divergence axis the GPU legs
//! exist to catch (see `crate::compare::MultiLegDiff`'s own module doc).
//! Reading the grid column-by-column, top-then-bottom, reproduces the
//! exact tiny-skia / vello-cpu / vello-gpu / urx-cpu / urx-gpu order.
//! `tiny-skia` has no GPU sibling — its own bottom cell, and any skipped
//! GPU leg's cell, is left as plain background: no placeholder, no
//! header, matching [`MultiLegRender::legs`]'s own "only what actually
//! rendered" convention. The panel header itself is rendered through
//! `tiny-skia` (already this crate's own dependency, no extra font/text
//! library needed) — a fixed, deterministic label strip, not part of the
//! compared scene content.

use std::io;
use std::path::Path;

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::render::MultiLegRender;

/// Height, in pixels, of the label strip painted above each panel.
const HEADER_HEIGHT: u32 = 28;
/// Gap, in pixels, between adjacent panels (both horizontal, between
/// columns, and vertical, between the CPU/GPU rows).
const PANEL_GAP: u32 = 8;
/// Grid shape: tiny-skia | vello family | urx family, CPU row over GPU
/// row.
const GRID_COLS: u32 = 3;
const GRID_ROWS: u32 = 2;
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

/// Write `render`'s available panels, each with its own text header,
/// into one family-grouped composite PNG at `path` — see this module's
/// own doc comment for the exact grid layout.
pub fn write_composite_png(render: &MultiLegRender, path: &Path) -> Result<(), CompositeError> {
    let panel_w = render.width;
    let panel_h = render.height;
    let total_w = panel_w * GRID_COLS + PANEL_GAP * (GRID_COLS - 1);
    let row_h = HEADER_HEIGHT + panel_h;
    let total_h = row_h * GRID_ROWS + PANEL_GAP * (GRID_ROWS - 1);

    // `grid[col][row]` — `None` cells (tiny-skia's own missing GPU
    // sibling, or an unavailable GPU leg) stay plain background: no
    // placeholder, no header.
    let grid: [[Option<(&str, &[u8])>; 2]; 3] = [
        [Some(("tiny-skia", render.tiny_skia.as_slice())), None],
        [Some(("vello-cpu", render.vello_cpu.as_slice())), render.vello_gpu.as_deref().map(|px| ("vello-gpu", px))],
        [Some(("urx-cpu", render.urx_cpu.as_slice())), render.urx_gpu.as_ref().map(|r| ("urx-gpu", r.pixels.as_slice()))],
    ];

    let mut composite = vec![0u8; (total_w * total_h * 4) as usize];
    // Fill the gutters/background first (`COMPOSITE_BG`, straight RGB —
    // written directly since the whole canvas is opaque, no premultiply
    // step needed for a flat fill).
    let bg = parse_hex_rgb(COMPOSITE_BG);
    for px in composite.chunks_exact_mut(4) {
        px.copy_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }

    for (col_idx, col) in grid.iter().enumerate() {
        let panel_x = col_idx as u32 * (panel_w + PANEL_GAP);
        for (row_idx, cell) in col.iter().enumerate() {
            let Some((label, pixels)) = cell else { continue };
            let panel_y = row_idx as u32 * (row_h + PANEL_GAP);
            let header = render_header_label(panel_w, label);
            blit(&mut composite, total_w, panel_x, panel_y, &header, panel_w, HEADER_HEIGHT);
            blit(&mut composite, total_w, panel_x, panel_y + HEADER_HEIGHT, pixels, panel_w, panel_h);
        }
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
    use crate::render::MultiLegRender;

    /// Composite geometry is FIXED (`GRID_COLS x GRID_ROWS`) regardless
    /// of GPU availability — a skipped GPU leg leaves its own cell
    /// blank, it never shrinks the canvas. Same test, same assertion
    /// shape as the pre-GPU three-panel version; only the expected
    /// dimensions grew from `3 panels x 1 row` to `3 panels x 2 rows`.
    #[test]
    fn write_composite_png_produces_a_correctly_sized_valid_png() {
        let render = MultiLegRender::capture(6, 6, |ctx| {
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
        assert_eq!(info.width, 6 * GRID_COLS + PANEL_GAP * (GRID_COLS - 1));
        assert_eq!(info.height, (HEADER_HEIGHT + 6) * GRID_ROWS + PANEL_GAP * (GRID_ROWS - 1));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_hex_rgb_parses_the_composite_background_constant() {
        assert_eq!(parse_hex_rgb(COMPOSITE_BG), [0x14, 0x16, 0x1c]);
    }
}
