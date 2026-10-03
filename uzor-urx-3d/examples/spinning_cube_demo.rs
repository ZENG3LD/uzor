//! Spinning-cube demo — first live 3D demo for URX 3D Wave 1.
//!
//! Opens a winit window, renders a single RGB-face cube spinning around
//! the Y axis through `Renderer3D::render` (the same code path the
//! tests exercise). FPS prints into the window title every 500ms.
//!
//! The scene holds the central spinning cube under one directional light.
//!
//! ## Try it
//!
//! ```bash
//! cargo run -p uzor-urx-3d --example spinning_cube_demo --release
//! ```

use std::sync::{Arc, Mutex};
use std::time::Instant;

use uzor_urx_3d::{
    Light, Mesh, MeshLit, MeshPbr, Node, PbrMaterial, PerspectiveCamera, PhongMaterial, Quat,
    Renderer3D, Scene3D, Texture3D, Vec3,
};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Clone, Debug)]
struct CubeSpec {
    pos: [f32; 3],
    scale: f32,
    tint: [f32; 4],
    /// Routing (priority: pbr > textured > lit > unlit):
    /// - default                  → Wave 3 unlit
    /// - `lit=true`               → Wave 4 Phong
    /// - `textured=true`          → Wave 5 textured-Phong
    /// - `pbr=true`               → Wave 6 PBR (uses atlas albedo,
    ///                              metalness + roughness fields)
    lit: bool,
    textured: bool,
    pbr: bool,
    metalness: f32,
    roughness: f32,
}

#[derive(Clone, Debug)]
struct LightSpec {
    /// "directional" or "point"
    kind: String,
    /// directional: direction vector; point: position
    vec: [f32; 3],
    color: [f32; 3],
    intensity: f32,
    range: f32,
}

#[derive(Default)]
struct SharedState {
    paused: bool,
    spin_rate_rps: f32,
    eye: [f32; 3],
    target: [f32; 3],
    cubes: Vec<CubeSpec>,
    pending_cubes: Vec<CubeSpec>,
    lights: Vec<LightSpec>,
    pending_lights: Vec<LightSpec>,
    pending_clear: bool,
    pending_clear_lights: bool,
    ambient: [f32; 3],
    /// When true, the central spinning cube renders through Phong
    /// pipeline (so default scene shows lighting).
    central_lit: bool,
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
            lights: vec![LightSpec {
                kind: "directional".into(),
                vec: [-0.4, -1.0, -0.3],
                color: [1.0, 0.95, 0.85],
                intensity: 1.1,
                range: 0.0,
            }],
            pending_lights: Vec::new(),
            pending_clear: false,
            pending_clear_lights: false,
            ambient: [0.10, 0.10, 0.14],
            central_lit: true,
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
    cube_mesh_lit: Arc<MeshLit>,
    cube_mesh_uv: Arc<uzor_urx_3d::MeshUv>,
    cube_mesh_pbr: Arc<MeshPbr>,
    // Built lazily on first draw (needs device).
    atlas: Option<Arc<Texture3D>>,
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
            instance: wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
            surface: None,
            device: None,
            queue: None,
            config: None,
            renderer: None,
            cube_mesh: Arc::new(Mesh::cube_rgb_faces()),
            cube_mesh_lit: Arc::new(MeshLit::cube_lit()),
            cube_mesh_uv: Arc::new(MeshLit::cube_uv()),
            cube_mesh_pbr: Arc::new(MeshPbr::cube_pbr()),
            atlas: None,
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

