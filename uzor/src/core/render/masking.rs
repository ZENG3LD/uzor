//! [`Masking`] — clipping and mask layers.

use super::painter::Painter;

/// Clipping and mask layers.
pub trait Masking: Painter {
    /// Clip subsequent draws to the current path (binary clip).
    fn clip(&mut self);

    /// Convenience: clip to a rectangle.
    ///
    /// Default: `begin_path` + `rect` + `clip`.
    fn clip_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.begin_path();
        self.rect(x, y, width, height);
        self.clip();
    }

    /// Push a mask layer using the current path.
    ///
    /// Pattern:
    /// ```text
    /// ctx.begin_path();
    /// ctx.rounded_rect(x, y, w, h, r);
    /// ctx.push_mask();   // use current path as the mask
    /// // draw content ...
    /// ctx.pop_mask();
    /// ```
    ///
    /// Default: `save` + `clip` (binary path clip fallback).
    /// Backends that support alpha masks (vello) may override with a real
    /// alpha-mask layer.
    fn push_mask(&mut self) {
        self.save();
        self.clip();
    }

    /// Pop the most recently pushed mask layer.
    ///
    /// Default: `restore` (matches [`push_mask`](Self::push_mask) default).
    fn pop_mask(&mut self) {
        self.restore();
    }
}
