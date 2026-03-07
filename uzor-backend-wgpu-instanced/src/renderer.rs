//! GPU pipeline setup, buffer management, and draw calls.
//!
//! `InstancedRenderer` owns all GPU resources (pipelines, buffers) and
//! exposes a single `render()` method that executes in 2 draw calls:
//! 1. Quad instances  (filled/bordered rounded rectangles)
//! 2. Line instances  (capsule SDF segments)
//!
//! Text rendering is stored as `TextAreaData` pending data and is not yet
//! rasterized to the screen — a new text rendering approach will be integrated
//! later.

use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use crate::instances::{LineInstance, QuadInstance};
use crate::shaders::{LINE_SHADER, QUAD_SHADER};
use crate::text::TextAreaData;

/// Initial capacity (number of instances) for both quad and line buffers.
const INITIAL_CAPACITY: usize = 1024;

/// Uniform data sent to both shaders.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Creates the vertex buffer layout for `QuadInstance`.
///
/// Each field maps to a shader `@location`.
fn quad_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    // QuadInstance fields in order:
    //  0: pos          [f32;2]  → Float32x2   @ offset  0
    //  1: size         [f32;2]  → Float32x2   @ offset  8
    //  2: color        [f32;4]  → Float32x4   @ offset 16
    //  3: corner_radius f32     → Float32     @ offset 32
    //  4: border_width  f32     → Float32     @ offset 36
    //  5: _pad0        [f32;2]  → Float32x2   @ offset 40
    //  6: border_color [f32;4]  → Float32x4   @ offset 48
    //  7: clip_rect    [f32;4]  → Float32x4   @ offset 64
    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset:  0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset:  8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x4, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32,   offset: 32 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32,   offset: 36 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32x2, offset: 40 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x4, offset: 48 },
        wgpu::VertexAttribute { shader_location: 7, format: Float32x4, offset: 64 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Creates the vertex buffer layout for `LineInstance`.
fn line_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    // LineInstance fields:
    //  0: start     [f32;2]  @ offset  0
    //  1: end       [f32;2]  @ offset  8
    //  2: color     [f32;4]  @ offset 16
    //  3: width      f32     @ offset 32
    //  4: _pad0     [f32;3]  @ offset 36
    //  5: clip_rect [f32;4]  @ offset 48
    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset:  0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset:  8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x4, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32,   offset: 32 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32x3, offset: 36 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32x4, offset: 48 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Creates an instance buffer with `capacity` elements of `elem_size` bytes.
fn make_instance_buffer(device: &wgpu::Device, capacity: usize, elem_size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instance_buffer"),
        size: (capacity * elem_size) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns all GPU resources and drives rendering each frame.
pub struct InstancedRenderer {
    quad_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,

    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    quad_buffer: wgpu::Buffer,
    quad_capacity: usize,

    line_buffer: wgpu::Buffer,
    line_capacity: usize,
}

impl InstancedRenderer {
    /// Create the renderer and all GPU resources.
    ///
    /// `format` must match the swap-chain / surface texture format.
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        // ── Uniform buffer ────────────────────────────────────────────────
        let uniform_data = Uniforms { screen_size: [1.0, 1.0], _pad: [0.0; 2] };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("instanced_uniforms"),
            contents: cast_slice(&[uniform_data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instanced_uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("instanced_uniform_bg"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // ── Shared pipeline layout ────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("instanced_pipeline_layout"),
            bind_group_layouts: &[&uniform_bgl],
            push_constant_ranges: &[],
        });

        // ── Alpha blend state ─────────────────────────────────────────────
        let blend = wgpu::BlendState::ALPHA_BLENDING;

        // ── Quad pipeline ─────────────────────────────────────────────────
        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad_shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
        });
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &quad_shader,
                entry_point: Some("vs_main"),
                buffers: &[quad_instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ── Line pipeline ─────────────────────────────────────────────────
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("line_shader"),
            source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
        });
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[line_instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ── Instance buffers ──────────────────────────────────────────────
        let quad_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<QuadInstance>());
        let line_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<LineInstance>());

        Self {
            quad_pipeline,
            line_pipeline,
            uniform_buffer,
            uniform_bind_group,
            quad_buffer,
            quad_capacity: INITIAL_CAPACITY,
            line_buffer,
            line_capacity: INITIAL_CAPACITY,
        }
    }

    /// Upload instances to a GPU buffer, growing it if needed.
    fn upload_instances<T: bytemuck::Pod>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &mut wgpu::Buffer,
        capacity: &mut usize,
        data: &[T],
    ) {
        if data.is_empty() {
            return;
        }
        let needed = data.len();
        if needed > *capacity {
            // Double capacity until sufficient
            while *capacity < needed {
                *capacity *= 2;
            }
            *buffer = make_instance_buffer(device, *capacity, std::mem::size_of::<T>());
        }
        queue.write_buffer(buffer, 0, cast_slice(data));
    }

    /// Render a complete frame.
    ///
    /// `clear_color` is the background color applied via `LoadOp::Clear`.
    /// `text_areas` is accepted for API compatibility but text is not yet
    /// rasterized — it will be rendered once a new text approach is integrated.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        quads: &[QuadInstance],
        lines: &[LineInstance],
        _text_areas: &[TextAreaData],
        clear_color: wgpu::Color,
    ) {
        // ── Update uniforms ───────────────────────────────────────────────
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));

        // ── Upload instance data ──────────────────────────────────────────
        Self::upload_instances(
            device, queue,
            &mut self.quad_buffer, &mut self.quad_capacity,
            quads,
        );
        Self::upload_instances(
            device, queue,
            &mut self.line_buffer, &mut self.line_capacity,
            lines,
        );

        // ── Command encoder + render pass ─────────────────────────────────
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("instanced_encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("instanced_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Draw call 1: quads
            if !quads.is_empty() {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_vertex_buffer(0, self.quad_buffer.slice(..));
                pass.draw(0..6, 0..quads.len() as u32);
            }

            // Draw call 2: lines
            if !lines.is_empty() {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, self.line_buffer.slice(..));
                pass.draw(0..6, 0..lines.len() as u32);
            }

            // Text rendering: stub — no draw call until new approach is integrated.
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
