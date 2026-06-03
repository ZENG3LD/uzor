//! Real on-screen winit window driven by `UrxEngine` with `Backend::FullGpu`.
//!
//! This is the first end-to-end consumer demo of the URX 1.6 compute
//! pipeline going through the engine façade (NOT bypassing it via
//! `uzor-urx-wgpu-full` directly). Proves the consumer pattern:
//!
//!   ┌──────────────────────────────────────────────────────┐
//!   │  per frame:                                          │
//!   │    1. consumer mutates region scenes / bounds        │
//!   │    2. engine.render(RenderTarget::FullGpu { ... })   │
//!   │    3. queue.submit(encoder.finish())                 │
//!   │    4. surface_texture.present()                      │
//!   └──────────────────────────────────────────────────────┘
//!
//! Scene: 64 animated coloured rects bouncing inside the window. Each
//! rect = one URX region. Engine resolves scene → SceneCmd → tile
//! compute → blit straight into the swapchain surface texture.
//!
//! Run:
//!   cargo run -p uzor-urx-engine --features full-gpu-backend \
//!     --example winit_full_gpu_demo --release
//!
//! Close the window or press Esc to exit.
//!
//! FPS / frame-time printed in the window title every ~500 ms.

#![cfg(feature = "full-gpu-backend")]

use std::sync::Arc;
use std::time::Instant;

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

const N_REGIONS: usize = 64;

struct AnimatedRect {
    cx:    f32,
    cy:    f32,
    vx:    f32,
    vy:    f32,
    half:  f32,
    color: [u8; 4],
}

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
    rects:         Vec<AnimatedRect>,
    last_tick:     Instant,
    fps_accum:     f32,
    fps_frames:    u32,
    fps_last:      Instant,
}

impl GpuState {
    fn new(window: Arc<Window>) -> Self {
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
        // Cap cmd buffer at N_REGIONS so we have headroom.
        let (bufs, storage_tex, storage_view) =
            TileBuffers::with_output_texture(&device, N_REGIONS as u32, w, h);
        let tex_w = bufs.tile_count_x * TILE_SIZE;
        let tex_h = bufs.tile_count_y * TILE_SIZE;
        let (dummy_tex, dummy_glyph) = TilePipeline::dummy_glyph_atlas(&device);

        let mut engine = UrxEngine::new_full_gpu(w, h);
        let mut rects = Vec::with_capacity(N_REGIONS);
        for i in 0..N_REGIONS {
            let half = 16.0 + (i % 5) as f32 * 4.0;
            let cx = 40.0 + ((i * 71) % 600) as f32;
            let cy = 40.0 + ((i * 113) % 400) as f32;
            let vx = if i % 2 == 0 { 80.0 } else { -80.0 };
            let vy = if i % 3 == 0 { 60.0 } else { -50.0 };
            let color = [
                ((i * 37) & 0xff) as u8,
                ((i * 71) & 0xff) as u8,
                ((i * 113) & 0xff) as u8,
                255,
            ];
            rects.push(AnimatedRect { cx, cy, vx, vy, half, color });

            let mut scene = Scene::new();
            scene.fill_rect_solid(
                UxRect::new(0.0, 0.0, (half * 2.0) as f64, (half * 2.0) as f64),
                Color::rgba8(color[0], color[1], color[2], color[3]),
            );
            engine.upsert_region(
                RegionId(i as u64),
                scene,
                UxRect::new((cx - half) as f64, (cy - half) as f64,
                             (cx + half) as f64, (cy + half) as f64),
                RenderCadence::HighHz,
            );
        }

        Self {
            window, surface, device, queue, config,
            pipeline, blit, bufs, storage_view, dummy_glyph,
            _dummy_tex: dummy_tex, _storage_tex: storage_tex,
            tex_w, tex_h, engine, rects,
            last_tick: Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
            fps_last:   Instant::now(),
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        self.config.width  = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        // Re-allocate storage + tile buffers for the new size.
        let (bufs, storage_tex, storage_view) =
            TileBuffers::with_output_texture(&self.device, N_REGIONS as u32, w, h);
        self.tex_w = bufs.tile_count_x * TILE_SIZE;
        self.tex_h = bufs.tile_count_y * TILE_SIZE;
        self.bufs = bufs;
        self.storage_view = storage_view;
        self._storage_tex = storage_tex;
        self.engine = UrxEngine::new_full_gpu(w, h);
        // Rebuild regions from current rects.
        for (i, r) in self.rects.iter().enumerate() {
            let mut scene = Scene::new();
            scene.fill_rect_solid(
                UxRect::new(0.0, 0.0, (r.half * 2.0) as f64, (r.half * 2.0) as f64),
                Color::rgba8(r.color[0], r.color[1], r.color[2], r.color[3]),
            );
            self.engine.upsert_region(
                RegionId(i as u64),
                scene,
                UxRect::new((r.cx - r.half) as f64, (r.cy - r.half) as f64,
                             (r.cx + r.half) as f64, (r.cy + r.half) as f64),
                RenderCadence::HighHz,
            );
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_tick).as_secs_f32().min(0.05);
        self.last_tick = now;

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        for (i, r) in self.rects.iter_mut().enumerate() {
            r.cx += r.vx * dt;
            r.cy += r.vy * dt;
            if r.cx - r.half < 0.0 || r.cx + r.half > w { r.vx = -r.vx; r.cx = r.cx.clamp(r.half, w - r.half); }
            if r.cy - r.half < 0.0 || r.cy + r.half > h { r.vy = -r.vy; r.cy = r.cy.clamp(r.half, h - r.half); }

            // Engine doesn't track bounds-changes natively for FullGpu yet
            // (no per-region cache); we re-upsert.
            let mut scene = Scene::new();
            scene.fill_rect_solid(
                UxRect::new(0.0, 0.0, (r.half * 2.0) as f64, (r.half * 2.0) as f64),
                Color::rgba8(r.color[0], r.color[1], r.color[2], r.color[3]),
            );
            self.engine.upsert_region(
                RegionId(i as u64),
                scene,
                UxRect::new((r.cx - r.half) as f64, (r.cy - r.half) as f64,
                             (r.cx + r.half) as f64, (r.cy + r.half) as f64),
                RenderCadence::HighHz,
            );
        }
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
            self.window.set_title(&format!(
                "URX 1.6 FullGpu engine demo — {N_REGIONS} regions @ {:.1} FPS / {:.2} ms",
                est_fps, avg_ms,
            ));
            self.fps_accum = 0.0;
            self.fps_frames = 0;
            self.fps_last = now;
        }
    }
}

#[derive(Default)]
struct DemoApp {
    state: Option<GpuState>,
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }
        let attrs = Window::default_attributes()
            .with_title("URX 1.6 FullGpu engine demo — starting…")
            .with_inner_size(winit::dpi::PhysicalSize::new(960u32, 720u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        self.state = Some(GpuState::new(window.clone()));
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
            WindowEvent::Resized(sz) => {
                state.resize(sz.width, sz.height);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.tick();
                state.render_frame();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event_loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = DemoApp::default();
    event_loop.run_app(&mut app).expect("run_app");
}
