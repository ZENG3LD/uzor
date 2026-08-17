//! Staged flow. Straight Bresenham links (─ │ / \). Hover a node lights its links.

use super::{empty, ink, on_line, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

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
        let hovered = hovered_node(self.nodes, cursor, &x_at, &y_at);
        if let Some(n) = self.nodes.iter().find(|n| {
            let x = x_at(n.stage);
            let y = y_at(n.slot);
            coord.y == y && coord.x >= x && coord.x < x + n.label.len().min(6)
        }) {
            let hot = hovered == Some(n.id);
            let i = coord.x - x_at(n.stage);
            return ink(
                n.label.chars().nth(i).unwrap_or(' '),
                hot,
                140.0,
                0.58,
            );
        }
        let _ = self.play.motion(ctx.time, cursor.intensity);
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
            let Some(ch) = on_line(coord.x, coord.y, x0, y0, x1, y1) else {
                continue;
            };
            let hot = hovered == Some(e.from) || hovered == Some(e.to);
            return ink(ch, hot, 195.0, 0.42);
        }
        empty()
    }
}

fn hovered_node(
    nodes: &[SankeyNode],
    cursor: &Cursor,
    x_at: &impl Fn(u8) -> usize,
    y_at: &impl Fn(u8) -> usize,
) -> Option<u8> {
    if !cursor.inside {
        return None;
    }
    let cx = cursor.x as usize;
    let cy = cursor.y as usize;
    nodes.iter().find_map(|n| {
        let x = x_at(n.stage);
        let y = y_at(n.slot);
        if cy == y && cx >= x && cx < x + n.label.len().min(6) {
            Some(n.id)
        } else {
            None
        }
    })
}
