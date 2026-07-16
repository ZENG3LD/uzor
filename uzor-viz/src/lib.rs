//! `uzor-viz` — data-visualization engine core for uzor (V1).
//!
//! Stateless layout + draw layer: scales (domain -> normalized `[0, 1]` ->
//! screen px), a single plot-area coordinate transform ([`PlotArea`]),
//! pure mark draw functions, axis/grid guides, a small theme, and three
//! composed V1 figures (bar / curve / histogram).
//!
//! See `nemo/docs/uzor-viz/uzor_viz_engine_architecture.md` §3 (crate
//! layout) and §4 (design laws) — this crate is the V1 milestone: scales +
//! coord + marks + axes, harvested from `mylittlechart`'s chart-engine
//! machinery per `nemo/docs/uzor-viz/mlc_harvest_inventory.md` §2.
//!
//! **NOT in V1** (later milestones — do not add here without a plan doc):
//! - `TimeScale` — calendar-aware tick generation, harvested from mlc's
//!   ~1900-line `time_scale.rs`. Its own harvest pass; V1 only ports the
//!   linear/log nice-number math (a few hundred lines), not the calendar
//!   system.
//! - `ColorScale` (OKLCH ramp).
//! - Interaction (zoom/pan/hover/crosshair/brush/linked selection) — `V2`.
//! - A figure registry/IR (mlc's `ChartTypeDef`+`DrawOps` two-table
//!   pattern) — figures here are hand-composed, not registry-dispatched.
//! - Animation / keyed data-join (enter/update/exit).
//!
//! Every mark/guide/figure function is a pure, stateless draw over
//! borrowed data (design law #3) — this crate holds no owned render
//! state anywhere.

pub mod coord;
pub mod figure;
pub mod guide;
pub mod mark;
pub mod scale;
pub mod theme;

pub use coord::PlotArea;
pub use figure::{BarFigure, CurveFigure, HistogramFigure};
pub use mark::MarkStyle;
pub use scale::{BandScale, LinearScale, LogScale, Scale, Tick};
pub use theme::VizTheme;

#[cfg(test)]
mod proof_tests {
    //! Headless proof: render each V1 figure via `uzor-export` at a fixed
    //! resolution with deterministic (seeded, no RNG/time) data, assert a
    //! valid PNG comes out, and ALSO write it to `uzor/out/` so a human
    //! can eyeball the result (design law #8 — deterministic demos).

    use std::path::PathBuf;

    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    use crate::theme::VizTheme;
    use crate::{BarFigure, CurveFigure, HistogramFigure};

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
        let theme = VizTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("bar figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("viz_v1_bars.png", &bytes);
    }

    #[test]
    fn curve_figure_renders_to_a_valid_png() {
        let figure = seeded_curve_figure();
        let theme = VizTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("curve figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("viz_v1_curve.png", &bytes);
    }

    #[test]
    fn histogram_figure_renders_to_a_valid_png() {
        let figure = seeded_histogram_figure();
        let theme = VizTheme::dark();
        let bytes = render_to_png(&export_spec(), |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64), &theme);
        })
        .expect("histogram figure should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("viz_v1_histogram.png", &bytes);
    }
}