        let renderer = Renderer3D::new(&device, &queue, format, (config.width, config.height), 64);
        let atlas = Arc::new(Texture3D::checkerboard(&device, &queue));

        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.config = Some(config);
        self.renderer = Some(renderer);
        self.atlas = Some(atlas);
    }

    fn reconcile_and_tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        let (paused, spin_rate, spawn, clear, light_spawn, light_clear, eye_override, tgt_override, reset_cam) = {
            let mut g = self.shared.lock().unwrap();
            let spawn = std::mem::take(&mut g.pending_cubes);
            let clear = std::mem::take(&mut g.pending_clear);
            let light_spawn = std::mem::take(&mut g.pending_lights);
            let light_clear = std::mem::take(&mut g.pending_clear_lights);
            let eye_override = g.pending_camera_eye.take();
            let tgt_override = g.pending_camera_target.take();
            let reset_cam = std::mem::take(&mut g.pending_reset_camera);
            (g.paused, g.spin_rate_rps, spawn, clear, light_spawn, light_clear, eye_override, tgt_override, reset_cam)
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
        if light_clear {
            g.lights.clear();
        }
        for l in light_spawn {
            g.lights.push(l);
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

        // wgpu 29: `get_current_texture()` returns `CurrentSurfaceTexture`
        // directly (no longer `Result<SurfaceTexture, SurfaceError>`) —
        // see `studio_demo.rs`'s own identical fix for the full mapping.
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(device, config);
                return;
            }
            other => {
                eprintln!("surface err: {:?}", other);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let (eye, target, cubes_extra, light_specs, ambient, central_lit) = {
            let g = self.shared.lock().unwrap();
            (
                g.eye,
                g.target,
                g.cubes.clone(),
                g.lights.clone(),
                g.ambient,
                g.central_lit,
            )
        };

        let aspect = config.width.max(1) as f32 / config.height.max(1) as f32;
        let camera = PerspectiveCamera::new(
            Vec3::from_array(eye),
            Vec3::from_array(target),
            aspect,
        );

        let mut scene = Scene3D::new();
        scene.clear_color = [0.04, 0.04, 0.08, 1.0];
        scene.ambient = ambient;
        for l in &light_specs {
            match l.kind.as_str() {
                "directional" => scene.push_light(Light::directional(
                    Vec3::from_array(l.vec),
                    l.color,
                    l.intensity,
                )),
                "point" => scene.push_light(Light::point(
                    Vec3::from_array(l.vec),
                    l.color,
                    l.intensity,
                    l.range.max(0.01),
                )),
                _ => {}
            }
        }

        // Central spinning cube — Phong by default so the live demo
        // shows the directional light on first launch.
        let central_node = if central_lit {
            Node::new_lit(self.cube_mesh_lit.clone())
        } else {
            Node::new(self.cube_mesh.clone())
        };
        scene.push(central_node.with_rotation(Quat::from_rotation_y(self.angle_rad)));

        // Agent-added cubes — each can pick unlit / lit / textured / PBR
        let atlas = self.atlas.clone();
        for c in &cubes_extra {
            let n = if c.pbr {
                if let Some(a) = atlas.as_ref() {
                    let mat = PbrMaterial::new(a.clone())
                        .with_metalness(c.metalness)
                        .with_roughness(c.roughness);
                    Node::new_pbr(self.cube_mesh_pbr.clone(), mat)
                } else {
                    Node::new(self.cube_mesh.clone())
                }
            } else if c.textured {
                if let Some(a) = atlas.as_ref() {
                    Node::new_textured(self.cube_mesh_uv.clone(), a.clone())
                } else {
                    Node::new(self.cube_mesh.clone())
                }
            } else if c.lit {
                Node::new_lit(self.cube_mesh_lit.clone())
            } else {
                Node::new(self.cube_mesh.clone())
            };
            scene.push(
                n.with_translation(Vec3::from_array(c.pos))
                    .with_scale(Vec3::splat(c.scale))
                    .with_rotation(Quat::from_rotation_y(self.angle_rad * 0.5))
                    .with_tint(c.tint)
                    .with_material(PhongMaterial::default()),
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
                    "urx-3d spinning cube — {:.0} FPS / {:.2} ms / {} nodes",
                    fps, frame_ms, 1 + cubes_extra.len()
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

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new(shared);
    event_loop.run_app(&mut app).expect("run app");
}
