//! Geometry + paint types. Thin abstraction over `kurbo` + `peniko`.
//!
//! Consumers depend ONLY on `urx_core::math::*` (or the flat re-exports
//! at the crate root). If we ever swap the underlying crates we update
//! this module and downstream code keeps compiling.

pub use kurbo::{Affine, BezPath, Point, Rect, Size, Vec2, RoundedRect, RoundedRectRadii};
pub use peniko::{
    BlendMode, Brush, Color, ColorStop, ColorStops, Compose, Extend, Gradient, GradientKind, Mix,
};

/// A linear-space RGBA color (8-bit per channel) — convenient for callers
/// that prefer raw bytes over `peniko::Color`. Convert via `to_peniko`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba8(pub [u8; 4]);

impl Rgba8 {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }
    pub fn to_peniko(self) -> Color {
        Color::rgba8(self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

impl From<Rgba8> for Color {
    fn from(c: Rgba8) -> Self {
        c.to_peniko()
    }
}
