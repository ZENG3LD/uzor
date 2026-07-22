//! `uzor-figures` — data-visualization engine core for uzor (V1 + V2 + V3 + V4).
//!
//! Stateless layout + draw layer: scales (domain -> normalized `[0, 1]` ->
//! screen px, plus [`scale::ColorScale`]: domain value -> CSS hex color),
//! a single plot-area coordinate transform ([`PlotArea`]), pure mark draw
//! functions, axis/grid/crosshair/tooltip/legend/colorbar guides, a small
//! theme, eight composed figures (bar / curve / histogram / timeline /
//! sankey / pie / waterfall / heatmap), and (V2) an interaction plane —
//! semantic input/output actions, hit-testing, hover/selection state, a 1D
//! brush, and a cross-figure selection bus.
//!
//! See `nemo/docs/uzor-engines/uzor_figures_engine_architecture.md` §3 (crate
//! layout) and §4 (design laws). V1 harvested scales/coord/marks/axes from
//! `mylittlechart`'s chart-engine machinery per
//! `nemo/docs/uzor-engines/mlc_harvest_inventory.md` §2; V2 harvests the input
//! pipeline (§4) and promotes `uzor-graph`'s `FocusSet` — see
//! [`mod@interact`]'s module docs for exactly what was generalized/dropped.
//! V4 (`scale::color`, `figure::{pie, waterfall, heatmap}`,
//! `guide::colorbar`) is the business-chart set — reuses
//! `uzor::ui::animation::math::color::Color`'s existing OKLCH lerp
//! machinery verbatim (no color math reimplemented) and bakes in the FT/
//! Economist chart-hygiene defaults from
//! `nemo/docs/uzor-engines/research_dataviz_sota_2026.md` §6.
//!
//! **NOT in this crate yet** (later milestones — do not add here without a
//! plan doc):
//! - A figure registry/IR (mlc's `ChartTypeDef`+`DrawOps` two-table
//!   pattern) — figures here are hand-composed, not registry-dispatched.
//! - Animation / keyed data-join (enter/update/exit).
//! - Zoom/pan (V2 shipped hover + one 1D brush only — see `interact`).
//!
//! Every mark/guide/figure function is a pure, stateless draw over
//! borrowed data (design law #3) — this crate holds no owned render
//! state anywhere; the V2 interaction structs ([`FocusSet`],
//! [`BrushState`], [`SelectionBus`]) are small, explicit, caller-owned
//! state a figure only ever BORROWS through
//! [`figure::FigureOverlay`], never retains.
//!
//! `uzor-graph`'s own `FocusSet` (formerly `uzor-graph/src/interaction/
//! focus.rs`) is now RE-POINTED onto this crate's [`FocusSet`] (Phase D,
//! 2026-07-17) — `uzor-graph` deleted its fork; [`FocusSet::select_many`]
//! is the one behavior that migration needed and generalized into this
//! crate (see `focus.rs`'s own module docs).

pub mod coord;
pub mod figure;
pub mod guide;
pub mod interact;
pub mod mark;
pub mod scale;
pub mod theme;
pub mod transform;

pub use coord::PlotArea;
pub use figure::{
    boxplot_stats, quartile, uniform_thin_indices, BarFigure, BarMode, BarSeries, BoxplotFigure, BoxplotStats, CurveFigure, CurveSeries,
    DagEdge, DagFigure, DagNode, HeatmapFigure, HistogramFigure, FigureOverlay, KpiFigure, PieFigure, PieSlice, PointRadius, SankeyFigure,
    SankeyLink, SankeyNode, ScatterFigure, ScatterPoint, TimelineEvent, TimelineFigure, WaterfallFigure, WaterfallItem, WaterfallKind,
    WHISKER_IQR_MULTIPLIER,
};
pub use guide::annotation::{draw_annotation_overlays, draw_annotation_underlays, Annotation};
pub use guide::axis::{draw_x_axis_formatted, draw_y_axis_formatted};
pub use guide::colorbar::{draw_colorbar, measure_colorbar, ColorbarSize};
pub use guide::labeler::{anchor_candidates, place_labels, OccupancyBitmap};
pub use guide::legend::{LegendEntry, LegendPosition};
pub use guide::wrap::{truncate_ellipsis, wrap_text};
pub use interact::{BrushState, FocusSet, HitZone, HoverInfo, SelectionBus, FigureInputAction, FigureOutputAction};
pub use mark::MarkStyle;
pub use scale::{BandScale, ColorScale, LinearScale, LogScale, NumberFormat, Scale, Tick, TimeScale};
pub use theme::FigureTheme;
pub use transform::lttb;

