//! Renderer3D — wgpu render pipeline + depth buffer + per-node uniform
//! ring.
//!
//! Wave 1 keeps it simple: vertex+fragment pipeline, Depth32Float, no
//! instancing (Wave 2 adds instance buffer). Each Node draws with its
//! own small uniform buffer (one model matrix + tint). The buffer is
//! cycled through a ring so we don't stall on a single uniform slot.

use crate::{camera::PerspectiveCamera, mesh::Vertex, scene3d::Scene3D};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct NodeUniform {
    model: [[f32; 4]; 4], // 64
    tint: [f32; 4],       // 16
    _pad: [f32; 44],      // pad to 256 to satisfy min_uniform_buffer_offset_alignment
}

const NODE_UNIFORM_SIZE: u64 = std::mem::size_of::<NodeUniform>() as u64;

pub struct Renderer3D {
    pipeline: wgpu::RenderPipeline,
    node_bgl: wgpu::BindGroupLayout,
    frame_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    /// Ring of per-node uniform buffers — one buffer, N offsets.
    node_buf: wgpu::Buffer,
    node_bgs: Vec<wgpu::BindGroup>,
    node_capacity: u32,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    color_format: wgpu::TextureFormat,
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

impl Renderer3D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        initial_size: (u32, u32),
        node_capacity: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.unlit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/unlit.wgsl").into()),
        });

        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.frame_bgl"),
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

        let node_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.node_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout"),
                bind_group_layouts: &[&frame_bgl, &node_bgl],
                immediate_size: 0,
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::vertex_buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.frame_buf"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("urx3d.frame_bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });

        let (node_buf, node_bgs) =
            Self::allocate_node_ring(device, &node_bgl, node_capacity);

        let depth_view = Self::create_depth(device, initial_size);

        Self {
            pipeline,
            node_bgl,
            frame_buf,
            frame_bg,
            node_buf,
            node_bgs,
            node_capacity,
            depth_view,
            depth_size: initial_size,
            color_format,
        }
    }

    fn allocate_node_ring(
        device: &wgpu::Device,
        node_bgl: &wgpu::BindGroupLayout,
        cap: u32,
    ) -> (wgpu::Buffer, Vec<wgpu::BindGroup>) {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.node_ring"),
            size: NODE_UNIFORM_SIZE * cap as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bgs = Vec::with_capacity(cap as usize);
        for i in 0..cap {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("urx3d.node_bg"),
                layout: node_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buf,
                        offset: NODE_UNIFORM_SIZE * i as u64,
                        size: std::num::NonZeroU64::new(NODE_UNIFORM_SIZE),
                    }),
                }],
            });
            bgs.push(bg);
        }
        (buf, bgs)
    }

    fn create_depth(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx3d.depth"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if size == self.depth_size {
            return;
        }
        self.depth_view = Self::create_depth(device, size);
        self.depth_size = size;
    }

    pub fn grow_node_ring(&mut self, device: &wgpu::Device, needed: u32) {
        if needed <= self.node_capacity {
            return;
        }
        let new_cap = needed.next_power_of_two().max(16);
        let (buf, bgs) = Self::allocate_node_ring(device, &self.node_bgl, new_cap);
        self.node_buf = buf;
        self.node_bgs = bgs;
        self.node_capacity = new_cap;
    }

    /// One-shot pass: encode the whole Scene3D into the target view.
    ///
    /// Vertex/index buffers are uploaded fresh every frame for Wave 1 —
    /// Wave 2 caches per-Mesh GPU buffers in a registry. This keeps the
    /// API surface minimal during bringup.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        camera: &PerspectiveCamera,
        scene: &Scene3D,
    ) {
        // 1. Frame uniform
        let frame = FrameUniform { view_proj: camera.view_proj().to_cols_array_2d() };
        queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));

        // 2. Grow node ring if needed
        self.grow_node_ring(device, scene.nodes.len() as u32);

        // 3. Per-node uniforms in one big write
        let mut node_data: Vec<NodeUniform> = Vec::with_capacity(scene.nodes.len());
        for n in &scene.nodes {
            node_data.push(NodeUniform {
                model: n.model_matrix().to_cols_array_2d(),
                tint: n.color_tint,
                _pad: [0.0; 44],
            });
        }
        if !node_data.is_empty() {
            queue.write_buffer(&self.node_buf, 0, bytemuck::cast_slice(&node_data));
        }

        // 4. Vertex/index buffers per node (Wave 1: fresh each frame).
        let mut vb_ib: Vec<(wgpu::Buffer, wgpu::Buffer, u32)> = Vec::with_capacity(scene.nodes.len());
        for n in &scene.nodes {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("urx3d.vb"),
                contents: bytemuck::cast_slice(&n.mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("urx3d.ib"),
                contents: bytemuck::cast_slice(&n.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            vb_ib.push((vb, ib, n.mesh.indices.len() as u32));
        }

        // 5. Render pass
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx3d.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: scene.clear_color[0] as f64,
                        g: scene.clear_color[1] as f64,
                        b: scene.clear_color[2] as f64,
                        a: scene.clear_color[3] as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.frame_bg, &[]);

        for (i, (vb, ib, n_idx)) in vb_ib.iter().enumerate() {
            pass.set_bind_group(1, &self.node_bgs[i], &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*n_idx, 0, 0..1);
        }
    }

    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }
}

/// Build a model matrix from translation/rotation/scale.
#[inline]
pub fn trs(t: glam::Vec3, r: glam::Quat, s: glam::Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(s, r, t)
}
