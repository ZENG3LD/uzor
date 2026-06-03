//! Spinning-cube demo — first live 3D demo for URX 3D Wave 1.
//!
//! Opens a winit window, renders a single RGB-face cube spinning around
//! the Y axis through `Renderer3D::render` (the same code path the
//! tests exercise). FPS prints into the window title every 500ms.
//!
//! ## Agentic 3-level HTTP API
//!
//! Listens on `127.0.0.1:17492` (localhost only, no auth).
//!
//! ### L1 — introspection
//! | Method | Path        | Body | Purpose |
//! |--------|-------------|------|---------|
//! | `GET`  | `/health`   | —    | `{ok: true}` |
//! | `GET`  | `/state`    | —    | snapshot: fps, frame_ms, nodes, paused, eye, target, spin_rate, win_w/h |
//!
//! ### L2 — semantic actions
//! | Method | Path                        | Body              | Purpose |
//! |--------|-----------------------------|-------------------|---------|
//! | `POST` | `/act/pause`                | —                 | freeze spin animation |
//! | `POST` | `/act/resume`               | —                 | unfreeze |
//! | `POST` | `/act/reset_camera`         | —                 | restore default eye + target |
//! | `POST` | `/act/spin_rate`            | `{rps}`           | rotations per second (negative = reverse) |
//!
//! ### L3 — structural scene ops
//! | Method | Path                        | Body                                    | Purpose |
//! |--------|-----------------------------|-----------------------------------------|---------|
//! | `POST` | `/scene/spawn_cube`         | `{pos: [x,y,z], scale: f, tint: [r,g,b,a]}` | add a cube node |
//! | `POST` | `/scene/spawn_batch`        | `[{pos,scale,tint},…]`                  | bulk-add many cubes in one request |
//! | `POST` | `/scene/preset/ring`        | `{n, radius?}`                          | spawn an N-cube ring on the XZ plane |
//! | `POST` | `/scene/preset/grid`        | `{nx, ny, nz, spacing?}`                | spawn an Nx×Ny×Nz cube grid |
//! | `POST` | `/scene/clear`              | —                                       | remove every node (re-spawn the central spinning cube) |
//! | `POST` | `/camera/look_at`           | `{eye: [x,y,z], target: [x,y,z]}`       | move the camera explicitly |
//!
//! ## Try it
//!
//! ```bash
//! cargo run -p uzor-urx-3d --example spinning_cube_demo --release
//!
//! # in another shell:
//! curl -s http://127.0.0.1:17492/state | jq
//! curl -s -X POST -H "content-type: application/json" \
//!     -d '{"rps": 2.0}' http://127.0.0.1:17492/act/spin_rate
//! curl -s -X POST -H "content-type: application/json" \
//!     -d '{"pos":[2,0,0],"scale":0.5,"tint":[1,0.5,0.2,1]}' \
//!     http://127.0.0.1:17492/scene/spawn_cube
//! curl -s -X POST -H "content-type: application/json" \
//!     -d '{"eye":[0,2,6],"target":[0,0,0]}' http://127.0.0.1:17492/camera/look_at
//! ```

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use uzor_urx_3d::{Mesh, Node, PerspectiveCamera, Quat, Renderer3D, Scene3D, Vec3};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const AGENT_PORT: u16 = 17492;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CubeSpec {
    pos: [f32; 3],
    scale: f32,
    tint: [f32; 4],
}

#[derive(Default)]
struct SharedState {
    paused: bool,
    spin_rate_rps: f32,
    eye: [f32; 3],
    target: [f32; 3],
    cubes: Vec<CubeSpec>,
    pending_cubes: Vec<CubeSpec>,
    pending_clear: bool,
    pending_camera_eye: Option<[f32; 3]>,
    pending_camera_target: Option<[f32; 3]>,
    pending_reset_camera: bool,
    fps: f32,
    frame_ms: f32,
    nodes: u32,
    win_w: u32,
    win_h: u32,
}

impl SharedState {
    fn new() -> Self {
        Self {
            paused: false,
            spin_rate_rps: 0.5,
            eye: [3.0, 3.0, 3.0],
            target: [0.0, 0.0, 0.0],
            cubes: Vec::new(),
            pending_cubes: Vec::new(),
            pending_clear: false,
            pending_camera_eye: None,
            pending_camera_target: None,
            pending_reset_camera: false,
            fps: 0.0,
            frame_ms: 0.0,
            nodes: 1,
            win_w: 960,
            win_h: 720,
        }
    }
}