#[cfg(test)]
mod proof_tests {
    //! Headless proof: render each V1 figure via `uzor-export` at a fixed
    //! resolution with deterministic (seeded, no RNG/time) data, assert a
    //! valid PNG comes out, and ALSO write it to `uzor/out/` so a human
    //! can eyeball the result (design law #8 — deterministic demos).

    use std::path::PathBuf;

    use uzor::types::Rect;
    use uzor_export::{render_to_png, render_to_svg, ExportSpec};

    use crate::theme::FigureTheme;
    use crate::transform::lttb;
    use crate::{
        Annotation, BarFigure, BarMode, BarSeries, BoxplotFigure, CurveFigure, CurveSeries, DagEdge, DagFigure, DagNode, FocusSet,
        HeatmapFigure, HistogramFigure, FigureOverlay, KpiFigure, LegendPosition, NumberFormat, PieFigure, PieSlice, PointRadius,
        SankeyFigure, SankeyLink, SankeyNode, ScatterFigure, ScatterPoint, TimeScale, TimelineEvent, TimelineFigure, WaterfallFigure,
        WaterfallItem, WaterfallKind,
    };

    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 500;
    // V3 (TimeScale) proof render is a different fixed size per task spec.
    const V3_WIDTH: u32 = 600;
    const V3_HEIGHT: u32 = 400;
    // V3 (TimelineFigure) proof render size per its own task spec.
    const TIMELINE_WIDTH: u32 = 800;
    const TIMELINE_HEIGHT: u32 = 400;
    // Phase C (SankeyFigure) proof render size per its own task spec.
    const SANKEY_WIDTH: u32 = 800;
    const SANKEY_HEIGHT: u32 = 450;

