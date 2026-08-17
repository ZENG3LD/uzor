//! Equal-width bins. Idle tofu. The hovered bin becomes a plasma field.

use super::{empty, fx_plasma, plot_frame, tofu, Play};
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
        if let Some(frame) = plot_frame(coord, ctx) {
            return frame;
        }
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let i = (col * n) / inner_w;
        if i >= n {
            return empty();
        }
        let x0 = (i * inner_w) / n;
        let x1 = ((i + 1) * inner_w) / n;
        let gap = if x1.saturating_sub(x0) >= 3 { 1 } else { 0 };
        if col < x0 || col >= x1.saturating_sub(gap) {
            return empty();
        }
        let v = counts[i];
        let max = counts.iter().copied().fold(1e-6_f64, f64::max);
        let h = ((v / max) * inner_h as f64).round() as usize;
        let from_bottom = inner_h - 1 - row;
        if from_bottom >= h {
            return empty();
        }
        let hue = 200.0 - i as f64 * (140.0 / n.max(1) as f64);
        let _ = self.play;
        if super::in_brush(coord, cursor, ctx.aspect, super::BRUSH) {
            fx_plasma(coord.x as f64, coord.y as f64, ctx.time)
        } else {
            tofu(false, hue, 0.42)
        }
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
