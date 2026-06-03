//! Real on-screen winit window driven by `UrxEngine` with `Backend::FullGpu`,
//! controllable from outside through a 3-level agentic HTTP API.
//!
//! This is the first end-to-end consumer demo of the URX 1.6 compute
//! pipeline going through the engine façade (NOT bypassing it via
//! `uzor-urx-wgpu-full` directly) AND wired up to an agentic control
//! plane in the same style as `tessera-kernel::system::agent3l`.
//!
//! ## 3-level agentic API
//!
//! Listens on `127.0.0.1:17491` (localhost only, no auth — proof-of-life
//! demo).
//!
//! ### L1 — raw introspection + pixel input
//! | Method | Path | Body / query | Purpose |
//! |---|---|---|---|
//! | `GET`  | `/health`     | — | `{ok: true}` |
//! | `GET`  | `/state`      | — | live snapshot: fps, frame_ms, regions, paused |
//! | `POST` | `/input/click`| `{x, y}` | select first rect under pixel (L1 hit-test) |
//!
//! ### L2 — semantic actions
//! | Method | Path | Body | Purpose |
//! |---|---|---|---|
//! | `POST` | `/act/pause`             | — | freeze physics |
//! | `POST` | `/act/resume`            | — | unfreeze physics |
//! | `POST` | `/act/reset_velocities`  | — | re-randomise velocities |
//! | `POST` | `/act/set_count`         | `{n}` | resize the rect pool to exactly N |
//!
//! ### L3 — structural scene ops
//! | Method | Path | Body | Purpose |
//! |---|---|---|---|
//! | `POST`  | `/scene/spawn_rect` | `{cx, cy, half, color}` | add one rect/region |
//! | `DELETE`| `/scene/region/:id` | — | remove region by id |
//! | `POST`  | `/scene/clear`      | — | remove every rect/region |
//!
//! ## Try it
//!
//! ```bash
//! cargo run -p uzor-urx-engine --features full-gpu-backend \
//!     --example winit_full_gpu_demo --release
//!
//! # in another shell:
//! curl -s http://127.0.0.1:17491/state | jq
//! curl -s -X POST http://127.0.0.1:17491/act/pause
//! curl -s -X POST -H "content-type: application/json" \
//!     -d '{"cx":480,"cy":360,"half":40,"color":[255,80,80,255]}' \
//!     http://127.0.0.1:17491/scene/spawn_rect
//! curl -s -X POST -H "content-type: application/json" \
//!     -d '{"n":256}' http://127.0.0.1:17491/act/set_count
//! curl -s -X POST http://127.0.0.1:17491/scene/clear
//! ```
//!
//! ## Architecture
//!
//! ```text
//!  HTTP thread (tokio rt)              main thread (winit + wgpu)
//!  ────────────────────                ─────────────────────────
//!     |                                       |
//!     | POST /scene/spawn_rect                |
//!     v                                       |
//!  push to spawn_queue ──────────────────►    |
//!     |                                       v
//!     |                                  drain spawn_queue, apply
//!     |                                       |
//!     | GET /state                            v
//!     v                                  publish metrics into shared state
//!  read metrics ◄──────────────────────       |
//! ```
//!
//! All cross-thread sharing is `Arc<Mutex<…>>` — agent never touches
//! the engine directly. The agent enqueues intent; the render thread
//! reconciles before each frame.

