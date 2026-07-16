//! Plot-area coordinate transform — the ONE mapping from scale-normalized
//! `[0, 1]` to screen pixels that every mark and (future) hit-test goes
//! through (design law #1, `uzor_figures_engine_architecture.md` §4.1). Never
//! recompute a pixel position independently elsewhere in this crate.
//!
//! Zoom/pan/interaction are out of scope for V1 — [`PlotArea`] is a plain
//! per-frame rectangle, not a stateful camera. That's a V2 concern (see
//! the crate-root docs).

use uzor::types::Rect;

use crate::scale::{BandScale, Scale};

/// The pixel rectangle a figure draws its plot (marks + guides) into.
#[derive(Debug, Clone, Copy)]
pub struct PlotArea {
    pub rect: Rect,
}

impl PlotArea {
    pub fn new(rect: Rect) -> Self {
        Self { rect }
    }

    /// Domain value on `s` -> screen x pixel, left-to-right.
    pub fn x(&self, s: &dyn Scale, v: f64) -> f64 {
        self.rect.x + s.map(v) * self.rect.width
    }

    /// Domain value on `s` -> screen y pixel. INVERTED relative to `map`:
    /// the scale's domain maximum lands at the top of the rect, matching
    /// every cartesian chart convention (values increase upward).
    pub fn y(&self, s: &dyn Scale, v: f64) -> f64 {
        self.rect.y + (1.0 - s.map(v)) * self.rect.height
    }

    /// Screen pixel `(left, right)` extent of band `i` on `s`.
    pub fn x_band(&self, s: &BandScale, i: usize) -> (f64, f64) {
        let (t0, t1) = s.band_range(i);
        (self.rect.x + t0 * self.rect.width, self.rect.x + t1 * self.rect.width)
    }

    /// Screen pixel `(top, bottom)` extent of band `i` on `s`, top-to-
    /// bottom in NATURAL READING ORDER (row/band index 0 sits at the TOP).
    ///
    /// Deliberately NOT [`PlotArea::y`]'s convention: `y` inverts a
    /// continuous scale's domain (larger value plotted higher — the usual
    /// cartesian-axis rule). A row-indexed categorical axis (e.g.
    /// [`crate::figure::TimelineFigure`]'s lane rows) has no "larger is
    /// higher" value semantics to invert — it reads top-down like any
    /// list, the same way [`PlotArea::x_band`] already reads left-to-right
    /// for column bands. This is that convention rotated 90 degrees.
    pub fn y_band(&self, s: &BandScale, i: usize) -> (f64, f64) {
        let (t0, t1) = s.band_range(i);
        (self.rect.y + t0 * self.rect.height, self.rect.y + t1 * self.rect.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;

    #[test]
    fn x_maps_domain_min_max_to_rect_edges() {
        let area = PlotArea::new(Rect::new(10.0, 20.0, 200.0, 100.0));
        let scale = LinearScale::new(0.0, 100.0);
        assert!((area.x(&scale, 0.0) - 10.0).abs() < 1e-9);
        assert!((area.x(&scale, 100.0) - 210.0).abs() < 1e-9);
    }

    #[test]
    fn y_is_inverted_domain_max_at_top() {
        let area = PlotArea::new(Rect::new(0.0, 0.0, 100.0, 200.0));
        let scale = LinearScale::new(0.0, 100.0);
        // Domain max (100) -> top of rect (y = rect.y = 0).
        assert!((area.y(&scale, 100.0) - 0.0).abs() < 1e-9);
        // Domain min (0) -> bottom of rect (y = rect.y + rect.height).
        assert!((area.y(&scale, 0.0) - 200.0).abs() < 1e-9);
    }

    #[test]
    fn x_band_matches_scale_band_range_scaled_into_the_rect() {
        let area = PlotArea::new(Rect::new(0.0, 0.0, 400.0, 100.0));
        let band = BandScale::new(vec!["a".to_owned(), "b".to_owned()], 0.0);
        let (x0, x1) = area.x_band(&band, 0);
        assert!((x0 - 0.0).abs() < 1e-9);
        assert!((x1 - 200.0).abs() < 1e-9);
        let (x0, x1) = area.x_band(&band, 1);
        assert!((x0 - 200.0).abs() < 1e-9);
        assert!((x1 - 400.0).abs() < 1e-9);
    }

    #[test]
    fn y_band_reads_top_down_in_natural_row_order() {
        let area = PlotArea::new(Rect::new(0.0, 0.0, 100.0, 400.0));
        let band = BandScale::new(vec!["row-0".to_owned(), "row-1".to_owned()], 0.0);
        // Row 0 (first category) must land at the TOP of the rect —
        // opposite of `y`'s inverted-value convention, where index/value 0
        // would land at the BOTTOM.
        let (top, bottom) = area.y_band(&band, 0);
        assert!((top - 0.0).abs() < 1e-9);
        assert!((bottom - 200.0).abs() < 1e-9);
        let (top, bottom) = area.y_band(&band, 1);
        assert!((top - 200.0).abs() < 1e-9);
        assert!((bottom - 400.0).abs() < 1e-9);
    }
}
