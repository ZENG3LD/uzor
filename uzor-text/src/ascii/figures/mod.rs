//! ASCII figure collection. Mirrors `uzor-figures` kinds as cell shaders.
//! No dependency on that crate — borrowed slices in, `Cell` out.
//!
//! Play: [`Play::Still`] ignores time, [`Play::Live`] always moves,
//! [`Play::Arm`] is still until the cursor is live (same trick as
//! hover-glitch chrome).

mod bars;
mod boxplot;
mod curve;
mod dag;
mod demo;
mod heatmap;
mod histogram;
mod kpi;
mod pie;
mod sankey;
mod scatter;
mod timeline;
mod waterfall;

pub use bars::Bars;
pub use boxplot::{BoxStat, Boxplot};
pub use curve::Curve;
pub use dag::{Dag, DagEdge, DagNode};
pub use demo::{
    DEMO_BARS, DEMO_BOX, DEMO_CURVE, DEMO_DAG_EDGES, DEMO_DAG_NODES, DEMO_FALL, DEMO_HEAT,
    DEMO_HEAT_COLS, DEMO_HEAT_ROWS, DEMO_HIST, DEMO_KPI, DEMO_PIE, DEMO_SANKEY_LINKS,
    DEMO_SANKEY_NODES, DEMO_SCATTER, DEMO_TIME, DEMO_TIME_LANES,
};
pub use heatmap::Heatmap;
pub use histogram::Histogram;
pub use kpi::{Kpi, KpiTile};
pub use pie::Pie;
pub use sankey::{Sankey, SankeyLink, SankeyNode};
pub use scatter::Scatter;
pub use timeline::{Timeline, TlEvent};
pub use waterfall::{Waterfall, WfItem, WfKind};

use super::{Cell, Coord, GridContext};

/// How time and cursor drive the figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Play {
    Still,
    Live,
    Arm,
}

impl Play {
    pub fn motion(self, time: f64, intensity: f64) -> f64 {
        match self {
            Self::Still => 0.0,
            Self::Live => time,
            Self::Arm => {
                if intensity > 0.28 {
                    time
                } else {
                    0.0
                }
            }
        }
    }

    pub fn armed(self, intensity: f64) -> bool {
        match self {
            Self::Still => false,
            Self::Live => true,
            Self::Arm => intensity > 0.28,
        }
    }
}

fn empty() -> Cell {
    Cell {
        ch: ' ',
        color: [0, 0, 0],
        alpha: 1.0,
        scale: 0.0,
    }
}

fn tofu(hover: bool, hue: f64, lit: f64) -> Cell {
    use crate::ascii::hsl;
    Cell {
        ch: '█',
        color: hsl(if hover { 48.0 } else { hue }, 0.58, if hover { 0.64 } else { lit }),
        alpha: 1.0,
        scale: 1.0,
    }
}

fn ink(ch: char, hover: bool, hue: f64, lit: f64) -> Cell {
    use crate::ascii::hsl;
    Cell {
        ch,
        color: hsl(if hover { 48.0 } else { hue }, 0.55, if hover { 0.64 } else { lit }),
        alpha: 1.0,
        scale: 1.0,
    }
}

const MATRIX: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ',
    'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ',
];

