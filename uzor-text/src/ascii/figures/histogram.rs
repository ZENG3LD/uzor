//! Equal-width bins. Tofu columns; hover recolors a bin.

use super::{empty, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

pub struct Histogram<'a> {
    pub samples: &'a [f64],
    pub bins: usize,
    pub play: Play,
}

impl CellShader for Histogram<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        let n = self.bins.max(1).min(16);
        let counts = bin_counts(self.samples, n);
        if ctx.cols < 3 || ctx.rows < 3 {
            return empty();
        }
        if coord.y == 0 || coord.y + 1 == ctx.rows || coord.x == 0 || coord.x + 1 == ctx.cols {
            return empty();
        }
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let i = (col * n) / inner_w;
        if i >= n {
            return empty();
        }
        let v = counts[i];
        let max = counts.iter().copied().fold(1e-6_f64, f64::max);
        let h = ((v / max) * inner_h as f64).round() as usize;
        let from_bottom = inner_h - 1 - row;
        if from_bottom >= h {
            return empty();
        }
        let hover = cursor.inside && {
            let hi = ((cursor.x - 1.0).max(0.0) as usize * n) / inner_w;
            hi == i
        };
        let _ = self.play.motion(ctx.time, cursor.intensity);
        tofu(hover, 210.0, 0.42)
    }
}

fn bin_counts(samples: &[f64], n: usize) -> [f64; 16] {
    let mut out = [0.0; 16];
    if samples.is_empty() || n == 0 {
        return out;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &s in samples {
        if s.is_finite() {
            lo = lo.min(s);
            hi = hi.max(s);
        }
    }
    if !lo.is_finite() {
        return out;
    }
    let span = (hi - lo).max(1e-6);
    for &s in samples {
        if !s.is_finite() {
            continue;
        }
        let i = (((s - lo) / span) * n as f64).floor() as usize;
        out[i.min(n - 1)] += 1.0;
    }
    out
}
