//! Glitch glyph effect — an ASCII-art letter/grid that shimmers and swaps
//! characters under an interaction intensity (hover / click).
//!
//! Rendering-agnostic, like the rest of `ui::effects::text`: `update` advances
//! the per-cell state from `(dt, intensity)`, and the caller reads `cell(col,
//! row)` to draw each glyph with its own `fill_text`. A glyph is laid out on a
//! `cols × rows` grid; only mask cells are "on". At rest the on-cells show a
//! steady glyph in the base colour; as intensity rises, cells randomly swap to
//! glitch characters, flicker, and cycle through an iridescent palette, and a
//! few off-cells briefly spark on.
//!
//! `GlitchGlyph::letter_m()` is a ready 5×7 "M" for the menu button.

/// Per-cell output the caller renders.
#[derive(Clone, Copy, Debug)]
pub struct GlitchCell {
    pub on: bool,
    pub ch: char,
    pub color: [u8; 3],
}

/// Visual configuration.
#[derive(Clone, Debug)]
pub struct GlitchStyle {
    /// Glyph shown by an on-cell at rest.
    pub rest_glyph: char,
    /// Pool of characters swapped in while glitching.
    pub glitch_chars: Vec<char>,
    /// Resting colour (RGB).
    pub base: [u8; 3],
    /// Iridescent palette cycled through while glitching.
    pub palette: Vec<[u8; 3]>,
}

impl Default for GlitchStyle {
    fn default() -> Self {
        Self {
            rest_glyph: '#',
            glitch_chars: "#@%&$*/\\|=+<>?▓▒░".chars().collect(),
            base: [251, 178, 106], // amber
            palette: vec![
                [251, 178, 106], // amber
                [255, 217, 176], // amber-hi
                [127, 214, 160], // green
                [224, 86, 86],   // red
                [180, 160, 255], // violet
            ],
        }
    }
}

/// An ASCII-art glyph that glitches under interaction intensity.
pub struct GlitchGlyph {
    cols: usize,
    rows: usize,
    mask: Vec<bool>,
    out: Vec<GlitchCell>,
    t: f64,
}

impl GlitchGlyph {
    /// Build from a row-major boolean mask (`rows × cols`, `true` = on-cell).
    pub fn from_mask(cols: usize, rows: usize, mask: Vec<bool>) -> Self {
        debug_assert_eq!(mask.len(), cols * rows);
        let out = vec![
            GlitchCell { on: false, ch: ' ', color: [0, 0, 0] };
            cols * rows
        ];
        Self { cols, rows, mask, out, t: 0.0 }
    }

    /// 5×7 capital "M".
    pub fn letter_m() -> Self {
        // X . . . X
        // X X . X X
        // X . X . X
        // X . X . X
        // X . . . X
        // X . . . X
        // X . . . X
        let rows_src = [
            "X...X",
            "XX.XX",
            "X.X.X",
            "X.X.X",
            "X...X",
            "X...X",
            "X...X",
        ];
        let cols = 5;
        let rows = rows_src.len();
        let mask: Vec<bool> = rows_src
            .iter()
            .flat_map(|r| r.chars().map(|c| c == 'X'))
            .collect();
        Self::from_mask(cols, rows, mask)
    }

    pub fn cols(&self) -> usize {
        self.cols
    }
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Advance the per-cell state. `intensity` in `[0, 1]` (0 = steady, 1 =
    /// full glitch). Deterministic enough but uses `fastrand` for the swaps.
    pub fn update(&mut self, dt: f64, intensity: f64, style: &GlitchStyle) {
        self.t += dt;
        let it = intensity.clamp(0.0, 1.0);
        let pal = if style.palette.is_empty() { &[style.base][..] } else { &style.palette[..] };

        for idx in 0..self.mask.len() {
            let on_mask = self.mask[idx];

            // A few off-cells spark on at high intensity.
            let spark = !on_mask && it > 0.15 && fastrand::f64() < 0.02 * it;
            let on = on_mask || spark;
            if !on {
                self.out[idx] = GlitchCell { on: false, ch: ' ', color: [0, 0, 0] };
                continue;
            }

            // Glyph: rest glyph, or a glitch swap with probability ∝ intensity.
            let swap = fastrand::f64() < 0.55 * it;
            let ch = if swap && !style.glitch_chars.is_empty() {
                style.glitch_chars[fastrand::usize(0..style.glitch_chars.len())]
            } else {
                style.rest_glyph
            };

            // Colour: amber at rest; iridescent cycle while glitching.
            let color = if it <= 0.001 {
                style.base
            } else {
                // Per-cell phase through the palette, time-driven.
                let phase = self.t * 6.0 + idx as f64 * 0.7;
                let n = pal.len() as f64;
                let f = (phase % n + n) % n;
                let a = f.floor() as usize % pal.len();
                let b = (a + 1) % pal.len();
                let m = f - f.floor();
                let lerp = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * m) as u8;
                let cyc = [
                    lerp(pal[a][0], pal[b][0]),
                    lerp(pal[a][1], pal[b][1]),
                    lerp(pal[a][2], pal[b][2]),
                ];
                // Blend base→cycled by intensity.
                let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * it) as u8;
                [mix(style.base[0], cyc[0]), mix(style.base[1], cyc[1]), mix(style.base[2], cyc[2])]
            };

            self.out[idx] = GlitchCell { on: true, ch, color };
        }
    }

    /// Read a cell (col, row).
    pub fn cell(&self, col: usize, row: usize) -> GlitchCell {
        self.out[row * self.cols + col]
    }
}
