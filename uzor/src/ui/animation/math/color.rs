//! Color interpolation with perceptual color spaces (OKLCH)
//!
//! Provides conversion between sRGB, Oklab, and OKLCH color spaces
//! for perceptually uniform color interpolation.
//!
//! ## Color Spaces
//!
//! - **sRGB**: Standard RGB with gamma encoding (web default)
//! - **Linear RGB**: sRGB with gamma removed (physically correct)
//! - **Oklab**: Perceptual color space (cartesian)
//! - **OKLCH**: Perceptual color space (polar, preserves hue)
//!
//! ## Why OKLCH?
//!
//! RGB interpolation creates muddy midpoints and uneven brightness.
//! OKLCH provides perceptually uniform transitions with no hue drift.
//!
//! ## References
//!
//! - [Oklab color space](https://bottosson.github.io/posts/oklab/)
//! - [OKLCH Color Picker](https://oklch.fyi/)

use super::timeline::Animatable;

/// RGBA color in sRGB color space (0.0..1.0 per channel)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// Oklab color (perceptual, cartesian coordinates)
#[derive(Debug, Clone, Copy)]
pub struct Oklab {
    pub l: f64, // Lightness 0..1
    pub a: f64, // Green-red axis
    pub b: f64, // Blue-yellow axis
}

/// OKLCH color (perceptual, polar coordinates - better for hue interpolation)
#[derive(Debug, Clone, Copy)]
pub struct Oklch {
    pub l: f64, // Lightness 0..1
    pub c: f64, // Chroma (saturation)
    pub h: f64, // Hue in degrees 0..360
}

/// Color space for interpolation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorSpace {
    /// Standard RGB interpolation (fast but produces muddy colors)
    Srgb,
    /// Oklab cartesian (perceptually uniform, good for most cases)
    Oklab,
    /// OKLCH polar (perceptually uniform, preserves hue)
    Oklch,
    /// Linear RGB (physically correct blending)
    LinearRgb,
}

impl Color {
    /// Create a new color from RGBA values (0.0..1.0)
    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    /// Create a new color from RGB values (opaque)
    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Parse color from hex string
    ///
    /// Supports: "#RGB", "#RRGGBB", "#RRGGBBAA"
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');

