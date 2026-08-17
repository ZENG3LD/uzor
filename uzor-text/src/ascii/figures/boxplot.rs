//! Tukey box. Tofu body, line whiskers. Hover recolors the group.

use super::{empty, ink, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

#[derive(Clone, Copy, Debug)]
pub struct BoxStat {
    pub lo: f64,
    pub q1: f64,
    pub med: f64,
    pub q3: f64,
    pub hi: f64,
    pub outliers: &'static [f64],
}

pub struct Boxplot<'a> {
    pub groups: &'a [BoxStat],
    pub play: Play,
}

impl CellShader for Boxplot<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.groups.is_empty() || ctx.cols < 6 || ctx.rows < 5 {
            return empty();
        }
        if coord.y == 0 || coord.y + 1 == ctx.rows || coord.x == 0 || coord.x + 1 == ctx.cols {
            return empty();
        }
        let n = self.groups.len();
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let i = (col * n) / inner_w;
        if i >= n {
            return empty();
        }
        let g = self.groups[i];
        let (lo, hi) = group_span(self.groups);
        let span = (hi - lo).max(1e-6);
        let to_row = |v: f64| -> isize {
            let u = ((v - lo) / span).clamp(0.0, 1.0);
            ((1.0 - u) * (inner_h.saturating_sub(1) as f64)).round() as isize
        };
        let y = row as isize;
        let hover = cursor.inside && {
            let hi = ((cursor.x - 1.0).max(0.0) as usize * n) / inner_w;
            hi == i
        };
        let _ = self.play.motion(ctx.time, cursor.intensity);
        let r_lo = to_row(g.lo);
        let r_hi = to_row(g.hi);
        let r_q1 = to_row(g.q1);
        let r_q3 = to_row(g.q3);
        let r_med = to_row(g.med);
        let box_lo = r_q1.min(r_q3);
        let box_hi = r_q1.max(r_q3);
        if y == r_med {
            return ink('─', hover, 50.0, 0.58);
        }
        if y >= box_lo && y <= box_hi {
            return tofu(hover, 200.0, 0.42);
        }
        let wlo = r_lo.min(r_hi);
        let whi = r_lo.max(r_hi);
        if y == wlo || y == whi {
            return ink('─', hover, 200.0, 0.40);
        }
        if y > wlo && y < whi {
            return ink('│', hover, 200.0, 0.40);
        }
        for &o in g.outliers {
            if y == to_row(o) {
                return tofu(hover, 8.0, 0.50);
            }
        }
        empty()
    }
}

fn group_span(groups: &[BoxStat]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for g in groups {
        lo = lo.min(g.lo);
        hi = hi.max(g.hi);
        for &o in g.outliers {
            lo = lo.min(o);
            hi = hi.max(o);
        }
    }
    if !lo.is_finite() {
        (0.0, 1.0)
    } else {
        (lo, hi)
    }
}
