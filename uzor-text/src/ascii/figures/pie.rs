//! Part-to-whole donut. Tofu wedges; hover recolors a slice.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::{empty, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

pub struct Pie<'a> {
    pub slices: &'a [f64],
    pub play: Play,
}

impl CellShader for Pie<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.slices.is_empty() || ctx.cols < 6 || ctx.rows < 5 {
            return empty();
        }
        let cx = ctx.cols as f64 / 2.0;
        let cy = ctx.rows as f64 / 2.0;
        let dx = (coord.x as f64 + 0.5 - cx) * ctx.aspect;
        let dy = coord.y as f64 + 0.5 - cy;
        let r = (dx * dx + dy * dy).sqrt();
        let rmax = (ctx.cols.min(ctx.rows) as f64) / 2.0 - 0.6;
        let rmin = rmax * 0.38;
        if r > rmax || r < rmin {
            return empty();
        }
        let total: f64 = self.slices.iter().copied().filter(|v| *v > 0.0).sum();
        if total <= 0.0 {
            return empty();
        }
        let _ = self.play.motion(ctx.time, cursor.intensity);
        let ang = (dy.atan2(dx) + FRAC_PI_2).rem_euclid(TAU);
        let idx = slice_at(self.slices, total, ang);
        let hover = if cursor.inside {
            let cdx = (cursor.x - cx) * ctx.aspect;
            let cdy = cursor.y - cy;
            let cr = (cdx * cdx + cdy * cdy).sqrt();
            cr >= rmin
                && cr <= rmax
                && slice_at(self.slices, total, (cdy.atan2(cdx) + FRAC_PI_2).rem_euclid(TAU)) == idx
        } else {
            false
        };
        tofu(hover, 30.0 + idx as f64 * 48.0, 0.40)
    }
}

fn slice_at(slices: &[f64], total: f64, ang: f64) -> usize {
    let mut acc = 0.0;
    let mut idx = 0;
    for (i, &v) in slices.iter().enumerate() {
        if v <= 0.0 {
            continue;
        }
        acc += (v / total) * TAU;
        if ang <= acc {
            return i;
        }
        idx = i;
    }
    idx
}
