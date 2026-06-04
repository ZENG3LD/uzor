//! Wave 19 — CPU-side particle system + GPU instanced billboard
//! rendering.
//!
//! Each particle is updated on the CPU (~100k-particle budget at
//! 60 FPS on the test box) and uploaded as an instance buffer to a
//! dedicated pipeline that draws screen-aligned quads. Particles
//! support velocity + gravity + linear age + 2-stop color gradient
//! (start → end) + size taper (start_size → end_size).
//!
//! Public API:
//!   - `Particle` (raw record)
//!   - `EmitterConfig` (per-emitter parameters)
//!   - `ParticleSystem` (one emitter + N particles + tick())
//!   - `ParticleRenderer::draw` — wgpu pipeline + per-frame instance
//!     buffer
//!
//! The renderer is OWNED by the consumer (Renderer3D doesn't bundle
//! it) so apps can have multiple emitters drawing through a single
//! shared particle pipeline.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::sync::Arc;

use crate::camera::PerspectiveCamera;
use crate::pipeline::HDR_FORMAT;

#[derive(Debug, Clone, Copy)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub lifetime: f32,
    pub size_start: f32,
    pub size_end: f32,
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
}

impl Particle {
    fn life_t(&self) -> f32 { (self.age / self.lifetime.max(1e-4)).clamp(0.0, 1.0) }
    fn current_size(&self) -> f32 {
        let t = self.life_t();
        self.size_start * (1.0 - t) + self.size_end * t
    }
    fn current_color(&self) -> [f32; 4] {
        let t = self.life_t();
        let mut c = [0.0f32; 4];
        for i in 0..4 {
            c[i] = self.color_start[i] * (1.0 - t) + self.color_end[i] * t;
        }
        c
    }
}

#[derive(Debug, Clone)]
pub struct EmitterConfig {
    /// Particles spawned per second.
    pub rate: f32,
    /// Maximum live particles at any moment.
    pub capacity: u32,
    /// World-space spawn point.
    pub position: Vec3,
    /// Per-particle initial velocity range; emitter samples uniformly
    /// between min and max each axis.
    pub vel_min: Vec3,
    pub vel_max: Vec3,
    /// Constant acceleration applied each tick (use for gravity).
    pub gravity: Vec3,
    pub lifetime: f32,
    pub size_start: f32,
    pub size_end: f32,
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            rate: 200.0,
            capacity: 4096,
            position: Vec3::ZERO,
            vel_min: Vec3::new(-1.0, 2.0, -1.0),
            vel_max: Vec3::new(1.0, 4.0, 1.0),
            gravity: Vec3::new(0.0, -2.0, 0.0),
            lifetime: 1.5,
            size_start: 0.12,
            size_end: 0.02,
            color_start: [1.0, 0.9, 0.3, 1.0],
            color_end: [1.0, 0.2, 0.0, 0.0],
        }
    }
}

pub struct ParticleSystem {
    pub config: EmitterConfig,
    particles: Vec<Particle>,
    spawn_accum: f32,
    /// Simple Linear Congruential Generator state — keeps the test
    /// deterministic without pulling in `rand`.
    rng_state: u64,
}

impl ParticleSystem {
    pub fn new(config: EmitterConfig) -> Self {
        Self {
            particles: Vec::with_capacity(config.capacity as usize),
            config,
            spawn_accum: 0.0,
            rng_state: 0xC0FFEE_DEAD_BEEFu64,
        }
    }

    /// Number of currently-live particles.
    pub fn live(&self) -> usize { self.particles.len() }

    pub fn particles(&self) -> &[Particle] { &self.particles }

