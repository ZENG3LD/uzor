//! # `uzor-figures` demo — bars / curve / histogram in one window
//!
//! Renders three deterministic, seeded synthetic datasets through the
//! three `uzor-figures` figures (`BarFigure`, `CurveFigure`,
//! `HistogramFigure`) inside a real uzor window, and wires the V2
//! interaction plane (`nemo/docs/uzor-engines/uzor_figures_engine_architecture.md`
//! §3 `interact/` block): hover a bar or a point on the curve for a
//! tooltip, drag on the curve panel to brush an X range — the brush
//! interval LINKS to the histogram panel, which highlights bins whose
//! domain-X overlaps it (a miniature of the report's #2<->#5 shared-
//! time-brush requirement).
//!
//! The window render callback and the headless `export_png` agent
//! action both call the exact same [`draw_frame`] function (which reads
//! the live [`SelectionBus`]/[`FocusSet`] state), so a PNG exported
//! through the agent surface is pixel-identical to what the window shows
//! at that resolution, INCLUDING whatever hover/brush/selection is
//! currently active — proof that GUI state and headless export share one
//! draw path.
//!
//! Run:
//! ```sh
//! cargo run -p uzor-examples --bin figures-demo
//! ```
//!
//! Agent-api control surface, port 17482 — `slot_id = "figures"`:
//!
//! - `GET  /blackbox/figures/state` — same shape as the `snapshot` action
//! - `POST /blackbox/figures/action {"name":"reseed","args":{"seed":42}}`
//!   — regenerate all three datasets from `seed` (also clears hover/brush/
//!   selection)
//! - `POST /blackbox/figures/action {"name":"set_theme","args":{"theme":"light"}}`
//!   — swap the `FigureTheme` (`"dark"` or `"light"`)
//! - `POST /blackbox/figures/action {"name":"set_hover","args":{"x":640.0,"y":400.0}}`
//!   — inject a synthetic hover position (absolute window pixels) —
//!   Tier-2 verification without a real mouse
//! - `POST /blackbox/figures/action {"name":"set_brush","args":{"x0":20.0,"x1":60.0}}`
//!   — inject a synthetic linked-brush domain interval
//! - `POST /blackbox/figures/action {"name":"export_png","args":{"path":"out/custom.png"}}`
//!   — headless-render the CURRENT 3-panel frame (with current
//!   hover/brush/selection state) at the current window size via
//!   `uzor_export::render_to_png_file`; `path` is optional (defaults to
//!   `uzor/out/figures_demo_export.png`); the reply carries the written path
//! - `POST /blackbox/figures/action {"name":"snapshot"}` — JSON
//!   summary: seed, theme name, dataset sizes, panel rects, current
//!   hover/brush/bars-selection state

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::input::{MouseButton, PlatformEvent};
use uzor::layout::agent::{AgentAction, AgentActionReply, BlackboxAgentSurface};
use uzor::layout::LayoutManager;
use uzor::platform::types::CornerStyle;
use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_desktop::AppRun as _;

use uzor_export::ExportSpec;
use uzor_figures::interact::hit;
use uzor_figures::{
    BarFigure, BrushState, CurveFigure, FocusSet, HistogramFigure, HitZone, HoverInfo, SelectionBus, FigureInputAction, FigureOutputAction, FigureOverlay, FigureTheme,
};

const AGENT_PORT: u16 = 17482;
const BLACKBOX_SLOT: &str = "figures";

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

/// Screen-pixel move distance beyond which a captured pointer-down is
/// treated as a genuine drag rather than a click — same role as
/// `uzor-graph::engine::CLICK_DRAG_THRESHOLD_PX`.
const CLICK_DRAG_THRESHOLD_PX: f64 = 4.0;

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

// ── Figure builders (shared by draw_frame AND pointer-routing hit-tests
//    — same construction every time, so plot-area/scale math used to
//    ROUTE an event always matches the math used to RENDER it) ────────

fn build_bar_figure(categories: Vec<String>, values: Vec<f64>) -> BarFigure {
    BarFigure::new(categories, values).with_title("Top categories").with_value_labels(true)
}

fn build_curve_figure(points: Vec<(f64, f64)>) -> CurveFigure {
    CurveFigure::new(points).with_title("Cumulative random walk").with_fill(true)
}

fn build_histogram_figure(samples: Vec<f64>) -> HistogramFigure {
    HistogramFigure::new(samples, HISTOGRAM_BIN_COUNT).with_title("Sample distribution")
}

