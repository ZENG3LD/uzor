//! Build-time icon utilities for uzor apps.
//!
//! Convert an SVG asset into PNG (single size) or `.ico` (multi-size Windows icon)
//! during `build.rs`. Designed to be called from a build script:
//!
//! ```no_run
//! // In build.rs:
//! fn main() {
//!     uzor_icon::svg_to_png("assets/logo.svg", "icon-32.png", 32).unwrap();
//!     uzor_icon::svg_to_ico("assets/logo.svg", "icon.ico", &[16, 32, 48, 256]).unwrap();
//! }
//! ```

use std::fs;
use std::io::Cursor;
use std::path::Path;

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use resvg::usvg::{Options, Tree};
use tiny_skia::{Pixmap, Transform};

// ── IconError ─────────────────────────────────────────────────────────────────

/// Error type for icon conversion operations.
#[derive(Debug)]
pub enum IconError {
    /// Underlying I/O failure (read source or write destination).
    Io(std::io::Error),
    /// SVG parse / tree construction failed.
    Svg(String),
    /// Rasterisation failed (e.g. zero-size pixmap).
    Render(String),
    /// Image encoding failed.
    Encode(String),
}

impl std::fmt::Display for IconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconError::Io(e) => write!(f, "icon I/O error: {e}"),
            IconError::Svg(s) => write!(f, "SVG error: {s}"),
            IconError::Render(s) => write!(f, "render error: {s}"),
            IconError::Encode(s) => write!(f, "encode error: {s}"),
        }
    }
}

impl std::error::Error for IconError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IconError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for IconError {
    fn from(e: std::io::Error) -> Self {
        IconError::Io(e)
    }
}

/// Alias for `Result<T, IconError>`.
pub type Result<T> = std::result::Result<T, IconError>;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Parse SVG bytes into a `resvg` tree.
fn parse_tree(svg: &[u8]) -> Result<Tree> {
    Tree::from_data(svg, &Options::default())
        .map_err(|e| IconError::Svg(e.to_string()))
}

/// Render `tree` into an RGBA `Pixmap` of `size × size` pixels.
fn render_to_pixmap(tree: &Tree, size: u32) -> Result<Pixmap> {
    let mut pixmap = Pixmap::new(size, size)
        .ok_or_else(|| IconError::Render(format!("cannot create {size}×{size} pixmap")))?;

    let svg_size = tree.size();
    let sx = size as f32 / svg_size.width();
    let sy = size as f32 / svg_size.height();
    let transform = Transform::from_scale(sx, sy);

    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Encode a `Pixmap`'s RGBA data as PNG bytes.
fn pixmap_to_png_bytes(pixmap: &Pixmap) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let encoder = PngEncoder::new(Cursor::new(&mut out));
    encoder
        .write_image(
            pixmap.data(),
            pixmap.width(),
            pixmap.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|e| IconError::Encode(e.to_string()))?;
    Ok(out)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Render an SVG file to a single-size PNG file.
///
/// # Arguments
///
/// * `svg_path` — path to the source `.svg` file.
/// * `out_path` — path where the `.png` will be written (created or overwritten).
/// * `size` — output image size in pixels (square: `size × size`).
///
/// # Errors
///
/// Returns [`IconError`] on I/O failure, SVG parse error, rasterisation error,
/// or PNG encoding failure.
pub fn svg_to_png(
    svg_path: impl AsRef<Path>,
    out_path: impl AsRef<Path>,
    size: u32,
) -> Result<()> {
    let svg = fs::read(svg_path)?;
    let png = svg_bytes_to_png_bytes(&svg, size)?;
    fs::write(out_path, png)?;
    Ok(())
}

/// Render an SVG file to a multi-size Windows `.ico` file.
///
/// # Arguments
///
/// * `svg_path` — path to the source `.svg` file.
/// * `out_path` — path where the `.ico` will be written (created or overwritten).
/// * `sizes`    — pixel sizes to embed (e.g. `&[16, 32, 48, 256]`).
///
/// # Errors
///
/// Returns [`IconError`] on any failure during read, render, or encode.
pub fn svg_to_ico(
    svg_path: impl AsRef<Path>,
    out_path: impl AsRef<Path>,
    sizes: &[u32],
) -> Result<()> {
    let svg = fs::read(svg_path)?;
    let ico = svg_bytes_to_ico_bytes(&svg, sizes)?;
    fs::write(out_path, ico)?;
    Ok(())
}

/// Convert raw SVG bytes to PNG bytes at the given size.
///
/// In-memory variant of [`svg_to_png`] — no filesystem I/O.
pub fn svg_bytes_to_png_bytes(svg: &[u8], size: u32) -> Result<Vec<u8>> {
    let tree = parse_tree(svg)?;
    let pixmap = render_to_pixmap(&tree, size)?;
    pixmap_to_png_bytes(&pixmap)
}

/// Convert raw SVG bytes to a flat RGBA buffer of `size × size × 4` bytes.
///
/// Useful for passing directly to [`winit::window::Icon::from_rgba`] or
/// [`tray_icon::Icon::from_rgba`].
pub fn svg_bytes_to_rgba(svg: &[u8], size: u32) -> Result<Vec<u8>> {
    let tree = parse_tree(svg)?;
    let pixmap = render_to_pixmap(&tree, size)?;
    Ok(pixmap.data().to_vec())
}

/// Convert raw SVG bytes to a multi-size `.ico` binary blob.
///
/// In-memory variant of [`svg_to_ico`] — no filesystem I/O.
pub fn svg_bytes_to_ico_bytes(svg: &[u8], sizes: &[u32]) -> Result<Vec<u8>> {
    let tree = parse_tree(svg)?;

    // Render each size and collect as PNG-encoded frames.
    let mut png_frames: Vec<Vec<u8>> = Vec::with_capacity(sizes.len());
    for &sz in sizes {
        let pixmap = render_to_pixmap(&tree, sz)?;
        let png = pixmap_to_png_bytes(&pixmap)?;
        png_frames.push(png);
    }

    // Build ICO from PNG frames.
    let frames: std::result::Result<Vec<IcoFrame<'_>>, _> = png_frames
        .iter()
        .zip(sizes.iter())
        .map(|(png, &sz)| IcoFrame::as_png(png, sz, sz, ColorType::Rgba8.into()))
        .collect();
    let frames = frames.map_err(|e| IconError::Encode(e.to_string()))?;

    let mut out = Vec::new();
    let encoder = IcoEncoder::new(Cursor::new(&mut out));
    encoder
        .encode_images(&frames)
        .map_err(|e| IconError::Encode(e.to_string()))?;

    Ok(out)
}
