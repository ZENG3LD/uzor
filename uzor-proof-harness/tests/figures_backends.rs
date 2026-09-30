//! Multi-backend divergence proofs for `uzor-figures`, moved out of
//! `uzor-figures/src/lib.rs` so the figures crate no longer dev-depends on
//! the whole GPU/render stack. The harness crate owns cross-backend proofs.

use std::path::PathBuf;

use uzor::types::Rect;
use uzor_figures::theme::FigureTheme;
use uzor_figures::{
    Annotation, BarFigure, BarMode, BarSeries, BoxplotFigure, CategoricalScale, ClassScale, ColorScale, CurveFigure,
    DagEdge, DagFigure, DagNode, GapPolicy, HeatmapFigure, HistogramFigure, FigureOverlay, KpiFigure, LabelOverflow,
    LegendEntry, LegendPosition, LegendSymbol, LinearScale, LogScale, MarkStyle, NumberFormat, PieFigure, PieSlice, PlotArea, PointRadius,
    QuantizeScale, SankeyFigure, SankeyLink, SankeyNode, Scale, ScatterFigure, ScatterPoint, SymlogScale, TimelineEvent,
    TimelineFigure, Viewport, WaterfallFigure, WaterfallItem, WaterfallKind, draw_annotation_overlays, draw_annotation_underlays,
    draw_colorbar, draw_discrete_colorbar, entries_from_class_scale, measure_discrete_colorbar,
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 500;
const TIMELINE_WIDTH: u32 = 800;
const TIMELINE_HEIGHT: u32 = 400;
const SANKEY_WIDTH: u32 = 800;
const SANKEY_HEIGHT: u32 = 450;

const MULTISERIES_WIDTH: u32 = 800;
const MULTISERIES_HEIGHT: u32 = 500;
const KPI_TILE_WIDTH: u32 = 260;
const KPI_TILE_HEIGHT: u32 = 150;
const KPI_ROW_WIDTH: u32 = KPI_TILE_WIDTH * 3;
const DAG_WIDTH: u32 = 700;
const DAG_HEIGHT: u32 = 500;

/// Deterministic sample set (a fixed pseudo-sequence spread over
/// `[0, 100)`) — fixed formula, no RNG/time.
fn seeded_histogram_figure() -> HistogramFigure {
    let samples: Vec<f64> = (0..500).map(|i| ((i * 97 + 13) % 1000) as f64 / 10.0).collect();
    HistogramFigure::new(samples, 20).with_title("Histogram (seeded)")
}

/// Deterministic ~12-event fixture spanning 60 days across 3 lanes,
/// mixed point + interval events, 3 `kind` values — generic vocabulary
/// only (`actor-a/b/c`), no case-specific naming (design law #8:
/// seeded, deterministic demos). One pair of nearby point events on
/// lane 0 (days 8 and 9) deliberately exercises label collision.
fn seeded_timeline_figure() -> TimelineFigure {
    const ANCHOR_2024_01_01: f64 = 1_704_067_200.0;
    const DAY_SECS: f64 = 86_400.0;

    let lane_names = vec!["actor-a".to_owned(), "actor-b".to_owned(), "actor-c".to_owned()];

    // (day_offset, lane, duration_days, kind, label).
    let specs: [(f64, usize, Option<f64>, usize, &str); 12] = [
        (2.0, 0, None, 0, "first-contact"),
        (5.0, 1, None, 1, "relay-a"),
        (8.0, 0, Some(4.0), 2, "burst-window"),
        (9.0, 0, None, 0, "note"),
        (14.0, 2, None, 1, "relay-b"),
        (18.0, 1, Some(6.0), 0, "hold-period"),
        (25.0, 2, None, 2, "flag-raised"),
        (30.0, 0, None, 1, "checkpoint"),
        (34.0, 1, None, 2, "relay-c"),
        (40.0, 2, Some(10.0), 0, "quiet-window"),
        (48.0, 0, None, 2, "final-note"),
        (55.0, 1, None, 1, "close-out"),
    ];

    let events: Vec<TimelineEvent> = specs
        .into_iter()
        .map(|(day_offset, lane, duration_days, kind, label)| {
            let ts = ANCHOR_2024_01_01 + day_offset * DAY_SECS;
            let end_ts = duration_days.map(|d| ts + d * DAY_SECS);
            TimelineEvent { ts, end_ts, lane, label: label.to_owned(), kind }
        })
        .collect();

    TimelineFigure::new(events, lane_names).with_title("Timeline (seeded)")
}

/// Deterministic 3-stage/7-node/8-link fixture — neutral vocabulary
/// only (`src-*`/`mixer-*`/`sink-*`, no case-specific naming, design
/// law #8), uneven weights including one dominant path
/// (`src-a -> mixer-a`, weight 50) and a deliberate conservation leak
/// at `mixer-a` (in 59, out 57 — this figure does not enforce
/// conservation, see `figure::sankey`'s own module docs).
fn seeded_sankey_figure() -> SankeyFigure {
    let nodes = vec![
        SankeyNode { id: "src-a".to_owned(), label: "src-a".to_owned(), stage: 0 },
        SankeyNode { id: "src-b".to_owned(), label: "src-b".to_owned(), stage: 0 },
        SankeyNode { id: "mixer-a".to_owned(), label: "mixer-a".to_owned(), stage: 1 },
        SankeyNode { id: "mixer-b".to_owned(), label: "mixer-b".to_owned(), stage: 1 },
        SankeyNode { id: "mixer-c".to_owned(), label: "mixer-c".to_owned(), stage: 1 },
        SankeyNode { id: "sink-a".to_owned(), label: "sink-a".to_owned(), stage: 2 },
        SankeyNode { id: "sink-b".to_owned(), label: "sink-b".to_owned(), stage: 2 },
    ];
    let links = vec![
        SankeyLink { from: 0, to: 2, weight: 50.0, kind: 0 }, // src-a -> mixer-a (dominant path)
        SankeyLink { from: 0, to: 3, weight: 6.0, kind: 1 },  // src-a -> mixer-b
        SankeyLink { from: 1, to: 2, weight: 9.0, kind: 0 },  // src-b -> mixer-a
        SankeyLink { from: 1, to: 4, weight: 14.0, kind: 2 }, // src-b -> mixer-c
        SankeyLink { from: 2, to: 5, weight: 48.0, kind: 0 }, // mixer-a -> sink-a
        SankeyLink { from: 2, to: 6, weight: 9.0, kind: 0 },  // mixer-a -> sink-b (leaks 2 vs its 59 in)
        SankeyLink { from: 3, to: 6, weight: 6.0, kind: 1 },  // mixer-b -> sink-b
        SankeyLink { from: 4, to: 5, weight: 14.0, kind: 2 }, // mixer-c -> sink-a
    ];
    SankeyFigure::new(nodes, links).with_title("Staged flow (seeded)")
}

/// Deterministic 3-series x 5-category stacked-bar fixture with some
/// negative values (deliberately exercises downward stacking below the
/// zero baseline) — fixed formula, no RNG/time.
fn seeded_stacked_bar_figure() -> BarFigure {
    let categories: Vec<String> = (0..5).map(|i| format!("cat-{i}")).collect();
    let series = vec![
        BarSeries { name: "revenue".to_owned(), values: vec![20.0, 15.0, 30.0, 10.0, 25.0] },
        BarSeries { name: "cost".to_owned(), values: vec![-8.0, -12.0, -5.0, -15.0, -6.0] },
        BarSeries { name: "adjustment".to_owned(), values: vec![5.0, -3.0, 4.0, -2.0, 6.0] },
    ];
    BarFigure::with_series(categories, series, BarMode::Stacked).with_title("Stacked bars (seeded, negatives)")
}

/// Deterministic 6-slice fixture — fixed values, no RNG.
fn seeded_pie_slices() -> Vec<PieSlice> {
    vec![
        PieSlice { label: "product-a".to_owned(), value: 420.0 },
        PieSlice { label: "product-b".to_owned(), value: 260.0 },
        PieSlice { label: "product-c".to_owned(), value: 180.0 },
        PieSlice { label: "product-d".to_owned(), value: 90.0 },
        PieSlice { label: "product-e".to_owned(), value: 35.0 },
        PieSlice { label: "product-f".to_owned(), value: 15.0 },
    ]
}

/// Deterministic "revenue walk" fixture: opening total, two gains, one
/// loss, a checkpoint subtotal, another gain/loss pair, and a closing
/// grand total — fixed values, no RNG.
fn seeded_waterfall_figure() -> WaterfallFigure {
    let items = vec![
        WaterfallItem { label: "opening".to_owned(), value: 120.0, kind: WaterfallKind::Total },
        WaterfallItem { label: "new deals".to_owned(), value: 45.0, kind: WaterfallKind::Delta },
        WaterfallItem { label: "upsells".to_owned(), value: 22.0, kind: WaterfallKind::Delta },
        WaterfallItem { label: "churn".to_owned(), value: -30.0, kind: WaterfallKind::Delta },
        WaterfallItem { label: "Q1 subtotal".to_owned(), value: 0.0, kind: WaterfallKind::Subtotal },
        WaterfallItem { label: "renewals".to_owned(), value: 38.0, kind: WaterfallKind::Delta },
        WaterfallItem { label: "refunds".to_owned(), value: -18.0, kind: WaterfallKind::Delta },
        WaterfallItem { label: "closing".to_owned(), value: 0.0, kind: WaterfallKind::Total },
    ];
    WaterfallFigure::new(items).with_title("Revenue walk (seeded)")
}

/// Deterministic 8x6 grid (fixed pseudo-formula, no RNG) — spans
/// negative and positive values so the default sequential ramp shows a
/// real low-to-high spread across the whole colorbar.
fn seeded_heatmap_figure() -> HeatmapFigure {
    let x_labels: Vec<String> = (0..8).map(|i| format!("wk-{i}")).collect();
    let y_labels: Vec<String> = (0..6).map(|i| format!("region-{i}")).collect();
    let values: Vec<Vec<f64>> = (0..6)
        .map(|row| (0..8).map(|col| (((row * 7 + col * 3) % 17) as f64 - 8.0) * 4.5).collect())
        .collect();
    HeatmapFigure::new(x_labels, y_labels, values).with_title("Regional activity (seeded)")
}

/// Deterministic ~180-point cloud (fixed pseudo-formula, no RNG),
/// value-mapped radius, an `HBand` "target range" + a `Callout` on a
/// real plotted point — proves scatter + value-mapped sizing +
/// annotations end to end.
fn seeded_scatter_figure() -> ScatterFigure {
    let points: Vec<ScatterPoint> = (0..180)
        .map(|i| {
            let fi = i as f64;
            let x = fi * 1.3 + ((i * 11) % 6) as f64 * 0.5;
            let y = 30.0 + (fi * 0.25) + ((i * 17) % 23) as f64 - 11.0;
            let value = 3.0 + ((i * 13) % 18) as f64;
            ScatterPoint::with_value(x, y, value)
        })
        .collect();
    let callout_point = points[140];
    ScatterFigure::new(points)
        .with_title("Sample metric vs. index (seeded, size-mapped)")
        .with_radius(PointRadius::ValueMapped { min_radius: 2.0, max_radius: 8.0 })
        .with_annotations(vec![
            Annotation::HBand { low: 30.0, high: 55.0, color: None, label: Some("target range".to_owned()) },
            Annotation::Callout { x: callout_point.x, y: callout_point.y, text: "notable reading".to_owned() },
        ])
}

/// Deterministic 4-category fixture: two normal-ish groups, one group
/// with a deliberate high outlier, and one `n == 3` edge-case group —
/// proves the quartile math's own small-`n` handling end to end
/// alongside the outlier-detection convention.
fn seeded_boxplot_figure() -> BoxplotFigure {
    let categories: Vec<String> = ["group-a", "group-b", "group-c", "group-d"].iter().map(|s| (*s).to_owned()).collect();
    let group_a: Vec<f64> = (0..30).map(|i| 40.0 + ((i * 7) % 25) as f64).collect();
    let group_b: Vec<f64> = (0..25).map(|i| 55.0 + ((i * 11) % 30) as f64).collect();
    let mut group_c: Vec<f64> = (0..28).map(|i| 35.0 + ((i * 5) % 20) as f64).collect();
    group_c.push(140.0); // deliberate high outlier
    let group_d: Vec<f64> = (0..3).map(|i| 60.0 + i as f64 * 5.0).collect(); // n == 3 edge case
    BoxplotFigure::new(categories, vec![group_a, group_b, group_c, group_d]).with_title("Sample distributions by group (seeded)")
}

/// Deterministic 15-node/4-layer fixture (fixed structure, no RNG):
/// one root (layer 0), 3 nodes (layer 1), 6 nodes (layer 2), 5 nodes
/// (layer 3) — a real branching-then-merging DAG, not a bare tree
/// (every layer-2/layer-3 node has more than one possible parent
/// path feeding it), one `category` per layer for palette color
/// variety.
fn seeded_dag_figure() -> DagFigure {
    let layer_sizes = [1usize, 3, 6, 5];
    let mut nodes = Vec::new();
    for (layer, &count) in layer_sizes.iter().enumerate() {
        for i in 0..count {
            let label = if layer == 0 { "root".to_owned() } else { format!("n{layer}-{i}") };
            nodes.push(DagNode { label, category: Some(layer) });
        }
    }
    // Node indices: 0 = root; 1..=3 = layer 1; 4..=9 = layer 2; 10..=14 = layer 3.
    let edges = vec![
        DagEdge { from: 0, to: 1 },
        DagEdge { from: 0, to: 2 },
        DagEdge { from: 0, to: 3 },
        DagEdge { from: 1, to: 4 },
        DagEdge { from: 1, to: 5 },
        DagEdge { from: 2, to: 5 },
        DagEdge { from: 2, to: 6 },
        DagEdge { from: 2, to: 7 },
        DagEdge { from: 3, to: 7 },
        DagEdge { from: 3, to: 8 },
        DagEdge { from: 3, to: 9 },
        DagEdge { from: 4, to: 10 },
        DagEdge { from: 5, to: 10 },
        DagEdge { from: 5, to: 11 },
        DagEdge { from: 6, to: 11 },
        DagEdge { from: 6, to: 12 },
        DagEdge { from: 7, to: 12 },
        DagEdge { from: 7, to: 13 },
        DagEdge { from: 8, to: 13 },
        DagEdge { from: 8, to: 14 },
        DagEdge { from: 9, to: 14 },
    ];
    DagFigure::new(nodes, edges).with_title("Layered DAG (seeded, 15 nodes / 4 layers)")
}

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("figures-proof-out")
}

