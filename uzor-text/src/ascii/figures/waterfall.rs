//! Running-total bridge. Tofu bars; hover recolors a column.

use super::{empty, ink, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WfKind {
    Delta,
    Total,
}

#[derive(Clone, Copy, Debug)]
pub struct WfItem {
    pub value: f64,
    pub kind: WfKind,
}

pub struct Waterfall<'a> {
    pub items: &'a [WfItem],
    pub play: Play,
}

impl CellShader for Waterfall<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.items.is_empty() || ctx.cols < 6 || ctx.rows < 4 {
            return empty();
        }
        if coord.y == 0 || coord.y + 1 == ctx.rows || coord.x == 0 || coord.x + 1 == ctx.cols {
            return empty();
        }
        let n = self.items.len();
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let i = (col * n) / inner_w;
        if i >= n {
            return empty();
        }
        let (lo, hi, y0, y1, pos) = extents(self.items, i);
        let span = (hi - lo).max(1e-6);
        let to_row = |y: f64| -> isize {
            let u = ((y - lo) / span).clamp(0.0, 1.0);
            ((1.0 - u) * (inner_h.saturating_sub(1) as f64)).round() as isize
        };
        let mut a = to_row(y0);
        let mut b = to_row(y1);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let y = row as isize;
        let _ = self.play.motion(ctx.time, cursor.intensity);
        let on_bar = y >= a && y <= b;
        let next_level = to_row(y1);
        let on_link = i + 1 < n && y == next_level && {
            let x_end = ((i + 1) * inner_w) / n;
            col + 1 == x_end || col == x_end
        };
        if !on_bar && !on_link {
            return empty();
        }
        let hover = cursor.inside && {
            let hi = ((cursor.x - 1.0).max(0.0) as usize * n) / inner_w;
            hi == i
        };
        if on_link && !on_bar {
            return ink('─', hover, 200.0, 0.40);
        }
        let hue = if matches!(self.items[i].kind, WfKind::Total) {
            220.0
        } else if pos {
            145.0
        } else {
            8.0
        };
        tofu(hover, hue, 0.42)
    }
}

fn extents(items: &[WfItem], i: usize) -> (f64, f64, f64, f64, bool) {
    let mut run = 0.0_f64;
    let mut lo = 0.0_f64;
    let mut hi = 0.0_f64;
    let mut y0 = 0.0;
    let mut y1 = 0.0;
    let mut pos = true;
    for (k, it) in items.iter().enumerate() {
        let start = match it.kind {
            WfKind::Total => 0.0,
            WfKind::Delta => run,
        };
        let end = match it.kind {
            WfKind::Total => {
                if it.value.abs() < 1e-9 {
                    run
                } else {
                    it.value
                }
            }
            WfKind::Delta => run + it.value,
        };
        lo = lo.min(start).min(end);
        hi = hi.max(start).max(end);
        if k == i {
            y0 = start;
            y1 = end;
            pos = end >= start;
        }
        run = end;
    }
    (lo, hi, y0, y1, pos)
}
