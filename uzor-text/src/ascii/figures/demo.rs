//! Canned series so a scratch panel can show the collection.

use super::{
    BoxStat, DagEdge, DagNode, KpiTile, SankeyLink, SankeyNode, TlEvent, WfItem, WfKind,
};

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
    DagNode { id: 0, label: "in", layer: 0, slot: 1 },
    DagNode { id: 1, label: "scan", layer: 1, slot: 0 },
    DagNode { id: 2, label: "gate", layer: 1, slot: 2 },
    DagNode { id: 3, label: "run", layer: 2, slot: 0 },
    DagNode { id: 4, label: "mix", layer: 2, slot: 2 },
    DagNode { id: 5, label: "out", layer: 3, slot: 1 },
];

pub const DEMO_DAG_EDGES: &[DagEdge] = &[
    DagEdge { from: 0, to: 1 },
    DagEdge { from: 0, to: 2 },
    DagEdge { from: 1, to: 3 },
    DagEdge { from: 1, to: 4 },
    DagEdge { from: 2, to: 3 },
    DagEdge { from: 2, to: 4 },
    DagEdge { from: 3, to: 5 },
    DagEdge { from: 4, to: 5 },
];

pub const DEMO_HIST: &[f64] = &[
    1.0, 1.2, 1.8, 2.0, 2.1, 2.4, 2.6, 2.9, 3.0, 3.2, 3.3, 3.5, 3.8, 4.0, 4.1, 4.4, 4.8, 5.0,
    5.2, 5.5, 5.9, 6.2, 6.8, 7.1, 7.4, 8.0, 8.5, 9.0, 2.2, 3.1, 4.2, 5.1, 3.7, 4.6, 5.8, 6.0,
];

pub const DEMO_PIE: &[f64] = &[42.0, 26.0, 18.0, 9.0, 3.5, 1.5];

pub const DEMO_SANKEY_NODES: &[SankeyNode] = &[
    SankeyNode { id: 0, label: "src-a", stage: 0, slot: 0 },
    SankeyNode { id: 1, label: "src-b", stage: 0, slot: 1 },
    SankeyNode { id: 2, label: "mix-a", stage: 1, slot: 0 },
    SankeyNode { id: 3, label: "mix-b", stage: 1, slot: 1 },
    SankeyNode { id: 4, label: "sink", stage: 2, slot: 0 },
];

pub const DEMO_SANKEY_LINKS: &[SankeyLink] = &[
    SankeyLink { from: 0, to: 2, weight: 50.0 },
    SankeyLink { from: 0, to: 3, weight: 6.0 },
    SankeyLink { from: 1, to: 2, weight: 9.0 },
    SankeyLink { from: 1, to: 3, weight: 14.0 },
    SankeyLink { from: 2, to: 4, weight: 55.0 },
    SankeyLink { from: 3, to: 4, weight: 20.0 },
];

pub const DEMO_SCATTER: &[(f64, f64)] = &[
    (1.0, 12.0),
    (2.0, 15.0),
    (3.0, 11.0),
    (4.0, 18.0),
    (5.0, 22.0),
    (6.0, 19.0),
    (7.0, 25.0),
    (8.0, 21.0),
    (9.0, 28.0),
    (10.0, 24.0),
    (11.0, 30.0),
    (12.0, 27.0),
    (3.5, 20.0),
    (6.5, 14.0),
    (8.5, 32.0),
    (9.5, 16.0),
    (4.5, 26.0),
    (7.5, 29.0),
];

pub const DEMO_TIME_LANES: usize = 3;

pub const DEMO_TIME: &[TlEvent] = &[
    TlEvent { lane: 0, t0: 0.08, t1: 0.08, label: "in" },
    TlEvent { lane: 0, t0: 0.18, t1: 0.32, label: "hold" },
    TlEvent { lane: 1, t0: 0.22, t1: 0.22, label: "note" },
    TlEvent { lane: 1, t0: 0.40, t1: 0.58, label: "burst" },
    TlEvent { lane: 2, t0: 0.35, t1: 0.35, label: "flag" },
    TlEvent { lane: 2, t0: 0.62, t1: 0.82, label: "quiet" },
    TlEvent { lane: 0, t0: 0.72, t1: 0.72, label: "out" },
    TlEvent { lane: 1, t0: 0.88, t1: 0.88, label: "end" },
];

pub const DEMO_FALL: &[WfItem] = &[
    WfItem { value: 40.0, kind: WfKind::Total },
    WfItem { value: 12.0, kind: WfKind::Delta },
    WfItem { value: 8.0, kind: WfKind::Delta },
    WfItem { value: -15.0, kind: WfKind::Delta },
    WfItem { value: 0.0, kind: WfKind::Total },
    WfItem { value: 10.0, kind: WfKind::Delta },
    WfItem { value: -6.0, kind: WfKind::Delta },
    WfItem { value: 0.0, kind: WfKind::Total },
];

pub const DEMO_BOX: &[BoxStat] = &[
    BoxStat { lo: 12.0, q1: 18.0, med: 22.0, q3: 28.0, hi: 34.0, outliers: &[] },
    BoxStat { lo: 20.0, q1: 28.0, med: 36.0, q3: 44.0, hi: 52.0, outliers: &[] },
    BoxStat { lo: 10.0, q1: 16.0, med: 20.0, q3: 26.0, hi: 32.0, outliers: &[48.0] },
    BoxStat { lo: 24.0, q1: 26.0, med: 28.0, q3: 30.0, hi: 32.0, outliers: &[] },
];

pub const DEMO_KPI: &[KpiTile] = &[
    KpiTile {
        label: "rev",
        value: "128k",
        delta: "+16%",
        up: true,
        spark: &[100.0, 104.0, 98.0, 110.0, 108.0, 120.0, 118.0, 128.0],
    },
    KpiTile {
        label: "churn",
        value: "4.8%",
        delta: "-1.3",
        up: true,
        spark: &[7.0, 6.6, 6.2, 5.8, 5.5, 5.1, 4.9, 4.8],
    },
    KpiTile {
        label: "users",
        value: "48k",
        delta: "0.0",
        up: true,
        spark: &[48.0, 47.5, 48.2, 48.0, 48.4, 48.1, 48.3, 48.2],
    },
];