// ── Multi-backend divergence proofs (`uzor-proof-harness`) ──────────
//
// Every proof above renders exclusively through `uzor-export::
// render_to_png`, which is HARDCODED to tiny-skia (`uzor-export/src/
// lib.rs:46,162`) — the workspace's last-resort fallback rasterizer
// family, not one of the two production families (vello, URX). Two
// real backend-specific defects (tiny-skia's `fill_text` ignoring a
// rotation transform; a shaper outline-rounding bug) were invisible
// to single-backend proofs before this section existed, and a NaN-
// severity claim below turns out to be tiny-skia-specific too. Owner
// directive: "если может быть какой-то диф в работе бекендов —
// тестируй на всех семьях; везде есть цпу варианты" — the four
// proofs below drive the SAME real production draw closures through
// every available backend leg (`uzor-proof-harness::MultiLegRender`
// — tiny-skia / vello-cpu / urx-cpu always, plus vello-gpu / urx-gpu
// when a GPU/software adapter is available, gracefully skipped
// otherwise) via `uzor-proof-harness` (a promoted, generalized copy
// of `uzor-examples/src/parity_harness.rs`'s own CPU-leg machinery,
// extended with the two GPU legs per a later owner follow-up: "могут
// ли быть расхождения на гпу? надо бы тоже включить их"), print the
// pairwise pixel-diff fractions, and write a labelled family-grouped
// composite PNG for eyeballing.
//
// These are ADDITIVE — same fixtures as the matching single-backend
// proof above, an additional render path, never a replacement (design
// law: additive, never remove). Chosen because backend divergence
// plausibly matters for each: NaN/gap-run line rasterization,
// rotated text (the known tiny-skia rotation gap), dense text (KPI
// tile row), and a linear gradient fill (heatmap + colorbar).
//
// Divergence policy (owner brief item 4): NOT a hard byte-tight
// parity gate — independent rasterizer families (and, now, a CPU vs
// GPU pipeline within the SAME family) legitimately differ in AA/
// text rasterization. `MultiLegDiff::all_within_budget` only trips on
// `uzor_proof_harness::STRUCTURAL_DEFECT_FRACTION` (a generous
// whole-image threshold tuned to catch a genuinely MISSING
// primitive, not an AA fringe — see that constant's own doc comment
// for the empirical basis), checked only across whichever pairs
// actually rendered on this machine.

use uzor_proof_harness::{ChannelTolerance, MultiLegDiff, MultiLegRender};

/// Print every URX GPU degrade counter that fired while rasterising
/// `render`'s scene (empty when the leg was skipped, or when
/// nothing degraded) — owner brief item 6: "for the URX GPU leg,
/// also report whether any degrade counters fired during these
/// scenes... a fired counter on an ordinary figure scene is a
/// finding."
fn print_urx_gpu_degrades(proof_label: &str, render: &MultiLegRender) {
    match &render.urx_gpu {
        None => println!("[{proof_label}] urx-gpu degrade counters: leg skipped, no adapter"),
        Some(r) if r.degrades.is_empty() => println!("[{proof_label}] urx-gpu degrade counters: none fired"),
        Some(r) => {
            for (kind, count) in &r.degrades {
                println!("[{proof_label}] urx-gpu degrade counter FIRED: {kind} x{count}");
            }
        }
    }
}