type Shared = Arc<Mutex<SharedState>>;

// ─────────────────────────────────────────────────────────────────────
// HTTP agentic surface
// ─────────────────────────────────────────────────────────────────────

mod http {
    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::{Json, Router};

    pub fn spawn(shared: Shared) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio rt");
            rt.block_on(async move {
                let app = Router::new()
                    .route("/health", get(health))
                    .route("/state", get(state))
                    .route("/act/pause", post(act_pause))
                    .route("/act/resume", post(act_resume))
                    .route("/act/reset_camera", post(act_reset_camera))
                    .route("/act/spin_rate", post(act_spin_rate))
                    .route("/scene/spawn_cube", post(scene_spawn_cube))
                    .route("/scene/spawn_batch", post(scene_spawn_batch))
                    .route("/scene/preset/ring", post(scene_preset_ring))
                    .route("/scene/preset/grid", post(scene_preset_grid))
                    .route("/scene/clear", post(scene_clear))
                    .route("/camera/look_at", post(camera_look_at))
                    .with_state(shared);
                let bind = format!("127.0.0.1:{}", AGENT_PORT);
                let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
                eprintln!("urx-3d agent HTTP listening on http://{bind}");
                axum::serve(listener, app).await.unwrap();
            });
        });
    }

    async fn health() -> Json<serde_json::Value> {
        Json(serde_json::json!({"ok": true}))
    }

    async fn state(State(s): State<Shared>) -> Json<serde_json::Value> {
        let g = s.lock().unwrap();
        Json(serde_json::json!({
            "fps": g.fps,
            "frame_ms": g.frame_ms,
            "nodes": g.nodes,
            "paused": g.paused,
            "spin_rate_rps": g.spin_rate_rps,
            "eye": g.eye,
            "target": g.target,
            "cubes": g.cubes.len(),
            "win_w": g.win_w,
            "win_h": g.win_h,
        }))
    }

    async fn act_pause(State(s): State<Shared>) -> StatusCode {
        s.lock().unwrap().paused = true;
        StatusCode::OK
    }

    async fn act_resume(State(s): State<Shared>) -> StatusCode {
        s.lock().unwrap().paused = false;
        StatusCode::OK
    }

    async fn act_reset_camera(State(s): State<Shared>) -> StatusCode {
        s.lock().unwrap().pending_reset_camera = true;
        StatusCode::OK
    }

    #[derive(Deserialize)]
    struct SpinRate {
        rps: f32,
    }

    async fn act_spin_rate(State(s): State<Shared>, Json(b): Json<SpinRate>) -> StatusCode {
        s.lock().unwrap().spin_rate_rps = b.rps;
        StatusCode::OK
    }

    async fn scene_spawn_cube(
        State(s): State<Shared>,
        Json(b): Json<CubeSpec>,
    ) -> StatusCode {
        s.lock().unwrap().pending_cubes.push(b);
        StatusCode::OK
    }

    async fn scene_spawn_batch(
        State(s): State<Shared>,
        Json(b): Json<Vec<CubeSpec>>,
    ) -> StatusCode {
        s.lock().unwrap().pending_cubes.extend(b);
        StatusCode::OK
    }

    #[derive(Deserialize)]
    struct PresetRing {
        n: u32,
        #[serde(default = "default_ring_radius")]
        radius: f32,
    }
    fn default_ring_radius() -> f32 {
        4.0
    }

    async fn scene_preset_ring(
        State(s): State<Shared>,
        Json(b): Json<PresetRing>,
    ) -> StatusCode {
        let n = b.n.min(4096);
        let mut cubes = Vec::with_capacity(n as usize);
        for i in 0..n {
            let theta = (i as f32 / n as f32) * std::f32::consts::TAU;
            let hue = i as f32 / n as f32;
            cubes.push(CubeSpec {
                pos: [theta.cos() * b.radius, 0.0, theta.sin() * b.radius],
                scale: 0.3,
                tint: hue_to_rgba(hue),
            });
        }
        s.lock().unwrap().pending_cubes.extend(cubes);
        StatusCode::OK
    }

    #[derive(Deserialize)]
    struct PresetGrid {
        nx: u32,
        ny: u32,
        nz: u32,
        #[serde(default = "default_grid_spacing")]
        spacing: f32,
    }
    fn default_grid_spacing() -> f32 {
        1.5
    }

    async fn scene_preset_grid(
        State(s): State<Shared>,
        Json(b): Json<PresetGrid>,
    ) -> StatusCode {
        let total = (b.nx * b.ny * b.nz) as usize;
        let total = total.min(8192);
        let mut cubes = Vec::with_capacity(total);
        let half_x = (b.nx as f32 - 1.0) / 2.0;
        let half_y = (b.ny as f32 - 1.0) / 2.0;
        let half_z = (b.nz as f32 - 1.0) / 2.0;
        for i in 0..b.nx {
            for j in 0..b.ny {
                for k in 0..b.nz {
                    if cubes.len() >= total {
                        break;
                    }
                    let hue = (i + j * b.nx + k * b.nx * b.ny) as f32
                        / (b.nx * b.ny * b.nz) as f32;
                    cubes.push(CubeSpec {
                        pos: [
                            (i as f32 - half_x) * b.spacing,
                            (j as f32 - half_y) * b.spacing,
                            (k as f32 - half_z) * b.spacing,
                        ],
                        scale: 0.35,
                        tint: hue_to_rgba(hue),
                    });
                }
            }
        }
        s.lock().unwrap().pending_cubes.extend(cubes);
        StatusCode::OK
    }

    fn hue_to_rgba(h: f32) -> [f32; 4] {
        // simple HSV→RGB with full saturation+value
        let c = 1.0;
        let h6 = h * 6.0;
        let x = c * (1.0 - ((h6 % 2.0) - 1.0).abs());
        let (r, g, b) = match h6 as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        [r, g, b, 1.0]
    }

    async fn scene_clear(State(s): State<Shared>) -> StatusCode {
        s.lock().unwrap().pending_clear = true;
        StatusCode::OK
    }

    #[derive(Deserialize)]
    struct LookAt {
        eye: [f32; 3],
        target: [f32; 3],
    }

    async fn camera_look_at(State(s): State<Shared>, Json(b): Json<LookAt>) -> StatusCode {
        let mut g = s.lock().unwrap();
        g.pending_camera_eye = Some(b.eye);
        g.pending_camera_target = Some(b.target);
        StatusCode::OK
    }
}

