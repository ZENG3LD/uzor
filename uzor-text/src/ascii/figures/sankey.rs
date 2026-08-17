//! Staged flow. Live pulses weight down each ribbon.

use super::{empty, Play};
use crate::ascii::{hsl, Cell, CellShader, Coord, Cursor, GridContext};

#[derive(Clone, Copy, Debug)]
pub struct SankeyNode {
    pub id: u8,
    pub label: &'static str,
    pub stage: u8,
    pub slot: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct SankeyLink {
    pub from: u8,
    pub to: u8,
    pub weight: f64,
}

pub struct Sankey<'a> {
    pub nodes: &'a [SankeyNode],
    pub links: &'a [SankeyLink],
    pub play: Play,
}

impl CellShader for Sankey<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.nodes.is_empty() || ctx.cols < 10 || ctx.rows < 4 {
            return empty();
        }
        let stages = self.nodes.iter().map(|n| n.stage).max().unwrap_or(0) as usize + 1;
        let slots = self.nodes.iter().map(|n| n.slot).max().unwrap_or(0) as usize + 1;
        let x_at = |stage: u8| -> usize {
            let span = ctx.cols.saturating_sub(8).max(1);
            3 + (stage as usize * span) / stages.max(1)
        };
        let y_at = |slot: u8| -> usize {
            let span = ctx.rows.saturating_sub(2).max(1);
            1 + (slot as usize * span) / slots.max(1)
        };
        if let Some(n) = self.nodes.iter().find(|n| {
            let x = x_at(n.stage);
            let y = y_at(n.slot);
            coord.y == y && coord.x >= x && coord.x < x + n.label.len().min(6)
        }) {
            let hover = cursor.inside && (cursor.y as usize) == y_at(n.slot);
            let i = coord.x - x_at(n.stage);
            return Cell {
                ch: n.label.chars().nth(i).unwrap_or('·'),
                color: hsl(if hover { 48.0 } else { 140.0 }, 0.55, 0.58),
                alpha: 1.0,
                scale: 1.0,
            };
        }
        let t = self.play.motion(ctx.time, cursor.intensity);
        let wmax = self
            .links
            .iter()
            .map(|l| l.weight)
            .fold(1e-6_f64, f64::max);
        for e in self.links {
            let Some(a) = self.nodes.iter().find(|n| n.id == e.from) else {
                continue;
            };
            let Some(b) = self.nodes.iter().find(|n| n.id == e.to) else {
                continue;
            };
            let x0 = x_at(a.stage) + a.label.len().min(6);
            let x1 = x_at(b.stage).saturating_sub(1);
            let y0 = y_at(a.slot);
            let y1 = y_at(b.slot);
            if x1 <= x0 {
                continue;
            }
            let u = (coord.x.saturating_sub(x0)) as f64 / (x1 - x0) as f64;
            if coord.x <= x0 || coord.x >= x1 {
                continue;
            }
            let y = y0 as f64 + (y1 as f64 - y0 as f64) * u;
            let thick = (e.weight / wmax * 1.6).clamp(0.4, 1.8);
            if (coord.y as f64 - y).abs() > thick {
                continue;
            }
            let pulse = t > 0.0 && ((u - (t * 0.35).rem_euclid(1.0)).abs() < 0.08);
            return Cell {
                ch: if pulse { '●' } else { '═' },
                color: hsl(if pulse { 50.0 } else { 195.0 }, 0.5, 0.42 + 0.15 * (e.weight / wmax)),
                alpha: 1.0,
                scale: 1.0,
            };
        }
        empty()
    }
}