// ── Demo state ───────────────────────────────────────────────────────

/// Which panel (if any) captured the current pointer-down — decides how
/// `PointerMoved`/`PointerUp` are interpreted until the next `PointerUp`.
/// Mirrors `uzor-graph::engine::PointerMode`'s "capture a gesture eagerly
/// on down, reinterpret as a click at release if it never moved much"
/// shape, narrowed to this demo's two gestures (curve = brush drag,
/// bars = click-to-select).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerCapture {
    None,
    Curve,
    Bars,
}

/// Long-lived state behind the blackbox. Owned by [`DemoApp`] and shared
/// with `LayoutManager` through `Arc<Mutex<DemoState>>` so the agent
/// surface can be locked from the HTTP thread.
struct DemoState {
    seed: u64,
    theme: FigureTheme,
    theme_name: &'static str,
    categories: Vec<String>,
    bar_values: Vec<f64>,
    curve_points: Vec<(f64, f64)>,
    hist_samples: Vec<f64>,
    /// Most recent window client size seen by `ui()`. `export_png` and
    /// `snapshot` use this as their render/report resolution.
    last_frame_size: (f64, f64),

    /// Cross-figure interaction state (V2) — hover position + linked
    /// brush interval. Read by `draw_frame` via `FigureOverlay`, written by
    /// pointer routing below (and by the `set_hover`/`set_brush` agent
    /// actions).
    bus: SelectionBus,
    /// Live drag state for the curve panel's brush gesture. Not itself
    /// read by `draw_frame` — its `domain_interval()` is what gets pushed
    /// into `bus.brush` on every drag update.
    curve_brush: BrushState,
    /// Click-to-select state for the bars panel.
    bars_focus: FocusSet,

    capture: PointerCapture,
    /// Pointer position at the start of the current capture (or the last
    /// `PointerMoved` while captured) — used to accumulate `drag_total`.
    pointer_anchor: (f64, f64),
    /// Cumulative screen-pixel move distance since the current capture
    /// began — a brush drag shorter than [`CLICK_DRAG_THRESHOLD_PX`] gets
    /// discarded as a plain click instead.
    drag_total: f64,
}

