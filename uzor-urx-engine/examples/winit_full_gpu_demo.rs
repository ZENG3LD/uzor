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
//! | `POST`  | `/scene/spawn_rect`        | `{cx, cy, half, color}`              | add one rect/region |
//! | `POST`  | `/scene/spawn_stroke`      | `{p0, p1, width, color, cap?}`       | add one stroke line region |
//! | `POST`  | `/scene/spawn_sparkline`   | `{points: [[x,y],…], width, color}`  | one region = a polyline |
//! | `POST`  | `/scene/spawn_bezier`      | `{p0, c0, c1, p1, width, color}`     | cubic Bézier — kurbo flattens into a Path cmd |
//! | `POST`  | `/scene/spawn_fill_path`   | `{points: [[x,y],…], color}`         | filled polygon (non-zero winding) |
//! | `POST`  | `/scene/spawn_multi_grad`  | `{bbox, stops: [[pos,r,g,b,a],…], direction}` | N-stop linear gradient rect |
//! | `POST`  | `/scene/spawn_image`       | `{bbox, uv?, tint?}`                 | sample the 64×64 checkerboard atlas into rect |
//! | `POST`  | `/scene/preset/dashboard`  | —                                    | replace scene with grid + sparkline preset |
//! | `DELETE`| `/scene/region/:id`        | —                                    | remove region by id |
//! | `POST`  | `/scene/clear`             | —                                    | remove every rect/region |
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
use uzor_urx_core::math::{Affine, BezPath, Brush, Color, Point, Rect as UxRect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StrokeModel {
    id:    u64,
    p0:    [f32; 2],
    p1:    [f32; 2],
    width: f32,
    color: [u8; 4],
    /// 0 = butt, 1 = round, 2 = square.
    cap:   u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PolylineModel {
    id:     u64,
    points: Vec<[f32; 2]>,
    width:  f32,
    color:  [u8; 4],
}

/// Cubic-Bézier path: caller supplies anchor + control points. The
/// demo packs them into a kurbo `BezPath` and pushes a `DrawCommand::
/// StrokePath` — the encoder will flatten via kurbo at upload time.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ImageModel {
    id:   u64,
    /// Screen-space rect to fill `[x0, y0, x1, y1]`.
    bbox: [f32; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MultiGradModel {
    id:        u64,
    /// Rect bbox `[x0, y0, x1, y1]`.
    bbox:      [f32; 4],
    /// Stops as `[position, r, g, b, a]`. `position` in `[0, 1]`.
    stops:     Vec<[f32; 5]>,
    /// 0=H, 1=V, 2=TLBR, 3=BLTR (matches `lin_dir::*`).
    direction: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FillPathModel {
    id:     u64,
    points: Vec<[f32; 2]>,
    color:  [u8; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BezierModel {
    id:    u64,
    /// Start anchor.
    p0:    [f32; 2],
    /// First control point.
    c0:    [f32; 2],
    /// Second control point.
    c1:    [f32; 2],
    /// End anchor.
    p1:    [f32; 2],
    width: f32,
    color: [u8; 4],
}

#[derive(Default)]
struct SharedScene {
    rects:        Vec<RectModel>,
    strokes:      Vec<StrokeModel>,
    polylines:    Vec<PolylineModel>,
    beziers:      Vec<BezierModel>,
    fills:        Vec<FillPathModel>,
    multigrads:   Vec<MultiGradModel>,
    images:       Vec<ImageModel>,
    next_id:      u64,
    paused:       bool,
    /// L3: rects queued by `/scene/spawn_rect`. Drained by main thread.
    pending_spawn:    Vec<RectModel>,
    pending_strokes:  Vec<StrokeModel>,
    pending_polys:    Vec<PolylineModel>,
    pending_beziers:    Vec<BezierModel>,
    pending_fills:      Vec<FillPathModel>,
    pending_multigrads: Vec<MultiGradModel>,
    pending_images:     Vec<ImageModel>,
    /// L3: clear-all flag.
    pending_clear: bool,
    /// L3: load dashboard preset flag.
    pending_preset_dashboard: bool,
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
    #[derive(Deserialize)]
    struct SpawnStrokeBody {
        p0: [f32; 2], p1: [f32; 2], width: f32, color: [u8; 4],
        #[serde(default)]
        cap: Option<u32>,
    }
    #[derive(Deserialize)]
    struct SpawnSparklineBody { points: Vec<[f32; 2]>, width: f32, color: [u8; 4] }
    #[derive(Deserialize)]
    struct SpawnBezierBody {
        p0: [f32; 2], c0: [f32; 2], c1: [f32; 2], p1: [f32; 2],
        width: f32, color: [u8; 4],
    }
    #[derive(Deserialize)]
    struct SpawnFillPathBody { points: Vec<[f32; 2]>, color: [u8; 4] }
    #[derive(Deserialize)]
    struct SpawnMultiGradBody {
        bbox:      [f32; 4],
        stops:     Vec<[f32; 5]>,
        direction: Option<u32>,
    }
    #[derive(Deserialize)]
    struct SpawnImageBody { bbox: [f32; 4] }
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
                    .route("/scene/spawn_stroke", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnStrokeBody>| async move {
                            let mut sc = s.scene.lock().unwrap();
                            let id = sc.next_id; sc.next_id += 1;
                            sc.pending_strokes.push(StrokeModel {
                                id,
                                p0: b.p0, p1: b.p1,
                                width: b.width.max(0.5),
                                color: b.color,
                                cap: b.cap.unwrap_or(1).min(2),
                            });
                            ok(format!("stroke queued, id={id}"))
                        },
                    ))
                    .route("/scene/spawn_sparkline", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnSparklineBody>| async move {
                            if b.points.len() < 2 {
                                return (StatusCode::BAD_REQUEST,
                                    Json(OkReply { ok: false,
                                        msg: "sparkline needs >= 2 points".into() }))
                                    .into_response();
                            }
                            let mut sc = s.scene.lock().unwrap();
                            let id = sc.next_id; sc.next_id += 1;
                            sc.pending_polys.push(PolylineModel {
                                id,
                                points: b.points,
                                width: b.width.max(0.5),
                                color: b.color,
                            });
                            (StatusCode::OK, ok(format!("sparkline queued, id={id}"))).into_response()
                        },
                    ))
                    .route("/scene/spawn_bezier", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnBezierBody>| async move {
                            let mut sc = s.scene.lock().unwrap();
                            let id = sc.next_id; sc.next_id += 1;
                            sc.pending_beziers.push(BezierModel {
                                id,
                                p0: b.p0, c0: b.c0, c1: b.c1, p1: b.p1,
                                width: b.width.max(0.5),
                                color: b.color,
                            });
                            ok(format!("bezier queued, id={id}"))
                        },
                    ))
                    .route("/scene/spawn_fill_path", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnFillPathBody>| async move {
                            if b.points.len() < 3 {
                                return (StatusCode::BAD_REQUEST,
                                    Json(OkReply { ok: false,
                                        msg: "fill_path needs >= 3 points".into() }))
                                    .into_response();
                            }
                            let mut sc = s.scene.lock().unwrap();
                            let id = sc.next_id; sc.next_id += 1;
                            sc.pending_fills.push(FillPathModel {
                                id, points: b.points, color: b.color,
                            });
                            (StatusCode::OK, ok(format!("fill_path queued, id={id}"))).into_response()
                        },
                    ))
                    .route("/scene/spawn_multi_grad", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnMultiGradBody>| async move {
                            if b.stops.len() < 2 {
                                return (StatusCode::BAD_REQUEST,
                                    Json(OkReply { ok: false,
                                        msg: "multi_grad needs >= 2 stops".into() }))
                                    .into_response();
                            }
                            let mut sc = s.scene.lock().unwrap();
                            let id = sc.next_id; sc.next_id += 1;
                            sc.pending_multigrads.push(MultiGradModel {
                                id, bbox: b.bbox, stops: b.stops,
                                direction: b.direction.unwrap_or(0).min(3),
                            });
                            (StatusCode::OK, ok(format!("multi_grad queued, id={id}"))).into_response()
                        },
                    ))
                    .route("/scene/spawn_image", post(
                        |AxState(s): AxState<AgentState>, Json(b): Json<SpawnImageBody>| async move {
                            let mut sc = s.scene.lock().unwrap();
                            let id = sc.next_id; sc.next_id += 1;
                            sc.pending_images.push(ImageModel { id, bbox: b.bbox });
                            ok(format!("image queued, id={id}"))
                        },
                    ))
                    .route("/scene/preset/dashboard", post(
                        |AxState(s): AxState<AgentState>| async move {
                            s.scene.lock().unwrap().pending_preset_dashboard = true;
                            ok("dashboard preset queued")
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
    storage_view:     wgpu::TextureView,
    dummy_glyph:      wgpu::TextureView,
    /// Procedural 64×64 checkerboard atlas — exposed via L3
    /// `/scene/spawn_image` (id=0 = full atlas).
    image_atlas_view: wgpu::TextureView,
    _image_atlas_tex: wgpu::Texture,
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
        // Pool capacity: MAX_N rects can each produce 3 cmds (body + 2
        // selected bands), plus headroom for stroke spam and a fat
        // polyline (e.g. 256-point sparkline = 255 line cmds). 4× MAX_N
        // is comfortable and still bounded.
        let cmd_cap = (MAX_N as u32) * 4;
        let (bufs, storage_tex, storage_view) =
            TileBuffers::with_output_texture(&device, cmd_cap, w, h);
        let tex_w = bufs.tile_count_x * TILE_SIZE;
        let tex_h = bufs.tile_count_y * TILE_SIZE;
        let (dummy_tex, dummy_glyph) = TilePipeline::dummy_glyph_atlas(&device);

        // Build a procedural 64×64 RGBA8 atlas: 8×8 checkerboard with
        // a contrasting alternating colour palette. Exposed to the
        // agentic API via `/scene/spawn_image` — the whole atlas is
        // mapped to the rect's UV `[0..1, 0..1]`.
        const ATLAS_PX: u32 = 64;
        let mut atlas_pixels = Vec::with_capacity((ATLAS_PX * ATLAS_PX * 4) as usize);
        for y in 0..ATLAS_PX {
            for x in 0..ATLAS_PX {
                let cell_x = x / 8;
                let cell_y = y / 8;
                let dark = (cell_x ^ cell_y) & 1 == 0;
                // Diagonal hue stripe so the texture has actual content
                // (not just two-tone) — easier to see when sampled UV is
                // misaligned.
                let hue_t = (x as f32 + y as f32) / (2.0 * ATLAS_PX as f32);
                let base_r = (255.0 * hue_t) as u8;
                let base_b = (255.0 * (1.0 - hue_t)) as u8;
                if dark {
                    atlas_pixels.extend_from_slice(&[40, 40, 60, 255]);
                } else {
                    atlas_pixels.extend_from_slice(&[base_r, 200, base_b, 255]);
                }
            }
        }
        let image_atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("urx-fullgpu-demo-image-atlas"),
            size:            wgpu::Extent3d { width: ATLAS_PX, height: ATLAS_PX, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          wgpu::TextureFormat::Rgba8Unorm,
            usage:           wgpu::TextureUsages::TEXTURE_BINDING
                           | wgpu::TextureUsages::COPY_DST,
            view_formats:    &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture:  &image_atlas_tex,
                mip_level: 0,
                origin:   wgpu::Origin3d::ZERO,
                aspect:   wgpu::TextureAspect::All,
            },
            &atlas_pixels,
            wgpu::TexelCopyBufferLayout {
                offset:         0,
                bytes_per_row:  Some(ATLAS_PX * 4),
                rows_per_image: Some(ATLAS_PX),
            },
            wgpu::Extent3d { width: ATLAS_PX, height: ATLAS_PX, depth_or_array_layers: 1 },
        );
        let image_atlas_view = image_atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());

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
            image_atlas_view, _image_atlas_tex: image_atlas_tex,
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
            for r in sc.rects.iter()     { self.engine.remove_region(RegionId(r.id)); }
            for s in sc.strokes.iter()   { self.engine.remove_region(RegionId(s.id)); }
            for p in sc.polylines.iter() { self.engine.remove_region(RegionId(p.id)); }
            for b in sc.beziers.iter()   { self.engine.remove_region(RegionId(b.id)); }
            for f in sc.fills.iter()     { self.engine.remove_region(RegionId(f.id)); }
            for mg in sc.multigrads.iter() { self.engine.remove_region(RegionId(mg.id)); }
            for im in sc.images.iter()     { self.engine.remove_region(RegionId(im.id)); }
            sc.rects.clear();
            sc.strokes.clear();
            sc.polylines.clear();
            sc.beziers.clear();
            sc.fills.clear();
            sc.multigrads.clear();
            sc.images.clear();
        }

        // ─ L3: dashboard preset (clear + populate grid + sparkline) ──
        if sc.pending_preset_dashboard {
            sc.pending_preset_dashboard = false;
            for r in sc.rects.iter()     { self.engine.remove_region(RegionId(r.id)); }
            for s in sc.strokes.iter()   { self.engine.remove_region(RegionId(s.id)); }
            for p in sc.polylines.iter() { self.engine.remove_region(RegionId(p.id)); }
            for b in sc.beziers.iter()   { self.engine.remove_region(RegionId(b.id)); }
            for f in sc.fills.iter()     { self.engine.remove_region(RegionId(f.id)); }
            for mg in sc.multigrads.iter() { self.engine.remove_region(RegionId(mg.id)); }
            for im in sc.images.iter()     { self.engine.remove_region(RegionId(im.id)); }
            sc.rects.clear();
            sc.strokes.clear();
            sc.polylines.clear();
            sc.beziers.clear();
            sc.fills.clear();
            sc.multigrads.clear();
            sc.images.clear();
            sc.paused = false;

            // 1. background rect (whole window dark slate)
            let id = sc.next_id; sc.next_id += 1;
            sc.rects.push(RectModel {
                id, cx: win_w * 0.5, cy: win_h * 0.5,
                vx: 0.0, vy: 0.0,
                half: (win_w.max(win_h)) * 0.5,
                color: [18, 20, 28, 255], selected: false,
            });
            // 2. axis lines — bottom horizontal + left vertical
            let axis_color = [80, 80, 100, 255];
            let id = sc.next_id; sc.next_id += 1;
            sc.strokes.push(StrokeModel {
                id,
                p0: [40.0, win_h - 60.0], p1: [win_w - 40.0, win_h - 60.0],
                width: 1.5, color: axis_color, cap: 0,
            });
            let id = sc.next_id; sc.next_id += 1;
            sc.strokes.push(StrokeModel {
                id,
                p0: [40.0, 60.0], p1: [40.0, win_h - 60.0],
                width: 1.5, color: axis_color, cap: 0,
            });
            // 3. grid lines — 5 horizontal
            for i in 0..5 {
                let y = 60.0 + (win_h - 120.0) * (i as f32 / 4.0);
                let id = sc.next_id; sc.next_id += 1;
                sc.strokes.push(StrokeModel {
                    id,
                    p0: [40.0, y], p1: [win_w - 40.0, y],
                    width: 0.5, color: [50, 52, 64, 255], cap: 0,
                });
            }
            // 4. sparkline — 64-point random walk fit into the panel
            let n = 64;
            let mut value: f32 = 0.5;
            let mut points = Vec::with_capacity(n);
            for i in 0..n {
                value += ((i * 13 % 17) as f32 / 16.0 - 0.5) * 0.05;
                value = value.clamp(0.05, 0.95);
                let x = 50.0 + (win_w - 90.0) * (i as f32 / (n - 1) as f32);
                let y = 70.0 + (win_h - 140.0) * (1.0 - value);
                points.push([x, y]);
            }
            let id = sc.next_id; sc.next_id += 1;
            sc.polylines.push(PolylineModel {
                id, points,
                width: 2.0, color: [100, 200, 255, 255],
            });
            // 5. cursor crosshair (one vertical line)
            let cursor_x = (win_w - 90.0) * 0.7 + 50.0;
            let id = sc.next_id; sc.next_id += 1;
            sc.strokes.push(StrokeModel {
                id,
                p0: [cursor_x, 60.0], p1: [cursor_x, win_h - 60.0],
                width: 1.0, color: [255, 200, 80, 200], cap: 1,
            });
        }

        // ─ L3: remove by id ──────────────────────────────────────────
        if !sc.pending_remove.is_empty() {
            let ids = std::mem::take(&mut sc.pending_remove);
            for id in ids {
                if let Some(pos) = sc.rects.iter().position(|r| r.id == id) {
                    sc.rects.remove(pos);
                    self.engine.remove_region(RegionId(id));
                } else if let Some(pos) = sc.strokes.iter().position(|s| s.id == id) {
                    sc.strokes.remove(pos);
                    self.engine.remove_region(RegionId(id));
                } else if let Some(pos) = sc.polylines.iter().position(|p| p.id == id) {
                    sc.polylines.remove(pos);
                    self.engine.remove_region(RegionId(id));
                } else if let Some(pos) = sc.beziers.iter().position(|b| b.id == id) {
                    sc.beziers.remove(pos);
                    self.engine.remove_region(RegionId(id));
                } else if let Some(pos) = sc.fills.iter().position(|f| f.id == id) {
                    sc.fills.remove(pos);
                    self.engine.remove_region(RegionId(id));
                } else if let Some(pos) = sc.multigrads.iter().position(|mg| mg.id == id) {
                    sc.multigrads.remove(pos);
                    self.engine.remove_region(RegionId(id));
                } else if let Some(pos) = sc.images.iter().position(|im| im.id == id) {
                    sc.images.remove(pos);
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
        if !sc.pending_strokes.is_empty() {
            let queued = std::mem::take(&mut sc.pending_strokes);
            for s_in in queued {
                if sc.strokes.len() < MAX_N {
                    sc.strokes.push(s_in);
                }
            }
        }
        if !sc.pending_polys.is_empty() {
            let queued = std::mem::take(&mut sc.pending_polys);
            for p_in in queued {
                if sc.polylines.len() < MAX_N {
                    sc.polylines.push(p_in);
                }
            }
        }
        if !sc.pending_beziers.is_empty() {
            let queued = std::mem::take(&mut sc.pending_beziers);
            for b_in in queued {
                if sc.beziers.len() < MAX_N {
                    sc.beziers.push(b_in);
                }
            }
        }
        if !sc.pending_fills.is_empty() {
            let queued = std::mem::take(&mut sc.pending_fills);
            for f_in in queued {
                if sc.fills.len() < MAX_N {
                    sc.fills.push(f_in);
                }
            }
        }
        if !sc.pending_multigrads.is_empty() {
            let queued = std::mem::take(&mut sc.pending_multigrads);
            for mg_in in queued {
                if sc.multigrads.len() < MAX_N {
                    sc.multigrads.push(mg_in);
                }
            }
        }
        if !sc.pending_images.is_empty() {
            let queued = std::mem::take(&mut sc.pending_images);
            for im_in in queued {
                if sc.images.len() < MAX_N {
                    sc.images.push(im_in);
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
        //
        // Skip pinned rects (vx==vy==0): these are static backgrounds /
        // presets that may be wider than the window's smaller axis,
        // which would make `clamp(half, win_w-half, ..)` go min > max
        // and panic.
        if !sc.paused {
            for r in sc.rects.iter_mut() {
                if r.vx == 0.0 && r.vy == 0.0 { continue; }
                r.cx += r.vx * dt;
                r.cy += r.vy * dt;
                if r.cx - r.half < 0.0 || r.cx + r.half > win_w {
                    let lo = r.half.min(win_w - r.half);
                    let hi = r.half.max(win_w - r.half);
                    r.vx = -r.vx;
                    r.cx = r.cx.clamp(lo, hi);
                }
                if r.cy - r.half < 0.0 || r.cy + r.half > win_h {
                    let lo = r.half.min(win_h - r.half);
                    let hi = r.half.max(win_h - r.half);
                    r.vy = -r.vy;
                    r.cy = r.cy.clamp(lo, hi);
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

        // ─ Push to engine: strokes ───────────────────────────────────
        for s_m in sc.strokes.iter() {
            let mut scene = Scene::new();
            scene.line_solid(
                uzor_urx_core::math::Vec2 { x: s_m.p0[0] as f64, y: s_m.p0[1] as f64 },
                uzor_urx_core::math::Vec2 { x: s_m.p1[0] as f64, y: s_m.p1[1] as f64 },
                s_m.width,
                Color::rgba8(s_m.color[0], s_m.color[1], s_m.color[2], s_m.color[3]),
            );
            // Stroke bbox in screen space — inflate by half width.
            let hw = (s_m.width * 0.5).max(1.0) as f64;
            let x0 = (s_m.p0[0].min(s_m.p1[0]) as f64) - hw;
            let y0 = (s_m.p0[1].min(s_m.p1[1]) as f64) - hw;
            let x1 = (s_m.p0[0].max(s_m.p1[0]) as f64) + hw;
            let y1 = (s_m.p0[1].max(s_m.p1[1]) as f64) + hw;
            self.engine.upsert_region(
                RegionId(s_m.id),
                scene,
                UxRect::new(x0, y0, x1, y1),
                RenderCadence::Static,
            );
        }

        // ─ Push to engine: polylines as N-1 line segments per region ──
        // Each polyline = ONE engine region; the engine encodes its
        // scene to (N-1) Stroke cmds. The bbox is the AABB of all
        // points + half-width inflation.
        for p_m in sc.polylines.iter() {
            if p_m.points.len() < 2 { continue; }
            let mut scene = Scene::new();
            let color = Color::rgba8(p_m.color[0], p_m.color[1], p_m.color[2], p_m.color[3]);
            for w in p_m.points.windows(2) {
                let a = w[0]; let b = w[1];
                scene.line_solid(
                    uzor_urx_core::math::Vec2 { x: a[0] as f64, y: a[1] as f64 },
                    uzor_urx_core::math::Vec2 { x: b[0] as f64, y: b[1] as f64 },
                    p_m.width,
                    color,
                );
            }
            let hw = (p_m.width * 0.5).max(1.0) as f64;
            let (mut x0, mut y0, mut x1, mut y1) = (
                f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY,
            );
            for pt in &p_m.points {
                let (x, y) = (pt[0] as f64, pt[1] as f64);
                if x < x0 { x0 = x; } if y < y0 { y0 = y; }
                if x > x1 { x1 = x; } if y > y1 { y1 = y; }
            }
            self.engine.upsert_region(
                RegionId(p_m.id),
                scene,
                UxRect::new(x0 - hw, y0 - hw, x1 + hw, y1 + hw),
                RenderCadence::Static,
            );
        }

        // ─ Push to engine: cubic Bézier curves via DrawCommand::StrokePath ──
        // Each Bézier = ONE engine region; the encoder runs kurbo flatten
        // on the BezPath and emits a Path cmd.
        for b in sc.beziers.iter() {
            let mut path = BezPath::new();
            path.move_to(Point::new(b.p0[0] as f64, b.p0[1] as f64));
            path.curve_to(
                Point::new(b.c0[0] as f64, b.c0[1] as f64),
                Point::new(b.c1[0] as f64, b.c1[1] as f64),
                Point::new(b.p1[0] as f64, b.p1[1] as f64),
            );
            let stroke = Stroke {
                width: b.width,
                ..Stroke::default()
            };
            let mut scene = Scene::new();
            scene.commands.push(DrawCommand::StrokePath {
                path,
                stroke,
                brush: Brush::Solid(Color::rgba8(b.color[0], b.color[1], b.color[2], b.color[3])),
                transform: Affine::IDENTITY,
            });

            // AABB from 4 control points + half-width pad.
            let hw = (b.width * 0.5).max(1.0) as f64;
            let xs = [b.p0[0] as f64, b.c0[0] as f64, b.c1[0] as f64, b.p1[0] as f64];
            let ys = [b.p0[1] as f64, b.c0[1] as f64, b.c1[1] as f64, b.p1[1] as f64];
            let x0 = xs.iter().cloned().fold(f64::INFINITY, f64::min) - hw;
            let y0 = ys.iter().cloned().fold(f64::INFINITY, f64::min) - hw;
            let x1 = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + hw;
            let y1 = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + hw;
            self.engine.upsert_region(
                RegionId(b.id),
                scene,
                UxRect::new(x0, y0, x1, y1),
                RenderCadence::Static,
            );
        }

        // ─ Push to engine: filled paths via DrawCommand::FillPath ───
        for f_m in sc.fills.iter() {
            if f_m.points.len() < 3 { continue; }
            let mut path = BezPath::new();
            path.move_to(Point::new(f_m.points[0][0] as f64, f_m.points[0][1] as f64));
            for p in f_m.points.iter().skip(1) {
                path.line_to(Point::new(p[0] as f64, p[1] as f64));
            }
            path.close_path();
            let mut scene = Scene::new();
            scene.commands.push(DrawCommand::FillPath {
                path,
                rule: uzor_urx_core::scene::FillRule::NonZero,
                brush: Brush::Solid(Color::rgba8(
                    f_m.color[0], f_m.color[1], f_m.color[2], f_m.color[3],
                )),
                transform: Affine::IDENTITY,
            });

            let (mut x0, mut y0, mut x1, mut y1) = (
                f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY,
            );
            for pt in &f_m.points {
                let (x, y) = (pt[0] as f64, pt[1] as f64);
                if x < x0 { x0 = x; } if y < y0 { y0 = y; }
                if x > x1 { x1 = x; } if y > y1 { y1 = y; }
            }
            self.engine.upsert_region(
                RegionId(f_m.id),
                scene,
                UxRect::new(x0, y0, x1, y1),
                RenderCadence::Static,
            );
        }

        // ─ Push to engine: image rects via DrawCommand::FillRect + Brush::Image ──
        // The encoder routes Brush::Image to SceneCmd::image with full
        // atlas UV and unmodulated tint. The dummy peniko::Image's data
        // is ignored — the actual texture is the demo's procedural
        // checkerboard, bound through image_atlas_view.
        for im in sc.images.iter() {
            let dummy_img = peniko::Image::new(
                peniko::Blob::new(std::sync::Arc::new(Vec::<u8>::new())),
                peniko::Format::Rgba8,
                1, 1,
            );
            let mut scene = Scene::new();
            scene.commands.push(DrawCommand::FillRect {
                rect: UxRect::new(im.bbox[0] as f64, im.bbox[1] as f64,
                                   im.bbox[2] as f64, im.bbox[3] as f64),
                radii: None,
                brush: Brush::Image(dummy_img),
                transform: Affine::IDENTITY,
            });
            self.engine.upsert_region(
                RegionId(im.id),
                scene,
                UxRect::new(im.bbox[0] as f64, im.bbox[1] as f64,
                             im.bbox[2] as f64, im.bbox[3] as f64),
                RenderCadence::Static,
            );
        }

        // ─ Push to engine: multi-stop linear gradients ───────────────
        // We construct a peniko Gradient with N stops and push it as
        // DrawCommand::FillRect; the encoder routes >2 stops to
        // SceneCmd::multi_lin_gradient.
        for mg in sc.multigrads.iter() {
            if mg.stops.len() < 2 { continue; }
            let mut scene = Scene::new();

            // Map direction enum to (start, end) line that the encoder
            // snaps to one of the four lin_dir constants.
            let (sx, sy, ex, ey) = match mg.direction {
                0 => (mg.bbox[0] as f64, 0.0, mg.bbox[2] as f64, 0.0), // H
                1 => (0.0, mg.bbox[1] as f64, 0.0, mg.bbox[3] as f64), // V
                2 => (mg.bbox[0] as f64, mg.bbox[1] as f64,            // TL→BR
                       mg.bbox[2] as f64, mg.bbox[3] as f64),
                _ => (mg.bbox[0] as f64, mg.bbox[3] as f64,            // BL→TR
                       mg.bbox[2] as f64, mg.bbox[1] as f64),
            };

            let mut color_stops = uzor_urx_core::math::ColorStops::new();
            for s in &mg.stops {
                color_stops.push(uzor_urx_core::math::ColorStop {
                    offset: s[0].clamp(0.0, 1.0),
                    color:  Color::rgba8(
                        s[1].clamp(0.0, 255.0) as u8,
                        s[2].clamp(0.0, 255.0) as u8,
                        s[3].clamp(0.0, 255.0) as u8,
                        s[4].clamp(0.0, 255.0) as u8,
                    ).into(),
                });
            }
            let gradient = uzor_urx_core::math::Gradient {
                kind: uzor_urx_core::math::GradientKind::Linear {
                    start: uzor_urx_core::math::Point::new(sx, sy),
                    end:   uzor_urx_core::math::Point::new(ex, ey),
                },
                stops: color_stops,
                ..Default::default()
            };

            scene.commands.push(DrawCommand::FillRect {
                rect: UxRect::new(mg.bbox[0] as f64, mg.bbox[1] as f64,
                                   mg.bbox[2] as f64, mg.bbox[3] as f64),
                radii: None,
                brush: Brush::Gradient(gradient),
                transform: Affine::IDENTITY,
            });
            self.engine.upsert_region(
                RegionId(mg.id),
                scene,
                UxRect::new(mg.bbox[0] as f64, mg.bbox[1] as f64,
                             mg.bbox[2] as f64, mg.bbox[3] as f64),
                RenderCadence::Static,
            );
        }

        let paused  = sc.paused;
        let regions = (sc.rects.len() + sc.strokes.len() + sc.polylines.len()
                        + sc.beziers.len() + sc.fills.len()
                        + sc.multigrads.len() + sc.images.len()) as u32;
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
            image_atlas_view: &self.image_atlas_view,
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
