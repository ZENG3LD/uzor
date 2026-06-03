//! Cold-start skeleton painter — first-frame CPU rasterisation while
//! GPU shaders compile.
//!
//! Pure CPU, zero GPU deps, zero crate deps beyond `urx-core::math`.
//! The whole point: GPU isn't ready yet. We must not pull in wgpu /
//! image / cosmic-text / kurbo here — those are the slow things we're
//! waiting for.
//!
//! What we paint (all optional):
//!   * background fill (always — solid premul colour)
//!   * centred logo (optional `Vec<u8>` decoded **straight-alpha RGBA8**
//!     by the caller; we don't link image-decode here)
//!   * spinner ring (animated via `now_us`)
//!   * caption (single line, built-in 5×7 bitmap font, ASCII subset)
//!
//! Implementation budget: ~400 LOC. Everything beyond that should not
//! be a boot-screen feature.

use crate::math::{Color, Rect};

#[derive(Debug, Clone)]
pub struct SkeletonSpec {
    pub bg:            Color,
    /// Optional logo: straight-alpha RGBA8 bytes + dims.
    /// Provided pre-decoded by the caller (skeleton can't depend on
    /// `image` — it would defeat the cold-start purpose).
    pub logo:          Option<SkeletonImage>,
    pub spinner:       bool,
    pub spinner_color: Option<Color>,
    pub caption:       Option<String>,
    pub caption_color: Option<Color>,
}

#[derive(Debug, Clone)]
pub struct SkeletonImage {
    pub width:  u32,
    pub height: u32,
    /// Straight-alpha RGBA8, length = width*height*4. Premultiplied
    /// internally on first paint.
    pub bytes:  Vec<u8>,
}

impl Default for SkeletonSpec {
    fn default() -> Self {
        Self {
            bg:            Color::rgba8(13, 17, 23, 255),
            logo:          None,
            spinner:       true,
            spinner_color: None,
            caption:       None,
            caption_color: None,
        }
    }
}

pub struct SkeletonFrame {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    spec:       SkeletonSpec,
    started_us: u64,
}

impl SkeletonFrame {
    pub fn new(width: u32, height: u32, spec: SkeletonSpec) -> Self {
        let pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        Self { width, height, pixels, spec, started_us: 0 }
    }

    pub fn set_started_us(&mut self, t_us: u64) { self.started_us = t_us; }
    pub fn spec(&self) -> &SkeletonSpec { &self.spec }

    pub fn render(&mut self, now_us: u64) {
        let bg = premul(self.spec.bg);
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bg);
        }

        let cx = self.width  as f64 / 2.0;
        let cy = self.height as f64 / 2.0;

        if let Some(logo) = &self.spec.logo {
            let lw = logo.width;
            let lh = logo.height;
            let x = (self.width.saturating_sub(lw)) as f64 / 2.0;
            let y = (self.height.saturating_sub(lh)) as f64 / 2.0 - 24.0;
            blit_logo(&mut self.pixels, self.width, self.height,
                      logo, x as i64, y as i64);
        }

        if self.spec.spinner {
            let elapsed_us = now_us.saturating_sub(self.started_us);
            let phase = (elapsed_us as f64 / 16_000.0) % std::f64::consts::TAU;
            let color = self.spec.spinner_color.unwrap_or(Color::rgba8(120, 180, 255, 255));
            draw_spinner(&mut self.pixels, self.width, self.height,
                         cx, cy + 32.0, 14.0, 2.5, phase, color);
        }

        if let Some(caption) = &self.spec.caption {
            let color = self.spec.caption_color.unwrap_or(Color::rgba8(160, 168, 180, 255));
            let tw = caption_pixel_width(caption);
            let x = cx - tw as f64 / 2.0;
            let y = cy + 60.0;
            draw_caption(&mut self.pixels, self.width, self.height,
                         caption, x as i64, y as i64, color);
        }
    }

    pub fn discard(&mut self) {
        self.pixels.clear();
        self.pixels.shrink_to_fit();
    }
}

