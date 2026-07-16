//! # `uzor-viz` V1 figures demo — bars / curve / histogram in one window
//!
//! Renders three deterministic, seeded synthetic datasets through the
//! three `uzor-viz` V1 figures (`BarFigure`, `CurveFigure`,
//! `HistogramFigure`) inside a real uzor window. Layout is a simple
//! 3-panel grid — bars top-left, histogram top-right, curve full-width
//! bottom — computed with plain rect math, no layout engine involved.
//!
//! The window render callback and the headless `export_png` agent
//! action both call the exact same [`draw_frame`] function, so a PNG
//! exported through the agent surface is pixel-identical to what the
//! window shows at that resolution — proof that GUI state and headless
//! export share one draw path.
//!
//! Run:
//! ```sh
//! cargo run -p uzor-examples --bin viz-charts-demo
//! ```
//!
//! Agent-api control surface, port 17482 — `slot_id = "viz-charts"`:
//!
//! - `GET  /blackbox/viz-charts/state` — same shape as the `snapshot` action
//! - `POST /blackbox/viz-charts/action {"name":"reseed","args":{"seed":42}}`
//!   — regenerate all three datasets from `seed`
//! - `POST /blackbox/viz-charts/action {"name":"set_theme","args":{"theme":"light"}}`
//!   — swap the `VizTheme` (`"dark"` or `"light"`)
//! - `POST /blackbox/viz-charts/action {"name":"export_png","args":{"path":"out/custom.png"}}`
//!   — headless-render the CURRENT 3-panel frame at the current window
//!   size via `uzor_export::render_to_png_file`; `path` is optional
//!   (defaults to `uzor/out/viz_demo_export.png`); the reply carries the
//!   written path
//! - `POST /blackbox/viz-charts/action {"name":"snapshot"}` — JSON
//!   summary: seed, theme name, dataset sizes, panel rects

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::layout::agent::{AgentAction, AgentActionReply, BlackboxAgentSurface};
use uzor::layout::LayoutManager;
use uzor::platform::types::CornerStyle;
use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_desktop::AppRun as _;

use uzor_export::ExportSpec;
use uzor_viz::{BarFigure, CurveFigure, HistogramFigure, VizTheme};

const AGENT_PORT: u16 = 17482;
const BLACKBOX_SLOT: &str = "viz-charts";

/// Fixed default seed — every dataset is reproducible until the agent
/// calls `reseed`.
const DEFAULT_SEED: u64 = 42;

const BAR_CATEGORY_COUNT: usize = 8;
const CURVE_POINT_COUNT: usize = 120;
const HISTOGRAM_SAMPLE_COUNT: usize = 500;
const HISTOGRAM_BIN_COUNT: usize = 24;

const DEFAULT_WINDOW_WIDTH: f64 = 1280.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;

const PANEL_PAD: f64 = 16.0;
/// Fraction of the inner (padded) height given to the top row (bars +
/// histogram); the remainder goes to the full-width curve panel.
const TOP_ROW_FRACTION: f64 = 0.55;

// ── Deterministic PRNG — small inline LCG, no `rand` dependency ────────

/// Minimal linear-congruential generator (Numerical-Recipes constants).
/// Seeded purely by the caller so every dataset this demo draws is
/// byte-for-byte reproducible for a given seed.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// Uniform `f64` in `[0, 1)`.
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ── Synthetic datasets ───────────────────────────────────────────────

/// `BAR_CATEGORY_COUNT` categories with random magnitudes, sorted
/// descending — a "top-N categories" ranking bar chart.
fn generate_bar_data(rng: &mut Lcg) -> (Vec<String>, Vec<f64>) {
    let mut rows: Vec<(String, f64)> =
        (0..BAR_CATEGORY_COUNT).map(|i| (format!("cat-{i:02}"), 10.0 + rng.next_f64() * 90.0)).collect();
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    rows.into_iter().unzip()
}

/// Cumulative random-walk over `CURVE_POINT_COUNT` steps.
fn generate_curve_data(rng: &mut Lcg) -> Vec<(f64, f64)> {
    let mut running = 0.0_f64;
    (0..CURVE_POINT_COUNT)
        .map(|i| {
            let step = (rng.next_f64() - 0.5) * 20.0;
            running += step;
            (i as f64, running)
        })
        .collect()
}

/// `HISTOGRAM_SAMPLE_COUNT` samples — average of 3 uniforms gives a mild
/// bell shape so the histogram isn't visually flat.
fn generate_histogram_data(rng: &mut Lcg) -> Vec<f64> {
    (0..HISTOGRAM_SAMPLE_COUNT)
        .map(|_| {
            let bell = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
            bell * 100.0
        })
        .collect()
}

// ── Demo state ───────────────────────────────────────────────────────

