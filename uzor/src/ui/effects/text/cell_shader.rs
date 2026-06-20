//! ASCII cell-shader engine — a character grid where a per-cell program
//! `main(coord, ctx, cursor) -> Cell` runs every frame, like a GLSL fragment
//! shader applied to text cells.
//!
//! Ported from **ertdfgcvb/play.core** (the engine model: boot/pre/main/post,
//! SDF shapes → character density), with the glitch / hover-radius patterns from
//! **hsrambo07/hover-effects** (sine-glitch + stochastic swaps + cursor mask).
//! See `nemo/docs/mirage/research/pretext-canvas-text-engine.md`.
//!
//! Rendering: `AsciiGrid::step` runs the shader into a flat buffer; `render`
//! draws the buffer through any [`RenderContext`]. Shapes are signed-distance
//! fields (`sd_*`); chars are chosen by density (`density_char`); colour can
//! cycle via [`hsl`]. Any shape expressible as an SDF or a bitmap mask can be
//! filled with characters this way — letters (the menu "M"), the bloom, etc.

use crate::render::{RenderContext, TextAlign, TextBaseline};

/// Per-cell grid position.
#[derive(Clone, Copy)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
    pub index: usize,
}

/// Per-frame grid + timing state handed to the shader.
#[derive(Clone, Copy)]
pub struct GridContext {
    pub frame: u64,
    /// Elapsed time in seconds.
    pub time: f64,
    pub cols: usize,
    pub rows: usize,
    /// Cell aspect = cell_width / cell_height (for circular SDFs).
    pub aspect: f64,
}

/// Pointer state in fractional grid cells + a host-fed interaction intensity.
#[derive(Clone, Copy, Default)]
pub struct Cursor {
    pub x: f64,
    pub y: f64,
    pub pressed: bool,
    pub inside: bool,
    /// 0 = idle, 1 = full interaction (hover eased + click pulse). The host
    /// sets this; shaders read it to drive glitch / colour.
    pub intensity: f64,
}

/// One rendered cell (the shader's output).
#[derive(Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub color: [u8; 3],
    pub alpha: f32,
}

impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ', color: [244, 244, 245], alpha: 1.0 }
    }
}

/// A per-cell program. `main` is invoked once per cell per frame.
pub trait CellShader {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell;
}

/// The grid + buffer that drives a [`CellShader`] and renders it.
pub struct AsciiGrid {
    cols: usize,
    rows: usize,
    buffer: Vec<Cell>,
    frame: u64,
    cursor: Cursor,
}

impl AsciiGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            buffer: vec![Cell::default(); cols * rows],
            frame: 0,
            cursor: Cursor::default(),
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }
    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }

    /// Run the shader over every cell for this frame.
    pub fn step<S: CellShader>(&mut self, shader: &S, time: f64, aspect: f64) {
        self.frame = self.frame.wrapping_add(1);
        let ctx = GridContext {
            frame: self.frame,
            time,
            cols: self.cols,
            rows: self.rows,
            aspect,
        };
        for y in 0..self.rows {
            for x in 0..self.cols {
                let index = y * self.cols + x;
                self.buffer[index] = shader.main(Coord { x, y, index }, &ctx, &self.cursor);
            }
        }
    }

    pub fn cell(&self, x: usize, y: usize) -> Cell {
        self.buffer[y * self.cols + x]
    }

    /// Draw the buffer into a `cols*cell_w × rows*cell_h` box at `(ox, oy)`.
    pub fn render(&self, ctx: &mut dyn RenderContext, ox: f64, oy: f64, cell_w: f64, cell_h: f64) {
        let fs = (cell_h + 1.0).max(5.0);
        ctx.set_font(&format!("{}px monospace", fs as i32));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        for y in 0..self.rows {
            for x in 0..self.cols {
                let c = self.cell(x, y);
                if c.ch == ' ' || c.alpha <= 0.01 {
                    continue;
                }
                ctx.set_global_alpha(c.alpha as f64);
                ctx.set_fill_color(&format!("rgb({},{},{})", c.color[0], c.color[1], c.color[2]));
                ctx.fill_text(&c.ch.to_string(), ox + x as f64 * cell_w, oy + y as f64 * cell_h);
            }
        }
        ctx.set_global_alpha(1.0);
    }
}

// ── SDF helpers (ports of play.core's sdf module) ───────────────────────────────