#[inline]
fn premul(c: Color) -> [u8; 4] {
    let a = c.a as u32;
    [
        ((c.r as u32 * a + 127) / 255) as u8,
        ((c.g as u32 * a + 127) / 255) as u8,
        ((c.b as u32 * a + 127) / 255) as u8,
        c.a,
    ]
}

#[inline]
fn blend_pixel(pixels: &mut [u8], w: u32, h: u32, x: i64, y: i64, src: [u8; 4]) {
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h { return; }
    let i = ((y as u32 * w + x as u32) * 4) as usize;
    let inv_a = 255 - src[3] as u32;
    for k in 0..4 {
        let dst = pixels[i + k] as u32;
        pixels[i + k] = (src[k] as u32 + (dst * inv_a + 127) / 255).min(255) as u8;
    }
}

fn blit_logo(pixels: &mut [u8], pw: u32, ph: u32, logo: &SkeletonImage, dx: i64, dy: i64) {
    let lw = logo.width as i64;
    let lh = logo.height as i64;
    for ly in 0..lh {
        for lx in 0..lw {
            let i = ((ly as u32 * logo.width + lx as u32) * 4) as usize;
            let a = logo.bytes[i + 3] as u32;
            let src = [
                ((logo.bytes[i]     as u32 * a + 127) / 255) as u8,
                ((logo.bytes[i + 1] as u32 * a + 127) / 255) as u8,
                ((logo.bytes[i + 2] as u32 * a + 127) / 255) as u8,
                a as u8,
            ];
            blend_pixel(pixels, pw, ph, dx + lx, dy + ly, src);
        }
    }
}

/// Spinner = a ring with a brighter arc, rotating. We sample the ring
/// SDF per pixel and weight by an arc-mask phase. ~3 lines of math,
/// no allocations.
fn draw_spinner(
    pixels: &mut [u8],
    pw: u32, ph: u32,
    cx: f64, cy: f64,
    radius: f64, thickness: f64,
    phase: f64,
    color: Color,
) {
    let r_outer = radius + thickness * 0.5 + 1.0;
    let x0 = (cx - r_outer).floor() as i64;
    let y0 = (cy - r_outer).floor() as i64;
    let x1 = (cx + r_outer).ceil() as i64;
    let y1 = (cy + r_outer).ceil() as i64;
    let base_a = color.a as f64;
    for py in y0..=y1 {
        let dy = py as f64 + 0.5 - cy;
        for px in x0..=x1 {
            let dx = px as f64 + 0.5 - cx;
            let d = (dx*dx + dy*dy).sqrt();
            // Ring SDF distance from the centerline (positive outside band).
            let ring_d = (d - radius).abs() - thickness * 0.5;
            if ring_d >= 0.5 { continue; }
            let cov = if ring_d <= -0.5 { 1.0 } else { 0.5 - ring_d };
            if cov <= 0.0 { continue; }
            // Arc mask: tail brighter at `phase`, fades around the ring.
            let ang = dy.atan2(dx);
            let mut delta = (ang - phase).rem_euclid(std::f64::consts::TAU);
            if delta > std::f64::consts::PI { delta = std::f64::consts::TAU - delta; }
            let mask = (1.0 - (delta / std::f64::consts::PI)).max(0.15);
            let alpha = (base_a * cov * mask).round().clamp(0.0, 255.0) as u8;
            if alpha == 0 { continue; }
            let src = [
                ((color.r as u32 * alpha as u32 + 127) / 255) as u8,
                ((color.g as u32 * alpha as u32 + 127) / 255) as u8,
                ((color.b as u32 * alpha as u32 + 127) / 255) as u8,
                alpha,
            ];
            blend_pixel(pixels, pw, ph, px, py, src);
        }
    }
}

// ---- 5x7 ASCII bitmap font (printable subset) -----------------------------
//
// 96 glyphs (' '..'~'). Each glyph = 5 columns × 7 rows packed bottom-up
// in a u8 (one column per byte, low 7 bits = rows). Source: classic
// public-domain bitmap font (Pico-8 style). Trimmed to the subset we
// actually need for boot captions (digits, letters, basic punctuation).

const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;

