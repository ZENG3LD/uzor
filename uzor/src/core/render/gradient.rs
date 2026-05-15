//! [`GradientPainter`] — gradient fill capabilities with solid-colour fallbacks.

use super::painter::Painter;

/// Gradient fills — default impls fall back to first-stop solid fill.
pub trait GradientPainter: Painter {
    /// Fill the current path with a linear gradient.
    ///
    /// `stops` — list of `(offset, color_hex)` pairs, offset in `0.0..=1.0`.
    /// `x1,y1` / `x2,y2` — gradient line start and end in canvas coordinates.
    ///
    /// Default falls back to a flat fill with the first stop color.
    fn fill_linear_gradient(
        &mut self,
        stops: &[(f32, &str)],
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        let _ = (x1, y1, x2, y2);
        if let Some((_, color)) = stops.first() {
            self.set_fill_color(color);
            self.fill();
        }
    }

    /// Fill the current path with a radial gradient.
    ///
    /// `cx`/`cy` — gradient centre; `r` — gradient radius.
    /// `stops` — list of `(offset, color_hex)` pairs sorted ascending by offset.
    /// `x,y,w,h` — bounding rectangle used by the default implementation for `fill_rect`.
    ///
    /// Default falls back to a flat fill with the first stop color.
    #[allow(clippy::too_many_arguments)]
    fn fill_radial_gradient(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        stops: &[(f32, &str)],
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        let _ = (cx, cy, r);
        if let Some((_, color)) = stops.first() {
            self.set_fill_color(color);
            self.begin_path();
            self.rect(x, y, w, h);
            self.fill();
        }
    }
}
