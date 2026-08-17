//! Categorical bars. Hover lights a column; Live/Arm can breathe.

use super::{empty, Play};
use crate::ascii::{density_char, hsl, Cell, CellShader, Coord, Cursor, GridContext};

pub struct Bars<'a> {
    pub values: &'a [f64],
    pub play: Play,
}

impl CellShader for Bars<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.values.is_empty() || ctx.cols < 3 || ctx.rows < 3 {
            return empty();
        }
        if coord.y == 0 || coord.y + 1 == ctx.rows || coord.x == 0 || coord.x + 1 == ctx.cols {
            return empty();
        }
        let n = self.values.len();
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let i = (col * n) / inner_w;
        if i >= n {
            return empty();
        }
        let mut v = self.values[i].max(0.0);
        let t = self.play.motion(ctx.time, cursor.intensity);
        if t > 0.0 {
            v *= 0.82 + 0.18 * (t * 1.7 + i as f64 * 0.6).sin();
        }
        let max = self
            .values
            .iter()
            .copied()
            .fold(1e-6_f64, |a, x| a.max(x.max(0.0)));
        let h = ((v / max) * inner_h as f64).round() as usize;
        let from_bottom = inner_h - 1 - row;
        if from_bottom >= h {
            return empty();
        }
        let hover = cursor.inside && {
            let hi = ((cursor.x - 1.0).max(0.0) as usize * n) / inner_w;
            hi == i
        };
        let den = ((from_bottom + 1) as f64 / h.max(1) as f64).clamp(0.15, 1.0);
        let lit = hover || (self.play.armed(cursor.intensity) && hover);
        let hue = if lit { 48.0 } else { 195.0 };
        Cell {
            ch: if hover { '█' } else { density_char(den) },
            color: hsl(hue, 0.55, 0.38 + 0.28 * den),
            alpha: 1.0,
            scale: 1.0,
        }
    }
}
