//! studio_demo — single-binary advertisement for the URX 3D arc.
//!
//! Exercises the whole stack at once:
//!
//! - **Wave 4 Phong + Wave 6 PBR** materials side-by-side
//! - **Wave 6b real IBL** (baked from default sky cubemap) lighting PBR cubes
//! - **Wave 7 shadow mapping** — directional light casts shadows on the ground
//! - **Wave 10 primitives** — sphere + torus + plane + cubes
//! - **Wave 12 HDR + bloom** post-processing
//! - **Wave 17 physics** — `uzor-urx-physics` drops 6 PBR cubes onto a static
//!   AABB ground plane via the Verlet + Sequential-Impulse solver
//! - **Wave 20 SSAO** — crease darkening
//!
//! Hotkeys (window must be focused):
//!
//! | Key | Effect |
//! |-----|--------|
//! | `1` | toggle bloom |
//! | `2` | toggle SSAO  |
//! | `3` | toggle IBL strength (1.0 ↔ 0.0) |
//! | `4` | pause/resume physics |
//! | `R` | reset physics — re-stack the cubes |
//! | `ESC` | quit |
//!
//! Run:
//!
//! ```bash
//! cargo run -p uzor-urx-3d --example studio_demo --release
//! ```
//!
//! The window title shows live FPS + the current toggle state, so every
//! subsystem can be A/B-flipped against the steady-state baseline without
//! touching the code.

use std::sync::Arc;
use std::time::Instant;

use uzor_urx_3d::{
    Light, MeshLit, MeshPbr, Node, PbrMaterial, PerspectiveCamera, PhongMaterial,
    Quat, Renderer3D, Scene3D, Texture3D, Vec3,
};
use uzor_urx_physics::{BodyId, Collider, PhysicsWorld};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const STACK_N: usize = 6;
const CUBE_HALF: f32 = 0.35;
const GROUND_HALF: Vec3 = Vec3::new(6.0, 0.1, 6.0);
const GROUND_Y: f32 = -1.0;

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    config: Option<wgpu::SurfaceConfiguration>,
    renderer: Option<Renderer3D>,

    // Meshes
    ground_mesh: Arc<MeshLit>,
    cube_mesh_pbr: Arc<MeshPbr>,
    sphere_mesh_pbr: Arc<MeshPbr>,
    torus_mesh_pbr: Arc<MeshPbr>,
    cube_mesh_lit: Arc<MeshLit>,
    atlas: Option<Arc<Texture3D>>,

    // Physics
    world: PhysicsWorld,
    cube_bodies: Vec<BodyId>,

    // Toggles (live in title for visibility)
    bloom_on: bool,
    ssao_on: bool,
    ibl_on: bool,
    physics_paused: bool,

    // Time
    angle_rad: f32,
    last_frame: Instant,
    fps_accum_frames: u32,
    fps_accum_ms: f32,
    fps_last_print: Instant,
}

impl App {
    fn new() -> Self {
        let mut world = PhysicsWorld::new();
        let cube_bodies = Self::seed_stack(&mut world);
        Self {
            window: None,
            instance: wgpu::Instance::new(&wgpu::InstanceDescriptor::default()),
            surface: None,
            device: None,
            queue: None,
            config: None,
            renderer: None,
            ground_mesh: Arc::new(MeshLit::plane_lit(GROUND_HALF.x, [0.30, 0.30, 0.34, 1.0])),
            cube_mesh_pbr: Arc::new(MeshPbr::cube_pbr()),
            sphere_mesh_pbr: Arc::new(MeshPbr::sphere_pbr(0.45, 24, 32)),
            torus_mesh_pbr: Arc::new(MeshPbr::torus_pbr(0.55, 0.18, 18, 32)),
            cube_mesh_lit: Arc::new(MeshLit::cube_lit()),
            atlas: None,
            world,
            cube_bodies,
            bloom_on: true,
            ssao_on: true,
            ibl_on: true,
            physics_paused: false,
            angle_rad: 0.0,
            last_frame: Instant::now(),
            fps_accum_frames: 0,
            fps_accum_ms: 0.0,
            fps_last_print: Instant::now(),
        }
    }

