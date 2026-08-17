//! Point cloud. Tofu dots; hover recolors the nearest.

use super::{empty, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

pub struct Scatter<'a> {
    pub pts: &'a [(f64, f64)],
    pub play: Play,
}

impl CellShader for Scatter<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.pts.len() < 2 || ctx.cols < 6 || ctx.rows < 4 {
            return empty();
        }
        if let Some(frame) = super::plot_frame(coord, ctx) {
            return frame;
        }
        let (xmin, xmax, ymin, ymax) = bounds(self.pts);
        let iw = ctx.cols.saturating_sub(2).max(1) as f64;
        let ih = ctx.rows.saturating_sub(2).max(1) as f64;
        let _ = self.play.motion(ctx.time, cursor.intensity);
        let mut hit = false;
        let mut best = f64::INFINITY;
        for &(x, y) in self.pts {
            let px = 1.0 + ((x - xmin) / (xmax - xmin).max(1e-6)) * (iw - 1.0);
            let py = 1.0 + (1.0 - (y - ymin) / (ymax - ymin).max(1e-6)) * (ih - 1.0);
            let d = (coord.x as f64 - px).abs() + (coord.y as f64 - py).abs();
            if d < 0.65 && d < best {
                best = d;
                hit = true;
            }
        }
        if !hit {
            return empty();
        }
        let hover = cursor.inside
            && (cursor.x - coord.x as f64).abs() < 1.4
            && (cursor.y - coord.y as f64).abs() < 1.4;
        if hover {
            let d = (cursor.x - coord.x as f64).hypot(cursor.y - coord.y as f64);
            tofu(false, 175.0, super::fx_ripple(d, ctx.time))
        } else {
            tofu(false, 175.0, 0.50)
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
