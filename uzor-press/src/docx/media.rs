//! `word/media/*.png` bookkeeping + the two rasterization paths DOCX needs
//! (a `:::diagram` figure, and a theme stroke-mark logo sentinel) — every
//! other embedded image goes through [`crate::press::Press::
//! read_image_bytes`] directly (the ORIGINAL PNG file bytes, no re-encode).

use std::path::Path;

use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec};
use uzor_figures::FigureTheme;
use uzor_typeset::TypesetFigure;

use crate::figures::StrokeMarkFigure;
use crate::parse::DiagramBox;
use crate::press::{Press, PressError};

use super::error::DocxError;

/// Word content gets zoomed/printed more aggressively than a screen PDF
/// proof — a dedicated, higher raster scale than the PDF proof PNG's own
/// `DPR = 2.0` (`press.rs`).
pub(crate) const DOCX_FIGURE_RASTER_SCALE: f64 = 3.0;

/// The logical (pt) square size a stroke-mark logo is rasterized at — large
/// enough to stay crisp after Word's own contain-fit scales it back down
/// to the cover/footer's much smaller drawn size.
const MARK_RASTER_SIZE_PT: f64 = 160.0;

/// Every asset this crate ever registers is PNG (`[Content_Types].xml`'s
/// own `Default Extension="png"` rule covers all of them crate-wide — no
/// per-asset content-type field is read anywhere).
#[derive(Clone)]
pub(crate) struct MediaAsset {
    pub rel_id: String,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub(crate) struct MediaRegistry {
    assets: Vec<MediaAsset>,
    next_index: u32,
    /// `word/media/{prefix}N.png` — document-body images use `"image"`,
    /// the footer's own registry uses `"footer-image"` so the two id
    /// sequences (each starting at 1, one per `.rels` part) never collide
    /// on the SAME `word/media/` file name (OOXML has no cross-part dedup,
    /// so a logo used on both the cover and the footer is embedded twice
    /// under two distinct file names).
    prefix: &'static str,
}

impl MediaRegistry {
    pub(crate) fn new() -> Self {
        Self::with_prefix("image")
    }

    pub(crate) fn with_prefix(prefix: &'static str) -> Self {
        Self { assets: Vec::new(), next_index: 1, prefix }
    }

    /// Registers `bytes` as the next `word/media/{prefix}N.png`, returns
    /// its own relationship id (`"rId1"`, `"rId2"`, ...) — scoped to
    /// whichever `.rels` part this registry's own owner writes it into.
    pub(crate) fn register_png(&mut self, bytes: Vec<u8>) -> String {
        let index = self.next_index;
        self.next_index += 1;
        let rel_id = format!("rId{index}");
        let prefix = self.prefix;
        self.assets.push(MediaAsset { rel_id: rel_id.clone(), file_name: format!("{prefix}{index}.png"), bytes });
        rel_id
    }

