//! `uzor-export` — headless render-to-file for the uzor render stack.
//!
//! Renders arbitrary uzor draw code into a PNG at a chosen pixel resolution
//! with **no window and no GPU dependency**: the CPU backend
//! (`uzor-render-tiny-skia`) rasterizes into an in-memory pixmap, which is
//! then PNG-encoded and returned as bytes or written to disk.
//!
//! This is the first brick of the uzor data-viz / document engine space
//! (see `nemo/docs/uzor-viz/uzor_engine_space_map.md`): headless export is
//! how uzor content becomes a report/document delivery artifact, and how a
//! CI job can verify rendering output pixel-for-pixel without a display or
//! GPU (Tier-2 verification tool).
//!
//! # Example
//!
//! ```
//! use uzor_export::{render_to_png, ExportSpec};
//!
//! let spec = ExportSpec {
//!     width_px: 200,
//!     height_px: 100,
//!     dpr: 1.0,
//!     background: Some([255, 255, 255, 255]),
//! };
//! let png_bytes = render_to_png(&spec, |ctx| {
//!     ctx.set_fill_color("#ff0000");
//!     ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
//! })
//! .expect("render");
//! assert!(png_bytes.starts_with(&[0x89, b'P', b'N', b'G']));
//! ```

use std::path::Path;

use uzor::render::RenderContext;
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

/// Parameters for a single headless render pass.
#[derive(Debug, Clone, Copy)]
pub struct ExportSpec {
    /// Physical pixmap width, in pixels.
    pub width_px: u32,
    /// Physical pixmap height, in pixels.
    pub height_px: u32,
    /// Device pixel ratio the draw code sees via [`RenderContext::dpr`].
    ///
    /// The physical pixmap size stays `width_px x height_px` regardless of
    /// this value — `dpr` only changes what draw code reads back from
    /// `ctx.dpr()`, exactly as it would on a real HiDPI display. Draw code
    /// that scales itself by `ctx.dpr()` gets a sharper result at the same
    /// physical pixel count; draw code that ignores `dpr()` is unaffected.
    pub dpr: f64,
    /// Background fill (straight, non-premultiplied RGBA — 0-255 per
    /// channel). `None` leaves the pixmap fully transparent.
    pub background: Option<[u8; 4]>,
}

/// Failure modes for a headless export.
#[derive(Debug)]
pub enum ExportError {
    /// `width_px` and/or `height_px` were zero — nothing to render.
    ZeroSize,
    /// The CPU render backend could not allocate or produce a pixmap for
    /// the requested size.
    Backend(String),
    /// PNG encoding of the finished pixmap failed.
    Encode(String),
    /// Writing the encoded PNG to disk failed.
    Io(std::io::Error),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::ZeroSize => {
                write!(f, "export size must be non-zero (width_px and height_px > 0)")
            }
            ExportError::Backend(msg) => write!(f, "render backend error: {msg}"),
            ExportError::Encode(msg) => write!(f, "PNG encode error: {msg}"),
            ExportError::Io(e) => write!(f, "I/O error writing export: {e}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io(e) => Some(e),
            ExportError::ZeroSize | ExportError::Backend(_) | ExportError::Encode(_) => None,
        }
    }
}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError::Io(e)
    }
}

/// Render `draw` headlessly and return encoded PNG bytes.
///
/// Constructs a CPU (`tiny-skia`) render context of `spec.width_px` x
/// `spec.height_px` pixels, fills the background (or leaves it fully
/// transparent), invokes `draw` exactly once against it, then PNG-encodes
/// the finished pixmap.
pub fn render_to_png(
    spec: &ExportSpec,
    draw: impl FnOnce(&mut dyn RenderContext),
) -> Result<Vec<u8>, ExportError> {
    if spec.width_px == 0 || spec.height_px == 0 {
        return Err(ExportError::ZeroSize);
    }

    // `TinySkiaCpuRenderContext::new` silently falls back to a 1x1 pixmap
    // when the backing `tiny_skia::Pixmap` allocation fails (e.g. absurd
    // dimensions) — validate up front so callers get a real error instead
    // of a silently wrong 1x1 image.
    if tiny_skia::Pixmap::new(spec.width_px, spec.height_px).is_none() {
        return Err(ExportError::Backend(format!(
            "failed to allocate a {}x{} pixmap",
            spec.width_px, spec.height_px
        )));
    }

    let mut ctx = TinySkiaCpuRenderContext::new(spec.width_px, spec.height_px, spec.dpr);

    let bg = match spec.background {
        Some([r, g, b, a]) => tiny_skia::Color::from_rgba8(r, g, b, a),
        None => tiny_skia::Color::TRANSPARENT,
    };
    ctx.clear(bg);

    draw(&mut ctx);

    ctx.pixmap()
        .encode_png()
        .map_err(|e| ExportError::Encode(e.to_string()))
}

/// Same as [`render_to_png`], but writes the encoded PNG straight to `path`.
pub fn render_to_png_file(
    path: &Path,
    spec: &ExportSpec,
    draw: impl FnOnce(&mut dyn RenderContext),
) -> Result<(), ExportError> {
    let bytes = render_to_png(spec, draw)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 200x100 spec, red 50x50 fill_rect at (10,10) on a white background —
    /// PNG magic bytes present, decoded dimensions match the spec exactly.
    #[test]
    fn renders_red_rect_on_white_background() {
        let spec = ExportSpec {
            width_px: 200,
            height_px: 100,
            dpr: 1.0,
            background: Some([255, 255, 255, 255]),
        };
        let bytes = render_to_png(&spec, |ctx| {
            ctx.set_fill_color("#ff0000");
            ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
        })
        .expect("render_to_png should succeed");

        assert!(
            bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            "output should start with the PNG magic bytes"
        );

        let decoder = png::Decoder::new(bytes.as_slice());
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        assert_eq!(info.width, spec.width_px);
        assert_eq!(info.height, spec.height_px);
    }

    /// Zero-size requests are rejected with `ExportError::ZeroSize`, never
    /// silently clamped to a 1x1 image.
    #[test]
    fn zero_size_is_rejected() {
        let spec = ExportSpec {
            width_px: 0,
            height_px: 0,
            dpr: 1.0,
            background: None,
        };
        let err = render_to_png(&spec, |_ctx| {}).expect_err("zero size must error");
        assert!(matches!(err, ExportError::ZeroSize));
    }

    /// `background: None` leaves the pixmap fully transparent — decoded
    /// pixel (0,0) alpha channel must be 0.
    #[test]
    fn transparent_background_has_zero_alpha_pixel() {
        let spec = ExportSpec {
            width_px: 20,
            height_px: 20,
            dpr: 1.0,
            background: None,
        };
        let bytes = render_to_png(&spec, |_ctx| {
            // No draw calls — the whole pixmap should stay transparent.
        })
        .expect("render_to_png should succeed");

        let decoder = png::Decoder::new(bytes.as_slice());
        let mut reader = decoder.read_info().expect("valid PNG header");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        reader.next_frame(&mut buf).expect("decode frame");

        // RGBA8, row-major — pixel (0,0) alpha is byte offset 3.
        assert_eq!(
            buf[3], 0,
            "pixel (0,0) alpha should be 0 for a transparent background"
        );
    }
}