/// GapPolicy/NaN proof (owner brief's own headline case). Two
/// captures:
/// - **RAW-NaN "before"** — the literal pre-fix code path
///   (`ctx.stroke_polyline` fed a NaN coordinate directly, no gap
///   handling at all, exactly `item_1_gap_policy_before_after_proof`'s
///   own LEFT panel) captured across all three legs and printed
///   (investigative only, not asserted). **Measured finding, not
///   assumed**: `tiny-skia` and `vello-cpu` render IDENTICALLY here
///   (`differing_fraction == 0.0000`) and BOTH drop the WHOLE stroked
///   path (a direct pixel count confirms 0 non-background pixels on
///   both) — the whole-path-drop severity is NOT tiny-skia-specific,
///   it's shared with vello-cpu. `urx-cpu` is the one leg that
///   diverges (`differing_fraction ~= 0.013` against the other two)
///   and draws a PARTIAL result (1437 non-background pixels) instead
///   of nothing — i.e. urx-cpu degrades more gracefully on a raw NaN
///   coordinate than either of the other two families. See this
///   crate's own multi-backend divergence report for the full
///   pixel-count verification.
/// - **`GapPolicy::Break` "after"** — the real, fixed
///   `mark::line::draw_polyline` production code path — this is the
///   one the composite PNG + budget assertion cover, since it's what
///   every real figure actually renders today.
#[test]
fn gap_policy_multi_backend_divergence_proof() {
    use uzor_figures::mark::line::draw_polyline;

    let theme = FigureTheme::dark();
    let panel_w = 400.0;
    let panel_h = 250.0;
    let plot_rect = Rect::new(30.0, 20.0, panel_w - 60.0, panel_h - 60.0);
    let area = uzor_figures::coord::PlotArea::new(plot_rect);
    let x_scale = uzor_figures::scale::LinearScale::new(0.0, 10.0);
    let y_scale = uzor_figures::scale::LinearScale::new(0.0, 10.0);
    // Same deliberate interior gap (NaN at index 4 of 9) as
    // `item_1_gap_policy_before_after_proof`.
    let points: Vec<(f64, f64)> =
        vec![(0.0, 2.0), (1.0, 5.0), (2.0, 3.0), (3.0, 7.0), (f64::NAN, f64::NAN), (5.0, 6.0), (6.0, 2.0), (7.0, 8.0), (8.0, 4.0)];

    // Investigative capture — RAW NaN, bypassing gap handling
    // entirely, the exact fixture that first surfaced tiny-skia's
    // whole-path-drop behavior.
    let raw_render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, panel_w, panel_h);
        let raw_screen: Vec<(f64, f64)> = points.iter().map(|&(px, py)| (area.x(&x_scale, px), area.y(&y_scale, py))).collect();
        ctx.stroke_polyline(&raw_screen, &theme.palette[0], 2.0);
    });
    let raw_diff = MultiLegDiff::compute(&raw_render, ChannelTolerance::default());
    for line in raw_diff.report_lines() {
        println!("[gap-policy RAW-NaN, investigative only] {line}");
    }

    // Real, fixed production path — `GapPolicy::Break`, the default.
    let style = MarkStyle { color: theme.palette[0].clone(), stroke_width: 2.0, gap_policy: GapPolicy::Break, ..Default::default() };
    let fixed_render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, panel_w, panel_h);
        draw_polyline(ctx, &area, &x_scale, &y_scale, &points, &style);
    });
    let fixed_diff = MultiLegDiff::compute(&fixed_render, ChannelTolerance::default());
    for line in fixed_diff.report_lines() {
        println!("[gap-policy GapPolicy::Break, production path] {line}");
    }
    print_urx_gpu_degrades("gap-policy GapPolicy::Break, production path", &fixed_render);
    uzor_proof_harness::write_composite_png(&fixed_render, &out_dir().join("figures_gap_policy_backends.png"))
        .expect("gap-policy multi-backend composite should write");
    assert!(fixed_diff.all_within_budget(), "GapPolicy::Break: structural backend divergence beyond the generous AA/text tolerance");
}

/// Label-rotation proof — `LabelOverflow::Rotate(45.0)` is literally a
/// backend text-transform capability (`ctx.save()/translate()/
/// rotate()/fill_text()/restore()`) — the exact call sequence
/// tiny-skia's own `fill_text` rotation-ignoring defect lived in.
#[test]
fn label_rotation_multi_backend_divergence_proof() {
    let categories: Vec<String> = (0..8).map(|i| format!("category-{i}")).collect();
    let values: Vec<f64> = (0..8).map(|i| 10.0 + (i as f64 * 6.0) % 40.0).collect();
    let panel_w = 450.0;
    let panel_h = 300.0;
    let theme = FigureTheme::dark();
    let figure = BarFigure::new(categories, values)
        .with_label_overflow(LabelOverflow::Rotate(45.0))
        .with_title("LabelOverflow::Rotate(45) — multi-backend");

    let render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, panel_w, panel_h), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[label-rotation] {line}");
    }
    print_urx_gpu_degrades("label-rotation", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_label_rotation_backends.png"))
        .expect("label-rotation multi-backend composite should write");
    assert!(diff.all_within_budget(), "LabelOverflow::Rotate(45): structural backend divergence beyond the generous AA/text tolerance");
}

/// Size sweep for the rotated-label glyph-CONTENT defect (owner
/// report, 2026-07-25): `urx-cpu`'s rendering of `"category-0"`
/// rotated 45° was geometrically correct (right anchor/tilt) on all
/// three legs but visually MANGLED (piled/doubled strokes) on
/// `urx-cpu` alone at typical 11px axis-label scale, while
/// tiny-skia/vello-cpu stayed legible. Renders the SAME string at
/// 11/22/44px — one composite per size — to distinguish a
/// rasterization/flattening-tolerance defect (which cleans up as the
/// glyph gets bigger) from a geometry/emission defect (which stays
/// broken regardless of scale). Draws `fill_text` directly (not
/// through `BarFigure`) for exact control of font size.
#[test]
fn label_rotation_size_sweep_diagnostic() {
    let theme = FigureTheme::dark();
    let panel_w = 220.0;
    let panel_h = 160.0;
    for size in [11.0_f64, 22.0, 44.0] {
        let render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
            ctx.set_fill_color(&theme.background);
            ctx.fill_rect(0.0, 0.0, panel_w, panel_h);
            ctx.set_fill_color(&theme.label_color);
            ctx.set_font(&format!("{size}px Roboto"));
            ctx.save();
            ctx.translate(panel_w * 0.35, panel_h * 0.65);
            ctx.rotate(-std::f64::consts::FRAC_PI_4);
            ctx.fill_text("category-0", 0.0, 0.0);
            ctx.restore();
        });
        let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
        for line in diff.report_lines() {
            println!("[label-rotation-sweep {size}px] {line}");
        }
        let name = format!("figures_label_rotation_sweep_{size:.0}px_backends.png");
        uzor_proof_harness::write_composite_png(&render, &out_dir().join(&name))
            .expect("label-rotation size-sweep composite should write");
    }
}

