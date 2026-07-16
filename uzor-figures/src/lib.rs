//! `uzor-figures` — data-visualization engine core for uzor (V1 + V2).
//!
//! Stateless layout + draw layer: scales (domain -> normalized `[0, 1]` ->
//! screen px), a single plot-area coordinate transform ([`PlotArea`]),
//! pure mark draw functions, axis/grid/crosshair/tooltip guides, a small
//! theme, three composed figures (bar / curve / histogram), and (V2) an
//! interaction plane — semantic input/output actions, hit-testing,
//! hover/selection state, a 1D brush, and a cross-figure selection bus.
//!
//! See `nemo/docs/uzor-engines/uzor_figures_engine_architecture.md` §3 (crate
//! layout) and §4 (design laws). V1 harvested scales/coord/marks/axes from
//! `mylittlechart`'s chart-engine machinery per
//! `nemo/docs/uzor-engines/mlc_harvest_inventory.md` §2; V2 harvests the input
//! pipeline (§4) and promotes `uzor-graph`'s `FocusSet` — see
//! [`mod@interact`]'s module docs for exactly what was generalized/dropped.
//!
//! **NOT in this crate yet** (later milestones — do not add here without a
//! plan doc):
//! - `TimeScale` — calendar-aware tick generation, harvested from mlc's
//!   ~1900-line `time_scale.rs`. Its own harvest pass; today only the
//!   linear/log nice-number math is ported, not the calendar system.
//! - `ColorScale` (OKLCH ramp).
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
//! `uzor-graph`'s own `FocusSet` (`uzor-graph/src/interaction/focus.rs`)
//! is NOT yet re-pointed onto this crate's [`FocusSet`] — that graph-side
//! migration is a deferred, separate arc (this task only generalizes the
//! concept into `uzor-figures`, per its brief; `uzor-graph` itself is
//! untouched).

pub mod coord;
pub mod figure;
pub mod guide;
pub mod interact;
pub mod mark;
pub mod scale;
pub mod theme;

pub use coord::PlotArea;
pub use figure::{BarFigure, CurveFigure, HistogramFigure, FigureOverlay};
pub use interact::{BrushState, FocusSet, HitZone, HoverInfo, SelectionBus, FigureInputAction, FigureOutputAction};
pub use mark::MarkStyle;
pub use scale::{BandScale, LinearScale, LogScale, Scale, Tick};
pub use theme::FigureTheme;

#[cfg(test)]
mod proof_tests {
    //! Headless proof: render each V1 figure via `uzor-export` at a fixed
    //! resolution with deterministic (seeded, no RNG/time) data, assert a
    //! valid PNG comes out, and ALSO write it to `uzor/out/` so a human
    //! can eyeball the result (design law #8 — deterministic demos).

    use std::path::PathBuf;

    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    use crate::theme::FigureTheme;
    use crate::{BarFigure, CurveFigure, HistogramFigure, FigureOverlay};

    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 500;

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

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
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
}