impl DemoState {
    fn new(seed: u64) -> Self {
        let mut state = Self {
            seed,
            theme: FigureTheme::dark(),
            theme_name: "dark",
            categories: Vec::new(),
            bar_values: Vec::new(),
            curve_points: Vec::new(),
            hist_samples: Vec::new(),
            last_frame_size: (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
            bus: SelectionBus::default(),
            curve_brush: BrushState::default(),
            bars_focus: FocusSet::empty(),
            capture: PointerCapture::None,
            pointer_anchor: (0.0, 0.0),
            drag_total: 0.0,
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

        // Stale interaction state (hover/brush/selection) would otherwise
        // reference a dataset that no longer exists.
        self.bus.clear();
        self.curve_brush.clear();
        self.bars_focus.clear_selection();
        self.bars_focus.set_hovered(None);
        self.capture = PointerCapture::None;
        self.drag_total = 0.0;
    }

    /// Swap the theme. Returns `false` (state left unchanged) for an
    /// unrecognized name.
    fn set_theme(&mut self, name: &str) -> bool {
        match name {
            "dark" => {
                self.theme = FigureTheme::dark();
                self.theme_name = "dark";
                true
            }
            "light" => {
                self.theme = FigureTheme::light();
                self.theme_name = "light";
                true
            }
            _ => false,
        }
    }

    fn panel_rects_now(&self) -> (Rect, Rect, Rect) {
        let (w, h) = self.last_frame_size;
        panel_rects(w, h)
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
            "interaction": {
                "hover": self.bus.hover.map(|h| json!({ "x": h.x, "y": h.y })),
                "brush": self.bus.brush.map(|(x0, x1)| json!({ "x0": x0, "x1": x1 })),
                "bars_selected": self.bars_focus.selected.iter().copied().collect::<Vec<_>>(),
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

    // ── Pointer routing (V2) ────────────────────────────────────────
    //
    // Raw `PlatformEvent`s -> semantic `FigureInputAction`s -> per-panel
    // handler (`BrushState`/`FocusSet`) -> `SelectionBus`. Mirrors
    // `uzor-graph::GraphEngine::on_event`'s shape (eager capture on
    // pointer-down, `drag_total` decides click vs. drag at release) —
    // this demo's own app-level composition, not part of the `uzor-figures`
    // engine itself (design doc's own "app-glued, not engine" boundary).

    fn handle_platform_event(&mut self, event: &PlatformEvent) -> bool {
        match *event {
            PlatformEvent::PointerMoved { x, y } => self.on_pointer_moved(x, y),
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => self.on_pointer_down(x, y),
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => self.on_pointer_up(x, y),
            PlatformEvent::PointerLeft => self.on_pointer_leave(),
            _ => false,
        }
    }

    fn on_pointer_down(&mut self, x: f64, y: f64) -> bool {
        let (bars_rect, _histogram_rect, curve_rect) = self.panel_rects_now();
        self.pointer_anchor = (x, y);
        self.drag_total = 0.0;

        if curve_rect.contains(x, y) {
            self.capture = PointerCapture::Curve;
            if let Some(x_scale) = build_curve_figure(self.curve_points.clone()).x_scale() {
                let area = build_curve_figure(self.curve_points.clone()).plot_area(curve_rect);
                let _ = self.curve_brush.handle(&FigureInputAction::DragStart { x, y }, &area, &x_scale);
            }
            return true;
        }
        if bars_rect.contains(x, y) {
            self.capture = PointerCapture::Bars;
            return true;
        }
        self.capture = PointerCapture::None;
        false
    }

    fn on_pointer_moved(&mut self, x: f64, y: f64) -> bool {
        self.bus.set_hover(Some(HoverInfo { x, y }));

        if self.capture == PointerCapture::Curve {
            let (dx, dy) = (x - self.pointer_anchor.0, y - self.pointer_anchor.1);
            self.drag_total += (dx * dx + dy * dy).sqrt();
            self.pointer_anchor = (x, y);

            let (_bars_rect, _histogram_rect, curve_rect) = self.panel_rects_now();
            if let Some(x_scale) = build_curve_figure(self.curve_points.clone()).x_scale() {
                let area = build_curve_figure(self.curve_points.clone()).plot_area(curve_rect);
                if let FigureOutputAction::BrushChanged { interval } = self.curve_brush.handle(&FigureInputAction::DragMove { x, y }, &area, &x_scale) {
                    self.bus.set_brush(interval);
                }
            }
        }

        true
    }

    fn on_pointer_up(&mut self, x: f64, y: f64) -> bool {
        let (bars_rect, _histogram_rect, curve_rect) = self.panel_rects_now();
        let handled = match self.capture {
            PointerCapture::Curve => {
                if let Some(x_scale) = build_curve_figure(self.curve_points.clone()).x_scale() {
                    let area = build_curve_figure(self.curve_points.clone()).plot_area(curve_rect);
                    let out = self.curve_brush.handle(&FigureInputAction::DragEnd { x, y }, &area, &x_scale);
                    if self.drag_total < CLICK_DRAG_THRESHOLD_PX {
                        // Too small a move to count as a brush drag —
                        // clear it so a plain click on the curve doesn't
                        // leave a phantom zero-width selection.
                        self.curve_brush.clear();
                        self.bus.set_brush(None);
                    } else if let FigureOutputAction::BrushChanged { interval } = out {
                        self.bus.set_brush(interval);
                    }
                }
                true
            }
            PointerCapture::Bars => {
                let bar_figure = build_bar_figure(self.categories.clone(), self.bar_values.clone());
                let area = bar_figure.plot_area(bars_rect);
                if hit::hit_zone(&area, x, y) == HitZone::Plot {
                    if let Some(idx) = hit::bar_index_at(&area, &bar_figure.band_scale(), x) {
                        self.bars_focus.toggle_selected(idx as u64);
                    }
                }
                true
            }
            PointerCapture::None => false,
        };
        self.capture = PointerCapture::None;
        handled
    }

    fn on_pointer_leave(&mut self) -> bool {
        self.bus.set_hover(None);
        self.capture = PointerCapture::None;
        true
    }
}

fn rect_json(r: Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "width": r.width, "height": r.height })
}

/// `uzor/out/` — the shared human-eyeball drop point every headless
/// proof render in this workspace writes into (same directory
/// `uzor-figures`'s own proof tests use). Resolved relative to this crate's
/// manifest dir so it doesn't hardcode a user-specific absolute path.
fn workspace_out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("out")
}

fn default_export_path() -> PathBuf {
    workspace_out_dir().join("figures_demo_export.png")
}

// ── BlackboxAgentSurface ─────────────────────────────────────────────

impl BlackboxAgentSurface for DemoState {
    fn agent_slot_id(&self) -> &str {
        BLACKBOX_SLOT
    }