/// Minimal `save() -> translate() -> rotate() -> fill_rect() ->
/// restore()` regression — the exact composition sequence
/// `guide::axis::draw_x_axis_impl`'s `LabelOverflow::Rotate` branch
/// (and every other rotated-content caller) issues, with a big-area
/// filled RECT standing in for the thin rotated text label above.
///
/// This is the GATED numeric assertion the transform-composition
/// audit specifically asked for: a `translate`/`rotate` composed in
/// the wrong (world-frame, kurbo `then_*`) order smears/mispositions
/// the WHOLE rect off its correct screen location on any backend
/// still doing that — a large, contiguous fraction of the canvas
/// flips from "matches" to "differs."
/// [`label_rotation_multi_backend_divergence_proof`] above did NOT
/// catch the original defect this way — a rotated TEXT label's own
/// ink is too small an area fraction of the canvas to push
/// `differing_fraction` past [`uzor_proof_harness::
/// STRUCTURAL_DEFECT_FRACTION`] even fully mispositioned, which is
/// exactly why that defect needed manual PNG inspection to root-cause
/// rather than tripping this crate's own existing multi-backend
/// budget gate. That test stays the eyeball/PNG composite; this one
/// is the number a CI run can actually fail on.
///
/// **Gates ONLY `tiny_skia_vs_vello_cpu`, not the full
/// [`MultiLegDiff::all_within_budget`]** — a real, disclosed, UNRELATED
/// limitation was found while calibrating this test: `uzor-urx-cpu`'s
/// `fill_rect_aa` snaps a rotated rect to the axis-aligned bounding box
/// of its transformed corners instead of rasterizing the true rotated
/// quad (`uzor-urx-wgpu/src/renderer.rs`'s own design §0.3 note,
/// pre-existing and out of this audit's scope). That means
/// `tiny_skia_vs_urx_cpu`/`vello_cpu_vs_urx_cpu` carry a persistent
/// ~9-10% differing-fraction floor for ANY rotated rect regardless of
/// whether translate/rotate compose correctly (measured empirically:
/// 0.0944 on the FIXED code, vs. 0.1294-0.1607 on the pre-fix `then_*`
/// code — too narrow a margin for a robust gate). `tiny-skia` and
/// `vello-cpu` both rasterize a genuinely rotated rect fill (no such
/// approximation), so that pair alone is the clean, confound-free
/// signal: `0.0000` fixed vs. `0.1294` broken — an unambiguous margin.
/// The other two pairs are still computed and printed for visibility.
///
/// **Uses a dedicated, TIGHTER tolerance, not
/// [`ChannelTolerance::default`]** — this scene is a single isolated
/// rect on a plain background (no dense text/gradient content to set
/// [`uzor_proof_harness::STRUCTURAL_DEFECT_FRACTION`]'s generous
/// `0.35` whole-figure budget), and the measured signal (`0.0000`
/// fixed vs. `0.1294` broken for the confound-free `tiny_skia_vs_
/// vello_cpu` pair — see doc comment above) never gets close to
/// `0.35` either way, so that generic budget cannot distinguish
/// fixed from broken here. `0.05` sits comfortably between the two
/// measured values.
#[test]
fn translate_rotate_fill_rect_multi_backend_regression_proof() {
    let panel = 400.0;
    let render = MultiLegRender::capture(panel as u32, panel as u32, |ctx| {
        ctx.set_fill_color("#101010");
        ctx.fill_rect(0.0, 0.0, panel, panel);
        ctx.save();
        ctx.set_fill_color("#33aaff");
        ctx.translate(200.0, 200.0);
        ctx.rotate(std::f64::consts::FRAC_PI_4);
        ctx.fill_rect(0.0, 0.0, 140.0, 100.0);
        ctx.restore();
    });
    let tol = ChannelTolerance {
        edge: uzor_proof_harness::STRUCTURAL_EDGE_TOLERANCE,
        max_differing_fraction: 0.05,
    };
    let diff = MultiLegDiff::compute(&render, tol);
    for line in diff.report_lines() {
        println!("[translate-rotate-rect] {line}");
    }
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_translate_rotate_rect_backends.png"))
        .expect("translate-rotate-rect multi-backend composite should write");
    assert!(
        diff.tiny_skia_vs_vello_cpu.within_budget,
        "save->translate->rotate->fill_rect->restore: tiny-skia vs vello-cpu structural divergence \
         — a mispositioned rotated rect (wrong transform-composition order)"
    );
}