fn mix_tick(i: usize, tick: u64) -> u64 {
    let mut x = tick
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((i as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    x ^= x >> 32;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn fx_matrix(index: usize, t: f64, hue: f64, lit: f64) -> Cell {
    let tick = (t * 14.0).floor() as u64;
    let ch = MATRIX[(mix_tick(index, tick) as usize) % MATRIX.len()];
    ink(ch, false, hue, lit)
}

fn fx_aurora(x: f64, y: f64, t: f64, base_hue: f64) -> (f64, f64) {
    let fx = x * 0.18;
    let fy = y * 0.16;
    let p = (fx + t * 1.0).sin()
        + (fy - t * 0.85).sin()
        + (fx * 0.6 + fy * 0.9 + t * 0.7).sin()
        + ((fx * fx + fy * fy).sqrt() * 0.6 - t * 1.1).sin();
    let v = (p * 0.125 + 0.5).clamp(0.0, 1.0);
    let hue = (base_hue + p * 28.0).rem_euclid(360.0);
    let lit = 0.30 + 0.38 * v;
    (hue, lit)
}

fn fx_scan(from_bottom: usize, t: f64) -> bool {
    let band = ((t * 7.0).rem_euclid(6.0)) as usize;
    from_bottom % 6 == band
}

fn fx_iris(t: f64, seed: f64) -> f64 {
    (t * 90.0 + seed * 40.0).rem_euclid(360.0)
}

fn fx_ripple(dist: f64, t: f64) -> f64 {
    0.36 + 0.28 * ((dist * 1.1 - t * 7.0).sin() * 0.5 + 0.5)
}

fn plot_frame(coord: Coord, ctx: &GridContext) -> Option<Cell> {
    let last_x = ctx.cols.saturating_sub(1);
    let last_y = ctx.rows.saturating_sub(1);
    if coord.x > 0 && coord.x < last_x && coord.y > 0 && coord.y < last_y {
        return None;
    }
    let ch = match (
        coord.x == 0,
        coord.x == last_x,
        coord.y == 0,
        coord.y == last_y,
    ) {
        (true, _, true, _) => '┌',
        (_, true, true, _) => '┐',
        (true, _, _, true) => '└',
        (_, true, _, true) => '┘',
        (_, _, true, _) | (_, _, _, true) => '─',
        (true, _, _, _) | (_, true, _, _) => '│',
        _ => return None,
    };
    Some(ink(ch, false, 220.0, 0.34))
}

fn line_glyph(dx: i32, dy: i32) -> char {
    let ax = dx.abs();
    let ay = dy.abs();
    if ay * 2 <= ax {
        '─'
    } else if ax * 2 <= ay {
        '│'
    } else if dx.signum() == dy.signum() {
        '\\'
    } else {
        '/'
    }
}

/// True when `(px,py)` is a Bresenham cell of the segment. Glyph follows slope.
fn on_line(px: usize, py: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> Option<char> {
    let (mut x, mut y) = (x0 as i32, y0 as i32);
    let (x1, y1) = (x1 as i32, y1 as i32);
    let dx = (x1 - x).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let dy = -(y1 - y).abs();
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let ch = line_glyph(x1 - x0 as i32, y1 - y0 as i32);
    loop {
        if x == px as i32 && y == py as i32 {
            return Some(ch);
        }
        if x == x1 && y == y1 {
            return None;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Named entries for a HUD strip. Fields stay in the app; these are figures.
pub const CATALOG: &[(&str, Play)] = &[
    ("bars", Play::Arm),
    ("curve", Play::Live),
    ("heat", Play::Arm),
    ("dag", Play::Live),
    ("hist", Play::Arm),
    ("pie", Play::Live),
    ("sankey", Play::Live),
    ("scatter", Play::Arm),
    ("time", Play::Live),
    ("fall", Play::Arm),
    ("box", Play::Arm),
    ("kpi", Play::Live),
];

pub fn catalog_play(name: &str) -> Option<Play> {
    CATALOG.iter().find(|(n, _)| *n == name).map(|(_, p)| *p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ascii::{AsciiGrid, CellShader, Coord, Cursor, GridContext};

    fn sample(shader: &impl CellShader, time: f64, cursor: Cursor) -> Vec<(char, [u8; 3])> {
        let mut g = AsciiGrid::new(24, 10);
        g.set_cursor(cursor);
        g.step(shader, time, 1.0);
        let mut out = Vec::new();
        g.for_each(|_, _, c| out.push((c.ch, c.color)));
        out
    }

    #[test]
    fn still_ignores_time() {
        let bars = Bars {
            values: DEMO_BARS,
            play: Play::Still,
        };
        assert_eq!(
            sample(&bars, 0.0, Cursor::default()),
            sample(&bars, 3.1, Cursor::default())
        );
    }

    #[test]
    fn live_moves() {
        let curve = Curve {
            ys: DEMO_CURVE,
            play: Play::Live,
        };
        assert_ne!(
            sample(&curve, 0.0, Cursor::default()),
            sample(&curve, 1.7, Cursor::default())
        );
    }

    #[test]
    fn arm_wakes_on_cursor() {
        let bars = Bars {
            values: DEMO_BARS,
            play: Play::Arm,
        };
        let idle = Cursor::default();
        let hot = Cursor {
            x: 6.0,
            y: 8.0,
            pressed: false,
            inside: true,
            intensity: 0.9,
        };
        assert_eq!(sample(&bars, 0.4, idle), sample(&bars, 1.8, idle));
        assert_ne!(sample(&bars, 0.4, idle), sample(&bars, 0.4, hot));
        let _ = (Coord { x: 0, y: 0, index: 0 }, GridContext {
            frame: 0,
            time: 0.0,
            cols: 1,
            rows: 1,
            aspect: 1.0,
        });
    }
}
