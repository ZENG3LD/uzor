//! Canned series so a scratch panel can show the collection.

use super::{DagEdge, DagNode};

pub const DEMO_BARS: &[f64] = &[3.0, 7.0, 4.0, 9.0, 5.0, 8.0, 2.0, 6.0, 10.0, 4.5, 7.5, 3.5];

pub const DEMO_CURVE: &[f64] = &[
    2.0, 2.4, 3.1, 4.0, 4.6, 5.0, 4.7, 3.8, 3.0, 2.6, 2.9, 4.2, 6.0, 7.1, 6.4, 5.2, 4.0, 3.2,
    3.6, 5.0,
];

pub const DEMO_HEAT_COLS: usize = 5;
pub const DEMO_HEAT_ROWS: usize = 5;

pub const DEMO_HEAT: &[f64] = &[
    0.1, 0.2, 0.3, 0.5, 0.4, 0.2, 0.3, 0.6, 0.8, 0.5, 0.4, 0.5, 0.9, 1.0, 0.6, 0.3, 0.4, 0.7,
    0.8, 0.4, 0.2, 0.3, 0.4, 0.5, 0.3,
];

pub const DEMO_DAG_NODES: &[DagNode] = &[
    DagNode {
        id: 0,
        label: "in",
        layer: 0,
        slot: 0,
    },
    DagNode {
        id: 1,
        label: "scan",
        layer: 1,
        slot: 0,
    },
    DagNode {
        id: 2,
        label: "gate",
        layer: 1,
        slot: 1,
    },
    DagNode {
        id: 3,
        label: "run",
        layer: 2,
        slot: 0,
    },
    DagNode {
        id: 4,
        label: "out",
        layer: 3,
        slot: 0,
    },
];

pub const DEMO_DAG_EDGES: &[DagEdge] = &[
    DagEdge { from: 0, to: 1 },
    DagEdge { from: 0, to: 2 },
    DagEdge { from: 1, to: 3 },
    DagEdge { from: 2, to: 3 },
    DagEdge { from: 3, to: 4 },
];