    fn rand_u32(&mut self) -> u32 {
        // LCG (Numerical Recipes)
        self.rng_state = self.rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_state >> 16) as u32
    }
    fn rand_f01(&mut self) -> f32 {
        (self.rand_u32() & 0xFFFFFF) as f32 / 0x1_000_000 as f32
    }
    fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

    /// Advance the simulation by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        // Age + integrate existing particles.
        let grav = self.config.gravity;
        for p in &mut self.particles {
            p.age += dt;
            p.vel += grav * dt;
            p.pos += p.vel * dt;
        }
        // Remove dead ones (swap_remove for O(1) per kill).
        let mut i = 0;
        while i < self.particles.len() {
            if self.particles[i].age >= self.particles[i].lifetime {
                self.particles.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // Spawn new ones up to capacity.
        self.spawn_accum += self.config.rate * dt;
        let to_spawn = self.spawn_accum.floor() as u32;
        self.spawn_accum -= to_spawn as f32;
        for _ in 0..to_spawn {
            if self.particles.len() as u32 >= self.config.capacity { break; }
            let vx = Self::lerp(self.config.vel_min.x, self.config.vel_max.x, self.rand_f01());
            let vy = Self::lerp(self.config.vel_min.y, self.config.vel_max.y, self.rand_f01());
            let vz = Self::lerp(self.config.vel_min.z, self.config.vel_max.z, self.rand_f01());
            self.particles.push(Particle {
                pos: self.config.position,
                vel: Vec3::new(vx, vy, vz),
                age: 0.0,
                lifetime: self.config.lifetime,
                size_start: self.config.size_start,
                size_end: self.config.size_end,
                color_start: self.config.color_start,
                color_end: self.config.color_end,
            });
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ParticleInstance {
    pos_size: [f32; 4],   // xyz = world pos, w = current size
    color:    [f32; 4],
}

impl ParticleInstance {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0,  shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ParticleFrame {
    view_proj: [[f32; 4]; 4],
    cam_right: [f32; 4],
    cam_up:    [f32; 4],
}

pub struct ParticleRenderer {
    pipeline: wgpu::RenderPipeline,
    frame_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    instance_buf: wgpu::Buffer,
    instance_capacity: u32,
}

impl ParticleRenderer {
    pub fn new(device: &wgpu::Device, _color_format_unused: wgpu::TextureFormat) -> Arc<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.particles"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particles.wgsl").into()),
        });
        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.particle_frame_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.particle_frame_buf"),
            size: std::mem::size_of::<ParticleFrame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("urx3d.particle_frame_bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("urx3d.particle_pipeline_layout"),
            bind_group_layouts: &[&frame_bgl],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline_particles"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[ParticleInstance::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    // Additive blending for fire/sparks aesthetic.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::pipeline::DEPTH_FORMAT,
                // Write disabled — particles read depth but don't write.
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.particle_instance_buf"),
            size: std::mem::size_of::<ParticleInstance>() as u64 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Arc::new(Self {
            pipeline, frame_buf, frame_bg, instance_buf, instance_capacity: 1024,
        })
    }

    /// Encode a single drawcall that paints the current particle set
    /// of `system` into `target_view` (an HDR texture — the composite
    /// pass will tonemap them with the rest of the scene).
    pub fn draw(
        self: &mut Arc<Self>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        camera: &PerspectiveCamera,
        system: &ParticleSystem,
    ) {
        // Build per-frame uniform: view_proj + cam_right + cam_up
        // (used by the vertex shader to billboard each particle).
        let view = glam::Mat4::look_at_rh(camera.eye, camera.target, glam::Vec3::Y);
        let cam_right = glam::Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let cam_up    = glam::Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let frame = ParticleFrame {
            view_proj: camera.view_proj().to_cols_array_2d(),
            cam_right: [cam_right.x, cam_right.y, cam_right.z, 0.0],
            cam_up:    [cam_up.x, cam_up.y, cam_up.z, 0.0],
        };
        queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));

        let live = system.live() as u32;
        if live == 0 { return; }

        let me = Arc::get_mut(self).expect("ParticleRenderer must be uniquely held during draw");
        // Grow instance buffer if needed.
        if live > me.instance_capacity {
            let cap = live.next_power_of_two();
            me.instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("urx3d.particle_instance_buf"),
                size: std::mem::size_of::<ParticleInstance>() as u64 * cap as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            me.instance_capacity = cap;
        }
        let mut instances: Vec<ParticleInstance> = Vec::with_capacity(live as usize);
        for p in system.particles() {
            instances.push(ParticleInstance {
                pos_size: [p.pos.x, p.pos.y, p.pos.z, p.current_size()],
                color: p.current_color(),
            });
        }
        queue.write_buffer(&me.instance_buf, 0, bytemuck::cast_slice(&instances));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx3d.pass_particles"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&me.pipeline);
        pass.set_bind_group(0, &me.frame_bg, &[]);
        pass.set_vertex_buffer(0, me.instance_buf.slice(..));
        // 4 vertices per instance (triangle strip), `live` instances.
        pass.draw(0..4, 0..live);
    }
}