/// Long-lived state behind the blackbox. Owned by [`DemoApp`] and shared
/// with `LayoutManager` through `Arc<Mutex<DemoState>>` so the agent
/// surface can be locked from the HTTP thread.
struct DemoState {
    seed: u64,
    theme: VizTheme,
    theme_name: &'static str,
    categories: Vec<String>,
    bar_values: Vec<f64>,
    curve_points: Vec<(f64, f64)>,
    hist_samples: Vec<f64>,
    /// Most recent window client size seen by `ui()`. `export_png` and
    /// `snapshot` use this as their render/report resolution.
    last_frame_size: (f64, f64),
}

impl DemoState {
    fn new(seed: u64) -> Self {
        let mut state = Self {
            seed,
            theme: VizTheme::dark(),
            theme_name: "dark",
            categories: Vec::new(),
            bar_values: Vec::new(),
            curve_points: Vec::new(),
            hist_samples: Vec::new(),
            last_frame_size: (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
        };
        state.reseed(seed);
        state
    }

    fn reseed(&mut self, seed: u64) {
        self.seed = seed;
        let mut rng = Lcg::new(seed);
        let (categories, bar_values) = generate_bar_data(&mut rng);
        self.categories = categories;
        self.bar_values = bar_values;
        self.curve_points = generate_curve_data(&mut rng);
        self.hist_samples = generate_histogram_data(&mut rng);
    }

    /// Swap the theme. Returns `false` (state left unchanged) for an
    /// unrecognized name.
    fn set_theme(&mut self, name: &str) -> bool {
        match name {
            "dark" => {
                self.theme = VizTheme::dark();
                self.theme_name = "dark";
                true
            }
            "light" => {
                self.theme = VizTheme::light();
                self.theme_name = "light";
                true
            }
            _ => false,
        }
    }

    fn snapshot_json(&self) -> Value {
        let (w, h) = self.last_frame_size;
        let (bars_rect, histogram_rect, curve_rect) = panel_rects(w, h);
        json!({
            "seed": self.seed,
            "theme": self.theme_name,
            "window_size": { "width": w, "height": h },
            "datasets": {
                "bar_categories": self.categories.len(),
                "curve_points": self.curve_points.len(),
                "histogram_samples": self.hist_samples.len(),
            },
            "panels": {
                "bars": rect_json(bars_rect),
                "histogram": rect_json(histogram_rect),
                "curve": rect_json(curve_rect),
            },
        })
    }

    /// Render the current frame headlessly at `last_frame_size` and
    /// write it to `path_arg` (or the default export path). Uses
    /// [`draw_frame`] — the same function the live window paints with.
    fn export_png(&self, path_arg: Option<&str>) -> AgentActionReply {
        let (w, h) = self.last_frame_size;
        let path = match path_arg {
            Some(p) => PathBuf::from(p),
            None => default_export_path(),
        };

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return AgentActionReply::err(format!(
                        "failed to create export directory {}: {e}",
                        parent.display()
                    ));
                }
            }
        }

        let spec = ExportSpec { width_px: w.max(1.0) as u32, height_px: h.max(1.0) as u32, dpr: 1.0, background: None };

        match uzor_export::render_to_png_file(&path, &spec, |ctx| draw_frame(ctx, w, h, self)) {
            Ok(()) => AgentActionReply::ok_with_log(json!({ "path": path.display().to_string() })),
            Err(e) => AgentActionReply::err(format!("export_png failed: {e}")),
        }
    }
}

fn rect_json(r: Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "width": r.width, "height": r.height })
}

/// `uzor/out/` — the shared human-eyeball drop point every headless
/// proof render in this workspace writes into (same directory
/// `uzor-viz`'s own proof tests use). Resolved relative to this crate's
/// manifest dir so it doesn't hardcode a user-specific absolute path.
fn workspace_out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("out")
}

fn default_export_path() -> PathBuf {
    workspace_out_dir().join("viz_demo_export.png")
}

// ── BlackboxAgentSurface ─────────────────────────────────────────────

impl BlackboxAgentSurface for DemoState {
    fn agent_slot_id(&self) -> &str {
        BLACKBOX_SLOT
    }

    fn agent_kind(&self) -> &str {
        "viz-charts"
    }

    fn agent_state(&self) -> Value {
        self.snapshot_json()
    }

    fn apply_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        match action.name.as_str() {
            "reseed" => {
                let Some(seed) = action.args.get("seed").and_then(Value::as_u64) else {
                    return AgentActionReply::err("reseed requires args.seed (u64)");
                };
                self.reseed(seed);
                AgentActionReply::ok_with_log(json!({ "seed": seed }))
            }
            "set_theme" => {
                let Some(name) = action.args.get("theme").and_then(Value::as_str) else {
                    return AgentActionReply::err("set_theme requires args.theme (\"dark\"|\"light\")");
                };
                if !self.set_theme(name) {
                    return AgentActionReply::err(format!("unknown theme {name:?} (expected \"dark\" or \"light\")"));
                }
                AgentActionReply::ok_with_log(json!({ "theme": self.theme_name }))
            }
            "export_png" => {
                let path_arg = action.args.get("path").and_then(Value::as_str);
                self.export_png(path_arg)
            }
            "snapshot" => AgentActionReply::ok_with_log(self.snapshot_json()),
            other => AgentActionReply::err(format!("unknown action {other:?}")),
        }
    }
}

