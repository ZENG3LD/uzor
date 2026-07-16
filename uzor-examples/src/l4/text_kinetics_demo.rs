//! # `uzor-text` kinetics demo — resize-morph between two `max_width`s
//!
//! Arc 2 Phase 3 proof-of-life for `uzor_text::kinetics`
//! (`build_morph`/`sample`/`sample_layout`, see
//! `nemo/docs/uzor-viz/uzor_text_arc2_design.md` §4 Phase 3 and
//! `uzor-text/CLAUDE.md`'s "Phase 3" section for the exact API + its
//! divergences from the doc's own sketch). One fixed seeded paragraph,
//! laid out at a narrow and a wide `max_width`, continuously morphing
//! (yoyo loop) between the two — every glyph's position lerps from its
//! narrow-layout slot to its wide-layout slot (the doc's "resize-morph"
//! case: same text, only width changes).
//!
//! Run:
//! ```sh
//! cargo run -p uzor-examples --bin text-kinetics-demo
//! ```
//!
//! Agent-api control surface, port 17484 — `slot_id = "text-kinetics"`.
//! (Port note: 17483 is already live-claimed elsewhere in the workspace
//! by tessera's Agentic3L widget-demo — the design doc's own Phase 1
//! section already flags this exact collision and reserves 17484 for the
//! next `uzor-text` demo; Phase 1/2 never actually built one, so this
//! Phase 3 demo is the first to claim it.)
//!
//! - `GET  /blackbox/text-kinetics/state` — same shape as `snapshot`
//! - `POST /blackbox/text-kinetics/action {"name":"set_t","args":{"t":0.5}}`
//!   — pause the loop and scrub to an exact `t` (clamped to `[0, 1]`)
//! - `POST /blackbox/text-kinetics/action {"name":"set_widths","args":{"from":180.0,"to":700.0}}`
//!   — rebuild both layouts + the morph at new narrow/wide `max_width`s,
//!   reset `t` to `0.0`
//! - `POST /blackbox/text-kinetics/action {"name":"play","args":{"duration_s":3.0}}`
//!   — (re)start the yoyo loop with a `duration_s` full `0 -> 1` sweep
//! - `POST /blackbox/text-kinetics/action {"name":"snapshot"}` — JSON
//!   summary: `t`, `playing`, both widths, matched/from_only/to_only
//!   glyph counts
//! - `POST /blackbox/text-kinetics/action {"name":"export_png","args":{"path":"out/custom.png"}}`
//!   — headless-render the CURRENT sampled frame via
//!   `uzor_export::render_to_png_file`; `path` optional (defaults to
//!   `uzor/out/text_morph_demo_export.png`)

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use uzor::fonts::FontFamily;
use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::layout::agent::{AgentAction, AgentActionReply, BlackboxAgentSurface};
use uzor::layout::LayoutManager;
use uzor::platform::types::CornerStyle;
use uzor::render::{RenderContext, RenderRegion, UNCAPPED_FPS};
use uzor::types::Rect;
use uzor_desktop::AppRun as _;

use uzor_export::ExportSpec;
use uzor_text::kinetics::{build_morph, sample_layout, MorphTransition};
use uzor_text::{draw_paragraph, layout_text, CosmicShaper, FontSpec, ParagraphLayout};

const AGENT_PORT: u16 = 17484;
const BLACKBOX_SLOT: &str = "text-kinetics";
const REGION_ID: &str = "text-kinetics:main";

const DEFAULT_WINDOW_WIDTH: f64 = 900.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 600.0;
const MARGIN: f64 = 30.0;

const DEFAULT_NARROW_WIDTH: f64 = 220.0;
const DEFAULT_WIDE_WIDTH: f64 = 780.0;
const DEFAULT_LOOP_SECONDS: f64 = 3.0;
const MIN_LOOP_SECONDS: f64 = 0.05;

const TEXT_COLOR: &str = "#e8e8e8";
const TEXT_COLOR_RGB: u32 = 0xe8_e8_e8_00;
const BACKGROUND_COLOR: &str = "#0d0f14";

