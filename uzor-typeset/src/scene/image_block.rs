//! [`ImageBlock`]/[`ImageFit`] — the image-block scene type (design doc
//! §2.1/§2.2). Like a figure, an image reports no *layout* intrinsic
//! size of its own choosing — [`BlockSizing`] is the same required,
//! explicit field `FigureBlock` uses (§2.2's asymmetry note applies
//! identically here). Unlike a figure, an image DOES carry its own
//! `intrinsic_width`/`intrinsic_height` (the source pixel dimensions) —
//! [`ImageFit`] decides how that intrinsic aspect reconciles with the
//! block's own resolved (width, height) rect.
//!
//! Paint seam: design law 4 says images paint through
//! `ImagePainter::draw_image_rgba`. [`crate::render::draw_page`]'s own
//! module docs record why that seam is NOT reachable from this crate's
//! current headless proof path (`ImagePainter` is opt-in, not part of the
//! `RenderContext` supertrait, and the `uzor-render-tiny-skia` CPU backend
//! `uzor-export::render_to_png` uses declares `draw_image_rgba` a
//! documented no-op) — this module only owns the scene TYPE + the pure
//! fit-rect geometry, never a backend workaround.

use crate::scene::figure_block::BlockSizing;

/// How an image's own `(intrinsic_width, intrinsic_height)` aspect
/// reconciles with the rect [`BlockSizing`] resolves for it — CSS
/// `object-fit`'s vocabulary lifted (matches `BreakControl`'s own
/// "lift the primitive vocabulary, not the spec" convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFit {
    /// Fill the resolved rect exactly, ignoring the intrinsic aspect
    /// (may distort the image).
    Stretch,
    /// Scale to fit entirely within the resolved rect, preserving aspect
    /// — centered, may letterbox.
    Contain,
    /// Scale to fully cover the resolved rect, preserving aspect — may
    /// crop.
    Cover,
}

/// A flow-participating raster image.
pub struct ImageBlock<'a> {
    /// RGBA pixel data, 4 bytes/pixel, row-major, top-to-bottom —
    /// `ImagePainter::draw_image_rgba`'s own expected layout.
    pub rgba: &'a [u8],
    pub intrinsic_width: u32,
    pub intrinsic_height: u32,
    pub sizing: BlockSizing,
    pub fit: ImageFit,
}

impl<'a> ImageBlock<'a> {
    pub fn new(rgba: &'a [u8], intrinsic_width: u32, intrinsic_height: u32, sizing: BlockSizing, fit: ImageFit) -> Self {
        Self { rgba, intrinsic_width, intrinsic_height, sizing, fit }
    }

    /// This image's own intrinsic aspect ratio (`width / height`), or
    /// `None` for a zero-height source (nothing to preserve).
    pub fn intrinsic_aspect(&self) -> Option<f64> {
        if self.intrinsic_height == 0 {
            None
        } else {
            Some(self.intrinsic_width as f64 / self.intrinsic_height as f64)
        }
    }

    /// The content sub-rect within `placed_width` x `placed_height` this
    /// image's pixels should paint into, per [`ImageFit`] — pure
    /// geometry, independent of whether a backend can actually composite
    /// the pixels (see this module's own doc comment). `(x, y, w, h)`,
    /// origin-relative to the block's own placed rect (never absolute —
    /// caller adds its own rect origin, same convention every other
    /// placed geometry in this crate uses).
    pub fn content_rect(&self, placed_width: f64, placed_height: f64) -> (f64, f64, f64, f64) {
        let Some(aspect) = self.intrinsic_aspect() else {
            return (0.0, 0.0, placed_width, placed_height);
        };
        match self.fit {
            ImageFit::Stretch => (0.0, 0.0, placed_width, placed_height),
            ImageFit::Contain => fit_rect(placed_width, placed_height, aspect, false),
            ImageFit::Cover => fit_rect(placed_width, placed_height, aspect, true),
        }
    }
}

/// Shared `Contain`/`Cover` math: pick the scale that either fits entirely
/// inside (`cover == false`) or fully covers (`cover == true`) the
/// `(box_w, box_h)` box while preserving `aspect`, then center the result.
fn fit_rect(box_w: f64, box_h: f64, aspect: f64, cover: bool) -> (f64, f64, f64, f64) {
    let box_aspect = if box_h > 0.0 { box_w / box_h } else { aspect };
    // The image is proportionally "wider" than the box when its own
    // aspect exceeds the box's.
    let wider_than_box = aspect > box_aspect;
    // `Contain`: scaling to the box's width is what stays fully inside
    // when the image is wider than the box (the resulting height then
    // undershoots the box height); `Cover` is exactly the opposite —
    // scaling to width is what OVERFLOWS (and therefore covers) in that
    // same case.
    let fit_to_width = if cover { !wider_than_box } else { wider_than_box };

    let (w, h) = if fit_to_width { (box_w, box_w / aspect) } else { (box_h * aspect, box_h) };

    (((box_w - w) / 2.0), ((box_h - h) / 2.0), w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_aspect_is_width_over_height() {
        let rgba = [0u8; 16];
        let image = ImageBlock::new(&rgba, 200, 100, BlockSizing::FillRegion, ImageFit::Stretch);
        assert_eq!(image.intrinsic_aspect(), Some(2.0));
    }

    #[test]
    fn intrinsic_aspect_is_none_for_zero_height() {
        let rgba = [0u8; 16];
        let image = ImageBlock::new(&rgba, 200, 0, BlockSizing::FillRegion, ImageFit::Stretch);
        assert_eq!(image.intrinsic_aspect(), None);
    }

    #[test]
    fn stretch_fills_the_placed_rect_exactly() {
        let rgba = [0u8; 16];
        let image = ImageBlock::new(&rgba, 200, 100, BlockSizing::FillRegion, ImageFit::Stretch);
        assert_eq!(image.content_rect(300.0, 100.0), (0.0, 0.0, 300.0, 100.0));
    }

    #[test]
    fn contain_letterboxes_a_wider_image_in_a_taller_box() {
        // 2:1 image in a 100x100 box -> width-limited: 100x50, vertically centered.
        let rgba = [0u8; 16];
        let image = ImageBlock::new(&rgba, 200, 100, BlockSizing::FillRegion, ImageFit::Contain);
        let (x, y, w, h) = image.content_rect(100.0, 100.0);
        assert_eq!((x, y, w, h), (0.0, 25.0, 100.0, 50.0));
    }

    #[test]
    fn cover_crops_a_wider_image_in_a_taller_box() {
        // 2:1 image in a 100x100 box, Cover -> height-limited: fills full
        // 100 height, width overflows (200), centered horizontally.
        let rgba = [0u8; 16];
        let image = ImageBlock::new(&rgba, 200, 100, BlockSizing::FillRegion, ImageFit::Cover);
        let (x, y, w, h) = image.content_rect(100.0, 100.0);
        assert_eq!((x, y, w, h), (-50.0, 0.0, 200.0, 100.0));
    }
}