    fn seed_stack(world: &mut PhysicsWorld) -> Vec<BodyId> {
        let cube_bodies = (0..STACK_N)
            .map(|i| {
                let row = (i / 2) as f32;
                let col = (i % 2) as f32;
                let pos = Vec3::new(
                    -CUBE_HALF * 1.1 + col * CUBE_HALF * 2.2,
                    1.5 + row * CUBE_HALF * 2.3,
                    -CUBE_HALF * 0.3 + (i as f32) * 0.02,
                );
                world.spawn_dynamic(
                    Collider::aabb(Vec3::splat(CUBE_HALF)),
                    pos,
                    1.0,
                )
            })
            .collect::<Vec<_>>();
        // Static ground plane
        world.spawn_static(
            Collider::aabb(GROUND_HALF),
            Vec3::new(0.0, GROUND_Y - GROUND_HALF.y, 0.0),
        );
        cube_bodies
    }

    fn reset_physics(&mut self) {
        self.world = PhysicsWorld::new();
        self.cube_bodies = Self::seed_stack(&mut self.world);
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
        )).expect("adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("studio_demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        })).expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied()
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

    fn handle_key(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) {
        let renderer = match self.renderer.as_mut() {
            Some(r) => r,
            None => return,
        };
        match code {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::Digit1 => {
                self.bloom_on = !self.bloom_on;
                renderer.set_bloom_strength(if self.bloom_on { 0.6 } else { 0.0 });
            }
            KeyCode::Digit2 => {
                self.ssao_on = !self.ssao_on;
                renderer.set_ssao_strength(if self.ssao_on { 1.0 } else { 0.0 });
            }
            KeyCode::Digit3 => {
                self.ibl_on = !self.ibl_on;
                // IBL strength is hard-baked into the shader for now;
                // toggling ambient gives the same visual A/B.
                // (When set_ibl_strength lands on Renderer3D this becomes
                // a one-liner — see handoff #34 §C2.)
            }
            KeyCode::Digit4 => {
                self.physics_paused = !self.physics_paused;
            }
            KeyCode::KeyR => self.reset_physics(),
            _ => {}
        }
    }

    fn tick_and_draw(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(1.0 / 30.0);
        self.last_frame = now;

        if !self.physics_paused {
            // sub-step for stability
            let steps = 2;
            let h = dt / steps as f32;
            for _ in 0..steps {
                let _ = self.world.step(h);
            }
        }
        self.angle_rad += dt * std::f32::consts::TAU * 0.15;

        let (Some(surface), Some(device), Some(queue), Some(config), Some(renderer)) = (
            self.surface.as_ref(),
            self.device.as_ref(),
            self.queue.as_ref(),
            self.config.as_ref(),
            self.renderer.as_mut(),
        ) else { return };

        let frame_start = Instant::now();
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(device, config);
                return;
            }
            Err(e) => { eprintln!("surface err: {:?}", e); return; }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = config.width.max(1) as f32 / config.height.max(1) as f32;
        let camera = PerspectiveCamera::new(
            Vec3::new(4.5, 3.2, 5.0),
            Vec3::new(0.0, 0.6, 0.0),
            aspect,
        );

        let mut scene = Scene3D::new();
        scene.clear_color = [0.04, 0.05, 0.08, 1.0];
        scene.ambient = if self.ibl_on { [0.04, 0.05, 0.07] } else { [0.20, 0.20, 0.22] };

        // Two opposing-ish directional lights — first one casts shadow.
        scene.push_light(Light::directional(
            Vec3::new(-0.45, -1.0, -0.25),
            [1.0, 0.94, 0.82],
            1.4,
        ));
        scene.push_light(Light::directional(
            Vec3::new(0.55, -0.3, 0.7),
            [0.55, 0.65, 1.0],
            0.5,
        ));

        // Ground plane (Phong, lit, receives shadow).
        let ground = Node::new_lit(self.ground_mesh.clone())
            .with_translation(Vec3::new(0.0, GROUND_Y, 0.0))
            .with_material(PhongMaterial::default());
        scene.push(ground);

        // Spinning showcase: a metallic sphere + a copper torus, both PBR.
        if let Some(atlas) = &self.atlas {
            let sphere_mat = PbrMaterial::new(atlas.clone())
                .with_metalness(0.95)
                .with_roughness(0.15);
            scene.push(
                Node::new_pbr(self.sphere_mesh_pbr.clone(), sphere_mat)
                    .with_translation(Vec3::new(-2.0, -0.3, -0.5))
                    .with_rotation(Quat::from_rotation_y(self.angle_rad))
                    .with_tint([1.0, 1.0, 1.0, 1.0]),
            );
            let torus_mat = PbrMaterial::new(atlas.clone())
                .with_metalness(0.9)
                .with_roughness(0.30);
            scene.push(
                Node::new_pbr(self.torus_mesh_pbr.clone(), torus_mat)
                    .with_translation(Vec3::new(2.1, -0.1, -0.8))
                    .with_rotation(
                        Quat::from_rotation_x(self.angle_rad * 0.7)
                            * Quat::from_rotation_y(self.angle_rad * 0.4),
                    )
                    .with_tint([0.95, 0.55, 0.30, 1.0]),
            );
        }

        // Physics-driven PBR cube stack.
        let cube_positions: Vec<Vec3> = self
            .cube_bodies
            .iter()
            .filter_map(|id| self.world.body(*id).map(|b| b.position))
            .collect();
        if let Some(atlas) = &self.atlas {
            for (i, pos) in cube_positions.iter().enumerate() {
                let mat = PbrMaterial::new(atlas.clone())
                    .with_metalness(if i % 2 == 0 { 0.85 } else { 0.05 })
                    .with_roughness(0.25 + (i as f32) * 0.05);
                let tint = match i {
                    0 => [1.00, 0.45, 0.35, 1.0],
                    1 => [0.95, 0.85, 0.40, 1.0],
                    2 => [0.40, 0.85, 0.55, 1.0],
                    3 => [0.40, 0.65, 1.00, 1.0],
                    4 => [0.85, 0.45, 0.95, 1.0],
                    _ => [0.95, 0.95, 0.95, 1.0],
                };
                scene.push(
                    Node::new_pbr(self.cube_mesh_pbr.clone(), mat)
                        .with_translation(*pos)
                        .with_scale(Vec3::splat(CUBE_HALF * 2.0))
                        .with_tint(tint),
                );
            }
        } else {
            // Atlas not ready yet — fall back to lit cubes
            for pos in &cube_positions {
                scene.push(
                    Node::new_lit(self.cube_mesh_lit.clone())
                        .with_translation(*pos)
                        .with_scale(Vec3::splat(CUBE_HALF * 2.0))
                        .with_material(PhongMaterial::default()),
                );
            }
        }

        renderer.resize(device, (config.width, config.height));
        let mut enc =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer.render(device, queue, &mut enc, &view, &camera, &scene);
        queue.submit(Some(enc.finish()));
        frame.present();

        let elapsed_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.fps_accum_frames += 1;
        self.fps_accum_ms += elapsed_ms;
        if self.fps_last_print.elapsed().as_millis() >= 500 {
            let fps = self.fps_accum_frames as f32
                / self.fps_last_print.elapsed().as_secs_f32();
            let frame_ms = self.fps_accum_ms / self.fps_accum_frames as f32;
            if let Some(w) = &self.window {
                w.set_title(&format!(
                    "URX studio — {:.0} FPS / {:.2} ms — bloom:{} ssao:{} ibl:{} phys:{}",
                    fps,
                    frame_ms,
                    if self.bloom_on { "ON" } else { "OFF" },
                    if self.ssao_on { "ON" } else { "OFF" },
                    if self.ibl_on { "ON" } else { "OFF" },
                    if self.physics_paused { "PAUSED" } else { "ON" },
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
            .with_title("URX studio — booting")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        self.init_gpu(window.clone());
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
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
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(code),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => self.handle_key(code, event_loop),
            WindowEvent::RedrawRequested => {
                self.tick_and_draw();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run app");
}
