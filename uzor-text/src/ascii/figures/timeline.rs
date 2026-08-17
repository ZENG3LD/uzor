//! Lane ribbon. Live walks a playhead; hover lights an event.

use super::{empty, Play};
use crate::ascii::{hsl, Cell, CellShader, Coord, Cursor, GridContext};

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
        let inner_w = ctx.cols.saturating_sub(2).max(1) as f64;
        let t = self.play.motion(ctx.time, cursor.intensity);
        if coord.y == mid && coord.x > 0 && coord.x + 1 < ctx.cols {
            let u = (coord.x - 1) as f64 / (inner_w - 1.0).max(1.0);
            let head = if t > 0.0 { (t * 0.12).rem_euclid(1.0) } else { -1.0 };
            if (u - head).abs() < 0.03 {
                return Cell {
                    ch: '│',
                    color: hsl(50.0, 0.7, 0.6),
                    alpha: 1.0,
                    scale: 1.0,
                };
            }
        }
        for (i, e) in self.events.iter().enumerate() {
            if e.lane as usize != lane {
                continue;
            }
            let x0 = 1 + (e.t0.clamp(0.0, 1.0) * (inner_w - 1.0)) as usize;
            let x1 = 1 + (e.t1.clamp(0.0, 1.0) * (inner_w - 1.0)) as usize;
            let point = (e.t1 - e.t0).abs() < 1e-6;
            if coord.y != mid {
                continue;
            }
            let on = if point {
                coord.x == x0
            } else {
                coord.x >= x0.min(x1) && coord.x <= x0.max(x1)
            };
            if !on {
                continue;
            }
            let hover = cursor.inside
                && (cursor.y as usize) == mid
                && (cursor.x as usize) >= x0.min(x1)
                && (cursor.x as usize) <= x0.max(x1).max(x0);
            let lab_i = coord.x.saturating_sub(x0);
            let ch = if point {
                '●'
            } else if hover {
                e.label.chars().nth(lab_i).unwrap_or('─')
            } else {
                '─'
            };
            return Cell {
                ch,
                color: hsl(if hover { 48.0 } else { 25.0 + i as f64 * 18.0 }, 0.55, 0.5),
                alpha: 1.0,
                scale: 1.0,
            };
        }
        if coord.y == mid && coord.x > 0 && coord.x + 1 < ctx.cols {
            return Cell {
                ch: '·',
                color: hsl(220.0, 0.1, 0.22),
                alpha: 1.0,
                scale: 1.0,
            };
        }
        empty()
    }
}