/// Fixed seeded sample paragraph — no lorem-ipsum RNG (design law 8).
const PARAGRAPH_TEXT: &str = "The quick brown fox jumps over the lazy dog \
    and then keeps running further down the road without stopping, a fixed \
    seeded sample paragraph that continuously re-wraps between a narrow \
    and a wide column so the Phase 3 kinetics morph has real glyphs to \
    interpolate between two different layouts of the exact same text.";

// ── Demo state ───────────────────────────────────────────────────────

/// Long-lived state behind the blackbox. Owned by [`DemoApp`] and shared
/// with `LayoutManager` through `Arc<Mutex<DemoState>>` so the agent
/// surface can be locked from the HTTP thread.
struct DemoState {
    font: FontSpec,

    from_width: f64,
    to_width: f64,
    from_layout: ParagraphLayout,
    to_layout: ParagraphLayout,
    morph: MorphTransition,

    /// Current scrub position in `[0, 1]`.
    t: f64,
    /// `+1.0` while sweeping toward `1.0`, `-1.0` while sweeping back
    /// toward `0.0` — the yoyo loop.
    direction: f64,
    playing: bool,
    /// Full `0 -> 1` sweep duration.
    play_duration: Duration,
    last_frame_at: Option<Instant>,

    canvas_rect: Rect,
    dirty: bool,
}

impl DemoState {
    fn new() -> Self {
        let font = FontSpec::new(FontFamily::Roboto, 20.0);
        let (from_layout, to_layout, morph) = build_layouts(&font, DEFAULT_NARROW_WIDTH, DEFAULT_WIDE_WIDTH);

        Self {
            font,
            from_width: DEFAULT_NARROW_WIDTH,
            to_width: DEFAULT_WIDE_WIDTH,
            from_layout,
            to_layout,
            morph,
            t: 0.0,
            direction: 1.0,
            playing: true,
            play_duration: Duration::from_secs_f64(DEFAULT_LOOP_SECONDS),
            last_frame_at: None,
            canvas_rect: Rect::new(0.0, 0.0, DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
            dirty: true,
        }
    }

    fn set_widths(&mut self, from_width: f64, to_width: f64) {
        let (from_layout, to_layout, morph) = build_layouts(&self.font, from_width, to_width);
        self.from_width = from_width;
        self.to_width = to_width;
        self.from_layout = from_layout;
        self.to_layout = to_layout;
        self.morph = morph;
        self.t = 0.0;
        self.last_frame_at = None;
        self.dirty = true;
    }

    fn set_t(&mut self, t: f64) {
        self.playing = false;
        self.t = t.clamp(0.0, 1.0);
        self.dirty = true;
    }

    fn play(&mut self, duration_s: f64) {
        self.playing = true;
        self.play_duration = Duration::from_secs_f64(duration_s.max(MIN_LOOP_SECONDS));
        self.last_frame_at = None;
        self.dirty = true;
    }

    /// Advance `t` by real elapsed time since the previous call, yoyo-ing
    /// at each end — a no-op while paused. Mirrors
    /// `uzor-graph::GraphEngine::tick_real_time`'s dt-clamp convention.
    fn advance(&mut self) {
        if !self.playing {
            return;
        }
        let now = Instant::now();
        let dt = match self.last_frame_at {
            Some(prev) => now.duration_since(prev).as_secs_f64().min(0.1),
            None => 1.0 / 60.0,
        };
        self.last_frame_at = Some(now);

        let sweep = self.play_duration.as_secs_f64().max(MIN_LOOP_SECONDS);
        self.t += (dt / sweep) * self.direction;
        if self.t >= 1.0 {
            self.t = 1.0;
            self.direction = -1.0;
        } else if self.t <= 0.0 {
            self.t = 0.0;
            self.direction = 1.0;
        }
        self.dirty = true;
    }

    /// The `RenderRegion` this demo wants: continuous while the loop
    /// plays, dirty-driven once paused — the same freeze/wake split
    /// `uzor-graph::GraphEngine::render_region` uses (design law 5).
    fn render_region(&self) -> RenderRegion {
        if self.playing {
            RenderRegion { id: REGION_ID, rect: self.canvas_rect, target_fps: UNCAPPED_FPS, dirty: true }
        } else {
            RenderRegion { id: REGION_ID, rect: self.canvas_rect, target_fps: 0, dirty: self.dirty }
        }
    }

    fn snapshot_json(&self) -> Value {
        json!({
            "t": self.t,
            "playing": self.playing,
            "from_width": self.from_width,
            "to_width": self.to_width,
            "loop_seconds": self.play_duration.as_secs_f64(),
            "glyphs": {
                "from": self.from_layout.glyphs.len(),
                "to": self.to_layout.glyphs.len(),
                "matched": self.morph.matched_count(),
                "from_only": self.morph.from_only_count(),
                "to_only": self.morph.to_only_count(),
            },
        })
    }

    /// Render the current frame headlessly at `canvas_rect`'s size and
    /// write it to `path_arg` (or the default export path). Uses
    /// [`draw_frame`] — the same function the live window paints with.
    fn export_png(&self, path_arg: Option<&str>) -> AgentActionReply {
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

        let (w, h) = (self.canvas_rect.width.max(1.0), self.canvas_rect.height.max(1.0));
        let spec = ExportSpec { width_px: w as u32, height_px: h as u32, dpr: 1.0, background: None };
        match uzor_export::render_to_png_file(&path, &spec, |ctx| draw_frame(ctx, w, h, self)) {
            Ok(()) => AgentActionReply::ok_with_log(json!({ "path": path.display().to_string() })),
            Err(e) => AgentActionReply::err(format!("export_png failed: {e}")),
        }
    }
}

/// Lay out [`PARAGRAPH_TEXT`] at both widths and build the morph between
/// them — the one place this demo touches `uzor_text::layout_text`, so
/// `new()` and `set_widths` always rebuild identically.
fn build_layouts(font: &FontSpec, from_width: f64, to_width: f64) -> (ParagraphLayout, ParagraphLayout, MorphTransition) {
    let shaper = CosmicShaper::headless();
    let from_layout = layout_text(PARAGRAPH_TEXT, font, from_width, &shaper);
    let to_layout = layout_text(PARAGRAPH_TEXT, font, to_width, &shaper);
    let morph = build_morph(&from_layout, &to_layout);
    (from_layout, to_layout, morph)
}

fn default_export_path() -> PathBuf {
    workspace_out_dir().join("text_morph_demo_export.png")
}

/// `uzor/out/` — the shared human-eyeball drop point every headless
/// proof render in this workspace writes into. Resolved relative to this
/// crate's manifest dir so it doesn't hardcode a user-specific absolute
/// path.
fn workspace_out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("out")
}

