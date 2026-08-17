//! Regular grid. Arm pops the cell under the cursor.

use super::{empty, Play};
use crate::ascii::{density_char, hsl, Cell, CellShader, Coord, Cursor, GridContext};

pub struct Heatmap<'a> {
    pub cols: usize,
    pub rows: usize,
    pub cells: &'a [f64],
    pub play: Play,
}

impl CellShader for Heatmap<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.cols == 0 || self.rows == 0 || self.cells.is_empty() {
            return empty();
        }
        let cw = (ctx.cols / self.cols).max(1);
        let rh = (ctx.rows / self.rows).max(1);
        let cx = coord.x / cw;
        let cy = coord.y / rh;
        if cx >= self.cols || cy >= self.rows {
            return empty();
        }
        let i = cy * self.cols + cx;
        let mut v = self.cells.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let t = self.play.motion(ctx.time, cursor.intensity);
        if t > 0.0 {
            v = (v + 0.08 * (t * 1.4 + i as f64 * 0.4).sin()).clamp(0.0, 1.0);
        }
        let hover = cursor.inside
            && (cursor.x as usize / cw) == cx
            && (cursor.y as usize / rh) == cy;
        Cell {
            ch: if hover { '◆' } else { density_char(0.2 + 0.8 * v) },
            color: hsl(220.0 - 180.0 * v, 0.62, 0.28 + 0.4 * v),
            alpha: 1.0,
            scale: if hover { 1.2 } else { 1.0 },
        }
    }
}
