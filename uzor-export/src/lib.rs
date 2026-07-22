//! `uzor-export` — headless render-to-file for the uzor render stack.
//!
//! Renders arbitrary uzor draw code into a PNG at a chosen pixel resolution
//! with **no window and no GPU dependency**: the CPU backend
//! (`uzor-render-tiny-skia`) rasterizes into an in-memory pixmap, which is
//! then PNG-encoded and returned as bytes or written to disk.
//!
//! This is the first brick of the uzor data-viz / document engine space
//! (see `nemo/docs/uzor-engines/uzor_engine_space_map.md`): headless export is
//! how uzor content becomes a report/document delivery artifact, and how a
//! CI job can verify rendering output pixel-for-pixel without a display or
//! GPU (Tier-2 verification tool).
//!
//! [`render_to_svg`]/[`render_to_svg_file`] are the SVG sibling of
//! [`render_to_png`]/[`render_to_png_file`] — same `ExportSpec` contract
//! (background rect painted first, `dpr` surfaced via `ctx.dpr()`), but
//! serializing through `uzor-render-svg::SvgRenderContext` into a
//! standalone, portable SVG document string instead of rasterizing into a
//! `tiny-skia` pixmap. See `nemo/docs/uzor-engines/research_export_sota_2026.md`
//! §6 for the SVG backend's own design rationale (outlined text,
//! transform-baked coordinates, base64-embedded raster images).
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
use uzor_render_svg::SvgRenderContext;
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

pub mod pdf;

pub use pdf::{
    FontId, PdfBuilder, PdfContentStream, PdfDate, PdfFont, PdfFontCache, PdfLink, PdfMeta, PdfOutlineEntry, PdfPageSpec, PdfRenderContext, PdfTagRole,
    PdfTextRun, StructElemId,
};

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
    /// (P5, `pdf` module) A [`PdfPageSpec::raster`]'s encoded PNG bytes
    /// could not be decoded.
    RasterDecode(String),
    /// (P5, `pdf` module) A [`PdfPageSpec::raster_px`] didn't match the
    /// decoded PNG's own header dimensions — never silently trusting one
    /// over the other.
    RasterDimensionMismatch {
        expected: (u32, u32),
        actual: (u32, u32),
    },
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
            ExportError::RasterDecode(msg) => write!(f, "PDF raster background decode error: {msg}"),
            ExportError::RasterDimensionMismatch { expected, actual } => write!(
                f,
                "PDF raster background dimension mismatch: spec declared {}x{}, decoded PNG is {}x{}",
                expected.0, expected.1, actual.0, actual.1
            ),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io(e) => Some(e),
            ExportError::ZeroSize
            | ExportError::Backend(_)
            | ExportError::Encode(_)
            | ExportError::RasterDecode(_)
            | ExportError::RasterDimensionMismatch { .. } => None,
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

/// Render `draw` headlessly and return a complete, standalone SVG document
/// string.
///
/// Same contract shape as [`render_to_png`]: constructs an
/// `SvgRenderContext` of `spec.width_px` x `spec.height_px` SVG user units,
/// paints the background rect FIRST when `spec.background` is `Some` (so
/// subsequent draw calls composite over it — SVG itself has no implicit
/// backdrop, unlike a PNG's opaque/transparent pixmap clear), invokes `draw`
/// exactly once against it, then serializes the accumulated draw calls into
/// the finished document via `SvgRenderContext::finish`. `spec.dpr` reaches
/// draw code via `ctx.dpr()` exactly as it would on a real HiDPI display —
/// it is not otherwise baked into the document's own coordinate space.
pub fn render_to_svg(
    spec: &ExportSpec,
    draw: impl FnOnce(&mut dyn RenderContext),
) -> Result<String, ExportError> {
    if spec.width_px == 0 || spec.height_px == 0 {
        return Err(ExportError::ZeroSize);
    }

    let mut ctx = SvgRenderContext::new(spec.width_px, spec.height_px, spec.dpr);

    if let Some([r, g, b, a]) = spec.background {
        let ctx_dyn: &mut dyn RenderContext = &mut ctx;
        ctx_dyn.set_fill_color(&format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
        ctx_dyn.fill_rect(0.0, 0.0, spec.width_px as f64, spec.height_px as f64);
    }

    draw(&mut ctx);

    Ok(ctx.finish())
}

/// Same as [`render_to_svg`], but writes the SVG document straight to `path`.
pub fn render_to_svg_file(
    path: &Path,
    spec: &ExportSpec,
    draw: impl FnOnce(&mut dyn RenderContext),
) -> Result<(), ExportError> {
    let svg = render_to_svg(spec, draw)?;
    std::fs::write(path, svg)?;
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

    /// SVG sibling of `renders_red_rect_on_white_background` — same fixture,
    /// `render_to_svg` instead of `render_to_png`.
    #[test]
    fn renders_red_rect_on_white_background_svg() {
        let spec = ExportSpec {
            width_px: 200,
            height_px: 100,
            dpr: 1.0,
            background: Some([255, 255, 255, 255]),
        };
        let svg = render_to_svg(&spec, |ctx| {
            ctx.set_fill_color("#ff0000");
            ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
        })
        .expect("render_to_svg should succeed");

        assert!(svg.starts_with("<svg"), "output should start with the <svg root element");
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"100\""));
        assert!(svg.contains("fill=\"#ff0000\""));
    }

    /// Zero-size requests are rejected the same way for the SVG path.
    #[test]
    fn svg_zero_size_is_rejected() {
        let spec = ExportSpec {
            width_px: 0,
            height_px: 0,
            dpr: 1.0,
            background: None,
        };
        let err = render_to_svg(&spec, |_ctx| {}).expect_err("zero size must error");
        assert!(matches!(err, ExportError::ZeroSize));
    }

    /// `background: Some(..)` paints a full-canvas background rect BEFORE
    /// `draw` runs — same "background first" contract `render_to_png` has.
    #[test]
    fn svg_background_paints_a_rect_before_the_draw_closure() {
        let spec = ExportSpec {
            width_px: 20,
            height_px: 20,
            dpr: 1.0,
            background: Some([10, 20, 30, 255]),
        };
        let svg = render_to_svg(&spec, |_ctx| {}).expect("render_to_svg should succeed");
        assert!(
            svg.contains("fill=\"#0a141e\""),
            "expected the background color as a fill, got: {svg}"
        );
    }
}
