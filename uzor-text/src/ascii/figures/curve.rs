//! Polyline. Live sweeps a bead; Arm wakes the sweep on hover.

use super::{empty, Play};
use crate::ascii::{hsl, Cell, CellShader, Coord, Cursor, GridContext};

pub struct Curve<'a> {
    pub ys: &'a [f64],
    pub play: Play,
}

impl CellShader for Curve<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.ys.len() < 2 || ctx.cols < 4 || ctx.rows < 3 {
            return empty();
        }
        if coord.y == 0 || coord.y + 1 == ctx.rows || coord.x == 0 || coord.x + 1 == ctx.cols {
            return empty();
        }
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let n = self.ys.len();
        let t = self.play.motion(ctx.time, cursor.intensity);
        let (lo, hi) = min_max(self.ys);
        let span = (hi - lo).max(1e-6);
        let y_at = |i: usize| -> isize {
            let mut y = self.ys[i];
            if t > 0.0 {
                y += 0.08 * span * (t * 2.0 + i as f64 * 0.3).sin();
            }
            let u = ((y - lo) / span).clamp(0.0, 1.0);
            ((1.0 - u) * (inner_h.saturating_sub(1) as f64)).round() as isize
        };
        let i0 = (col * (n - 1)) / inner_w;
        let i1 = (i0 + 1).min(n - 1);
        let y0 = y_at(i0);
        let y1 = y_at(i1);
        let y = row as isize;
        let on = y == y0 || y == y1 || (y > y0.min(y1) && y < y0.max(y1));
        if !on {
            return empty();
        }
        let sweep = if t > 0.0 {
            let head = ((t * 0.35).rem_euclid(1.0) * inner_w as f64) as usize;
            col.abs_diff(head) < 2
        } else {
            false
        };
        let near = cursor.inside && (cursor.x - coord.x as f64).abs() < 1.2;
        Cell {
            ch: if sweep || near { '●' } else { '─' },
            color: hsl(if sweep { 50.0 } else { 168.0 }, 0.6, 0.5),
            alpha: 1.0,
            scale: 1.0,
        }
    }
}

fn min_max(ys: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &y in ys {
        lo = lo.min(y);
        hi = hi.max(y);
    }
    if !lo.is_finite() {
        (0.0, 1.0)
    } else {
        (lo, hi)
    }
}
