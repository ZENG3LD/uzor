//! Lane ribbon. Interval tofu, point tofu. Hover recolors an event.

use super::{empty, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

#[derive(Clone, Copy, Debug)]
pub struct TlEvent {
    pub lane: u8,
    pub t0: f64,
    pub t1: f64,
    pub label: &'static str,
}

pub struct Timeline<'a> {
    pub lanes: usize,
    pub events: &'a [TlEvent],
    pub play: Play,
}

impl CellShader for Timeline<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        let lanes = self.lanes.max(1);
        if ctx.cols < 8 || ctx.rows < lanes + 2 {
            return empty();
        }
        let lane_h = (ctx.rows.saturating_sub(1)).max(1) / lanes;
        if lane_h == 0 || coord.y == 0 {
            return empty();
        }
        let lane = (coord.y - 1) / lane_h;
        if lane >= lanes {
            return empty();
        }
        let mid = 1 + lane * lane_h + lane_h / 2;
        if coord.y != mid || coord.x == 0 || coord.x + 1 >= ctx.cols {
            return empty();
        }
        let inner_w = ctx.cols.saturating_sub(2).max(1) as f64;
        let _ = self.play.motion(ctx.time, cursor.intensity);
        for (i, e) in self.events.iter().enumerate() {
            if e.lane as usize != lane {
                continue;
            }
            let x0 = 1 + (e.t0.clamp(0.0, 1.0) * (inner_w - 1.0)) as usize;
            let x1 = 1 + (e.t1.clamp(0.0, 1.0) * (inner_w - 1.0)) as usize;
            let lo = x0.min(x1);
            let hi = x0.max(x1);
            if coord.x < lo || coord.x > hi {
                continue;
            }
            let hover = cursor.inside
                && (cursor.y as usize) == mid
                && (cursor.x as usize) >= lo
                && (cursor.x as usize) <= hi;
            return tofu(hover, 25.0 + i as f64 * 18.0, 0.48);
        }
        empty()
    }
}