// ─────────────────────────────────────────────────────────────────────
// Winit / wgpu app
// ─────────────────────────────────────────────────────────────────────

struct App {
    shared: Shared,
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    config: Option<wgpu::SurfaceConfiguration>,
    renderer: Option<Renderer3D>,
    cube_mesh: Arc<Mesh>,
    angle_rad: f32,
    last_frame: Instant,
    fps_accum_frames: u32,
    fps_accum_ms: f32,
    fps_last_print: Instant,
}

impl App {
    fn new(shared: Shared) -> Self {
        Self {
            shared,
            window: None,
            instance: wgpu::Instance::new(&wgpu::InstanceDescriptor::default()),
            surface: None,
            device: None,
            queue: None,
            config: None,
            renderer: None,
            cube_mesh: Arc::new(Mesh::cube_rgb_faces()),
            angle_rad: 0.0,
            last_frame: Instant::now(),
            fps_accum_frames: 0,
            fps_accum_ms: 0.0,
            fps_last_print: Instant::now(),
        }
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        let surface = self.instance.create_surface(window.clone()).expect("surface");

        let adapter = pollster::block_on(self.instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            },
        ))
        .expect("adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("urx3d-demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer3D::new(&device, format, (config.width, config.height), 64);

        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.config = Some(config);
        self.renderer = Some(renderer);
    }

    fn reconcile_and_tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        let (paused, spin_rate, spawn, clear, eye_override, tgt_override, reset_cam) = {
            let mut g = self.shared.lock().unwrap();
            let spawn = std::mem::take(&mut g.pending_cubes);
            let clear = std::mem::take(&mut g.pending_clear);
            let eye_override = g.pending_camera_eye.take();
            let tgt_override = g.pending_camera_target.take();
            let reset_cam = std::mem::take(&mut g.pending_reset_camera);
            (g.paused, g.spin_rate_rps, spawn, clear, eye_override, tgt_override, reset_cam)
        };
        if !paused {
            self.angle_rad += dt * spin_rate * std::f32::consts::TAU;
        }
        let mut g = self.shared.lock().unwrap();
        if clear {
            g.cubes.clear();
        }
        for c in spawn {
            g.cubes.push(c);
        }
        if reset_cam {
            g.eye = [3.0, 3.0, 3.0];
            g.target = [0.0, 0.0, 0.0];
        }
        if let Some(e) = eye_override {
            g.eye = e;
        }
        if let Some(t) = tgt_override {
            g.target = t;
        }
    }

    fn draw(&mut self) {
        let (Some(surface), Some(device), Some(queue), Some(config), Some(renderer)) = (
            self.surface.as_ref(),
            self.device.as_ref(),
            self.queue.as_ref(),
            self.config.as_ref(),
            self.renderer.as_mut(),
        ) else {
            return;
        };

        let frame_start = Instant::now();

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(device, config);
                return;
            }
            Err(e) => {
                eprintln!("surface err: {:?}", e);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let (eye, target, cubes_extra) = {
            let g = self.shared.lock().unwrap();
            (g.eye, g.target, g.cubes.clone())
        };

        let aspect = config.width.max(1) as f32 / config.height.max(1) as f32;
        let camera = PerspectiveCamera::new(
            Vec3::from_array(eye),
            Vec3::from_array(target),
            aspect,
        );

        let mut scene = Scene3D::new();
        scene.clear_color = [0.04, 0.04, 0.08, 1.0];

        // Central spinning cube
        scene.push(
            Node::new(self.cube_mesh.clone())
                .with_rotation(Quat::from_rotation_y(self.angle_rad)),
        );

        // Agent-added cubes
        for c in &cubes_extra {
            scene.push(
                Node::new(self.cube_mesh.clone())
                    .with_translation(Vec3::from_array(c.pos))
                    .with_scale(Vec3::splat(c.scale))
                    .with_rotation(Quat::from_rotation_y(self.angle_rad * 0.5))
                    .with_tint(c.tint),
            );
        }

        renderer.resize(device, (config.width, config.height));

        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer.render(device, queue, &mut enc, &view, &camera, &scene);
        queue.submit(Some(enc.finish()));
        frame.present();

        // FPS bookkeeping
        let elapsed_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.fps_accum_frames += 1;
        self.fps_accum_ms += elapsed_ms;

        let since_print = self.fps_last_print.elapsed();
        if since_print.as_millis() >= 500 {
            let fps = self.fps_accum_frames as f32 / since_print.as_secs_f32();
            let frame_ms = self.fps_accum_ms / self.fps_accum_frames as f32;
            {
                let mut g = self.shared.lock().unwrap();
                g.fps = fps;
                g.frame_ms = frame_ms;
                g.nodes = 1 + cubes_extra.len() as u32;
                g.win_w = config.width;
                g.win_h = config.height;
            }
            if let Some(w) = &self.window {
                w.set_title(&format!(
                    "urx-3d spinning cube — {:.0} FPS / {:.2} ms / {} nodes (HTTP :{})",
                    fps, frame_ms, 1 + cubes_extra.len(), AGENT_PORT
                ));
            }
            self.fps_accum_frames = 0;
            self.fps_accum_ms = 0.0;
            self.fps_last_print = Instant::now();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("urx-3d spinning cube — booting")
            .with_inner_size(winit::dpi::LogicalSize::new(960.0, 720.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        self.init_gpu(window.clone());
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(device), Some(config)) =
                    (&self.surface, &self.device, self.config.as_mut())
                {
                    config.width = size.width.max(1);
                    config.height = size.height.max(1);
                    surface.configure(device, config);
                }
            }
            WindowEvent::RedrawRequested => {
                self.reconcile_and_tick();
                self.draw();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let shared = Arc::new(Mutex::new(SharedState::new()));
    http::spawn(shared.clone());

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new(shared);
    event_loop.run_app(&mut app).expect("run app");
}