// ── Layout + draw ────────────────────────────────────────────────────

/// Bars top-left, histogram top-right, curve full-width bottom. Plain
/// rect math — no layout engine.
fn panel_rects(w: f64, h: f64) -> (Rect, Rect, Rect) {
    let inner_w = (w - PANEL_PAD * 3.0).max(0.0);
    let inner_h = (h - PANEL_PAD * 3.0).max(0.0);
    let half_w = inner_w / 2.0;
    let top_h = inner_h * TOP_ROW_FRACTION;
    let bottom_h = (inner_h - top_h).max(0.0);

    let bars_rect = Rect::new(PANEL_PAD, PANEL_PAD, half_w, top_h);
    let histogram_rect = Rect::new(PANEL_PAD * 2.0 + half_w, PANEL_PAD, half_w, top_h);
    let curve_rect = Rect::new(PANEL_PAD, PANEL_PAD * 2.0 + top_h, (w - PANEL_PAD * 2.0).max(0.0), bottom_h);

    (bars_rect, histogram_rect, curve_rect)
}

/// Draw the full 3-panel frame into a `w x h` surface. Called by both
/// the live window render callback and `export_png` — the one shared
/// draw path that makes headless export prove GUI state.
fn draw_frame(ctx: &mut dyn RenderContext, w: f64, h: f64, state: &DemoState) {
    ctx.set_fill_color(&state.theme.background);
    ctx.fill_rect(0.0, 0.0, w, h);

    let (bars_rect, histogram_rect, curve_rect) = panel_rects(w, h);

    let bars = BarFigure::new(state.categories.clone(), state.bar_values.clone())
        .with_title("Top categories")
        .with_value_labels(true);
    bars.render(ctx, bars_rect, &state.theme);

    let histogram = HistogramFigure::new(state.hist_samples.clone(), HISTOGRAM_BIN_COUNT).with_title("Sample distribution");
    histogram.render(ctx, histogram_rect, &state.theme);

    let curve =
        CurveFigure::new(state.curve_points.clone()).with_title("Cumulative random walk").with_fill(true);
    curve.render(ctx, curve_rect, &state.theme);
}

// ── App ──────────────────────────────────────────────────────────────

struct DemoApp {
    state: Arc<Mutex<DemoState>>,
}

impl DemoApp {
    fn new() -> Self {
        Self { state: Arc::new(Mutex::new(DemoState::new(DEFAULT_SEED))) }
    }

    fn lock(state: &Arc<Mutex<DemoState>>) -> MutexGuard<'_, DemoState> {
        match state.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl App<NoPanel> for DemoApp {
    fn init(&mut self, _key: &WindowKey, layout: &mut LayoutManager<NoPanel>) {
        layout.register_blackbox_agent(BLACKBOX_SLOT, self.state.clone());
    }

    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        let raw_w = win.rect.width;
        let raw_h = win.rect.height;

        let mut state = Self::lock(&self.state);
        if raw_w > 0.0 && raw_h > 0.0 {
            state.last_frame_size = (raw_w, raw_h);
        }

        let w = raw_w.max(1.0);
        let h = raw_h.max(1.0);
        draw_frame(win.render, w, h, &state);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DemoApp::new())
        .agent_api(AGENT_PORT)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor-viz — charts demo")
                .size(1280, 800)
                .min_size(960, 600)
                .decorations(false)
                .background(0xFF_0d_0f_14)
                .corner_style(CornerStyle::Rounded)
                .border_color(0x00_4d_90_fe),
        )
        .icon_from_png(include_bytes!("../../assets/icon.png"))?
        .run()?;
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use uzor_export::render_to_png;

    use super::*;

    /// Proves `draw_frame` works headlessly end-to-end (no window, no
    /// GPU) — the same function the live app uses. Writes the result to
    /// `uzor/out/viz_demo_frame.png` for a human to eyeball.
    #[test]
    fn draw_frame_renders_headlessly_to_a_valid_png() {
        let state = DemoState::new(DEFAULT_SEED);
        let spec = ExportSpec { width_px: 1280, height_px: 800, dpr: 1.0, background: None };

        let bytes = render_to_png(&spec, |ctx| draw_frame(ctx, 1280.0, 800.0, &state))
            .expect("draw_frame should render headlessly");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "output should start with the PNG magic bytes");

        let dir = workspace_out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join("viz_demo_frame.png"), &bytes).expect("write proof PNG");
    }
}