    fn export_spec() -> ExportSpec {
        ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: None }
    }

    fn out_dir() -> PathBuf {
        // Fixed path per task spec (not CARGO_MANIFEST_DIR-relative) —
        // `uzor/out/` is the shared human-eyeball drop point for every
        // headless proof render in this workspace.
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn write_proof_text(name: &str, contents: &str) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), contents).expect("write proof SVG");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    /// Minimal string-level SVG sanity check (no full XML parser
    /// dependency — `usvg` is heavy for a unit test): a single well-formed
    /// `<svg>` root, and every open tag has a matching close (or is
    /// self-closing) — a tiny stack scan, not a validating parser.
    /// Duplicated (not shared via a test-utils crate) in `uzor-typeset`'s
    /// own equivalent proof test — a ~20-line helper isn't worth a new
    /// dev-dependency.
    fn assert_svg_is_well_formed(svg: &str) {
        let trimmed = svg.trim();
        assert!(trimmed.starts_with("<svg"), "SVG must start with the <svg root element");
        assert!(trimmed.ends_with("</svg>"), "SVG must end with a closed </svg> root");

        let mut stack: Vec<&str> = Vec::new();
        let mut idx = 0usize;
        while let Some(rel_start) = svg[idx..].find('<') {
            let start = idx + rel_start;
            let Some(rel_end) = svg[start..].find('>') else { break };
            let end = start + rel_end;
            let tag = &svg[start + 1..end];
            idx = end + 1;

            if let Some(name) = tag.strip_prefix('/') {
                let name = name.split_whitespace().next().unwrap_or("");
                let top = stack.pop().unwrap_or("");
                assert_eq!(top, name, "mismatched SVG closing tag </{name}>");
            } else if !tag.ends_with('/') {
                let name = tag.split_whitespace().next().unwrap_or("");
                stack.push(name);
            }
        }
        assert!(stack.is_empty(), "unbalanced SVG tags left open: {stack:?}");
    }

    /// Deterministic 10-category bar dataset — fixed values, no RNG.
    fn seeded_bar_figure() -> BarFigure {
        let categories: Vec<String> = (0..10).map(|i| format!("cat-{i}")).collect();
        let values: Vec<f64> = (0..10).map(|i| 12.0 + (i as f64 * 7.0) % 53.0).collect();
        BarFigure::new(categories, values).with_title("Bar top-N (seeded)").with_value_labels(true)
    }

    /// Deterministic cumulative-curve dataset (running sum of a fixed
    /// pseudo-sequence) — fixed formula, no RNG/time.
    fn seeded_curve_figure() -> CurveFigure {
        let mut running = 0.0;
        let points: Vec<(f64, f64)> = (0..60)
            .map(|i| {
                let step = ((i * 37 + 11) % 23) as f64 - 10.0;
                running += step;
                (i as f64, running)
            })
            .collect();
        CurveFigure::new(points).with_title("Cumulative curve (seeded)").with_fill(true)
    }

    /// Deterministic sample set (a fixed pseudo-sequence spread over
    /// `[0, 100)`) — fixed formula, no RNG/time.
    fn seeded_histogram_figure() -> HistogramFigure {
        let samples: Vec<f64> = (0..500).map(|i| ((i * 97 + 13) % 1000) as f64 / 10.0).collect();
        HistogramFigure::new(samples, 20).with_title("Histogram (seeded)")
    }

    #[test]
    fn bar_figure_renders_to_a_valid_png() {
        let figure = seeded_bar_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("bar figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_v1_bars.png", &bytes);
    }

    /// SVG sibling of `bar_figure_renders_to_a_valid_png` above — same
    /// seeded fixture, `uzor_export::render_to_svg` instead of
    /// `render_to_png`. Validation is string-level (a lightweight
    /// balanced-tag scan, not a full XML parser — `usvg` is heavy for a
    /// unit test per this task's own instruction); the coordinator
    /// verifies visually by opening the written file.
    #[test]
    fn bar_figure_renders_to_a_well_formed_standalone_svg() {
        let figure = seeded_bar_figure();
        let theme = FigureTheme::dark();
        let svg = render_to_svg(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("bar figure should render to SVG");

        assert_svg_is_well_formed(&svg);
        let rect_count = svg.matches("<rect").count();
        let path_count = svg.matches("<path").count();
        assert!(rect_count > 0, "expected at least one <rect> element (bars), got 0");
        assert!(path_count > 0, "expected at least one <path> element (axis lines/text outlines), got 0");

        write_proof_text("figures_bars.svg", &svg);
    }

    #[test]
    fn curve_figure_renders_to_a_valid_png() {
        let figure = seeded_curve_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("curve figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_v1_curve.png", &bytes);
    }

    #[test]
    fn histogram_figure_renders_to_a_valid_png() {
        let figure = seeded_histogram_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("histogram figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_v1_histogram.png", &bytes);
    }

    // ── V2 interaction-plane proofs ─────────────────────────────────

    #[test]
    fn curve_figure_overlay_renders_hover_crosshair_and_marker_to_a_valid_png() {
        let figure = seeded_curve_figure();
        let theme = FigureTheme::dark();
        let rect = Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64);
        // Roughly centered over the plot — exact nearest-point selection
        // math is already proven by `interact::hit`'s unit tests; this
        // proof is about the render PATH (hover -> crosshair + marker +
        // tooltip actually drawing), not pixel-exact point identity.
        let overlay = FigureOverlay { hover_px: Some((rect.width / 2.0, rect.height / 2.0)), brush: None, focus: None };

        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        })
        .expect("curve figure with hover overlay should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_v2_curve_overlay.png", &bytes);
    }

    #[test]
    fn histogram_figure_overlay_renders_brush_highlight_to_a_valid_png() {
        let figure = seeded_histogram_figure();
        let theme = FigureTheme::dark();
        let rect = Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64);
        // Seeded samples span roughly [0, 100) — this interval overlaps a
        // real subset of bins without covering all of them.
        let overlay = FigureOverlay { hover_px: None, brush: Some((20.0, 60.0)), focus: None };

        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        })
        .expect("histogram figure with brush overlay should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_v2_histogram_brush.png", &bytes);
    }

    // ── V3 (TimeScale) proof ─────────────────────────────────────────

    /// Deterministic 90-daily-point dataset (fixed pseudo-sequence, no
    /// RNG/time) anchored at a real UTC date (2024-01-01) — proves
    /// [`TimeScale`] wired as a [`CurveFigure`] X-axis end to end via
    /// [`CurveFigure::with_x_scale`].
    fn seeded_daily_timescale_curve_figure() -> CurveFigure {
        const ANCHOR_2024_01_01: f64 = 1_704_067_200.0;
        const DAY_SECS: f64 = 86_400.0;
        const DAYS: i64 = 90;

        let mut running = 0.0;
        let points: Vec<(f64, f64)> = (0..DAYS)
            .map(|i| {
                let step = ((i * 41 + 7) % 29) as f64 - 14.0;
                running += step;
                (ANCHOR_2024_01_01 + i as f64 * DAY_SECS, running)
            })
            .collect();

        let x_min = points[0].0;
        let x_max = points[points.len() - 1].0;
        let time_scale = TimeScale::new(x_min, x_max);

        CurveFigure::new(points).with_title("Daily curve over 90 days (TimeScale X-axis)").with_x_scale(time_scale)
    }

    #[test]
    fn curve_figure_with_time_scale_x_axis_renders_to_a_valid_png() {
        let figure = seeded_daily_timescale_curve_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: V3_WIDTH, height_px: V3_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, V3_WIDTH as f64, V3_HEIGHT as f64);

        let bytes = render_to_png(&spec, |ctx| {
            figure.render(ctx, rect, &theme);
        })
        .expect("curve figure with TimeScale x-axis should render");
        assert_eq!(decoded_png_dims(&bytes), (V3_WIDTH, V3_HEIGHT));
        write_proof_png("figures_v3_timescale.png", &bytes);
    }

    // ── V3 (TimelineFigure) proof ────────────────────────────────────────

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

    #[test]
    fn timeline_figure_renders_to_a_valid_png() {
        let figure = seeded_timeline_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: TIMELINE_WIDTH, height_px: TIMELINE_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, TIMELINE_WIDTH as f64, TIMELINE_HEIGHT as f64);

        let bytes = render_to_png(&spec, |ctx| {
            figure.render(ctx, rect, &theme);
        })
        .expect("timeline figure should render");
        assert_eq!(decoded_png_dims(&bytes), (TIMELINE_WIDTH, TIMELINE_HEIGHT));
        write_proof_png("figures_v3_timeline.png", &bytes);
    }

    #[test]
    fn timeline_figure_overlay_renders_hover_crosshair_and_event_highlight_to_a_valid_png() {
        let figure = seeded_timeline_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: TIMELINE_WIDTH, height_px: TIMELINE_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, TIMELINE_WIDTH as f64, TIMELINE_HEIGHT as f64);
        let area = figure.plot_area(rect);
        let time_scale = figure.time_scale().expect("seeded fixture has events");
        let lanes = figure.lane_scale();

        // Hover roughly over the interval event on lane 0 ("burst-window",
        // day 8-12) — exercises the crosshair + hovered-event highlight +
        // tooltip render path together, same "roughly centered" proof
        // discipline as the curve/histogram V2 overlay proofs (pixel-exact
        // hit-test identity is already covered by `figure::timeline`'s own
        // unit tests). `focus` additionally pins the SAME event selected
        // (persistent outline), so the proof also shows the FocusSet path.
        let hover_ts = 1_704_067_200.0 + 10.0 * 86_400.0;
        let hx = area.x(&time_scale, hover_ts);
        let (top, bottom) = area.y_band(&lanes, 0);
        let hy = (top + bottom) / 2.0;

        let mut focus = FocusSet::empty();
        focus.select(2); // specs index 2 == "burst-window"
        let overlay = FigureOverlay { hover_px: Some((hx, hy)), brush: None, focus: Some(&focus) };

        let bytes = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        })
        .expect("timeline figure with hover overlay should render");
        assert_eq!(decoded_png_dims(&bytes), (TIMELINE_WIDTH, TIMELINE_HEIGHT));
        write_proof_png("figures_v3_timeline_hover.png", &bytes);
    }

    // ── Phase C (SankeyFigure) proof ─────────────────────────────────────

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

    #[test]
    fn sankey_figure_renders_to_a_valid_png() {
        let figure = seeded_sankey_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: SANKEY_WIDTH, height_px: SANKEY_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, SANKEY_WIDTH as f64, SANKEY_HEIGHT as f64);

        let bytes = render_to_png(&spec, |ctx| {
            figure.render(ctx, rect, &theme);
        })
        .expect("sankey figure should render");
        assert_eq!(decoded_png_dims(&bytes), (SANKEY_WIDTH, SANKEY_HEIGHT));
        write_proof_png("figures_v3_sankey.png", &bytes);
    }

    #[test]
    fn sankey_figure_overlay_renders_hover_highlight_and_dim_to_a_valid_png() {
        let figure = seeded_sankey_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: SANKEY_WIDTH, height_px: SANKEY_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, SANKEY_WIDTH as f64, SANKEY_HEIGHT as f64);

        // Hover over "mixer-b" (node index 3) — exercises the
        // highlight-connected/dim-the-rest render path together with the
        // tooltip (pixel-exact hit-test identity is already covered by
        // `figure::sankey`'s own unit tests).
        let layout = figure.layout(rect);
        let r = layout.node_rects[3];
        let hover_px = (r.x + r.width / 2.0, r.y + r.height / 2.0);
        let overlay = FigureOverlay { hover_px: Some(hover_px), brush: None, focus: None };

        let bytes = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        })
        .expect("sankey figure with hover overlay should render");
        assert_eq!(decoded_png_dims(&bytes), (SANKEY_WIDTH, SANKEY_HEIGHT));
        write_proof_png("figures_v3_sankey_hover.png", &bytes);
    }

    // ── multi-series + legend proofs ──────────────────────────────────

    const MULTISERIES_WIDTH: u32 = 800;
    const MULTISERIES_HEIGHT: u32 = 500;

    /// Deterministic 3-series x 5-category grouped-bar fixture — fixed
    /// formula, no RNG/time.
    fn seeded_grouped_bar_figure() -> BarFigure {
        let categories: Vec<String> = (0..5).map(|i| format!("cat-{i}")).collect();
        let series = vec![
            BarSeries { name: "alpha".to_owned(), values: (0..5).map(|i| 10.0 + (i as f64 * 6.0) % 30.0).collect() },
            BarSeries { name: "beta".to_owned(), values: (0..5).map(|i| 18.0 + (i as f64 * 9.0) % 40.0).collect() },
            BarSeries { name: "gamma".to_owned(), values: (0..5).map(|i| 6.0 + (i as f64 * 13.0) % 25.0).collect() },
        ];
        BarFigure::with_series(categories, series, BarMode::Grouped)
            .with_title("Grouped bars (seeded, 3 series)")
            .with_legend(LegendPosition::Top)
    }

    #[test]
    fn grouped_bar_figure_renders_to_a_valid_png() {
        let figure = seeded_grouped_bar_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: MULTISERIES_WIDTH, height_px: MULTISERIES_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, MULTISERIES_WIDTH as f64, MULTISERIES_HEIGHT as f64);

        let bytes = render_to_png(&spec, |ctx| {
            figure.render(ctx, rect, &theme);
        })
        .expect("grouped bar figure should render");
        assert_eq!(decoded_png_dims(&bytes), (MULTISERIES_WIDTH, MULTISERIES_HEIGHT));
        write_proof_png("figures_multiseries_grouped.png", &bytes);
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

    #[test]
    fn stacked_bar_figure_renders_to_a_valid_png() {
        let figure = seeded_stacked_bar_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: MULTISERIES_WIDTH, height_px: MULTISERIES_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, MULTISERIES_WIDTH as f64, MULTISERIES_HEIGHT as f64);

        let bytes = render_to_png(&spec, |ctx| {
            figure.render(ctx, rect, &theme);
        })
        .expect("stacked bar figure should render");
        assert_eq!(decoded_png_dims(&bytes), (MULTISERIES_WIDTH, MULTISERIES_HEIGHT));
        write_proof_png("figures_multiseries_stacked.png", &bytes);
    }

    /// Deterministic 3-line curve fixture (independent seeded pseudo-walks,
    /// one fixed multiplier per series) — fixed formula, no RNG/time.
    fn seeded_multi_curve_figure() -> CurveFigure {
        let build = |seed: i64| -> Vec<(f64, f64)> {
            let mut running = 0.0;
            (0..40)
                .map(|i| {
                    let step = ((i * seed + 11) % 17) as f64 - 8.0;
                    running += step;
                    (i as f64, running)
                })
                .collect()
        };
        let series = vec![
            CurveSeries { name: "series-a".to_owned(), points: build(31) },
            CurveSeries { name: "series-b".to_owned(), points: build(47) },
            CurveSeries { name: "series-c".to_owned(), points: build(59) },
        ];
        CurveFigure::with_series(series).with_title("Multi-line curve (seeded, 3 series)").with_legend(LegendPosition::Right)
    }

    #[test]
    fn multi_curve_figure_overlay_renders_hover_and_series_tooltip_to_a_valid_png() {
        let figure = seeded_multi_curve_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: MULTISERIES_WIDTH, height_px: MULTISERIES_HEIGHT, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, MULTISERIES_WIDTH as f64, MULTISERIES_HEIGHT as f64);
        // Roughly centered over the plot — exact nearest-point-across-
        // series selection is already proven by `interact::hit`'s own unit
        // tests; this proof is about the render PATH (hover -> crosshair +
        // marker + series-aware tooltip + legend all drawing together).
        let overlay = FigureOverlay { hover_px: Some((rect.width / 2.0, rect.height / 2.0)), brush: None, focus: None };

        let bytes = render_to_png(&spec, |ctx| {
            figure.render_with(ctx, rect, &theme, &overlay);
        })
        .expect("multi curve figure with hover overlay should render");
        assert_eq!(decoded_png_dims(&bytes), (MULTISERIES_WIDTH, MULTISERIES_HEIGHT));
        write_proof_png("figures_multicurve.png", &bytes);
    }

    // ── V4 (business-chart set) proofs ────────────────────────────────

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

    #[test]
    fn pie_figure_renders_to_a_valid_png() {
        let figure = PieFigure::new(seeded_pie_slices()).with_title("Revenue by product (seeded)").with_legend(LegendPosition::Right);
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("pie figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_pie.png", &bytes);
    }

    #[test]
    fn donut_figure_renders_to_a_valid_png() {
        let figure = PieFigure::new(seeded_pie_slices())
            .donut(0.55)
            .with_title("Revenue by product — donut (seeded)")
            .with_legend(LegendPosition::Right);
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("donut figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_donut.png", &bytes);
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

    #[test]
    fn waterfall_figure_renders_to_a_valid_png() {
        let figure = seeded_waterfall_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("waterfall figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_waterfall.png", &bytes);
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

    #[test]
    fn heatmap_figure_renders_to_a_valid_png() {
        let figure = seeded_heatmap_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("heatmap figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_heatmap.png", &bytes);
    }

    // ── LTTB downsampling proof (harvest wave) ────────────────────────

    /// Deterministic 2000-point noisy series (fixed pseudo-noise formula,
    /// no RNG) with ONE deliberate spike (index 1337, value 400.0 — far
    /// outside the noise band) planted deep in the interior — the exact
    /// fixture [`crate::transform::lttb`]'s own spike-survival test uses,
    /// reused here so the visual proof and the data-level assertion below
    /// are provably the same series.
    fn seeded_noisy_curve_with_spike() -> Vec<(f64, f64)> {
        const N: usize = 2000;
        const SPIKE_INDEX: usize = 1337;
        const SPIKE_Y: f64 = 400.0;
        (0..N)
            .map(|i| {
                let noise = ((i * 97 + 13) % 23) as f64 - 11.0;
                let wave = ((i as f64) * 0.05).sin() * 15.0;
                if i == SPIKE_INDEX { (i as f64, SPIKE_Y) } else { (i as f64, wave + noise) }
            })
            .collect()
    }

    const LTTB_WIDTH: u32 = 900;
    const LTTB_HEIGHT: u32 = 400;
    const LTTB_DOWNSAMPLE_TO: usize = 100;

    #[test]
    fn lttb_downsampled_curve_preserves_shape_and_the_spike_survives() {
        let points = seeded_noisy_curve_with_spike();

        // Data-level proof (design law 8: not just eyeballing) — the
        // spike must be one of the points LTTB actually keeps.
        let downsampled = lttb(&points, LTTB_DOWNSAMPLE_TO);
        assert_eq!(downsampled.len(), LTTB_DOWNSAMPLE_TO);
        assert!(
            downsampled.iter().any(|&(x, y)| (x - 1337.0).abs() < 1e-9 && (y - 400.0).abs() < 1e-9),
            "the planted spike must survive a 2000 -> {LTTB_DOWNSAMPLE_TO} LTTB downsample"
        );

        // Visual proof — two panels side by side on ONE canvas: raw
        // (2000 pts, every point drawn) on the left, the SAME series
        // downsampled to 100 pts (via `CurveFigure::with_downsample`,
        // exercising the real render-time integration, not just the
        // pure `lttb` fn) on the right — labeled which is which.
        let theme = FigureTheme::dark();
        let raw_figure = CurveFigure::new(points.clone()).with_title(format!("raw ({} pts)", points.len()));
        let downsampled_figure =
            CurveFigure::new(points.clone()).with_downsample(LTTB_DOWNSAMPLE_TO).with_title(format!("LTTB-downsampled ({LTTB_DOWNSAMPLE_TO} pts)"));

        let spec = ExportSpec { width_px: LTTB_WIDTH, height_px: LTTB_HEIGHT, dpr: 1.0, background: None };
        let bytes = render_to_png(&spec, |ctx| {
            let panel_w = LTTB_WIDTH as f64 / 2.0;
            raw_figure.render(ctx, Rect::new(0.0, 0.0, panel_w, LTTB_HEIGHT as f64), &theme);
            downsampled_figure.render(ctx, Rect::new(panel_w, 0.0, panel_w, LTTB_HEIGHT as f64), &theme);
        })
        .expect("LTTB downsample proof should render");
        assert_eq!(decoded_png_dims(&bytes), (LTTB_WIDTH, LTTB_HEIGHT));
        write_proof_png("figures_lttb.png", &bytes);
    }

    // ── typography-gap WAVE 4 (statistical/business set) proofs ─────────

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

    #[test]
    fn scatter_figure_renders_to_a_valid_png() {
        let figure = seeded_scatter_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("scatter figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_scatter.png", &bytes);
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

    #[test]
    fn boxplot_figure_renders_to_a_valid_png() {
        let figure = seeded_boxplot_figure();
        let theme = FigureTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("boxplot figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("figures_boxplot.png", &bytes);
    }

    const KPI_TILE_WIDTH: u32 = 260;
    const KPI_TILE_HEIGHT: u32 = 150;
    const KPI_ROW_WIDTH: u32 = KPI_TILE_WIDTH * 3;

    /// Three seeded KPI tiles side by side on one canvas — big number +
    /// colored delta (one up, one down, one flat) + an optional trailing
    /// sparkline, proving the "dashboard grid" use case end to end.
    #[test]
    fn kpi_tile_row_renders_to_a_valid_png() {
        let theme = FigureTheme::dark();

        let sparkline: Vec<f64> = (0..24).map(|i| 100.0 + ((i * 7) % 22) as f64 - ((i as f64) * 0.4)).collect();
        let revenue = KpiFigure::new("Revenue", 128_430.0)
            .with_previous_value(110_000.0)
            .with_format(NumberFormat::Currency("$"))
            .with_sparkline(sparkline);
        // `NumberFormat::Percent` multiplies by 100 (it expects the
        // underlying FRACTION, see that variant's own docs) — a churn rate
        // already expressed as "4.8%"/"6.1%" is stored as 0.048/0.061.
        let churn = KpiFigure::new("Churn Rate", 0.048).with_previous_value(0.061).with_format(NumberFormat::Percent);
        let active_users = KpiFigure::new("Active Users", 48_213.0).with_previous_value(48_213.0).with_format(NumberFormat::Si);

        let spec = ExportSpec { width_px: KPI_ROW_WIDTH, height_px: KPI_TILE_HEIGHT, dpr: 1.0, background: None };
        let bytes = render_to_png(&spec, |ctx| {
            let w = KPI_TILE_WIDTH as f64;
            let h = KPI_TILE_HEIGHT as f64;
            revenue.render(ctx, Rect::new(0.0, 0.0, w, h), &theme);
            churn.render(ctx, Rect::new(w, 0.0, w, h), &theme);
            active_users.render(ctx, Rect::new(2.0 * w, 0.0, w, h), &theme);
        })
        .expect("KPI tile row should render");
        assert_eq!(decoded_png_dims(&bytes), (KPI_ROW_WIDTH, KPI_TILE_HEIGHT));
        write_proof_png("figures_kpi.png", &bytes);
    }

    // ── arc B wave 2 (DagFigure) proof ──────────────────────────────────

    const DAG_WIDTH: u32 = 700;
    const DAG_HEIGHT: u32 = 500;

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

    #[test]
    fn dag_figure_renders_to_a_valid_png() {
        let figure = seeded_dag_figure();
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: DAG_WIDTH, height_px: DAG_HEIGHT, dpr: 1.0, background: None };
        let bytes = render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, DAG_WIDTH as f64, DAG_HEIGHT as f64), &theme);
        })
        .expect("dag figure should render");
        assert_eq!(decoded_png_dims(&bytes), (DAG_WIDTH, DAG_HEIGHT));
        write_proof_png("figures_dag.png", &bytes);
    }
}
