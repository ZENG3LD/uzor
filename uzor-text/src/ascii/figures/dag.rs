//! Layered node/edge sketch. Live sends a pulse down each edge.

use super::{empty, Play};
use crate::ascii::{hsl, Cell, CellShader, Coord, Cursor, GridContext};

#[derive(Clone, Copy, Debug)]
pub struct DagNode {
    pub id: u8,
    pub label: &'static str,
    pub layer: u8,
    pub slot: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct DagEdge {
    pub from: u8,
    pub to: u8,
}

pub struct Dag<'a> {
    pub nodes: &'a [DagNode],
    pub edges: &'a [DagEdge],
    pub play: Play,
}

impl CellShader for Dag<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.nodes.is_empty() || ctx.cols < 8 || ctx.rows < 4 {
            return empty();
        }
        let layers = self.nodes.iter().map(|n| n.layer).max().unwrap_or(0) as usize + 1;
        let slots = self.nodes.iter().map(|n| n.slot).max().unwrap_or(0) as usize + 1;
        let x_at = |layer: u8| -> usize {
            let span = ctx.cols.saturating_sub(6).max(1);
            3 + (layer as usize * span) / layers.max(1)
        };
        let y_at = |slot: u8| -> usize {
            let span = ctx.rows.saturating_sub(2).max(1);
            1 + (slot as usize * span) / slots.max(1)
        };
        if let Some(n) = self.nodes.iter().find(|n| {
            let x = x_at(n.layer);
            let y = y_at(n.slot);
            coord.y == y && coord.x >= x && coord.x < x + n.label.len().min(6)
        }) {
            let hover = cursor.inside
                && (cursor.y as usize) == y_at(n.slot)
                && (cursor.x as usize) >= x_at(n.layer);
            let i = (coord.x - x_at(n.layer)) as usize;
            let ch = n.label.chars().nth(i).unwrap_or('·');
            return Cell {
                ch,
                color: hsl(if hover { 48.0 } else { 130.0 }, 0.55, 0.58),
                alpha: 1.0,
                scale: 1.0,
            };
        }
        let t = self.play.motion(ctx.time, cursor.intensity);
        for e in self.edges {
            let Some(a) = self.nodes.iter().find(|n| n.id == e.from) else {
                continue;
            };
            let Some(b) = self.nodes.iter().find(|n| n.id == e.to) else {
                continue;
            };
            let x0 = x_at(a.layer) + a.label.len().min(6);
            let x1 = x_at(b.layer).saturating_sub(1);
            let y0 = y_at(a.slot);
            let y1 = y_at(b.slot);
            if x1 <= x0 {
                continue;
            }
            let on_h = coord.y == y0 && coord.x > x0 && coord.x < x1 && y0 == y1;
            let on_bend = y0 != y1
                && ((coord.x == x0 && between(coord.y, y0, y1))
                    || (coord.y == y1 && coord.x > x0 && coord.x < x1));
            if !on_h && !on_bend {
                continue;
            }
            let along = if on_h {
                (coord.x - x0) as f64 / (x1 - x0) as f64
            } else if coord.x == x0 {
                0.3
            } else {
                0.7
            };
            let pulse = t > 0.0 && ((along - (t * 0.4).rem_euclid(1.0)).abs() < 0.08);
            return Cell {
                ch: if pulse {
                    '●'
                } else if on_h || coord.y == y1 {
                    '─'
                } else {
                    '│'
                },
                color: hsl(if pulse { 50.0 } else { 200.0 }, 0.45, 0.45),
                alpha: 1.0,
                scale: 1.0,
            };
        }
        empty()
    }
}

fn between(v: usize, a: usize, b: usize) -> bool {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    v > lo && v < hi
}
