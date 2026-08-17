//! Regular grid. Tofu cells; the brush under the cursor rains matrix glyphs.

use super::{empty, fx_matrix, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

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
        let v = self.cells.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let _ = self.play.motion(ctx.time, cursor.intensity);
        let near = cursor.inside && {
            let hx = (cursor.x as usize / cw).min(self.cols.saturating_sub(1));
            let hy = (cursor.y as usize / rh).min(self.rows.saturating_sub(1));
            cx.abs_diff(hx) + cy.abs_diff(hy) <= 1
        };
        if near {
            fx_matrix(i.wrapping_add(coord.index), ctx.time)
        } else {
            tofu(false, 220.0 - 180.0 * v, 0.28 + 0.38 * v)
        }
    }
}