/// Signed distance to a circle of radius `r` centred at origin.
pub fn sd_circle(px: f64, py: f64, r: f64) -> f64 {
    (px * px + py * py).sqrt() - r
}

/// Signed distance to an axis-aligned box of half-extents `(bx, by)`.
pub fn sd_box(px: f64, py: f64, bx: f64, by: f64) -> f64 {
    let dx = px.abs() - bx;
    let dy = py.abs() - by;
    let ox = dx.max(0.0);
    let oy = dy.max(0.0);
    (ox * ox + oy * oy).sqrt() + dx.max(dy).min(0.0)
}

/// Density ramp, sparse → dense. Index by a 0..1 value.
pub const DENSITY: &[u8] = b" .:-=+*oOX#%@";

/// Pick a density character for a 0..1 value (0 = sparse, 1 = dense).
pub fn density_char(v: f64) -> char {
    let n = DENSITY.len();
    let i = ((v.clamp(0.0, 1.0)) * (n - 1) as f64).round() as usize;
    DENSITY[i.min(n - 1)] as char
}

// ── HSL → RGB (for iridescent cycling) ──────────────────────────────────────────

/// `h` in degrees, `s`/`l` in 0..1.
pub fn hsl(h: f64, s: f64, l: f64) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = ((h % 360.0) + 360.0) % 360.0 / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        (((r1 + m) * 255.0).clamp(0.0, 255.0)) as u8,
        (((g1 + m) * 255.0).clamp(0.0, 255.0)) as u8,
        (((b1 + m) * 255.0).clamp(0.0, 255.0)) as u8,
    ]
}

// ── GlitchLetter — a ready CellShader: an ASCII-art letter that glitches ─────────

/// A bitmap-mask glyph rendered through the cell-shader: on-cells show a steady
/// char at rest; under `cursor.intensity` they swap to glitch chars, spark, and
/// cycle an iridescent hue. `letter_m()` is a 5×7 "M".
pub struct GlitchLetter {
    pub cols: usize,
    pub rows: usize,
    mask: Vec<bool>,
    rest: char,
    glitch_chars: Vec<char>,
    base: [u8; 3],
}

impl GlitchLetter {
    pub fn from_rows(rows_src: &[&str], rest: char, base: [u8; 3]) -> Self {
        let rows = rows_src.len();
        let cols = rows_src.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut mask = vec![false; cols * rows];
        for (y, line) in rows_src.iter().enumerate() {
            for (x, c) in line.chars().enumerate() {
                mask[y * cols + x] = c == 'X';
            }
        }
        Self {
            cols,
            rows,
            mask,
            rest,
            glitch_chars: "#@%&$*/\\|=+<>?".chars().collect(),
            base,
        }
    }

    /// 5×7 capital "M" in amber.
    pub fn letter_m() -> Self {
        Self::from_rows(
            &["X...X", "XX.XX", "X.X.X", "X.X.X", "X...X", "X...X", "X...X"],
            '#',
            [251, 178, 106],
        )
    }
}

impl CellShader for GlitchLetter {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        let on = self.mask.get(coord.index).copied().unwrap_or(false);
        let it = cursor.intensity.clamp(0.0, 1.0);

        // A few off-cells spark on at higher intensity.
        let spark = !on && it > 0.15 && fastrand::f64() < 0.02 * it;
        if !on && !spark {
            return Cell { ch: ' ', color: self.base, alpha: 1.0 };
        }

        // Glyph: steady rest char, or a glitch swap with probability ∝ intensity.
        let swap = fastrand::f64() < 0.55 * it;
        let ch = if swap && !self.glitch_chars.is_empty() {
            self.glitch_chars[fastrand::usize(0..self.glitch_chars.len())]
        } else {
            self.rest
        };

        // Colour: amber at rest; iridescent hue cycle while glitching.
        let color = if it <= 0.001 {
            self.base
        } else {
            let hue = (ctx.time * 120.0 + coord.index as f64 * 16.0 + it * 200.0) % 360.0;
            let cyc = hsl(hue, 0.85, 0.62);
            let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * it) as u8;
            [mix(self.base[0], cyc[0]), mix(self.base[1], cyc[1]), mix(self.base[2], cyc[2])]
        };

        Cell { ch, color, alpha: if spark { 0.6 } else { 1.0 } }
    }
}