    fn agent_kind(&self) -> &str {
        "figures"
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
            "set_hover" => {
                let x = action.args.get("x").and_then(Value::as_f64);
                let y = action.args.get("y").and_then(Value::as_f64);
                let (Some(x), Some(y)) = (x, y) else {
                    return AgentActionReply::err("set_hover requires args.x and args.y (f64)");
                };
                self.bus.set_hover(Some(HoverInfo { x, y }));
                AgentActionReply::ok_with_log(json!({ "x": x, "y": y }))
            }
            "set_brush" => {
                let x0 = action.args.get("x0").and_then(Value::as_f64);
                let x1 = action.args.get("x1").and_then(Value::as_f64);
                let (Some(x0), Some(x1)) = (x0, x1) else {
                    return AgentActionReply::err("set_brush requires args.x0 and args.x1 (f64)");
                };
                self.bus.set_brush(Some((x0, x1)));
                AgentActionReply::ok_with_log(json!({ "x0": x0, "x1": x1 }))
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

/// Draw the full 3-panel frame into a `w x h` surface, reacting to
/// `state`'s live `SelectionBus`/`FocusSet` interaction state. Called by
/// both the live window render callback and `export_png` — the one
/// shared draw path that makes headless export prove GUI state,
/// hover/brush/selection included.
fn draw_frame(ctx: &mut dyn RenderContext, w: f64, h: f64, state: &DemoState) {
    ctx.set_fill_color(&state.theme.background);
    ctx.fill_rect(0.0, 0.0, w, h);

    let (bars_rect, histogram_rect, curve_rect) = panel_rects(w, h);
    let hover_px = state.bus.hover.map(|h| (h.x, h.y));

    let bars = build_bar_figure(state.categories.clone(), state.bar_values.clone());
    let bars_overlay = FigureOverlay { hover_px, brush: None, focus: Some(&state.bars_focus) };
    bars.render_with(ctx, bars_rect, &state.theme, &bars_overlay);

    let histogram = build_histogram_figure(state.hist_samples.clone());
    let histogram_overlay = FigureOverlay { hover_px: None, brush: state.bus.brush, focus: None };
    histogram.render_with(ctx, histogram_rect, &state.theme, &histogram_overlay);

    let curve = build_curve_figure(state.curve_points.clone());
    let curve_overlay = FigureOverlay { hover_px, brush: None, focus: None };
    curve.render_with(ctx, curve_rect, &state.theme, &curve_overlay);
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

    fn on_event(&mut self, event: &PlatformEvent) -> bool {
        let mut state = Self::lock(&self.state);
        state.handle_platform_event(event)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DemoApp::new())
        .agent_api(AGENT_PORT)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor-figures — figures demo")
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
    /// `uzor/out/figures_demo_frame.png` for a human to eyeball.
    #[test]
    fn draw_frame_renders_headlessly_to_a_valid_png() {
        let state = DemoState::new(DEFAULT_SEED);
        let spec = ExportSpec { width_px: 1280, height_px: 800, dpr: 1.0, background: None };

        let bytes = render_to_png(&spec, |ctx| draw_frame(ctx, 1280.0, 800.0, &state))
            .expect("draw_frame should render headlessly");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "output should start with the PNG magic bytes");

        let dir = workspace_out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join("figures_demo_frame.png"), &bytes).expect("write proof PNG");
    }

    /// Proves `draw_frame` also renders correctly with a SYNTHETIC
    /// interaction state (hover + linked brush) injected directly into
    /// the `SelectionBus` — exactly what the `set_hover`/`set_brush`
    /// agent actions do, verified here without an HTTP round-trip.
    /// Writes `uzor/out/figures_demo_frame_interactive.png`.
    #[test]
    fn draw_frame_with_synthetic_interaction_state_renders_to_a_valid_png() {
        let mut state = DemoState::new(DEFAULT_SEED);
        let (_bars_rect, _histogram_rect, curve_rect) = panel_rects(1280.0, 800.0);
        state.bus.set_hover(Some(HoverInfo { x: curve_rect.center_x(), y: curve_rect.center_y() }));
        state.bus.set_brush(Some((20.0, 60.0)));

        let spec = ExportSpec { width_px: 1280, height_px: 800, dpr: 1.0, background: None };
        let bytes = render_to_png(&spec, |ctx| draw_frame(ctx, 1280.0, 800.0, &state))
            .expect("draw_frame with synthetic interaction state should render headlessly");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));