#![cfg(feature = "full-gpu-backend")]

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use uzor_urx_core::math::{Color, Rect as UxRect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_engine::cadence::RenderCadence;
use uzor_urx_engine::engine::{RenderTarget, UrxEngine};
use uzor_urx_wgpu_full::{BlitPipeline, TileBuffers, TilePipeline, TILE_SIZE};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const AGENT_PORT:    u16    = 17491;
const DEFAULT_N:     usize  = 64;
const MAX_N:         usize  = 4096;

// ─────────────────────────────────────────────────────────────────────
// Shared agentic state
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RectModel {
    id:     u64,
    cx:     f32,
    cy:     f32,
    vx:     f32,
    vy:     f32,
    half:   f32,
    color:  [u8; 4],
    /// L1 hit-test toggle: drawn with a brighter outline frame when set.
    selected: bool,
}

#[derive(Default)]
struct SharedScene {
    rects:        Vec<RectModel>,
    next_id:      u64,
    paused:       bool,
    /// L3: rects queued by `/scene/spawn_rect`. Drained by main thread.
    pending_spawn: Vec<RectModel>,
    /// L3: clear-all flag.
    pending_clear: bool,
    /// L3: ids enqueued by `DELETE /scene/region/:id`.
    pending_remove: Vec<u64>,
    /// L2: re-randomise velocities flag.
    pending_reset_vel: bool,
    /// L2: target count change.  None = no change.
    pending_set_count: Option<usize>,
    /// L1: pixel click hit-test request. Main thread resolves to a rect id.
    pending_click: Option<(f32, f32)>,
}

#[derive(Default, Clone, Serialize)]
struct LiveMetrics {
    fps:       f32,
    frame_ms:  f32,
    frame_n:   u64,
    regions:   u32,
    paused:    bool,
    win_w:     u32,
    win_h:     u32,
    selected:  Option<u64>,
}

#[derive(Clone, Default)]
struct AgentState {
    scene:   Arc<Mutex<SharedScene>>,
    metrics: Arc<Mutex<LiveMetrics>>,
}

// ─────────────────────────────────────────────────────────────────────
// Initial scene factory + helpers
// ─────────────────────────────────────────────────────────────────────

fn spawn_rect_at_index(i: usize, win_w: f32, win_h: f32, next_id: &mut u64) -> RectModel {
    let half = 16.0 + (i % 5) as f32 * 4.0;
    let cx   = ((i * 71) % 600) as f32 + 40.0;
    let cy   = ((i * 113) % 400) as f32 + 40.0;
    let vx   = if i % 2 == 0 { 80.0 } else { -80.0 };
    let vy   = if i % 3 == 0 { 60.0 } else { -50.0 };
    let color = [
        ((i * 37) & 0xff) as u8,
        ((i * 71) & 0xff) as u8,
        ((i * 113) & 0xff) as u8,
        255,
    ];
    let id = *next_id;
    *next_id += 1;
    RectModel {
        id,
        cx: cx.clamp(half, win_w - half),
        cy: cy.clamp(half, win_h - half),
        vx, vy, half, color, selected: false,
    }
}

fn populate_initial(scene: &mut SharedScene, n: usize, win_w: f32, win_h: f32) {
    scene.rects.clear();
    scene.next_id = 0;
    for i in 0..n.min(MAX_N) {
        let r = spawn_rect_at_index(i, win_w, win_h, &mut scene.next_id);
        scene.rects.push(r);
    }
}

// ─────────────────────────────────────────────────────────────────────
// Agent HTTP server (axum on tokio multi-thread)
// ─────────────────────────────────────────────────────────────────────

fn spawn_agent_server(state: AgentState) -> std::thread::JoinHandle<()> {
    use axum::extract::{Path as AxPath, State as AxState};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::{delete, get, post};
    use axum::{Json, Router};

    #[derive(Deserialize)]
    struct ClickBody { x: f32, y: f32 }
    #[derive(Deserialize)]
    struct SetCountBody { n: usize }
    #[derive(Deserialize)]
    struct SpawnBody { cx: f32, cy: f32, half: f32, color: [u8; 4] }
    #[derive(Serialize)]
    struct OkReply { ok: bool, msg: String }
    fn ok(msg: impl Into<String>) -> Json<OkReply> {
        Json(OkReply { ok: true, msg: msg.into() })
    }

    std::thread::Builder::new()
        .name("urx-fullgpu-demo-agent".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .thread_name("urx-demo-agent-worker")
                .build()
                .expect("tokio rt");

            rt.block_on(async move {
                let app = Router::new()
                    // L1 ----------------------------------------------------------------
                    .route("/health", get(|| async { Json(serde_json::json!({"ok": true})) }))
                    .route("/state",  get(|AxState(s): AxState<AgentState>| async move {
                        let m = s.metrics.lock().unwrap().clone();
                        Json(m)
                    }))
                    .route("/input/click", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<ClickBody>| async move {
                            s.scene.lock().unwrap().pending_click = Some((b.x, b.y));
                            ok(format!("queued click @ ({:.1}, {:.1})", b.x, b.y))
                        },
                    ))
                    // L2 ----------------------------------------------------------------
                    .route("/act/pause", post(
                        |AxState(s): AxState<AgentState>| async move {
                            s.scene.lock().unwrap().paused = true;
                            ok("paused")
                        },
                    ))
                    .route("/act/resume", post(
                        |AxState(s): AxState<AgentState>| async move {
                            s.scene.lock().unwrap().paused = false;
                            ok("resumed")
                        },
                    ))
                    .route("/act/reset_velocities", post(
                        |AxState(s): AxState<AgentState>| async move {
                            s.scene.lock().unwrap().pending_reset_vel = true;
                            ok("reset velocities queued")
                        },
                    ))
                    .route("/act/set_count", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SetCountBody>| async move {
                            if b.n > MAX_N {
                                return (StatusCode::BAD_REQUEST,
                                    Json(OkReply { ok: false,
                                        msg: format!("n must be <= {MAX_N}, got {}", b.n)
                                    })).into_response();
                            }
                            s.scene.lock().unwrap().pending_set_count = Some(b.n);
                            (StatusCode::OK, ok(format!("set_count={} queued", b.n))).into_response()
                        },
                    ))
                    // L3 ----------------------------------------------------------------
                    .route("/scene/spawn_rect", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnBody>| async move {
                            let mut sc = s.scene.lock().unwrap();
                            if sc.rects.len() + sc.pending_spawn.len() >= MAX_N {
                                return (StatusCode::INSUFFICIENT_STORAGE,
                                    Json(OkReply { ok: false,
                                        msg: format!("scene already at MAX_N={MAX_N}"),
                                    })).into_response();
                            }
                            let id = sc.next_id;
                            sc.next_id += 1;
                            let half = b.half.clamp(4.0, 200.0);
                            let m = RectModel {
                                id, cx: b.cx, cy: b.cy, vx: 0.0, vy: 0.0,
                                half, color: b.color, selected: false,
                            };
                            sc.pending_spawn.push(m);
                            (StatusCode::OK, ok(format!("spawn queued, id={id}"))).into_response()
                        },
                    ))
                    .route("/scene/region/:id", delete(
                        |AxState(s): AxState<AgentState>, AxPath(id): AxPath<u64>| async move {
                            s.scene.lock().unwrap().pending_remove.push(id);
                            ok(format!("remove queued id={id}"))
                        },
                    ))
                    .route("/scene/clear", post(
                        |AxState(s): AxState<AgentState>| async move {
                            s.scene.lock().unwrap().pending_clear = true;
                            ok("clear queued")
                        },
                    ))
                    .with_state(state);

                let listener = tokio::net::TcpListener::bind(("127.0.0.1", AGENT_PORT))
                    .await
                    .expect("bind 127.0.0.1:17491");
                eprintln!("[urx-fullgpu-demo agent] listening on http://127.0.0.1:{}", AGENT_PORT);
                axum::serve(listener, app).await.expect("axum serve");
            });
        })
        .expect("spawn agent thread")
}