// Each glyph is 5 bytes (one per column). Lowest bit = top row.
// Compact encoding: index by (c as u32 - 0x20). Unknown → space.
static FONT_5X7: [[u8; 5]; 96] = {
    let mut t = [[0u8; 5]; 96];
    // Helper macro-less inline initialisers — only fill the cells we use.
    // Rest stay zero (rendered as blank space).
    // The encoded characters cover digits, uppercase letters, and a few
    // punctuation marks — sufficient for boot captions.
    macro_rules! glyph {
        ($ch:literal, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr) => {
            t[($ch as u32 - 0x20) as usize] = [$a, $b, $c, $d, $e];
        };
    }
    glyph!(' ', 0,0,0,0,0);
    glyph!('.', 0b0000000, 0b1100000, 0b1100000, 0,0);
    glyph!(',', 0b0000000, 0b1100000, 0b1110000, 0,0);
    glyph!('-', 0b0001000, 0b0001000, 0b0001000, 0b0001000, 0b0001000);
    glyph!('0', 0b0111110, 0b1010001, 0b1001001, 0b1000101, 0b0111110);
    glyph!('1', 0b0000000, 0b1000010, 0b1111111, 0b1000000, 0b0000000);
    glyph!('2', 0b1100010, 0b1010001, 0b1001001, 0b1000101, 0b1000110);
    glyph!('3', 0b0100010, 0b1000001, 0b1001001, 0b1001001, 0b0110110);
    glyph!('4', 0b0011000, 0b0010100, 0b0010010, 0b1111111, 0b0010000);
    glyph!('5', 0b0100111, 0b1000101, 0b1000101, 0b1000101, 0b0111001);
    glyph!('6', 0b0111100, 0b1001010, 0b1001001, 0b1001001, 0b0110000);
    glyph!('7', 0b0000001, 0b1110001, 0b0001001, 0b0000101, 0b0000011);
    glyph!('8', 0b0110110, 0b1001001, 0b1001001, 0b1001001, 0b0110110);
    glyph!('9', 0b0000110, 0b1001001, 0b1001001, 0b0101001, 0b0011110);
    glyph!(':', 0b0000000, 0b0110110, 0b0110110, 0,0);
    glyph!('A', 0b1111110, 0b0010001, 0b0010001, 0b0010001, 0b1111110);
    glyph!('B', 0b1111111, 0b1001001, 0b1001001, 0b1001001, 0b0110110);
    glyph!('C', 0b0111110, 0b1000001, 0b1000001, 0b1000001, 0b0100010);
    glyph!('D', 0b1111111, 0b1000001, 0b1000001, 0b0100010, 0b0011100);
    glyph!('E', 0b1111111, 0b1001001, 0b1001001, 0b1001001, 0b1000001);
    glyph!('F', 0b1111111, 0b0001001, 0b0001001, 0b0001001, 0b0000001);
    glyph!('G', 0b0111110, 0b1000001, 0b1001001, 0b1001001, 0b1111010);
    glyph!('H', 0b1111111, 0b0001000, 0b0001000, 0b0001000, 0b1111111);
    glyph!('I', 0b0000000, 0b1000001, 0b1111111, 0b1000001, 0b0000000);
    glyph!('J', 0b0100000, 0b1000000, 0b1000001, 0b0111111, 0b0000001);
    glyph!('K', 0b1111111, 0b0001000, 0b0010100, 0b0100010, 0b1000001);
    glyph!('L', 0b1111111, 0b1000000, 0b1000000, 0b1000000, 0b1000000);
    glyph!('M', 0b1111111, 0b0000010, 0b0000100, 0b0000010, 0b1111111);
    glyph!('N', 0b1111111, 0b0000100, 0b0001000, 0b0010000, 0b1111111);
    glyph!('O', 0b0111110, 0b1000001, 0b1000001, 0b1000001, 0b0111110);
    glyph!('P', 0b1111111, 0b0001001, 0b0001001, 0b0001001, 0b0000110);
    glyph!('Q', 0b0111110, 0b1000001, 0b1010001, 0b0100001, 0b1011110);
    glyph!('R', 0b1111111, 0b0001001, 0b0011001, 0b0101001, 0b1000110);
    glyph!('S', 0b1000110, 0b1001001, 0b1001001, 0b1001001, 0b0110001);
    glyph!('T', 0b0000001, 0b0000001, 0b1111111, 0b0000001, 0b0000001);
    glyph!('U', 0b0111111, 0b1000000, 0b1000000, 0b1000000, 0b0111111);
    glyph!('V', 0b0011111, 0b0100000, 0b1000000, 0b0100000, 0b0011111);
    glyph!('W', 0b1111111, 0b0100000, 0b0011000, 0b0100000, 0b1111111);
    glyph!('X', 0b1100011, 0b0010100, 0b0001000, 0b0010100, 0b1100011);
    glyph!('Y', 0b0000111, 0b0001000, 0b1110000, 0b0001000, 0b0000111);
    glyph!('Z', 0b1100001, 0b1010001, 0b1001001, 0b1000101, 0b1000011);
    t
};

