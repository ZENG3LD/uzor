//! Polyline. One stroke family (─ / \ │). Hover recolors the column.

use super::{empty, ink, line_glyph, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

pub struct Curve<'a> {
    pub ys: &'a [f64],
    pub play: Play,
}

impl CellShader for Curve<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.ys.len() < 2 || ctx.cols < 4 || ctx.rows < 3 {
            return empty();
        }
        if let Some(frame) = super::plot_frame(coord, ctx) {
            return frame;
        }
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let n = self.ys.len();
        let (lo, hi) = min_max(self.ys);
        let span = (hi - lo).max(1e-6);
        let y_at = |i: usize| -> isize {
            let u = ((self.ys[i] - lo) / span).clamp(0.0, 1.0);
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
        let ch = line_glyph(1, (y1 - y0) as i32);
        let hover = cursor.inside && (cursor.x - coord.x as f64).abs() < 1.2;
        let t = self.play.motion(ctx.time, cursor.intensity);
        let lit = if t > 0.0 { 0.48 + 0.08 * (t * 1.4).sin() } else { 0.50 };
        ink(ch, hover, 168.0, lit)
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
