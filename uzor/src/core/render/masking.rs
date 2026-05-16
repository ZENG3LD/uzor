//! [`Masking`] — clipping and mask layers.

use super::painter::Painter;

/// Clipping and mask layers.
pub trait Masking: Painter {
    // All implementors are also `Painter` via the supertrait bound, so the
    // default impl of `push_clip_svg_path` can pass `self` directly to
    // `emit_svg_path` which takes `&mut dyn Painter`.  No `where Self: Painter`
    // clause is needed — the bound is already guaranteed — keeping this method
    // dyn-safe and usable through `&mut dyn Masking` / `&mut dyn RenderContext`.
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

    /// Push a clip region defined by an SVG path `d` string.
    ///
    /// Parses `d`, emits the path commands, then calls [`push_mask`](Self::push_mask)
    /// to make it the active clip region.  Pop the clip with [`pop_mask`](Self::pop_mask)
    /// — there is **no** separate `pop_clip_svg_path`; the mask stack is shared.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.push_clip_svg_path("M 10 10 L 90 10 L 90 90 L 10 90 Z");
    /// // draw clipped content …
    /// ctx.pop_mask();
    /// ```
    ///
    /// Backends with native SVG-clip support (e.g. vello-gpu via
    /// `scene.push_clip_path`) may override this method for better performance.
    fn push_clip_svg_path(&mut self, d: &str) {
        crate::core::render::path::emit_svg_path_generic(self, d);
        self.push_mask();
    }

    /// Push a clip region defined by an SVG path `d` string using the
    /// **even-odd** fill rule.
    ///
    /// Effective for "all except shape" patterns where you draw an outer rect
    /// (CW) then an inner shape (CCW) — or any two subpaths with opposite
    /// winding — and the XOR result becomes the clip region.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Outer rect CW, inner circle CCW → ring-shaped clip.
    /// ctx.push_clip_svg_path_even_odd(
    ///     "M 0 0 L 200 0 L 200 200 L 0 200 Z \
    ///      M 100 100 m -40 0 a 40 40 0 1 0 80 0 a 40 40 0 1 0 -80 0 Z"
    /// );
    /// ctx.fill_rect(0.0, 0.0, 200.0, 200.0); // fills only the ring
    /// ctx.pop_mask();
    /// ```
    ///
    /// Pop with [`pop_mask`](Self::pop_mask) — shared mask stack.
    ///
    /// **Default impl**: delegates to [`push_clip_svg_path`](Self::push_clip_svg_path)
    /// (non-zero winding).  Backends override to honour even-odd properly.
    fn push_clip_svg_path_even_odd(&mut self, d: &str) {
        self.push_clip_svg_path(d);
    }
}