/// Draw the current sampled morph frame into a `w x h` surface. Called by
/// both the live window render callback and `export_png` — the one
/// shared draw path that makes headless export prove GUI state.
fn draw_frame(ctx: &mut dyn RenderContext, w: f64, h: f64, state: &DemoState) {
    ctx.set_fill_color(BACKGROUND_COLOR);
    ctx.fill_rect(0.0, 0.0, w, h);

    let sampled = sample_layout(&state.morph, state.t, TEXT_COLOR_RGB);
    draw_paragraph(ctx, (MARGIN, MARGIN), &sampled, TEXT_COLOR, false);
}

// ── BlackboxAgentSurface ─────────────────────────────────────────────

impl BlackboxAgentSurface for DemoState {
    fn agent_slot_id(&self) -> &str {
        BLACKBOX_SLOT
    }

    fn agent_kind(&self) -> &str {
        "text-kinetics"
    }

    fn agent_state(&self) -> Value {
        self.snapshot_json()
    }

    fn apply_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        match action.name.as_str() {
            "set_t" => {
                let Some(t) = action.args.get("t").and_then(Value::as_f64) else {
                    return AgentActionReply::err("set_t requires args.t (f64)");
                };
                self.set_t(t);
                AgentActionReply::ok_with_log(json!({ "t": self.t }))
            }
            "set_widths" => {
                let from = action.args.get("from").and_then(Value::as_f64);
                let to = action.args.get("to").and_then(Value::as_f64);
                let (Some(from), Some(to)) = (from, to) else {
                    return AgentActionReply::err("set_widths requires args.from and args.to (f64)");
                };
                if from <= 0.0 || to <= 0.0 {
                    return AgentActionReply::err("set_widths requires positive widths");
                }
                self.set_widths(from, to);
                AgentActionReply::ok_with_log(json!({ "from": from, "to": to }))
            }
            "play" => {
                let duration_s = action.args.get("duration_s").and_then(Value::as_f64).unwrap_or(DEFAULT_LOOP_SECONDS);
                self.play(duration_s);
                AgentActionReply::ok_with_log(json!({ "duration_s": self.play_duration.as_secs_f64() }))
            }
            "snapshot" => AgentActionReply::ok_with_log(self.snapshot_json()),
            "export_png" => {
                let path_arg = action.args.get("path").and_then(Value::as_str);
                self.export_png(path_arg)
            }
            other => AgentActionReply::err(format!("unknown action {other:?}")),
        }
    }
}

