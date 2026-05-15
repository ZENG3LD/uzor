//! [`ShapeHelpers`] — shape convenience helpers with default impls over [`Painter`].

use super::painter::Painter;

/// Shape convenience helpers — all have default impls over [`Painter`].
///
/// Backends may override (e.g. vello-gpu overrides [`rounded_rect_corners`]
/// with native kurbo rounded-rect for better precision).
pub trait ShapeHelpers: Painter {
    /// Stroke a rectangle.
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Fill a rectangle.
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Add a rounded rectangle to the current path using arc-based geometry.
    ///
    /// No stroke or fill is performed — call [`fill`](Painter::fill) or
    /// [`stroke`](Painter::stroke) afterwards.
    fn rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64) {
        let r = r.min(w / 2.0).min(h / 2.0);
        self.move_to(x + r, y);
        self.line_to(x + w - r, y);
        self.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        self.line_to(x + w, y + h - r);
        self.arc(
            x + w - r,
            y + h - r,
            r,
            0.0,
            std::f64::consts::FRAC_PI_2,
        );
        self.line_to(x + r, y + h);
        self.arc(
            x + r,
            y + h - r,
            r,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
        );
        self.line_to(x, y + r);
        self.arc(
            x + r,
            y + r,
            r,
            std::f64::consts::PI,
            std::f64::consts::PI * 1.5,
        );
        self.close_path();
    }

    /// Add a rounded rectangle with per-corner radii to the current path.
    ///
    /// Corner order: `tl` (top-left), `tr` (top-right), `br` (bottom-right),
    /// `bl` (bottom-left). Each radius is clamped to `min(w, h) / 2`.
    ///
    /// Default falls back to a uniform-radius [`rounded_rect`](Self::rounded_rect)
    /// using `min(tl, tr, br, bl)` so we under-round rather than over-round on
    /// backends lacking native per-corner support.
    #[allow(clippy::too_many_arguments)]
    fn rounded_rect_corners(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        tl: f64,
        tr: f64,
        br: f64,
        bl: f64,
    ) {
        let max_r = (w / 2.0).min(h / 2.0).max(0.0);
        let tl = tl.clamp(0.0, max_r);
        let tr = tr.clamp(0.0, max_r);
        let br = br.clamp(0.0, max_r);
        let bl = bl.clamp(0.0, max_r);
        let r = tl.min(tr).min(br).min(bl);
        self.rounded_rect(x, y, w, h, r);
    }

    /// Fill a rounded rectangle.
    ///
    /// Default: `begin_path` + [`rounded_rect`](Self::rounded_rect) + `fill`.
    fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        self.begin_path();
        self.rounded_rect(x, y, w, h, radius);
        self.fill();
    }

    /// Stroke a rounded rectangle.
    ///
    /// Default: `begin_path` + [`rounded_rect`](Self::rounded_rect) + `stroke`.
    fn stroke_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        self.begin_path();
        self.rounded_rect(x, y, w, h, radius);
        self.stroke();
    }

    /// Fill a rounded rectangle with per-corner radii.
    ///
    /// Default: `begin_path` + [`rounded_rect_corners`](Self::rounded_rect_corners) + `fill`.
    #[allow(clippy::too_many_arguments)]
    fn fill_rounded_rect_corners(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        tl: f64,
        tr: f64,
        br: f64,
        bl: f64,
    ) {
        self.begin_path();
        self.rounded_rect_corners(x, y, w, h, tl, tr, br, bl);
        self.fill();
    }

    /// Stroke a rounded rectangle with per-corner radii.
    ///
    /// Default: `begin_path` + [`rounded_rect_corners`](Self::rounded_rect_corners) + `stroke`.
    #[allow(clippy::too_many_arguments)]
    fn stroke_rounded_rect_corners(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        tl: f64,
        tr: f64,
        br: f64,
        bl: f64,
    ) {
        self.begin_path();
        self.rounded_rect_corners(x, y, w, h, tl, tr, br, bl);
        self.stroke();
    }
}
