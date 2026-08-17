//! Dashboard tiles: label, value, delta, sparkline.

use super::{empty, Play};
use crate::ascii::{density_char, hsl, Cell, CellShader, Coord, Cursor, GridContext};

#[derive(Clone, Copy, Debug)]
pub struct KpiTile {
    pub label: &'static str,
    pub value: &'static str,
    pub delta: &'static str,
    pub up: bool,
    pub spark: &'static [f64],
}

pub struct Kpi<'a> {
    pub tiles: &'a [KpiTile],
    pub play: Play,
}

impl CellShader for Kpi<'_> {
    fn main(&self, coord: Coord, ctx: &GridContext, cursor: &Cursor) -> Cell {
        if self.tiles.is_empty() || ctx.cols < 10 || ctx.rows < 5 {
            return empty();
        }
        let n = self.tiles.len();
        let tw = (ctx.cols / n).max(4);
        let i = coord.x / tw;
        if i >= n {
            return empty();
        }
        let tile = self.tiles[i];
        let lx = coord.x - i * tw;
        if lx == 0 || lx + 1 == tw {
            return empty();
        }
        let hover = cursor.inside && (cursor.x as usize) / tw == i;
        let t = self.play.motion(ctx.time, cursor.intensity);
        if coord.y == 1 {
            return glyph(tile.label, lx.saturating_sub(1), if hover { 48.0 } else { 200.0 }, 0.45);
        }
        if coord.y == 2 {
            return glyph(tile.value, lx.saturating_sub(1), if hover { 50.0 } else { 160.0 }, 0.62);
        }
        if coord.y == 3 {
            return glyph(
                tile.delta,
                lx.saturating_sub(1),
                if tile.up { 140.0 } else { 8.0 },
                0.5,
            );
        }
        if coord.y + 2 >= ctx.rows {
            return empty();
        }
        if tile.spark.len() < 2 {
            return empty();
        }
        let inner = tw.saturating_sub(2).max(1);
        let col = lx.saturating_sub(1);
        let si = (col * (tile.spark.len() - 1)) / inner;
        let (lo, hi) = min_max(tile.spark);
        let mut y = tile.spark[si];
        if t > 0.0 {
            y += 0.06 * (hi - lo).max(1.0) * (t * 2.2 + si as f64).sin();
        }
        let u = ((y - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
        let spark_rows = ctx.rows.saturating_sub(5).max(1);
        let row = coord.y.saturating_sub(4);
        if row >= spark_rows {
            return empty();
        }
        let want = ((1.0 - u) * (spark_rows.saturating_sub(1) as f64)).round() as usize;
        if row != want {
            return empty();
        }
        Cell {
            ch: density_char(0.45 + 0.4 * u),
            color: hsl(if hover { 48.0 } else { 175.0 }, 0.5, 0.45),
            alpha: 1.0,
            scale: 1.0,
        }
    }
}

fn glyph(s: &str, i: usize, hue: f64, lit: f64) -> Cell {
    match s.chars().nth(i) {
        Some(ch) => Cell {
            ch,
            color: hsl(hue, 0.5, lit),
            alpha: 1.0,
            scale: 1.0,
        },
        None => empty(),
    }
}

fn min_max(ys: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &y in ys {
        lo = lo.min(y);
        hi = hi.max(y);
    }
    if !lo.is_finite() {
        (0.0, 1.0)
    } else {
        (lo, hi)
    }
}