// ── App ──────────────────────────────────────────────────────────────

struct DemoApp {
    state: Arc<Mutex<DemoState>>,
}

impl DemoApp {
    fn new() -> Self {
        Self { state: Arc::new(Mutex::new(DemoState::new())) }
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
            state.canvas_rect = Rect::new(0.0, 0.0, raw_w, raw_h);
        }
        state.advance();

        let w = raw_w.max(1.0);
        let h = raw_h.max(1.0);
        draw_frame(win.render, w, h, &state);
        state.dirty = false;
    }

    fn regions(&mut self) -> Vec<RenderRegion> {
        let state = Self::lock(&self.state);
        vec![state.render_region()]
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DemoApp::new())
        .agent_api(AGENT_PORT)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor-text — kinetics morph demo")
                .size(DEFAULT_WINDOW_WIDTH as u32, DEFAULT_WINDOW_HEIGHT as u32)
                .min_size(500, 300)
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
    /// GPU) — the same function the live app uses. Writes
    /// `uzor/out/text_morph_demo_frame.png` for a human to eyeball.
    #[test]
    fn draw_frame_renders_headlessly_to_a_valid_png() {
        let state = DemoState::new();
        let spec =
            ExportSpec { width_px: DEFAULT_WINDOW_WIDTH as u32, height_px: DEFAULT_WINDOW_HEIGHT as u32, dpr: 1.0, background: None };

        let bytes = render_to_png(&spec, |ctx| draw_frame(ctx, DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT, &state))
            .expect("draw_frame should render headlessly");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));

        let dir = workspace_out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join("text_morph_demo_frame.png"), &bytes).expect("write proof PNG");
    }

    /// `set_t`/`set_widths`/`play` mutate state without panicking and
    /// keep `t` inside `[0, 1]`.
    #[test]
    fn agent_actions_mutate_state_and_keep_t_in_range() {
        let mut state = DemoState::new();

        state.set_t(1.5);
        assert_eq!(state.t, 1.0);
        assert!(!state.playing);

        state.set_t(-0.5);
        assert_eq!(state.t, 0.0);

        state.set_widths(160.0, 640.0);
        assert_eq!(state.from_width, 160.0);
        assert_eq!(state.to_width, 640.0);
        assert_eq!(state.t, 0.0);
        assert!(state.morph.matched_count() > 0);

        state.play(2.0);
        assert!(state.playing);
        assert_eq!(state.play_duration, Duration::from_secs_f64(2.0));
    }

    /// `advance()` never overshoots `[0, 1]` and flips `direction` at the
    /// `1.0` boundary. Deterministic — forces a large synthetic `dt` via
    /// `last_frame_at` instead of sleeping on the wall clock.
    #[test]
    fn advance_yoyos_at_the_boundary_without_overshoot() {
        let mut state = DemoState::new();
        state.play(0.05);
        state.t = 0.99;
        state.direction = 1.0;
        // `advance()`'s dt clamps to 0.1s regardless of how stale
        // `last_frame_at` is, so a far-past instant deterministically
        // exercises the clamp-and-flip path with no real sleep needed.
        state.last_frame_at = Some(Instant::now() - Duration::from_secs(10));

        state.advance();

        assert!((0.0..=1.0).contains(&state.t));
        assert_eq!(state.t, 1.0, "a dt/sweep step this large must clamp exactly to the boundary");
        assert_eq!(state.direction, -1.0, "hitting the 1.0 boundary must flip direction to reverse");
    }
}