    pub(crate) fn assets(&self) -> &[MediaAsset] {
        &self.assets
    }
}

/// Rasterizes `rows` via `figures::Diagram` at its own natural (width,
/// height) — the SAME figure/height formula `press::Press::diagram_figure`
/// shares with the PDF path's `push_diagram` — and returns PNG bytes plus
/// the LOGICAL (pt) width/height for `<wp:extent>` sizing. Transparent
/// background (`ExportSpec.background: None`) matches `Diagram::render`'s
/// own behavior of never painting a background rect.
pub(crate) fn diagram_png(press: &Press, rows: &[Vec<DiagramBox>]) -> Result<(Vec<u8>, f64, f64), DocxError> {
    let (figure, height_pt) = press.diagram_figure(rows);
    let width_pt = press.body_width();
    let bytes = rasterize_figure(&figure, width_pt, height_pt, None)?;
    Ok((bytes, width_pt, height_pt))
}

/// Resolve a front-matter `logo:` value to embeddable PNG bytes + pixel
/// dimensions. A file name equal to the theme's logo sentinel has no PNG —
/// DOCX rasterizes the SAME stroke mark the PDF path paints directly,
/// reusing its `TypesetFigure` impl exactly like `diagram_png` does for
/// diagrams. The wordmark is not part of the raster. Every other value
/// reads the ORIGINAL PNG file bytes via [`Press::read_image_bytes`]
/// (no RGBA re-encode).
pub(crate) fn resolve_logo_png(press: &Press, raw: &Path) -> Result<(Vec<u8>, u32, u32), DocxError> {
    if press.names_logo_sentinel(raw) {
        let mark = press.palette().logo_mark.clone().ok_or_else(|| DocxError::Image {
            path: raw.to_path_buf(),
            source: PressError::Image { path: raw.to_path_buf(), message: "theme logo sentinel has no mark".to_owned() },
        })?;
        let logo = StrokeMarkFigure {
            viewbox: mark.viewbox,
            stroke_width: mark.stroke_width,
            polylines: mark.polylines,
            color_hex: crate::press::Palette::hex(press.palette().accent),
        };
        let size_px = (MARK_RASTER_SIZE_PT * DOCX_FIGURE_RASTER_SCALE).round().max(1.0) as u32;
        let bytes = rasterize_figure(&logo, MARK_RASTER_SIZE_PT, MARK_RASTER_SIZE_PT, None)?;
        return Ok((bytes, size_px, size_px));
    }
    let bytes = press.read_image_bytes(raw).map_err(|source| DocxError::Image { path: raw.to_path_buf(), source })?;
    let (w, h) = press.image_dimensions(raw).map_err(|source| DocxError::Image { path: raw.to_path_buf(), source })?;
    Ok((bytes, w, h))
}

fn rasterize_figure(figure: &dyn TypesetFigure, width_pt: f64, height_pt: f64, background: Option<[u8; 4]>) -> Result<Vec<u8>, DocxError> {
    let width_px = (width_pt * DOCX_FIGURE_RASTER_SCALE).round().max(1.0) as u32;
    let height_px = (height_pt * DOCX_FIGURE_RASTER_SCALE).round().max(1.0) as u32;
    let spec = ExportSpec { width_px, height_px, dpr: DOCX_FIGURE_RASTER_SCALE, background };
    // `Diagram::render`/`StrokeMarkFigure::render` both ignore their own
    // `_theme: &FigureTheme` parameter (neither reads any field of it) —
    // any placeholder value is correct here, so a bare `FigureTheme::dark()`
    // avoids threading a real `Theme` through the rasterization path.
    let figure_theme = FigureTheme::dark();
    render_to_png(&spec, |ctx| {
        ctx.scale(DOCX_FIGURE_RASTER_SCALE, DOCX_FIGURE_RASTER_SCALE);
        figure.render(ctx, Rect::new(0.0, 0.0, width_pt, height_pt), &figure_theme);
    })
    .map_err(|e| DocxError::Raster(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_png_returns_strictly_increasing_distinct_ids_and_file_names() {
        let mut reg = MediaRegistry::new();
        let a = reg.register_png(vec![1, 2, 3]);
        let b = reg.register_png(vec![4, 5, 6]);
        assert_ne!(a, b);
        assert_eq!(a, "rId1");
        assert_eq!(b, "rId2");
        let assets = reg.assets();
        assert_eq!(assets[0].file_name, "image1.png");
        assert_eq!(assets[1].file_name, "image2.png");
        assert_ne!(assets[0].file_name, assets[1].file_name);
    }

    #[test]
    fn diagram_png_returns_the_same_height_formula_press_diagram_figure_computes() {
        let press = Press::new(crate::press::Format::Doc, crate::press::Palette::light(), crate::preset::Preset::column(), std::path::PathBuf::new());
        let rows = vec![vec![DiagramBox { title: "A".to_owned(), subtitle: None }], vec![DiagramBox { title: "B".to_owned(), subtitle: None }]];
        let (_figure, expected_height) = press.diagram_figure(&rows);
        let (bytes, width_pt, height_pt) = diagram_png(&press, &rows).expect("rendering a 2-row diagram to PNG must succeed");
        assert!(!bytes.is_empty());
        assert_eq!(width_pt, press.body_width());
        assert_eq!(height_pt, expected_height);
    }
}
