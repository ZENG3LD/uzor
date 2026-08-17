//! Categorical bars. Idle tofu. The hovered column becomes a plasma field.

use super::{empty, fx_plasma, plot_frame, tofu, Play};
use crate::ascii::{Cell, CellShader, Coord, Cursor, GridContext};

pub struct Bars<'a> {
    pub values: &'a [f64],
    pub play: Play,
}

impl CellShader for Bars<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.values.is_empty() || ctx.cols < 3 || ctx.rows < 3 {
            return empty();
        }
        if let Some(frame) = plot_frame(coord, ctx) {
            return frame;
        }
        let n = self.values.len();
        let inner_w = ctx.cols.saturating_sub(2).max(1);
        let inner_h = ctx.rows.saturating_sub(2).max(1);
        let col = coord.x - 1;
        let row = coord.y - 1;
        let i = (col * n) / inner_w;
        if i >= n {
            return empty();
        }
        let x0 = (i * inner_w) / n;
        let x1 = ((i + 1) * inner_w) / n;
        let gap = if x1.saturating_sub(x0) >= 3 { 1 } else { 0 };
        if col < x0 || col >= x1.saturating_sub(gap) {
            return empty();
        }
        let v = self.values[i].max(0.0);
        let max = self
            .values
            .iter()
            .copied()
            .fold(1e-6_f64, |a, x| a.max(x.max(0.0)));
        let h = ((v / max) * inner_h as f64).round() as usize;
        let from_bottom = inner_h - 1 - row;
        if from_bottom >= h {
            return empty();
        }
        let hover = cursor.inside && {
            let hi = ((cursor.x - 1.0).max(0.0) as usize * n) / inner_w;
            hi == i
        };
        let hue = 18.0 + i as f64 * (280.0 / n.max(1) as f64);
        let _ = self.play;
        if hover {
            fx_plasma(coord.x as f64, coord.y as f64, ctx.time)
        } else {
            tofu(false, hue, 0.42)
        }
    }
}
