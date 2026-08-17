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

use super::Cell;

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

    fn sample(shader: &impl CellShader, time: f64, cursor: Cursor) -> Vec<char> {
        let mut g = AsciiGrid::new(24, 10);
        g.set_cursor(cursor);
        g.step(shader, time, 1.0);
        let mut out = Vec::new();
        g.for_each(|_, _, c| out.push(c.ch));
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