        let dir = workspace_out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join("figures_demo_frame_interactive.png"), &bytes).expect("write proof PNG");
    }
}

// ── Wave 6 Commit 2: app-level screenshot-diff harness fixture ───────
//
// `docs/uzor-engines/plans/urx-wave6-autodetect-cutover-design-2026-07-25.md`
// §4.2/§4.3/§4.5 — the FIRST reference fixture for the new
// `uzor_examples::parity_harness`: `draw_frame` is already fully
// headless-provable (the `mod tests` above proves it), so this fixture
// needs ZERO extraction — it reuses `draw_frame`/`DemoState` verbatim,
// just feeding them through the harness's recording/rendering legs
// instead of `uzor_export::render_to_png`.
#[cfg(test)]
mod screenshot_diff {
    use uzor_examples::parity_harness::{
        compare_tight, dump_comparison_pngs, record_via_urx_ctx, render_via_urx_cpu, render_via_urx_native,
        render_via_vello_cpu, ChannelTolerance,
    };

    use super::*;

    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 800;

    /// Byte-tight urx-native-vs-urx-cpu leg (design §4.4) — hard,
    /// automated gate. A real app fixture composing Waves 1-4's
    /// primitives landing outside the base tolerance tier
    /// (`ChannelTolerance::default()`, the SAME `edge=24`/
    /// `interior=2`/`max_differing_fraction=0.02` numbers validated
    /// per-primitive at the crate level) is a genuine compose-level bug
    /// (batching order, state bleed between draw calls) the primitive
    /// suite structurally cannot see.
    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn figures_demo_frame_native_matches_cpu_within_the_base_tier() {
        let state = DemoState::new(DEFAULT_SEED);
        let scene = record_via_urx_ctx(WIDTH, HEIGHT, |ctx| draw_frame(ctx, WIDTH as f64, HEIGHT as f64, &state));

        let cpu = render_via_urx_cpu(&scene, WIDTH, HEIGHT);
        let Some(native) = render_via_urx_native(&scene, WIDTH, HEIGHT) else {
            eprintln!(
                "figures_demo_frame_native_matches_cpu_within_the_base_tier: no GPU/software adapter available; skipping"
            );
            return;
        };

        let tol = ChannelTolerance::default();
        let report = compare_tight(&cpu, &native, tol);
        println!(
            "figures_demo_frame urx-cpu-vs-native: {:.3}% differing (edge {}), budget {:.2}%, max_channel_diff {}",
            report.differing_fraction * 100.0,
            tol.edge,
            tol.max_differing_fraction * 100.0,
            report.max_channel_diff,
        );
        if !report.within_budget {
            dump_comparison_pngs("figures_demo_urx_cpu_vs_native", WIDTH, HEIGHT, &cpu, &native);
        }
        assert!(
            report.within_budget,
            "figures_demo frame exceeded the base tolerance tier: {:.3}% differing (budget {:.2}%), max_channel_diff {}",
            report.differing_fraction * 100.0,
            tol.max_differing_fraction * 100.0,
            report.max_channel_diff,
        );
    }

    /// urx-native vs vello — visual-only leg (design §4.4): dumps
    /// comparison PNGs under `target/parity-app/` UNCONDITIONALLY for
    /// human review, never a computed threshold (different rasterisers,
    /// different AA algorithms, different color-management defaults —
    /// byte parity was never vello's promise). Both legs paint the SAME
    /// `draw_frame` content via the SAME `DemoState`, so any visible
    /// difference is genuinely about the two rasterisers, not a
    /// content mismatch.
    #[test]
    #[ignore = "needs a headless GPU adapter; dumps PNGs for human review, not a hard gate"]
    fn figures_demo_frame_native_vs_vello_visual_dump() {
        let state = DemoState::new(DEFAULT_SEED);
        let vello = render_via_vello_cpu(WIDTH, HEIGHT, |ctx| draw_frame(ctx, WIDTH as f64, HEIGHT as f64, &state));

        let scene = record_via_urx_ctx(WIDTH, HEIGHT, |ctx| draw_frame(ctx, WIDTH as f64, HEIGHT as f64, &state));
        let Some(native) = render_via_urx_native(&scene, WIDTH, HEIGHT) else {
            eprintln!("figures_demo_frame_native_vs_vello_visual_dump: no GPU/software adapter available; skipping");
            return;
        };

        dump_comparison_pngs("figures_demo_urx_native_vs_vello", WIDTH, HEIGHT, &native, &vello);
    }
}