#[inline]
fn glyph_columns(ch: char) -> [u8; 5] {
    let upper = ch.to_ascii_uppercase();
    let i = (upper as u32).saturating_sub(0x20) as usize;
    if i < FONT_5X7.len() { FONT_5X7[i] } else { [0; 5] }
}

fn caption_pixel_width(caption: &str) -> usize {
    let chars = caption.chars().count();
    if chars == 0 { 0 } else { chars * (GLYPH_W + 1) - 1 }
}

fn draw_caption(pixels: &mut [u8], pw: u32, ph: u32, caption: &str, x: i64, y: i64, color: Color) {
    let prem = premul(color);
    let mut pen_x = x;
    for ch in caption.chars() {
        let cols = glyph_columns(ch);
        for col_i in 0..GLYPH_W {
            let col_bits = cols[col_i];
            for row_i in 0..GLYPH_H {
                if (col_bits >> row_i) & 1 != 0 {
                    blend_pixel(pixels, pw, ph, pen_x + col_i as i64, y + row_i as i64, prem);
                }
            }
        }
        pen_x += (GLYPH_W + 1) as i64;
    }
}

/// Bounding-box helper for centering content in a skeleton frame.
#[inline]
pub fn centered_rect(frame_w: u32, frame_h: u32, w: u32, h: u32) -> Rect {
    let x = ((frame_w.saturating_sub(w)) / 2) as f64;
    let y = ((frame_h.saturating_sub(h)) / 2) as f64;
    Rect::new(x, y, x + w as f64, y + h as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_fills_solid() {
        let mut f = SkeletonFrame::new(20, 20, SkeletonSpec {
            spinner: false, caption: None, logo: None,
            bg: Color::rgba8(80, 90, 100, 255), ..SkeletonSpec::default()
        });
        f.render(0);
        let i = (5 * 20 + 5) * 4;
        assert_eq!(&f.pixels[i..i+4], &[80, 90, 100, 255]);
    }

    #[test]
    fn spinner_paints_ring_pixels() {
        let mut f = SkeletonFrame::new(80, 80, SkeletonSpec::default());
        f.render(8_000);
        // Some pixel inside the ring band must be != bg.
        let bg = premul(f.spec.bg);
        let mut hit = false;
        for y in 0..80 {
            for x in 0..80 {
                let i = (y * 80 + x) * 4;
                if &f.pixels[i..i+4] != bg.as_slice() { hit = true; break; }
            }
            if hit { break; }
        }
        assert!(hit, "spinner must alter at least one pixel");
    }

    #[test]
    fn caption_renders_distinct_pixels() {
        let mut f = SkeletonFrame::new(200, 200, SkeletonSpec {
            spinner: false, logo: None,
            caption: Some("LOADING".into()),
            caption_color: Some(Color::rgba8(255, 255, 255, 255)),
            ..SkeletonSpec::default()
        });
        f.render(0);
        // At least 10 pixels must be non-bg (caption rendered).
        let bg = premul(f.spec.bg);
        let count = f.pixels.chunks_exact(4).filter(|c| *c != bg.as_slice()).count();
        assert!(count >= 10, "caption rendered too few pixels: {}", count);
    }
}
