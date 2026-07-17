//! Minimal 2D affine transform — the SAME matrix shape `tiny_skia::Transform`
//! uses (`sx, ky, kx, sy, tx, ty`), reimplemented locally so this crate has
//! no dependency on `tiny-skia` itself. [`crate::context::SvgRenderContext`]
//! bakes this matrix directly into every emitted coordinate (see
//! `lib.rs`'s own module doc for why) rather than emitting a nested
//! `<g transform="...">` per `save`/`restore` scope.

/// `(x, y) -> (sx*x + kx*y + tx, ky*x + sy*y + ty)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform {
    pub sx: f64,
    pub ky: f64,
    pub kx: f64,
    pub sy: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Transform {
    pub fn identity() -> Self {
        Self { sx: 1.0, ky: 0.0, kx: 0.0, sy: 1.0, tx: 0.0, ty: 0.0 }
    }

    /// Map a local point through this matrix.
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.sx * x + self.kx * y + self.tx, self.ky * x + self.sy * y + self.ty)
    }

    /// `self` composed with a translation applied BEFORE it — matches
    /// [`uzor::render::Painter::translate`]'s "offset the current LOCAL
    /// coordinate system" semantics (a later `translate` call is expressed
    /// in whatever frame earlier `translate`/`scale`/`rotate` calls already
    /// established).
    pub fn pre_translate(&self, dx: f64, dy: f64) -> Self {
        Self {
            sx: self.sx,
            ky: self.ky,
            kx: self.kx,
            sy: self.sy,
            tx: self.sx * dx + self.kx * dy + self.tx,
            ty: self.ky * dx + self.sy * dy + self.ty,
        }
    }

    /// `self` composed with a scale applied BEFORE it.
    pub fn pre_scale(&self, sx: f64, sy: f64) -> Self {
        Self {
            sx: self.sx * sx,
            ky: self.ky * sx,
            kx: self.kx * sy,
            sy: self.sy * sy,
            tx: self.tx,
            ty: self.ty,
        }
    }

    /// `self` composed with a rotation (radians, clockwise on screen —
    /// matches `Painter::rotate`'s documented Canvas2D convention) applied
    /// BEFORE it.
    pub fn pre_rotate(&self, angle_rad: f64) -> Self {
        let (s, c) = angle_rad.sin_cos();
        Self {
            sx: self.sx * c + self.kx * s,
            ky: self.ky * c + self.sy * s,
            kx: -self.sx * s + self.kx * c,
            sy: -self.ky * s + self.sy * c,
            tx: self.tx,
            ty: self.ty,
        }
    }

    /// Approximate uniform scale factor of this matrix — the average of
    /// the two basis-vector lengths. Used to scale stroke width / dash
    /// arrays / radial-gradient radii, since those are emitted as separate
    /// SVG attributes (not baked into path coordinates the way fill/stroke
    /// GEOMETRY is) and must still visually track `Painter::scale`.
    /// Exact for uniform scale-only matrices; an approximation (not exact
    /// per-direction) under rotation+non-uniform-scale combinations — a
    /// documented, minor limitation, the same class of approximation
    /// `uzor-render-tiny-skia`'s own `render_scale` heuristic accepts for
    /// glyph rasterization.
    pub fn scale_factor(&self) -> f64 {
        let x_len = (self.sx * self.sx + self.ky * self.ky).sqrt();
        let y_len = (self.kx * self.kx + self.sy * self.sy).sqrt();
        ((x_len + y_len) / 2.0).max(1e-9)
    }

    /// `true` when this matrix has no rotation/skew component — a plain
    /// translate+scale, representable by an axis-aligned SVG `<rect>`/
    /// `<image>` element instead of a `<path>`/`transform` fallback.
    pub fn is_axis_aligned(&self) -> bool {
        self.kx.abs() < 1e-9 && self.ky.abs() < 1e-9
    }
}

#[cfg(test)]
mod tests {
    use super::Transform;

    #[test]
    fn identity_is_a_no_op() {
        let t = Transform::identity();
        assert_eq!(t.apply(3.0, 4.0), (3.0, 4.0));
    }

    #[test]
    fn translate_then_scale_matches_canvas_semantics() {
        // ctx.translate(10, 20); ctx.scale(2, 3); point (0,0) -> (10, 20);
        // point (5, 5) -> (10 + 2*5, 20 + 3*5) = (20, 35) — the SAME
        // composition order `Painter::translate`/`Painter::scale` apply on
        // a real canvas (a later call affects the LOCAL frame the earlier
        // calls already established).
        let t = Transform::identity().pre_translate(10.0, 20.0).pre_scale(2.0, 3.0);
        assert_eq!(t.apply(0.0, 0.0), (10.0, 20.0));
        assert_eq!(t.apply(5.0, 5.0), (20.0, 35.0));
    }

    #[test]
    fn rotate_90_degrees_maps_x_axis_onto_y_axis() {
        let t = Transform::identity().pre_rotate(std::f64::consts::FRAC_PI_2);
        let (x, y) = t.apply(1.0, 0.0);
        assert!((x - 0.0).abs() < 1e-9);
        assert!((y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn axis_aligned_detects_rotation() {
        assert!(Transform::identity().pre_translate(5.0, 5.0).pre_scale(2.0, 2.0).is_axis_aligned());
        assert!(!Transform::identity().pre_rotate(0.3).is_axis_aligned());
    }

    #[test]
    fn scale_factor_matches_uniform_scale_exactly() {
        let t = Transform::identity().pre_scale(2.5, 2.5);
        assert!((t.scale_factor() - 2.5).abs() < 1e-9);
    }
}