/// Text-heavy proof — the same seeded 3-tile KPI row as
/// `kpi_tile_row_renders_to_a_valid_png` (big headline numbers +
/// colored deltas + a sparkline + `Currency`/`Percent`/`Si`
/// formatting) — dense, varied text is exactly where a shaper/
/// font-rasterization divergence would show up.
#[test]
fn kpi_tile_row_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let sparkline: Vec<f64> = (0..24).map(|i| 100.0 + ((i * 7) % 22) as f64 - ((i as f64) * 0.4)).collect();
    let revenue = KpiFigure::new("Revenue", 128_430.0)
        .with_previous_value(110_000.0)
        .with_format(NumberFormat::Currency("$"))
        .with_sparkline(sparkline);
    let churn = KpiFigure::new("Churn Rate", 0.048).with_previous_value(0.061).with_format(NumberFormat::Percent);
    let active_users = KpiFigure::new("Active Users", 48_213.0).with_previous_value(48_213.0).with_format(NumberFormat::Si);

    let w = KPI_TILE_WIDTH as f64;
    let h = KPI_TILE_HEIGHT as f64;
    let render = MultiLegRender::capture(KPI_ROW_WIDTH, KPI_TILE_HEIGHT, |ctx| {
        revenue.render(ctx, Rect::new(0.0, 0.0, w, h), &theme);
        churn.render(ctx, Rect::new(w, 0.0, w, h), &theme);
        active_users.render(ctx, Rect::new(2.0 * w, 0.0, w, h), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[kpi-tile-row] {line}");
    }
    print_urx_gpu_degrades("kpi-tile-row", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_kpi_backends.png"))
        .expect("KPI multi-backend composite should write");
    assert!(diff.all_within_budget(), "KPI tile row: structural backend divergence beyond the generous AA/text tolerance");
}

/// Gradient/color-heavy proof — the same seeded 8x6 heatmap as
/// `heatmap_figure_renders_to_a_valid_png` (a sequential `ColorScale`
/// ramp filling every cell + `guide::colorbar::draw_colorbar`'s own
/// linear-gradient `GradientPainter` fill) — exactly the kind of
/// content where a gradient-stop/banding divergence between
/// rasterizer families would show up.
#[test]
fn heatmap_multi_backend_divergence_proof() {
    let figure = seeded_heatmap_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(WIDTH, HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[heatmap] {line}");
    }
    print_urx_gpu_degrades("heatmap", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_heatmap_backends.png"))
        .expect("heatmap multi-backend composite should write");
    assert!(diff.all_within_budget(), "heatmap: structural backend divergence beyond the generous AA/text tolerance");
}

// ── Whole-catalog five-leg sweep (owner brief, 2026-07-25) ──────────
//
// The eight proofs above only ever converted FOUR scenes (GapPolicy/
// curve, label-rotation/bar, KPI, heatmap) — every other figure kind
// and every guide that renders independently of a full figure had
// NEVER been driven through more than tiny-skia. Each proof below
// reuses the SAME seeded fixture (or, for a guide, a small dedicated
// scene calling that guide's own draw fn directly, not through a
// whole figure) its single-backend sibling already established —
// same "additive, same fixtures, new render path" discipline the
// section above documents — chosen specifically to exercise each
// figure/guide's own distinctive primitive mix (arcs, ribbons,
// whiskers, layered nodes+edges, event marks, connectors, gradients,
// ...) rather than re-testing the bar/rect/text primitives the
// existing four scenes already cover.

/// Boxplot proof — whisker/box/outlier-point primitive mix (the same
/// seeded 4-group fixture as `boxplot_figure_renders_to_a_valid_png`,
/// incl. its deliberate high outlier and `n == 3` edge-case group).
#[test]
fn boxplot_multi_backend_divergence_proof() {
    let figure = seeded_boxplot_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(WIDTH, HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[boxplot] {line}");
    }
    print_urx_gpu_degrades("boxplot", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_boxplot_backends.png"))
        .expect("boxplot multi-backend composite should write");
    assert!(diff.all_within_budget(), "boxplot: structural backend divergence beyond the generous AA/text tolerance");
}

/// DAG proof — layered rounded-box nodes + cubic-bezier edges with
/// direction arrowheads (the same seeded 15-node/4-layer fixture as
/// `dag_figure_renders_to_a_valid_png`).
#[test]
fn dag_multi_backend_divergence_proof() {
    let figure = seeded_dag_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(DAG_WIDTH, DAG_HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, DAG_WIDTH as f64, DAG_HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[dag] {line}");
    }
    print_urx_gpu_degrades("dag", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_dag_backends.png"))
        .expect("dag multi-backend composite should write");
    assert!(diff.all_within_budget(), "dag: structural backend divergence beyond the generous AA/text tolerance");
}

/// Histogram proof — binned-bar primitive mix (the same seeded
/// 500-sample/20-bin fixture as `histogram_figure_renders_to_a_valid_png`).
#[test]
fn histogram_multi_backend_divergence_proof() {
    let figure = seeded_histogram_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(WIDTH, HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[histogram] {line}");
    }
    print_urx_gpu_degrades("histogram", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_histogram_backends.png"))
        .expect("histogram multi-backend composite should write");
    assert!(diff.all_within_budget(), "histogram: structural backend divergence beyond the generous AA/text tolerance");
}

/// Pie proof — hand-built cubic-bezier wedge arcs + a discrete legend
/// (the same seeded 6-slice fixture as `pie_figure_renders_to_a_valid_png`)
/// — `figure::pie::append_arc`'s own module docs already flag a real
/// `uzor-render-tiny-skia::arc_to_cubics` kappa-constant bug found via
/// this exact wedge-arc shape, worked around locally; this proof is
/// the first time that primitive is driven through every OTHER leg.
#[test]
fn pie_multi_backend_divergence_proof() {
    let figure = PieFigure::new(seeded_pie_slices()).with_title("Revenue by product — multi-backend").with_legend(LegendPosition::Right);
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(WIDTH, HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[pie] {line}");
    }
    print_urx_gpu_degrades("pie", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_pie_backends.png"))
        .expect("pie multi-backend composite should write");
    assert!(diff.all_within_budget(), "pie: structural backend divergence beyond the generous AA/text tolerance");
}

/// Sankey proof — cubic-bezier ribbon bands between staged node columns
/// (the same seeded 3-stage/7-node/8-link fixture, incl. its
/// deliberate conservation leak, as `sankey_figure_renders_to_a_valid_png`).
#[test]
fn sankey_multi_backend_divergence_proof() {
    let figure = seeded_sankey_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(SANKEY_WIDTH, SANKEY_HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, SANKEY_WIDTH as f64, SANKEY_HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[sankey] {line}");
    }
    print_urx_gpu_degrades("sankey", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_sankey_backends.png"))
        .expect("sankey multi-backend composite should write");
    assert!(diff.all_within_budget(), "sankey: structural backend divergence beyond the generous AA/text tolerance");
}

/// Scatter proof — a value-sized point cloud plus the `HBand`/`Callout`
/// annotation layer drawn interleaved with marks (the same seeded
/// ~180-point fixture as `scatter_figure_renders_to_a_valid_png`).
#[test]
fn scatter_multi_backend_divergence_proof() {
    let figure = seeded_scatter_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(WIDTH, HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[scatter] {line}");
    }
    print_urx_gpu_degrades("scatter", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_scatter_backends.png"))
        .expect("scatter multi-backend composite should write");
    assert!(diff.all_within_budget(), "scatter: structural backend divergence beyond the generous AA/text tolerance");
}

/// Timeline proof — point-event circle markers + interval-event rounded
/// bars across a calendar `TimeScale` X-axis / lane `BandScale` Y-axis
/// (the same seeded ~12-event/3-lane fixture as
/// `timeline_figure_renders_to_a_valid_png`).
#[test]
fn timeline_multi_backend_divergence_proof() {
    let figure = seeded_timeline_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(TIMELINE_WIDTH, TIMELINE_HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, TIMELINE_WIDTH as f64, TIMELINE_HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[timeline] {line}");
    }
    print_urx_gpu_degrades("timeline", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_timeline_backends.png"))
        .expect("timeline multi-backend composite should write");
    assert!(diff.all_within_budget(), "timeline: structural backend divergence beyond the generous AA/text tolerance");
}

/// Waterfall proof — grounded/floating bars plus thin arrival-level
/// connector lines (the same seeded 8-item "revenue walk" fixture as
/// `waterfall_figure_renders_to_a_valid_png`).
#[test]
fn waterfall_multi_backend_divergence_proof() {
    let figure = seeded_waterfall_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(WIDTH, HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[waterfall] {line}");
    }
    print_urx_gpu_degrades("waterfall", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_waterfall_backends.png"))
        .expect("waterfall multi-backend composite should write");
    assert!(diff.all_within_budget(), "waterfall: structural backend divergence beyond the generous AA/text tolerance");
}

// ── Guides that render independently of a whole figure ──────────────
//
// `legend`/`colorbar`/`annotation`/`crosshair`/`tooltip`/`grid`/`axis`
// each have their own real draw fn, callable directly against a bare
// `PlotArea`/`ColorScale` — every scene below calls that fn DIRECTLY
// (never through a full `BarFigure`/`HeatmapFigure`/...), so a
// divergence surfaced here is attributable to the guide's own
// primitive alone, not confounded by whichever figure happens to host
// it elsewhere.

const GRID_WIDTH: u32 = 420;
const GRID_HEIGHT: u32 = 280;

/// `grid` proof — `draw_x_grid`/`draw_y_grid` alone (thin gridline
/// strokes only, no axis ticks/labels/title chrome) over a bare
/// `LinearScale` x `LinearScale` plot area.
#[test]
fn grid_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let plot_rect = Rect::new(20.0, 20.0, GRID_WIDTH as f64 - 40.0, GRID_HEIGHT as f64 - 40.0);
    let area = PlotArea::new(plot_rect);
    let x_scale = LinearScale::new(0.0, 100.0);
    let y_scale = LinearScale::new(0.0, 50.0);

    let render = MultiLegRender::capture(GRID_WIDTH, GRID_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, GRID_WIDTH as f64, GRID_HEIGHT as f64);
        uzor_figures::guide::grid::draw_x_grid(ctx, &area, &x_scale, &theme, 6);
        uzor_figures::guide::grid::draw_y_grid(ctx, &area, &y_scale, &theme, 5);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[grid] {line}");
    }
    print_urx_gpu_degrades("grid", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_grid_guide_backends.png"))
        .expect("grid guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "grid: structural backend divergence beyond the generous AA/text tolerance");
}

const AXIS_WIDTH: u32 = 420;
const AXIS_HEIGHT: u32 = 280;

/// `axis` proof — `draw_x_axis`/`draw_y_axis` alone (axis line + tick
/// marks + tick-label text, no gridlines) over the same plot-area
/// shape as the `grid` proof above, so the two primitives can be
/// eyeballed side by side without either confounding the other.
#[test]
fn axis_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let plot_rect = Rect::new(50.0, 20.0, AXIS_WIDTH as f64 - 70.0, AXIS_HEIGHT as f64 - 60.0);
    let area = PlotArea::new(plot_rect);
    let x_scale = LinearScale::new(0.0, 100.0);
    let y_scale = LinearScale::new(0.0, 50.0);

    let render = MultiLegRender::capture(AXIS_WIDTH, AXIS_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, AXIS_WIDTH as f64, AXIS_HEIGHT as f64);
        uzor_figures::guide::axis::draw_x_axis(ctx, &area, &x_scale, &theme, 6);
        uzor_figures::guide::axis::draw_y_axis(ctx, &area, &y_scale, &theme, 5);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[axis] {line}");
    }
    print_urx_gpu_degrades("axis", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_axis_guide_backends.png"))
        .expect("axis guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "axis: structural backend divergence beyond the generous AA/text tolerance");
}

const LEGEND_WIDTH: u32 = 260;
const LEGEND_HEIGHT: u32 = 220;

/// `legend` proof — `draw_legend` alone at `LegendPosition::Right`
/// (greedy column-wrap), with all three `LegendSymbol` swatch shapes
/// present (square/line/circle) so a swatch-shape defect on any one
/// leg is directly visible.
#[test]
fn legend_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let entries = vec![
        LegendEntry { label: "revenue".to_owned(), color: theme.palette[0].clone(), symbol: LegendSymbol::Square },
        LegendEntry { label: "signal".to_owned(), color: theme.palette[1].clone(), symbol: LegendSymbol::Line },
        LegendEntry { label: "sample".to_owned(), color: theme.palette[2].clone(), symbol: LegendSymbol::Circle },
        LegendEntry { label: "forecast".to_owned(), color: theme.palette[3 % theme.palette.len()].clone(), symbol: LegendSymbol::Line },
    ];

    let render = MultiLegRender::capture(LEGEND_WIDTH, LEGEND_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, LEGEND_WIDTH as f64, LEGEND_HEIGHT as f64);
        let rect = Rect::new(10.0, 10.0, LEGEND_WIDTH as f64 - 20.0, LEGEND_HEIGHT as f64 - 20.0);
        uzor_figures::guide::legend::draw_legend(ctx, rect, &theme, &entries, LegendPosition::Right);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[legend] {line}");
    }
    print_urx_gpu_degrades("legend", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_legend_guide_backends.png"))
        .expect("legend guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "legend: structural backend divergence beyond the generous AA/text tolerance");
}

const COLORBAR_WIDTH: u32 = 160;
const COLORBAR_HEIGHT: u32 = 280;

/// `colorbar` proof — `draw_colorbar` alone (a real `GradientPainter`
/// linear-gradient fill via `ColorScale::diverging`'s own stop list) —
/// the exact primitive `render-urx`'s gradient-fill-never-emitted/
/// leaking defect and the GPU Linear-gradient dropped-intermediate-
/// stop defect were both found through, isolated here from a full
/// heatmap's own grid-cell content.
#[test]
fn colorbar_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let scale = ColorScale::diverging(-10.0, 0.0, 10.0);

    let render = MultiLegRender::capture(COLORBAR_WIDTH, COLORBAR_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, COLORBAR_WIDTH as f64, COLORBAR_HEIGHT as f64);
        let rect = Rect::new(20.0, 20.0, COLORBAR_WIDTH as f64 - 40.0, COLORBAR_HEIGHT as f64 - 40.0);
        draw_colorbar(ctx, rect, &theme, &scale);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[colorbar] {line}");
    }
    print_urx_gpu_degrades("colorbar", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_colorbar_guide_backends.png"))
        .expect("colorbar guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "colorbar: structural backend divergence beyond the generous AA/text tolerance");
}

const ANNOTATION_WIDTH: u32 = 500;
const ANNOTATION_HEIGHT: u32 = 320;

/// `annotation` proof — `draw_annotation_underlays`/
/// `draw_annotation_overlays` alone, all four `Annotation` variants at
/// once (`HBand` fill/label, `HLine`/`VLine` dashed reference lines,
/// `Callout` leader+box), with a stand-in filled rect painted BETWEEN
/// the two passes (the same "a mark" convention this guide's own unit
/// tests use) so the underlay-before/overlay-after layering is
/// actually visible, not just asserted.
#[test]
fn annotation_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let plot_rect = Rect::new(30.0, 20.0, ANNOTATION_WIDTH as f64 - 60.0, ANNOTATION_HEIGHT as f64 - 60.0);
    let area = PlotArea::new(plot_rect);
    let x_scale = LinearScale::new(0.0, 100.0);
    let y_scale = LinearScale::new(0.0, 50.0);
    let annotations = vec![
        Annotation::HBand { low: 12.0, high: 28.0, color: None, label: Some("target range".to_owned()) },
        Annotation::HLine { value: 40.0, color: None, label: Some("threshold".to_owned()) },
        Annotation::VLine { value: 70.0, color: None, label: Some("event".to_owned()) },
        Annotation::Callout { x: 60.0, y: 20.0, text: "notable reading".to_owned() },
    ];

    let render = MultiLegRender::capture(ANNOTATION_WIDTH, ANNOTATION_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, ANNOTATION_WIDTH as f64, ANNOTATION_HEIGHT as f64);
        draw_annotation_underlays(ctx, &area, &y_scale, &theme, &annotations);
        ctx.set_fill_color(&theme.palette[0]);
        let mark_x = area.x(&x_scale, 40.0);
        let mark_y = area.y(&y_scale, 32.0);
        ctx.fill_rect(mark_x, mark_y, 30.0, 60.0);
        draw_annotation_overlays(ctx, &area, &x_scale, &y_scale, &theme, &annotations);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[annotation] {line}");
    }
    print_urx_gpu_degrades("annotation", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_annotation_guide_backends.png"))
        .expect("annotation guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "annotation: structural backend divergence beyond the generous AA/text tolerance");
}

const CROSSHAIR_WIDTH: u32 = 420;
const CROSSHAIR_HEIGHT: u32 = 280;

/// `crosshair` proof — `draw_crosshair` alone (dashed vertical+
/// horizontal hair-lines through a resolved domain point, plus small
/// axis-cursor labels), drawn over a plain grid for spatial context.
#[test]
fn crosshair_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let plot_rect = Rect::new(30.0, 20.0, CROSSHAIR_WIDTH as f64 - 60.0, CROSSHAIR_HEIGHT as f64 - 60.0);
    let area = PlotArea::new(plot_rect);
    let x_scale = LinearScale::new(0.0, 100.0);
    let y_scale = LinearScale::new(0.0, 50.0);

    let render = MultiLegRender::capture(CROSSHAIR_WIDTH, CROSSHAIR_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, CROSSHAIR_WIDTH as f64, CROSSHAIR_HEIGHT as f64);
        uzor_figures::guide::grid::draw_x_grid(ctx, &area, &x_scale, &theme, 6);
        uzor_figures::guide::grid::draw_y_grid(ctx, &area, &y_scale, &theme, 5);
        uzor_figures::guide::crosshair::draw_crosshair(ctx, &area, &theme, 55.0, 28.0, &x_scale, &y_scale);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[crosshair] {line}");
    }
    print_urx_gpu_degrades("crosshair", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_crosshair_guide_backends.png"))
        .expect("crosshair guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "crosshair: structural backend divergence beyond the generous AA/text tolerance");
}

const TOOLTIP_WIDTH: u32 = 260;
const TOOLTIP_HEIGHT: u32 = 160;

/// `tooltip` proof — `draw_tooltip` alone (opaque box + key/value
/// two-column row list), the flip-to-fit box every hover/callout path
/// in this crate shares.
#[test]
fn tooltip_guide_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let bounds = Rect::new(0.0, 0.0, TOOLTIP_WIDTH as f64, TOOLTIP_HEIGHT as f64);
    let lines: Vec<(String, String)> =
        vec![("category".to_owned(), "north".to_owned()), ("value".to_owned(), "42.0".to_owned()), ("delta".to_owned(), "+3.5%".to_owned())];

    let render = MultiLegRender::capture(TOOLTIP_WIDTH, TOOLTIP_HEIGHT, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, TOOLTIP_WIDTH as f64, TOOLTIP_HEIGHT as f64);
        uzor_figures::guide::tooltip::draw_tooltip(ctx, &theme, (TOOLTIP_WIDTH as f64 * 0.5, TOOLTIP_HEIGHT as f64 * 0.5), &lines, bounds);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[tooltip] {line}");
    }
    print_urx_gpu_degrades("tooltip", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_tooltip_guide_backends.png"))
        .expect("tooltip guide multi-backend composite should write");
    assert!(diff.all_within_budget(), "tooltip: structural backend divergence beyond the generous AA/text tolerance");
}

// ── Engine-strengthening WAVE 4a (missing scale kinds + categorical
// palette) — multi-backend proofs ─────────────────────────────────

/// Deterministic 41-point fixture whose X values are deliberately
/// symmetric-log-shaped (dense near zero, spreading out to +-100,000
/// across 5 decades either side) — the exact data shape
/// [`LogScale`](uzor_figures::scale::LogScale) cannot represent (it floors
/// every non-positive value) and [`SymlogScale`] exists for (a money-
/// flow/P&L/delta series spanning zero across orders of magnitude).
/// Fixed formula, no RNG/time.
fn seeded_symlog_curve_figure() -> CurveFigure {
    let mut running = 0.0;
    let points: Vec<(f64, f64)> = (-20..=20_i32)
        .map(|i| {
            let x = if i == 0 { 0.0 } else { (i as f64).signum() * 10.0_f64.powf((i as f64).abs() / 4.0) };
            let step = ((i * 13 + 7) % 19) as f64 - 9.0;
            running += step;
            (x, running)
        })
        .collect();
    let x_min = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let x_max = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    CurveFigure::new(points)
        .with_title("SymlogScale X-axis (seeded, spans zero across 5 decades)")
        .with_x_scale(SymlogScale::new(x_min, x_max))
}

/// `SymlogScale` proof — a real [`CurveFigure`] wired via
/// `with_x_scale` (the SAME "accept any `Scale` impl" extension point
/// [`TimeScale`] already proved out) over data that genuinely crosses
/// zero across orders of magnitude.
#[test]
fn symlog_scale_curve_figure_multi_backend_divergence_proof() {
    let figure = seeded_symlog_curve_figure();
    let theme = FigureTheme::dark();
    let panel_w = 700.0;
    let panel_h = 400.0;

    let render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, panel_w, panel_h), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[symlog-scale] {line}");
    }
    print_urx_gpu_degrades("symlog-scale", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave4a_symlog_backends.png"))
        .expect("symlog-scale multi-backend composite should write");
    assert!(diff.all_within_budget(), "symlog X-axis CurveFigure: structural backend divergence beyond the generous AA/text tolerance");
}

/// Deterministic 40-point fixture, X log-spaced across 5 decades
/// (`1 .. 100,000`) — fixed formula, no RNG.
fn seeded_dense_log_curve_figure() -> CurveFigure {
    let mut running = 0.0;
    let points: Vec<(f64, f64)> = (0..40_i32)
        .map(|i| {
            let t = i as f64 / 39.0;
            let x = 10.0_f64.powf(t * 5.0); // 1 .. 100,000, log-spaced
            let step = ((i * 17 + 5) % 23) as f64 - 11.0;
            running += step;
            (x, running)
        })
        .collect();
    let x_min = points.first().map(|p| p.0).unwrap_or(1.0);
    let x_max = points.last().map(|p| p.0).unwrap_or(1.0);
    CurveFigure::new(points)
        .with_title("LogScale X-axis (seeded, dense — decade labels survive densification)")
        .with_x_scale(LogScale::new(x_min, x_max))
}

/// `LogScale` priority-aware label-skip proof — a deliberately
/// NARROW panel over a 5-decade domain (5 decades at `TARGET_X_TICKS
/// == 6` triggers 2x/5x subdivisions too, per `LogScale::ticks`' own
/// cadence rule — 15 candidate ticks total) forces real label
/// collisions; every decade boundary must still read, only
/// intermediate 2x/5x subdivisions ever drop.
#[test]
fn log_scale_dense_decade_labels_survive_multi_backend_divergence_proof() {
    let figure = seeded_dense_log_curve_figure();
    let theme = FigureTheme::dark();
    let panel_w = 420.0;
    let panel_h = 320.0;

    let render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, panel_w, panel_h), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[log-scale-dense] {line}");
    }
    print_urx_gpu_degrades("log-scale-dense", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave4a_dense_log_backends.png"))
        .expect("dense log-scale multi-backend composite should write");
    assert!(diff.all_within_budget(), "dense LogScale X-axis CurveFigure: structural backend divergence beyond the generous AA/text tolerance");
}

/// Deterministic 10-category fixture spanning `[0, 100)` — fixed
/// formula, no RNG.
fn seeded_binned_bar_values() -> (Vec<String>, Vec<f64>) {
    let categories: Vec<String> = (0..10).map(|i| format!("region-{i}")).collect();
    let values: Vec<f64> = (0..10).map(|i| 5.0 + ((i as f64 * 37.0) % 95.0)).collect();
    (categories, values)
}

/// `QuantizeScale` + discrete legend/colorbar proof — bars colored by
/// which of 4 uniform-width VALUE CLASSES each one falls into (a
/// risk/class-coloring use case, not the continuous per-series color a
/// plain [`BarFigure`] would use), with BOTH new discrete-swatch guide
/// entry points this wave adds: [`entries_from_class_scale`] feeding
/// the EXISTING [`uzor_figures::guide::legend::draw_legend`] pipeline, and
/// [`draw_discrete_colorbar`] (the banded, non-gradient sibling of
/// [`draw_colorbar`]) — both driven by the SAME [`QuantizeScale`] and
/// class-color list, so the legend and colorbar agree on every class's
/// own color.
#[test]
fn quantize_scale_binned_bars_with_discrete_legend_multi_backend_divergence_proof() {
    let (categories, values) = seeded_binned_bar_values();
    let theme = FigureTheme::dark();
    let quantize = QuantizeScale::new(0.0, 100.0, 4);
    // Low -> high class colors, deliberately NOT theme.palette (a
    // caller's own risk-tier ramp) — proves `entries_from_class_scale`/
    // `draw_discrete_colorbar` both honor caller-supplied colors.
    let class_colors: Vec<String> =
        vec!["#2f6f4f".to_owned(), "#c9a227".to_owned(), "#c9662f".to_owned(), "#a4302f".to_owned()];

    let panel_w = 700.0;
    let panel_h = 320.0;
    let plot_rect = Rect::new(60.0, 20.0, 400.0, panel_h - 60.0);
    let band = uzor_figures::scale::BandScale::new(categories, 0.2);
    let y_scale = LinearScale::new(0.0, 100.0);
    let area = PlotArea::new(plot_rect);

    let render = MultiLegRender::capture(panel_w as u32, panel_h as u32, |ctx| {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(0.0, 0.0, panel_w, panel_h);
        uzor_figures::guide::grid::draw_y_grid(ctx, &area, &y_scale, &theme, 5);
        for (i, &v) in values.iter().enumerate() {
            let (x0, x1) = area.x_band(&band, i);
            let y_top = area.y(&y_scale, v);
            let y_base = area.y(&y_scale, 0.0);
            let class = quantize.class_index(v);
            ctx.set_fill_color(&class_colors[class % class_colors.len()]);
            ctx.fill_rect(x0, y_top.min(y_base), (x1 - x0).max(0.0), (y_base - y_top).abs());
        }
        uzor_figures::guide::axis::draw_x_axis(ctx, &area, &band, &theme, band.len());
        uzor_figures::guide::axis::draw_y_axis(ctx, &area, &y_scale, &theme, 5);

        // Measure BOTH discrete-swatch guides before placing them
        // (this crate's own design law #1 discipline) so the
        // colorbar's own class labels never collide with the
        // legend's — `measure_discrete_colorbar`/`measure_legend`
        // both need a live `ctx`, so this layout step happens INSIDE
        // the capture closure, same as every real figure's own
        // `render_with`.
        let entries = entries_from_class_scale(&theme, &quantize, &class_colors);
        let legend_size = uzor_figures::guide::legend::measure_legend(ctx, &theme, &entries, LegendPosition::Right, 160.0, panel_h - 40.0);
        let legend_rect = Rect::new(panel_w - 20.0 - legend_size.width, 20.0, legend_size.width, legend_size.height.max(1.0));
        uzor_figures::guide::legend::draw_legend(ctx, legend_rect, &theme, &entries, LegendPosition::Right);

        let colorbar_size = measure_discrete_colorbar(ctx, &theme, &quantize);
        let colorbar_rect = Rect::new(legend_rect.x - 20.0 - colorbar_size.width, 20.0, colorbar_size.width, panel_h - 60.0);
        draw_discrete_colorbar(ctx, colorbar_rect, &theme, &quantize, &class_colors);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[binning-scale-legend] {line}");
    }
    print_urx_gpu_degrades("binning-scale-legend", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave4a_binning_legend_backends.png"))
        .expect("binning-scale discrete legend multi-backend composite should write");
    assert!(diff.all_within_budget(), "binning-scale discrete legend: structural backend divergence beyond the generous AA/text tolerance");
}

/// Deterministic 8-slice fixture — fixed values, no RNG.
fn seeded_categorical_pie_slices() -> Vec<PieSlice> {
    vec![
        PieSlice { label: "alpha".to_owned(), value: 30.0 },
        PieSlice { label: "beta".to_owned(), value: 25.0 },
        PieSlice { label: "gamma".to_owned(), value: 20.0 },
        PieSlice { label: "delta".to_owned(), value: 15.0 },
        PieSlice { label: "epsilon".to_owned(), value: 12.0 },
        PieSlice { label: "zeta".to_owned(), value: 9.0 },
        PieSlice { label: "eta".to_owned(), value: 6.0 },
        PieSlice { label: "theta".to_owned(), value: 3.0 },
    ]
}

/// `CategoricalScale::default_palette` proof — two panels, SAME 8
/// slices: LEFT is this figure's own pre-existing default
/// (`theme.palette` indexing, unchanged); RIGHT opts into
/// [`PieFigure::with_category_palette`]`(CategoricalScale::
/// default_palette())`, the colour-blind-safe Okabe-Ito set — proves
/// the option is real, reachable, and visibly distinct from the
/// default identity, without that default having changed.
#[test]
fn categorical_scale_default_palette_multi_backend_divergence_proof() {
    let theme = FigureTheme::dark();
    let panel_w = 380.0;
    let panel_h = 420.0;

    let default_fig =
        PieFigure::new(seeded_categorical_pie_slices()).with_title("theme.palette (default)").with_legend(LegendPosition::Right);
    let categorical_fig = PieFigure::new(seeded_categorical_pie_slices())
        .with_title("CategoricalScale::default_palette (Okabe-Ito)")
        .with_legend(LegendPosition::Right)
        .with_category_palette(CategoricalScale::default_palette());

    let render = MultiLegRender::capture((panel_w * 2.0) as u32, panel_h as u32, |ctx| {
        default_fig.render(ctx, Rect::new(0.0, 0.0, panel_w, panel_h), &theme);
        categorical_fig.render(ctx, Rect::new(panel_w, 0.0, panel_w, panel_h), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[categorical-palette] {line}");
    }
    print_urx_gpu_degrades("categorical-palette", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave4a_categorical_palette_backends.png"))
        .expect("categorical palette multi-backend composite should write");
    assert!(diff.all_within_budget(), "categorical palette pie figure: structural backend divergence beyond the generous AA/text tolerance");
}

// ── Engine-strengthening WAVE 4b (the data transform layer) —
// multi-backend proofs for the two figures wired to the new
// `transform` layer ──────────────────────────────────────────────

/// Stacked-bar proof — the SAME seeded 3-series/5-category negative-
/// value fixture as `stacked_bar_figure_renders_to_a_valid_png`,
/// driven through every backend leg. `BarMode::Stacked`'s own
/// geometry now flows through [`uzor_figures::transform::stack::stack`]
/// (see `figure::bars`'s own module doc for the full refactor) —
/// this is the "wire the refactor through a real consumer, proven
/// across every render backend" half of that item's own gate, the
/// SAME discipline `histogram_multi_backend_divergence_proof` already
/// established for the binning refactor below.
#[test]
fn stacked_bar_multi_backend_divergence_proof() {
    let figure = seeded_stacked_bar_figure();
    let theme = FigureTheme::dark();
    let render = MultiLegRender::capture(MULTISERIES_WIDTH, MULTISERIES_HEIGHT, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, MULTISERIES_WIDTH as f64, MULTISERIES_HEIGHT as f64), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[stacked-bar] {line}");
    }
    print_urx_gpu_degrades("stacked-bar", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave4b_stacked_bar_backends.png"))
        .expect("stacked-bar multi-backend composite should write");
    assert!(diff.all_within_budget(), "stacked bar (transform::stack-backed): structural backend divergence beyond the generous AA/text tolerance");
}

/// Stacked-bar OPTION proof — the same fixture, two panels: LEFT the
/// pre-existing [`uzor_figures::transform::StackOffset::Diverging`] default
/// (byte-identical to every stacked-bar render before this wave),
/// RIGHT the NEW opt-in [`uzor_figures::transform::StackOffset::Zero`] —
/// visibly different bar geometry once negative values are involved,
/// proving the additive option is real and reachable, not just a
/// library-only capability nothing ever renders.
#[test]
fn stacked_bar_offset_option_multi_backend_divergence_proof() {
    let diverging = seeded_stacked_bar_figure().with_title("StackOffset::Diverging (default)");
    let zero = seeded_stacked_bar_figure().with_title("StackOffset::Zero (opt-in)").with_stack_offset(uzor_figures::transform::StackOffset::Zero);
    let theme = FigureTheme::dark();
    let panel_w = MULTISERIES_WIDTH as f64;
    let panel_h = MULTISERIES_HEIGHT as f64;

    let render = MultiLegRender::capture((panel_w * 2.0) as u32, panel_h as u32, |ctx| {
        diverging.render(ctx, Rect::new(0.0, 0.0, panel_w, panel_h), &theme);
        zero.render(ctx, Rect::new(panel_w, 0.0, panel_w, panel_h), &theme);
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[stacked-bar-offset-option] {line}");
    }
    print_urx_gpu_degrades("stacked-bar-offset-option", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave4b_stacked_bar_offset_option_backends.png"))
        .expect("stacked-bar offset-option multi-backend composite should write");
    assert!(diff.all_within_budget(), "stacked bar offset option: structural backend divergence beyond the generous AA/text tolerance");
}

// ── Engine-strengthening WAVE 5 (Viewport pan/zoom/fit) — multi-
// backend proofs ────────────────────────────────────────────────────

const VIEWPORT_PANEL_WIDTH: f64 = 420.0;
const VIEWPORT_PANEL_HEIGHT: f64 = 300.0;

/// Deterministic 120-point fixture spanning a wide, ordinary numeric X
/// range — fixed formula, no RNG/time. Shared by every Wave 5 proof
/// below so the "unwindowed vs. windowed" panels are always the SAME
/// underlying series.
fn seeded_viewport_curve_figure() -> CurveFigure {
    let points: Vec<(f64, f64)> = (0..120)
        .map(|i| {
            let x = i as f64 * 5.0;
            let y = (x * 0.02).sin() * 30.0 + ((i * 7) % 13) as f64 - 6.0 + 50.0;
            (x, y)
        })
        .collect();
    CurveFigure::new(points)
}

/// **Zoom proof.** LEFT: the figure's own unwindowed full-domain
/// render. RIGHT: the SAME figure through a [`uzor_figures::interact::viewport::Viewport`]
/// zoomed 8x around the domain's own center — a real
/// [`CurveFigure::render_with_viewport`] call, not a synthetic crop.
#[test]
fn viewport_zoom_multi_backend_divergence_proof() {
    let figure = seeded_viewport_curve_figure().with_title("full domain (no viewport)");
    let full_domain = figure.x_scale().expect("120 points").domain();
    let zoomed_figure = seeded_viewport_curve_figure().with_title("zoomed 8x (Viewport)");

    let mut vp = Viewport::new(full_domain);
    vp.zoom_at((full_domain.0 + full_domain.1) / 2.0, 8.0);

    let render = MultiLegRender::capture((VIEWPORT_PANEL_WIDTH * 2.0) as u32, VIEWPORT_PANEL_HEIGHT as u32, |ctx| {
        figure.render(ctx, Rect::new(0.0, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT), &theme_for_viewport_proofs());
        zoomed_figure.render_with_viewport(
            ctx,
            Rect::new(VIEWPORT_PANEL_WIDTH, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT),
            &theme_for_viewport_proofs(),
            &FigureOverlay::default(),
            Some(&vp),
        );
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[viewport-zoom] {line}");
    }
    print_urx_gpu_degrades("viewport-zoom", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave5_viewport_zoom_backends.png"))
        .expect("viewport-zoom multi-backend composite should write");
    assert!(diff.all_within_budget(), "viewport zoom: structural backend divergence beyond the generous AA/text tolerance");
}

/// **Pan proof.** LEFT: a zoomed viewport window. RIGHT: the SAME
/// zoom level, panned 60% of a window-span to the right — proves pan
/// moves the visible window (not just zoom).
#[test]
fn viewport_pan_multi_backend_divergence_proof() {
    let figure_before = seeded_viewport_curve_figure().with_title("zoomed (before pan)");
    let figure_after = seeded_viewport_curve_figure().with_title("panned right (after pan)");
    let full_domain = figure_before.x_scale().expect("120 points").domain();

    let mut vp_before = Viewport::new(full_domain);
    vp_before.zoom_at((full_domain.0 + full_domain.1) / 2.0, 5.0);
    let mut vp_after = vp_before;
    vp_after.pan(vp_after.span() * 0.6);

    let render = MultiLegRender::capture((VIEWPORT_PANEL_WIDTH * 2.0) as u32, VIEWPORT_PANEL_HEIGHT as u32, |ctx| {
        figure_before.render_with_viewport(
            ctx,
            Rect::new(0.0, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT),
            &theme_for_viewport_proofs(),
            &FigureOverlay::default(),
            Some(&vp_before),
        );
        figure_after.render_with_viewport(
            ctx,
            Rect::new(VIEWPORT_PANEL_WIDTH, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT),
            &theme_for_viewport_proofs(),
            &FigureOverlay::default(),
            Some(&vp_after),
        );
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[viewport-pan] {line}");
    }
    print_urx_gpu_degrades("viewport-pan", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave5_viewport_pan_backends.png"))
        .expect("viewport-pan multi-backend composite should write");
    assert!(diff.all_within_budget(), "viewport pan: structural backend divergence beyond the generous AA/text tolerance");
}

/// **Fit-to-data proof.** LEFT: a deeply zoomed viewport window.
/// RIGHT: the SAME viewport after [`uzor_figures::interact::viewport::Viewport::fit_to_data`]
/// — must look identical to the plain unwindowed render (own
/// `viewport_reset_and_fit_domain_correctness` regression in
/// `figure::curve` proves the underlying domain math; this composite
/// is the visual counterpart).
#[test]
fn viewport_fit_to_data_multi_backend_divergence_proof() {
    let figure_zoomed = seeded_viewport_curve_figure().with_title("zoomed 12x (before fit)");
    let figure_fit = seeded_viewport_curve_figure().with_title("fit_to_data (after fit)");
    let full_domain = figure_zoomed.x_scale().expect("120 points").domain();

    let mut vp_zoomed = Viewport::new(full_domain);
    vp_zoomed.zoom_at((full_domain.0 + full_domain.1) / 2.0, 12.0);
    let mut vp_fit = vp_zoomed;
    vp_fit.fit_to_data();

    let render = MultiLegRender::capture((VIEWPORT_PANEL_WIDTH * 2.0) as u32, VIEWPORT_PANEL_HEIGHT as u32, |ctx| {
        figure_zoomed.render_with_viewport(
            ctx,
            Rect::new(0.0, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT),
            &theme_for_viewport_proofs(),
            &FigureOverlay::default(),
            Some(&vp_zoomed),
        );
        figure_fit.render_with_viewport(
            ctx,
            Rect::new(VIEWPORT_PANEL_WIDTH, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT),
            &theme_for_viewport_proofs(),
            &FigureOverlay::default(),
            Some(&vp_fit),
        );
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[viewport-fit-to-data] {line}");
    }
    print_urx_gpu_degrades("viewport-fit-to-data", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave5_viewport_fit_backends.png"))
        .expect("viewport-fit-to-data multi-backend composite should write");
    assert!(diff.all_within_budget(), "viewport fit-to-data: structural backend divergence beyond the generous AA/text tolerance");
}

/// **Tick re-derivation under zoom proof.** LEFT: the full domain's
/// own sparse ticks (a wide numeric range). RIGHT: a deep zoom into a
/// small sub-window — the axis must show a visibly DENSER (finer-step)
/// tick set re-derived from the WINDOW, never the leftover ticks from
/// the full domain (the item this crate's own audit named: "ticks must
/// re-derive for the visible window... the viewport must feed them,
/// not bypass them").
#[test]
fn viewport_dense_tick_redive_under_zoom_multi_backend_divergence_proof() {
    let figure_full = seeded_viewport_curve_figure().with_title("full domain (sparse ticks)");
    let figure_zoomed = seeded_viewport_curve_figure().with_title("deep zoom (dense re-derived ticks)");
    let full_domain = figure_full.x_scale().expect("120 points").domain();

    let mut vp = Viewport::new(full_domain);
    vp.zoom_at((full_domain.0 + full_domain.1) / 2.0, 25.0);

    let render = MultiLegRender::capture((VIEWPORT_PANEL_WIDTH * 2.0) as u32, VIEWPORT_PANEL_HEIGHT as u32, |ctx| {
        figure_full.render(ctx, Rect::new(0.0, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT), &theme_for_viewport_proofs());
        figure_zoomed.render_with_viewport(
            ctx,
            Rect::new(VIEWPORT_PANEL_WIDTH, 0.0, VIEWPORT_PANEL_WIDTH, VIEWPORT_PANEL_HEIGHT),
            &theme_for_viewport_proofs(),
            &FigureOverlay::default(),
            Some(&vp),
        );
    });
    let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
    for line in diff.report_lines() {
        println!("[viewport-dense-tick-redive] {line}");
    }
    print_urx_gpu_degrades("viewport-dense-tick-redive", &render);
    uzor_proof_harness::write_composite_png(&render, &out_dir().join("figures_wave5_viewport_dense_ticks_backends.png"))
        .expect("viewport-dense-tick-redive multi-backend composite should write");
    assert!(diff.all_within_budget(), "viewport dense-tick re-derivation: structural backend divergence beyond the generous AA/text tolerance");
}

fn theme_for_viewport_proofs() -> FigureTheme {
    FigureTheme::dark()
}