// ─────────────────────────────────────────────────────────────────────
// GPU state — winit window + wgpu surface + engine
// ─────────────────────────────────────────────────────────────────────

struct GpuState {
    window:        Arc<Window>,
    surface:       wgpu::Surface<'static>,
    device:        wgpu::Device,
    queue:         wgpu::Queue,
    config:        wgpu::SurfaceConfiguration,
    pipeline:      TilePipeline,
    blit:          BlitPipeline,
    bufs:          TileBuffers,
    storage_view:  wgpu::TextureView,
    dummy_glyph:   wgpu::TextureView,
    _dummy_tex:    wgpu::Texture,
    _storage_tex:  wgpu::Texture,
    tex_w:         u32,
    tex_h:         u32,
    engine:        UrxEngine,
    last_tick:     Instant,
    fps_accum:     f32,
    fps_frames:    u32,
    fps_last:      Instant,
    agent:         AgentState,
    cmd_cap:       u32,
}

impl GpuState {
    fn new(window: Arc<Window>, agent: AgentState) -> Self {
        let size = window.inner_size();
        let w = size.width.max(64);
        let h = size.height.max(64);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface  = instance.create_surface(window.clone()).expect("surface");
        let adapter  = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:       wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface:     Some(&surface),
        })).expect("adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label:                 Some("urx-fullgpu-demo-device"),
                required_features:     wgpu::Features::empty(),
                required_limits:       wgpu::Limits::default(),
                memory_hints:          wgpu::MemoryHints::default(),
                trace:                 wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            },
        )).expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied()
            .find(|f| matches!(f,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm))
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage:        wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width:        w,
            height:       h,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode:   caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let pipeline = TilePipeline::new(&device);
        let blit     = BlitPipeline::new(&device, format);
        // Pool capacity: MAX_N rects, each potentially producing 2 cmds
        // (1 fill + 1 outline frame for "selected"); also leave 1× slack
        // for spawns mid-frame.
        let cmd_cap = (MAX_N as u32) * 2;
        let (bufs, storage_tex, storage_view) =
            TileBuffers::with_output_texture(&device, cmd_cap, w, h);
        let tex_w = bufs.tile_count_x * TILE_SIZE;
        let tex_h = bufs.tile_count_y * TILE_SIZE;
        let (dummy_tex, dummy_glyph) = TilePipeline::dummy_glyph_atlas(&device);

        let engine = UrxEngine::new_full_gpu(w, h);

        // Initial scene = DEFAULT_N rects.
        {
            let mut sc = agent.scene.lock().unwrap();
            populate_initial(&mut sc, DEFAULT_N, w as f32, h as f32);
        }
        agent.metrics.lock().unwrap().win_w = w;
        agent.metrics.lock().unwrap().win_h = h;

        Self {
            window, surface, device, queue, config,
            pipeline, blit, bufs, storage_view, dummy_glyph,
            _dummy_tex: dummy_tex, _storage_tex: storage_tex,
            tex_w, tex_h, engine,
            last_tick: Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
            fps_last:   Instant::now(),
            agent,
            cmd_cap,
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        self.config.width  = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        let (bufs, storage_tex, storage_view) =
            TileBuffers::with_output_texture(&self.device, self.cmd_cap, w, h);
        self.tex_w = bufs.tile_count_x * TILE_SIZE;
        self.tex_h = bufs.tile_count_y * TILE_SIZE;
        self.bufs = bufs;
        self.storage_view = storage_view;
        self._storage_tex = storage_tex;
        self.engine = UrxEngine::new_full_gpu(w, h);
        let mut m = self.agent.metrics.lock().unwrap();
        m.win_w = w;
        m.win_h = h;
    }

    /// Pull every queued agent intent into concrete scene changes and
    /// then update region state on the engine.
    fn reconcile_and_tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_tick).as_secs_f32().min(0.05);
        self.last_tick = now;

        let win_w = self.config.width  as f32;
        let win_h = self.config.height as f32;

        let mut sc = self.agent.scene.lock().unwrap();

        // ─ L3: clear ─────────────────────────────────────────────────
        if sc.pending_clear {
            sc.pending_clear = false;
            // Remove all engine regions BEFORE clearing the model (we
            // need the ids that still exist).
            for r in sc.rects.iter() {
                self.engine.remove_region(RegionId(r.id));
            }
            sc.rects.clear();
        }

        // ─ L3: remove by id ──────────────────────────────────────────
        if !sc.pending_remove.is_empty() {
            let ids = std::mem::take(&mut sc.pending_remove);
            for id in ids {
                if let Some(pos) = sc.rects.iter().position(|r| r.id == id) {
                    sc.rects.remove(pos);
                    self.engine.remove_region(RegionId(id));
                }
            }
        }

        // ─ L3: spawn queued ──────────────────────────────────────────
        if !sc.pending_spawn.is_empty() {
            let queued = std::mem::take(&mut sc.pending_spawn);
            for r in queued {
                if sc.rects.len() < MAX_N {
                    sc.rects.push(r);
                }
            }
        }

        // ─ L2: set_count ─────────────────────────────────────────────
        if let Some(target) = sc.pending_set_count.take() {
            let target = target.min(MAX_N);
            // Shrink: drop tail.
            while sc.rects.len() > target {
                if let Some(r) = sc.rects.pop() {
                    self.engine.remove_region(RegionId(r.id));
                }
            }
            // Grow: append new defaults at indices we don't already have.
            let mut spawn_idx = sc.rects.len();
            while sc.rects.len() < target {
                let r = spawn_rect_at_index(spawn_idx, win_w, win_h, &mut sc.next_id);
                sc.rects.push(r);
                spawn_idx += 1;
            }
        }

        // ─ L2: reset velocities ──────────────────────────────────────
        if sc.pending_reset_vel {
            sc.pending_reset_vel = false;
            for (i, r) in sc.rects.iter_mut().enumerate() {
                r.vx = if i % 2 == 0 { 80.0 } else { -80.0 };
                r.vy = if i % 3 == 0 { 60.0 } else { -50.0 };
            }
        }

        // ─ L1: click hit-test → toggle "selected" on first match ─────
        let mut selected_id: Option<u64> = None;
        if let Some((cx, cy)) = sc.pending_click.take() {
            for r in sc.rects.iter_mut() { r.selected = false; }
            if let Some(hit) = sc.rects.iter_mut().rev().find(|r| {
                (cx - r.cx).abs() <= r.half && (cy - r.cy).abs() <= r.half
            }) {
                hit.selected = true;
                selected_id  = Some(hit.id);
            }
        } else {
            selected_id = sc.rects.iter().find(|r| r.selected).map(|r| r.id);
        }

        // ─ Physics tick (paused gate) ────────────────────────────────
        if !sc.paused {
            for r in sc.rects.iter_mut() {
                r.cx += r.vx * dt;
                r.cy += r.vy * dt;
                if r.cx - r.half < 0.0 || r.cx + r.half > win_w {
                    r.vx = -r.vx;
                    r.cx = r.cx.clamp(r.half, win_w - r.half);
                }
                if r.cy - r.half < 0.0 || r.cy + r.half > win_h {
                    r.vy = -r.vy;
                    r.cy = r.cy.clamp(r.half, win_h - r.half);
                }
            }
        }

        // ─ Push to engine: upsert all alive regions ──────────────────
        for r in sc.rects.iter() {
            let mut scene = Scene::new();
            let body_w = (r.half * 2.0) as f64;
            let body_h = (r.half * 2.0) as f64;
            scene.fill_rect_solid(
                UxRect::new(0.0, 0.0, body_w, body_h),
                Color::rgba8(r.color[0], r.color[1], r.color[2], r.color[3]),
            );
            // L1 selected → bright white inner band painted on top.
            if r.selected {
                scene.fill_rect_solid(
                    UxRect::new(2.0, 2.0, body_w - 2.0, 6.0),
                    Color::rgba8(255, 255, 255, 255),
                );
                scene.fill_rect_solid(
                    UxRect::new(2.0, body_h - 6.0, body_w - 2.0, body_h - 2.0),
                    Color::rgba8(255, 255, 255, 255),
                );
            }
            self.engine.upsert_region(
                RegionId(r.id),
                scene,
                UxRect::new((r.cx - r.half) as f64, (r.cy - r.half) as f64,
                             (r.cx + r.half) as f64, (r.cy + r.half) as f64),
                RenderCadence::HighHz,
            );
        }

        let paused  = sc.paused;
        let regions = sc.rects.len() as u32;
        drop(sc);

        // Publish to metrics for L1 /state polling.
        let mut m = self.agent.metrics.lock().unwrap();
        m.regions  = regions;
        m.paused   = paused;
        m.selected = selected_id;
    }

    fn render_frame(&mut self) {
        let frame_t = Instant::now();
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("surface error: {e:?}");
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("urx-fullgpu-demo-encoder") },
        );
        let res = self.engine.render(RenderTarget::FullGpu {
            pipeline:         &self.pipeline,
            blit:             &self.blit,
            bufs:             &self.bufs,
            device:           &self.device,
            queue:            &self.queue,
            encoder:          &mut enc,
            storage_view:     &self.storage_view,
            target_view:      &view,
            glyph_atlas_view: &self.dummy_glyph,
            src_w:            self.tex_w,
            src_h:            self.tex_h,
        });
        if let Err(e) = res {
            eprintln!("engine render error: {e:?}");
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();

        let dt = frame_t.elapsed().as_secs_f32() * 1000.0;
        self.fps_accum += dt;
        self.fps_frames += 1;
        let now = Instant::now();
        if (now - self.fps_last).as_secs_f32() >= 0.5 {
            let avg_ms  = self.fps_accum / self.fps_frames as f32;
            let est_fps = 1000.0 / avg_ms.max(0.01);
            let regions = self.engine.region_count();
            self.window.set_title(&format!(
                "URX 1.6 FullGpu engine demo — {} regions @ {:.1} FPS / {:.2} ms — agent :{AGENT_PORT}",
                regions, est_fps, avg_ms,
            ));
            // Publish to /state.
            let mut m = self.agent.metrics.lock().unwrap();
            m.fps      = est_fps;
            m.frame_ms = avg_ms;
            m.frame_n += self.fps_frames as u64;
            self.fps_accum = 0.0;
            self.fps_frames = 0;
            self.fps_last = now;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// winit app shell
// ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct DemoApp {
    state: Option<GpuState>,
    agent: AgentState,
    // L1: cursor position for click-to-select.
    last_cursor: (f32, f32),
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }
        let attrs = Window::default_attributes()
            .with_title("URX 1.6 FullGpu engine demo — starting…")
            .with_inner_size(winit::dpi::PhysicalSize::new(960u32, 720u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        self.state = Some(GpuState::new(window.clone(), self.agent.clone()));
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return; };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event: ke, .. } => {
                if let PhysicalKey::Code(KeyCode::Escape) = ke.physical_key {
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state: bs, .. } => {
                if bs == winit::event::ElementState::Pressed {
                    self.agent.scene.lock().unwrap().pending_click = Some(self.last_cursor);
                }
            }
            WindowEvent::Resized(sz) => {
                state.resize(sz.width, sz.height);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.reconcile_and_tick();
                state.render_frame();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let agent = AgentState {
        scene:   Arc::new(Mutex::new(SharedScene::default())),
        metrics: Arc::new(Mutex::new(LiveMetrics::default())),
    };
    let _agent_join = spawn_agent_server(agent.clone());

    let event_loop = EventLoop::new().expect("event_loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = DemoApp { agent, ..Default::default() };
    event_loop.run_app(&mut app).expect("run_app");
}
