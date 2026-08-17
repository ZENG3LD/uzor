//! Part-to-whole donut. Live spins; hover lights a slice.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::{empty, Play};
use crate::ascii::{density_char, hsl, Cell, CellShader, Coord, Cursor, GridContext};

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
        let spin = self.play.motion(ctx.time, cursor.intensity) * 0.35;
        let ang = (dy.atan2(dx) + FRAC_PI_2 - spin).rem_euclid(TAU);
        let mut acc = 0.0;
        let mut idx = 0;
        for (i, &v) in self.slices.iter().enumerate() {
            if v <= 0.0 {
                continue;
            }
            acc += (v / total) * TAU;
            if ang <= acc {
                idx = i;
                break;
            }
            idx = i;
        }
        let hover = if cursor.inside {
            let cdx = (cursor.x - cx) * ctx.aspect;
            let cdy = cursor.y - cy;
            let cr = (cdx * cdx + cdy * cdy).sqrt();
            if cr >= rmin && cr <= rmax {
                let cang = (cdy.atan2(cdx) + FRAC_PI_2 - spin).rem_euclid(TAU);
                let mut a = 0.0;
                let mut hi = 0;
                for (i, &v) in self.slices.iter().enumerate() {
                    if v <= 0.0 {
                        continue;
                    }
                    a += (v / total) * TAU;
                    if cang <= a {
                        hi = i;
                        break;
                    }
                    hi = i;
                }
                hi == idx
            } else {
                false
            }
        } else {
            false
        };
        let den = ((r - rmin) / (rmax - rmin).max(1e-6)).clamp(0.2, 1.0);
        let hue = 30.0 + idx as f64 * 48.0;
        Cell {
            ch: if hover { '◆' } else { density_char(0.35 + 0.55 * den) },
            color: hsl(hue, 0.62, if hover { 0.62 } else { 0.32 + 0.28 * den }),
            alpha: 1.0,
            scale: if hover { 1.15 } else { 1.0 },
        }
    }
}
