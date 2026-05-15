//! [`Painter`] — core path construction, fill/stroke, transform, and state.
//!
//! Every backend must implement this trait. No opt-out methods.

/// Core drawing trait — path construction, fill/stroke, transforms, state.
///
/// Every backend must implement this. No opt-out methods.
pub trait Painter {
    // =========================================================================
    // State
    // =========================================================================

    /// Save the current drawing state (transforms, styles) onto an internal stack.
    fn save(&mut self);

    /// Restore the most recently saved drawing state.
    fn restore(&mut self);

    // =========================================================================
    // Transforms
    // =========================================================================

    /// Translate the coordinate origin by `(x, y)`.
    fn translate(&mut self, x: f64, y: f64);

    /// Rotate around the current origin by `angle` radians.
    fn rotate(&mut self, angle: f64);

    /// Scale from the current origin by `(x, y)`.
    fn scale(&mut self, x: f64, y: f64);

    // =========================================================================
    // Style setters
    // =========================================================================

    /// Set fill color (CSS hex string, e.g. `"#RRGGBB"` or `"#RRGGBBAA"`).
    fn set_fill_color(&mut self, color: &str);

    /// Set global alpha (transparency) in `0.0..=1.0`.
    fn set_global_alpha(&mut self, alpha: f64);

    /// Set stroke color (CSS hex string).
    fn set_stroke_color(&mut self, color: &str);

    /// Set stroke width in pixels.
    fn set_stroke_width(&mut self, width: f64);

    /// Set line dash pattern (empty slice = solid line).
    fn set_line_dash(&mut self, pattern: &[f64]);

    /// Set line cap style: `"butt"`, `"round"`, or `"square"`.
    fn set_line_cap(&mut self, cap: &str);

    /// Set line join style: `"miter"`, `"round"`, or `"bevel"`.
    fn set_line_join(&mut self, join: &str);

    // =========================================================================
    // Style helpers (default impls)
    // =========================================================================

    /// Set fill color with an additional alpha multiplier.
    ///
    /// Default: calls [`set_fill_color`] then [`set_global_alpha`].
    fn set_fill_color_alpha(&mut self, color: &str, alpha: f64) {
        self.set_fill_color(color);
        self.set_global_alpha(alpha.clamp(0.0, 1.0));
    }

    /// Reset global alpha to 1.0 (fully opaque).
    fn reset_alpha(&mut self) {
        self.set_global_alpha(1.0);
    }

    // =========================================================================
    // Path construction
    // =========================================================================

    /// Begin a new path, discarding any previously accumulated path data.
    fn begin_path(&mut self);

    /// Move the current point to `(x, y)` without drawing.
    fn move_to(&mut self, x: f64, y: f64);

    /// Draw a straight line from the current point to `(x, y)`.
    fn line_to(&mut self, x: f64, y: f64);

    /// Close the current subpath by drawing a line back to its start.
    fn close_path(&mut self);

    /// Append a rectangle to the current path (no stroke/fill).
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64);

    /// Append an arc to the current path.
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64);

    /// Append an ellipse arc to the current path.
    #[allow(clippy::too_many_arguments)]
    fn ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
    );

    /// Append a quadratic Bézier curve to the current path.
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64);

    /// Append a cubic Bézier curve to the current path.
    fn bezier_curve_to(
        &mut self,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    );

    // =========================================================================
    // Commit
    // =========================================================================

    /// Stroke the current path using the current stroke style.
    fn stroke(&mut self);

    /// Fill the current path using the current fill style.
    fn fill(&mut self);
}
