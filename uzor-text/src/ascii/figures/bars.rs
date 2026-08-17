//! Categorical bars. One glyph: tofu. Hover recolors the column.

use super::{empty, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

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
        let v = self.values[i].max(0.0);
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
        let _ = self.play.motion(ctx.time, cursor.intensity);
        tofu(hover, 195.0, 0.42)
    }
}
