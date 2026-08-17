//! Point cloud. Arm twinkles the nearest point.

use super::{empty, Play};
use crate::ascii::{hsl, Cell, CellShader, Coord, Cursor, GridContext};

pub struct Scatter<'a> {
    pub pts: &'a [(f64, f64)],
    pub play: Play,
}

impl CellShader for Scatter<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.pts.len() < 2 || ctx.cols < 6 || ctx.rows < 4 {
            return empty();
        }
        if coord.y == 0 || coord.y + 1 == ctx.rows || coord.x == 0 || coord.x + 1 == ctx.cols {
            return empty();
        }
        let (xmin, xmax, ymin, ymax) = bounds(self.pts);
        let iw = ctx.cols.saturating_sub(2).max(1) as f64;
        let ih = ctx.rows.saturating_sub(2).max(1) as f64;
        let t = self.play.motion(ctx.time, cursor.intensity);
        let mut hit = None;
        let mut best = f64::INFINITY;
        for (i, &(x, y)) in self.pts.iter().enumerate() {
            let px = 1.0 + ((x - xmin) / (xmax - xmin).max(1e-6)) * (iw - 1.0);
            let py = 1.0 + (1.0 - (y - ymin) / (ymax - ymin).max(1e-6)) * (ih - 1.0);
            let d = (coord.x as f64 - px).abs() + (coord.y as f64 - py).abs();
            if d < 0.65 && d < best {
                best = d;
                hit = Some(i);
            }
        }
        let Some(i) = hit else {
            return empty();
        };
        let near = cursor.inside
            && (cursor.x - coord.x as f64).abs() < 1.4
            && (cursor.y - coord.y as f64).abs() < 1.4;
        let twinkle = t > 0.0 && ((t * 3.0 + i as f64).sin() > 0.55);
        Cell {
            ch: if near || twinkle { '●' } else { '·' },
            color: hsl(if near { 48.0 } else { 175.0 }, 0.55, 0.52),
            alpha: 1.0,
            scale: if near { 1.2 } else { 1.0 },
        }
    }
}

fn bounds(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut xmin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for &(x, y) in pts {
        xmin = xmin.min(x);
        xmax = xmax.max(x);
        ymin = ymin.min(y);
        ymax = ymax.max(y);
    }
    if !xmin.is_finite() {
        (0.0, 1.0, 0.0, 1.0)
    } else {
        (xmin, xmax, ymin, ymax)
    }
}