        match hex.len() {
            3 => {
                // #RGB
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? as f64 / 15.0;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? as f64 / 15.0;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? as f64 / 15.0;
                Some(Color::rgb(r, g, b))
            }
            6 => {
                // #RRGGBB
                let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;
                Some(Color::rgb(r, g, b))
            }
            8 => {
                // #RRGGBBAA
                let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f64 / 255.0;
                Some(Color::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Convert color to hex string (#RRGGBB or #RRGGBBAA)
    pub fn to_hex(&self) -> String {
        let r = (self.r.clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (self.g.clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (self.b.clamp(0.0, 1.0) * 255.0).round() as u8;

        if self.a >= 1.0 {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        } else {
            let a = (self.a.clamp(0.0, 1.0) * 255.0).round() as u8;
            format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
        }
    }

    /// Convert sRGB to linear RGB (gamma decode)
    pub fn to_linear(&self) -> (f64, f64, f64) {
        (
            srgb_to_linear(self.r),
            srgb_to_linear(self.g),
            srgb_to_linear(self.b),
        )
    }

    /// Create color from linear RGB (gamma encode)
    pub fn from_linear(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self {
            r: linear_to_srgb(r),
            g: linear_to_srgb(g),
            b: linear_to_srgb(b),
            a,
        }
    }

    /// Convert to Oklab color space
    pub fn to_oklab(&self) -> Oklab {
        let (r, g, b) = self.to_linear();
        linear_rgb_to_oklab(r, g, b)
    }

    /// Create color from Oklab
    pub fn from_oklab(lab: Oklab, alpha: f64) -> Self {
        let (r, g, b) = oklab_to_linear_rgb(lab);
        Self::from_linear(r, g, b, alpha)
    }

    /// Convert to OKLCH color space
    pub fn to_oklch(&self) -> Oklch {
        let lab = self.to_oklab();
        oklab_to_oklch(lab)
    }

    /// Create color from OKLCH
    pub fn from_oklch(lch: Oklch, alpha: f64) -> Self {
        let lab = oklch_to_oklab(lch);
        Self::from_oklab(lab, alpha)
    }

    /// Interpolate between two colors in the given color space
    ///
    /// # Arguments
    ///
    /// * `other` - Target color
    /// * `t` - Interpolation factor (0.0 = self, 1.0 = other)
    /// * `space` - Color space to use for interpolation
    pub fn lerp(&self, other: &Color, t: f64, space: ColorSpace) -> Color {
        match space {
            ColorSpace::Srgb => Color {
                r: self.r + (other.r - self.r) * t,
                g: self.g + (other.g - self.g) * t,
                b: self.b + (other.b - self.b) * t,
                a: self.a + (other.a - self.a) * t,
            },
            ColorSpace::LinearRgb => {
                let (r1, g1, b1) = self.to_linear();
                let (r2, g2, b2) = other.to_linear();
                let r = r1 + (r2 - r1) * t;
                let g = g1 + (g2 - g1) * t;
                let b = b1 + (b2 - b1) * t;
                let a = self.a + (other.a - self.a) * t;
                Color::from_linear(r, g, b, a)
            }
            ColorSpace::Oklab => {
                let lab1 = self.to_oklab();
                let lab2 = other.to_oklab();
                let lab = Oklab {
                    l: lab1.l + (lab2.l - lab1.l) * t,
                    a: lab1.a + (lab2.a - lab1.a) * t,
                    b: lab1.b + (lab2.b - lab1.b) * t,
                };
                let a = self.a + (other.a - self.a) * t;
                Color::from_oklab(lab, a)
            }
            ColorSpace::Oklch => {
                let lch1 = self.to_oklch();
                let lch2 = other.to_oklch();
                let lch = Oklch {
                    l: lch1.l + (lch2.l - lch1.l) * t,
                    c: lch1.c + (lch2.c - lch1.c) * t,
                    h: lerp_hue(lch1.h, lch2.h, t),
                };
                let a = self.a + (other.a - self.a) * t;
                Color::from_oklch(lch, a)
            }
        }
    }

    /// Shorthand for OKLCH interpolation (recommended for perceptual quality)
    pub fn lerp_oklch(&self, other: &Color, t: f64) -> Color {
        self.lerp(other, t, ColorSpace::Oklch)
    }
}

// Common colors
impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
}

/// Implement Animatable for Color (uses OKLCH by default)
impl Animatable for Color {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        // Use OKLCH by default for perceptual quality
        Color::lerp(self, target, t, ColorSpace::Oklch)
    }
}

// === sRGB ↔ Linear RGB Conversion ===

/// Convert sRGB channel to linear RGB (gamma decode)
fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert linear RGB channel to sRGB (gamma encode)
fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

// === Linear RGB ↔ Oklab Conversion ===

/// Convert linear RGB to Oklab color space
///
/// Reference: https://bottosson.github.io/posts/oklab/
fn linear_rgb_to_oklab(r: f64, g: f64, b: f64) -> Oklab {
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    Oklab {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

/// Convert Oklab to linear RGB color space
fn oklab_to_linear_rgb(lab: Oklab) -> (f64, f64, f64) {
    let l_ = lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b;
    let m_ = lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b;
    let s_ = lab.l - 0.0894841775 * lab.a - 1.2914855480 * lab.b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

    (r, g, b)
}

// === Oklab ↔ OKLCH Conversion ===

/// Convert Oklab (cartesian) to OKLCH (polar)
fn oklab_to_oklch(lab: Oklab) -> Oklch {
    let c = (lab.a * lab.a + lab.b * lab.b).sqrt();
    let h = lab.b.atan2(lab.a).to_degrees();
    let h = if h < 0.0 { h + 360.0 } else { h };
    Oklch { l: lab.l, c, h }
}

/// Convert OKLCH (polar) to Oklab (cartesian)
fn oklch_to_oklab(lch: Oklch) -> Oklab {
    let h_rad = lch.h.to_radians();
    Oklab {
        l: lch.l,
        a: lch.c * h_rad.cos(),
        b: lch.c * h_rad.sin(),
    }
}

/// Interpolate hue values using shortest path around color wheel
fn lerp_hue(h1: f64, h2: f64, t: f64) -> f64 {
    // Calculate shortest distance around the color wheel
    let diff = ((h2 - h1) % 360.0 + 540.0) % 360.0 - 180.0;
    let result = h1 + diff * t;
    ((result % 360.0) + 360.0) % 360.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_hex_parsing() {
        // #RRGGBB
        let c = Color::from_hex("#FF0000").unwrap();
        assert!(approx_eq(c.r, 1.0));
        assert!(approx_eq(c.g, 0.0));
        assert!(approx_eq(c.b, 0.0));
        assert!(approx_eq(c.a, 1.0));

        // #RRGGBBAA
        let c = Color::from_hex("#00FF0080").unwrap();
        assert!(approx_eq(c.r, 0.0));
        assert!(approx_eq(c.g, 1.0));
        assert!(approx_eq(c.b, 0.0));
        assert!(approx_eq(c.a, 0.5019607843137255)); // 128/255

        // Invalid
        assert!(Color::from_hex("#ZZZZZZ").is_none());
    }

    #[test]
    fn test_hex_roundtrip() {
        let original = Color::rgba(0.5, 0.25, 0.75, 1.0);
        let hex = original.to_hex();
        let parsed = Color::from_hex(&hex).unwrap();

        // Allow small rounding error from 8-bit quantization
        assert!((original.r - parsed.r).abs() < 0.01);
        assert!((original.g - parsed.g).abs() < 0.01);
        assert!((original.b - parsed.b).abs() < 0.01);
    }

    #[test]
    fn test_srgb_linear_roundtrip() {
        let original = Color::rgb(0.5, 0.25, 0.75);
        let (r, g, b) = original.to_linear();
        let back = Color::from_linear(r, g, b, original.a);

        assert!(approx_eq(original.r, back.r));
        assert!(approx_eq(original.g, back.g));
        assert!(approx_eq(original.b, back.b));
    }

    #[test]
    fn test_oklab_roundtrip() {
        let original = Color::rgb(0.3, 0.6, 0.9);
        let lab = original.to_oklab();
        let back = Color::from_oklab(lab, original.a);

        assert!(approx_eq(original.r, back.r));
        assert!(approx_eq(original.g, back.g));
        assert!(approx_eq(original.b, back.b));
    }

    #[test]
    fn test_oklch_roundtrip() {
        let original = Color::rgb(0.3, 0.6, 0.9);
        let lch = original.to_oklch();
        let back = Color::from_oklch(lch, original.a);

        assert!(approx_eq(original.r, back.r));
        assert!(approx_eq(original.g, back.g));
        assert!(approx_eq(original.b, back.b));
    }

    #[test]
    fn test_hue_interpolation_shortest_path() {
        // 350° to 10° should go through 0°, not through 180°
        let h = lerp_hue(350.0, 10.0, 0.5);
        assert!(approx_eq(h, 0.0));

        // 10° to 350° should also go through 0°
        let h = lerp_hue(10.0, 350.0, 0.5);
        assert!(approx_eq(h, 0.0));

        // 90° to 270° is exactly 180° apart
        // The algorithm normalizes this to -180°, so it goes backward through 0°
        // 90 + (-180 * 0.5) = 90 - 90 = 0
        let h = lerp_hue(90.0, 270.0, 0.5);
        assert!(approx_eq(h, 0.0) || approx_eq(h, 180.0)); // Either path is valid for 180° distance
    }

    #[test]
    fn test_black_to_white_oklch() {
        let black = Color::BLACK;
        let white = Color::WHITE;

        let mid = black.lerp_oklch(&white, 0.5);

        // Midpoint should be roughly gray
        // In OKLCH, black->white stays on the L axis (no hue shift)
        assert!((mid.r - mid.g).abs() < 0.1);
        assert!((mid.g - mid.b).abs() < 0.1);
    }

    #[test]
    fn test_red_to_blue_oklch() {
        let red = Color::RED;
        let blue = Color::BLUE;

        // OKLCH should produce a vivid purple midpoint (not muddy gray like RGB)
        let mid_oklch = red.lerp(&blue, 0.5, ColorSpace::Oklch);
        let mid_rgb = red.lerp(&blue, 0.5, ColorSpace::Srgb);

        // OKLCH midpoint should be more saturated than RGB midpoint
        let oklch_chroma = mid_oklch.to_oklch().c;
        let rgb_chroma = mid_rgb.to_oklch().c;

        assert!(oklch_chroma > rgb_chroma);
    }

    #[test]
    fn test_alpha_interpolation() {
        let c1 = Color::rgba(1.0, 0.0, 0.0, 0.0);
        let c2 = Color::rgba(0.0, 0.0, 1.0, 1.0);

        let mid = c1.lerp_oklch(&c2, 0.5);
        assert!(approx_eq(mid.a, 0.5));
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK, Color::rgb(0.0, 0.0, 0.0));
        assert_eq!(Color::WHITE, Color::rgb(1.0, 1.0, 1.0));
        assert_eq!(Color::RED, Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(Color::GREEN, Color::rgb(0.0, 1.0, 0.0));
        assert_eq!(Color::BLUE, Color::rgb(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_animatable_impl() {
        use crate::animation::timeline::Animatable;

        let c1 = Color::RED;
        let c2 = Color::BLUE;

        // Animatable::lerp should use OKLCH by default
        let mid = Animatable::lerp(&c1, &c2, 0.5);
        assert!(mid.to_oklch().c > 0.1); // Should preserve chroma
    }
}
