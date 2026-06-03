//! Color conversion — peniko `Color` / `Brush` → premultiplied RGBA8.
//!
//! Centralised here so every primitive uses the same conversion. The
//! premultiplied form is what `Pixmap::blend_pixel` and `set_pixel`
//! expect.

use uzor_urx_core::math::{Brush, Color};

/// Convert a `peniko::Color` to premultiplied `[r, g, b, a]` bytes.
/// Rounding: round-half-up (`+127 / 255` is correct for u8).
#[inline]
pub fn color_to_premul(c: Color) -> [u8; 4] {
    let a = c.a as u32;
    [
        ((c.r as u32 * a + 127) / 255) as u8,
        ((c.g as u32 * a + 127) / 255) as u8,
        ((c.b as u32 * a + 127) / 255) as u8,
        c.a,
    ]
}

/// Scale a premultiplied color by a coverage factor `[0, 255]`.
/// Used for analytic AA edges: a pixel that is `60%` covered gets
/// its premultiplied bytes scaled by `0.60`.
#[inline]
pub fn premul_scale(rgba: [u8; 4], cov: u8) -> [u8; 4] {
    let c = cov as u32;
    [
        ((rgba[0] as u32 * c + 127) / 255) as u8,
        ((rgba[1] as u32 * c + 127) / 255) as u8,
        ((rgba[2] as u32 * c + 127) / 255) as u8,
        ((rgba[3] as u32 * c + 127) / 255) as u8,
    ]
}

/// Resolve a `Brush` to a flat `Color`. Gradients are stub'd to first
/// color stop for now (Phase 3 minimal). Real gradient impl lands in
/// follow-up when we ship our scanline gradient evaluator.
pub fn brush_to_color(brush: &Brush) -> Color {
    match brush {
        Brush::Solid(c)        => *c,
        Brush::Gradient(g)     => g.stops.first().map(|s| s.color).unwrap_or(Color::rgba8(0, 0, 0, 0)),
        Brush::Image(_)        => Color::rgba8(0, 0, 0, 0),
    }
}
