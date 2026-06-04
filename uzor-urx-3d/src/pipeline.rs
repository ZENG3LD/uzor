//! Renderer3D — wgpu render pipeline + depth buffer.
//!
//! Wave 1: per-node draw_indexed.
//! Wave 2: + Mesh GPU buffer cache (Arc identity dedup of VB/IB).
//! Wave 3: + instancing — nodes are grouped by Mesh identity; each
//!         group becomes ONE draw_indexed_instanced with the per-
//!         instance Model + tint packed in a vertex buffer at
//!         step_mode=Instance. Collapses the N-draw loop into
//!         #unique-meshes draws.

use crate::{
    camera::PerspectiveCamera,
    light::LightArrayRaw,
    mesh::{Vertex, VertexLit},
    mesh_cache::MeshCache,
    scene3d::{NodeMesh, Scene3D},
};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use std::collections::BTreeMap;
use std::sync::Arc;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4], // xyz = camera position, w = unused (alignment)
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct NodeUniform {
    model: [[f32; 4]; 4], // 64
    tint: [f32; 4],       // 16
    _pad: [f32; 44],      // pad to 256 to satisfy min_uniform_buffer_offset_alignment
}

const NODE_UNIFORM_SIZE: u64 = std::mem::size_of::<NodeUniform>() as u64;

/// Per-instance vertex-buffer record for the instanced pipeline.
/// 4 vec4 columns of the model matrix + tint = 80 bytes / instance.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
}

impl InstanceRaw {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // model columns at locations 2..5
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0,  shader_location: 2 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 3 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 4 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 5 },
                // tint at location 6
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 6 },
            ],
        }
    }
}

/// Wave 4 per-instance record for the Phong path.
/// Layout: model (64) + tint (16) + material (16) = 96 bytes / instance.
/// material = [ambient_k, diffuse_k, specular_k, shininess]
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct InstanceLitRaw {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
    material: [f32; 4],
}

impl InstanceLitRaw {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceLitRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0,  shader_location: 3 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 4 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 5 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 6 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 7 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 80, shader_location: 8 },
            ],
        }
    }
}

pub struct Renderer3D {
    pipeline: wgpu::RenderPipeline,
    pipeline_instanced: wgpu::RenderPipeline,
    pipeline_phong: wgpu::RenderPipeline,
    node_bgl: wgpu::BindGroupLayout,
    frame_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    frame_bg_inst: wgpu::BindGroup,
    /// Phong path uses a combined BG (frame + lights at bindings 0+1).
    lights_buf: wgpu::Buffer,
    frame_bg_phong: wgpu::BindGroup,
    /// Ring of per-node uniform buffers — used by the non-instanced
    /// fallback path (kept for backwards compatibility / non-grouped
    /// rendering once we add per-node features that instancing can't
    /// express).
    node_buf: wgpu::Buffer,
    node_bgs: Vec<wgpu::BindGroup>,
    node_capacity: u32,
    /// Instance buffer (vertex-buffer attached at step_mode=Instance).
    instance_buf: wgpu::Buffer,
    instance_capacity: u32,
    /// Lit instance buffer for the Phong path.
    instance_lit_buf: wgpu::Buffer,
    instance_lit_capacity: u32,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    color_format: wgpu::TextureFormat,
    mesh_cache: MeshCache,
    mesh_lit_cache: crate::mesh_cache::MeshLitCache,
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

        // ── Instanced pipeline (Wave 3) ────────────────────────────────
        let shader_inst = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.unlit_instanced"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/unlit_instanced.wgsl").into()),
        });
        let frame_bgl_inst = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.frame_bgl_inst"),
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
        let pipeline_layout_inst =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout_inst"),
                bind_group_layouts: &[&frame_bgl_inst],
                immediate_size: 0,
            });
        let pipeline_instanced = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline_instanced"),
            layout: Some(&pipeline_layout_inst),
            vertex: wgpu::VertexState {
                module: &shader_inst,
                entry_point: Some("vs_main"),
                buffers: &[
                    Vertex::vertex_buffer_layout(),
                    InstanceRaw::vertex_buffer_layout(),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_inst,
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
        let frame_bg_inst = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("urx3d.frame_bg_inst"),
            layout: &frame_bgl_inst,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });

        let instance_capacity = node_capacity.max(64);
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_buf"),
            size: std::mem::size_of::<InstanceRaw>() as u64 * instance_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Phong (Wave 4) ─────────────────────────────────────────────
        let shader_phong = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.phong_instanced"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/phong_instanced.wgsl").into()),
        });
        let frame_bgl_phong = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.frame_bgl_phong"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let lights_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.lights_buf"),
            size: std::mem::size_of::<LightArrayRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bg_phong = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("urx3d.frame_bg_phong"),
            layout: &frame_bgl_phong,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lights_buf.as_entire_binding() },
            ],
        });
        let pipeline_layout_phong =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout_phong"),
                bind_group_layouts: &[&frame_bgl_phong],
                immediate_size: 0,
            });
        let pipeline_phong = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline_phong"),
            layout: Some(&pipeline_layout_phong),
            vertex: wgpu::VertexState {
                module: &shader_phong,
                entry_point: Some("vs_main"),
                buffers: &[
                    VertexLit::vertex_buffer_layout(),
                    InstanceLitRaw::vertex_buffer_layout(),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_phong,
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

        let instance_lit_capacity = node_capacity.max(64);
        let instance_lit_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_lit_buf"),
            size: std::mem::size_of::<InstanceLitRaw>() as u64 * instance_lit_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let depth_view = Self::create_depth(device, initial_size);

        Self {
            pipeline,
            pipeline_instanced,
            pipeline_phong,
            node_bgl,
            frame_buf,
            frame_bg,
            frame_bg_inst,
            lights_buf,
            frame_bg_phong,
            node_buf,
            node_bgs,
            node_capacity,
            instance_buf,
            instance_capacity,
            instance_lit_buf,
            instance_lit_capacity,
            depth_view,
            depth_size: initial_size,
            color_format,
            mesh_cache: MeshCache::new(),
            mesh_lit_cache: crate::mesh_cache::MeshLitCache::new(),
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

    fn grow_instance_buf(&mut self, device: &wgpu::Device, needed: u32) {
        if needed <= self.instance_capacity {
            return;
        }
        let new_cap = needed.next_power_of_two().max(64);
        self.instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_buf"),
            size: std::mem::size_of::<InstanceRaw>() as u64 * new_cap as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_capacity = new_cap;
    }

    fn grow_instance_lit_buf(&mut self, device: &wgpu::Device, needed: u32) {
        if needed <= self.instance_lit_capacity {
            return;
        }
        let new_cap = needed.next_power_of_two().max(64);
        self.instance_lit_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_lit_buf"),
            size: std::mem::size_of::<InstanceLitRaw>() as u64 * new_cap as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_lit_capacity = new_cap;
    }

    /// One-shot pass: encode the whole Scene3D into the target view.
    ///
    /// Wave 3 path: nodes are grouped by `Arc<Mesh>` identity; each
    /// group becomes ONE `draw_indexed_instanced` with an instance
    /// buffer of model matrices + tints. So a 10k-cube scene with one
    /// shared Mesh = ONE drawcall, regardless of node count.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        camera: &PerspectiveCamera,
        scene: &Scene3D,
    ) {
        // 1. Frame uniform (view_proj + eye position for spec calc).
        let frame = FrameUniform {
            view_proj: camera.view_proj().to_cols_array_2d(),
            eye: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
        };
        queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));

        // 2. Light uniform — uploaded once per frame; phong shader uses
        //    it, unlit shader doesn't bind binding=1.
        let lights = LightArrayRaw::from_lights(&scene.lights, scene.ambient);
        queue.write_buffer(&self.lights_buf, 0, bytemuck::bytes_of(&lights));

        // 3. Refresh mesh caches.
        self.mesh_cache.begin_frame();
        self.mesh_lit_cache.begin_frame();

        // 4. Split nodes by material into two groupings, both keyed on
        //    Arc<Mesh*> identity for instancing.
        let mut groups_unlit: BTreeMap<usize, (Arc<crate::mesh::Mesh>, Vec<usize>)> =
            BTreeMap::new();
        let mut groups_lit: BTreeMap<usize, (Arc<crate::mesh::MeshLit>, Vec<usize>)> =
            BTreeMap::new();
        for (i, n) in scene.nodes.iter().enumerate() {
            match &n.geometry {
                NodeMesh::Unlit(m) => {
                    let k = Arc::as_ptr(m) as usize;
                    groups_unlit.entry(k).or_insert_with(|| (m.clone(), Vec::new())).1.push(i);
                }
                NodeMesh::Lit(m) => {
                    let k = Arc::as_ptr(m) as usize;
                    groups_lit.entry(k).or_insert_with(|| (m.clone(), Vec::new())).1.push(i);
                }
            }
        }

        struct GroupDraw {
            vb: wgpu::Buffer,
            ib: wgpu::Buffer,
            index_count: u32,
            first_instance: u32,
            instance_count: u32,
        }

        // 5. Build unlit instance buffer.
        let total_unlit: u32 = groups_unlit.values().map(|(_, v)| v.len() as u32).sum();
        self.grow_instance_buf(device, total_unlit.max(1));
        let mut unlit_instances: Vec<InstanceRaw> = Vec::with_capacity(total_unlit as usize);
        let mut unlit_draws: Vec<GroupDraw> = Vec::with_capacity(groups_unlit.len());
        for (_k, (mesh, node_indices)) in groups_unlit.iter() {
            let entry = self.mesh_cache.get_or_upload(device, mesh);
            let first = unlit_instances.len() as u32;
            for &idx in node_indices {
                let n = &scene.nodes[idx];
                unlit_instances.push(InstanceRaw {
                    model: n.model_matrix().to_cols_array_2d(),
                    tint: n.color_tint,
                });
            }
            let count = unlit_instances.len() as u32 - first;
            unlit_draws.push(GroupDraw {
                vb: entry.vb.clone(),
                ib: entry.ib.clone(),
                index_count: entry.index_count,
                first_instance: first,
                instance_count: count,
            });
        }
        if !unlit_instances.is_empty() {
            queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&unlit_instances));
        }

        // 6. Build lit instance buffer.
        let total_lit: u32 = groups_lit.values().map(|(_, v)| v.len() as u32).sum();
        self.grow_instance_lit_buf(device, total_lit.max(1));
        let mut lit_instances: Vec<InstanceLitRaw> = Vec::with_capacity(total_lit as usize);
        let mut lit_draws: Vec<GroupDraw> = Vec::with_capacity(groups_lit.len());
        for (_k, (mesh, node_indices)) in groups_lit.iter() {
            let entry = self.mesh_lit_cache.get_or_upload(device, mesh);
            let first = lit_instances.len() as u32;
            for &idx in node_indices {
                let n = &scene.nodes[idx];
                let mat = n.material;
                lit_instances.push(InstanceLitRaw {
                    model: n.model_matrix().to_cols_array_2d(),
                    tint: n.color_tint,
                    material: [
                        mat.ambient_strength,
                        mat.diffuse_strength,
                        mat.specular_strength,
                        mat.shininess,
                    ],
                });
            }
            let count = lit_instances.len() as u32 - first;
            lit_draws.push(GroupDraw {
                vb: entry.vb.clone(),
                ib: entry.ib.clone(),
                index_count: entry.index_count,
                first_instance: first,
                instance_count: count,
            });
        }
        if !lit_instances.is_empty() {
            queue.write_buffer(&self.instance_lit_buf, 0, bytemuck::cast_slice(&lit_instances));
        }

        // 7. One render pass — both pipelines.
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

        // Unlit pass
        if !unlit_draws.is_empty() {
            pass.set_pipeline(&self.pipeline_instanced);
            pass.set_bind_group(0, &self.frame_bg_inst, &[]);
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            for g in &unlit_draws {
                pass.set_vertex_buffer(0, g.vb.slice(..));
                pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                let end = g.first_instance + g.instance_count;
                pass.draw_indexed(0..g.index_count, 0, g.first_instance..end);
            }
        }

        // Lit (Phong) pass
        if !lit_draws.is_empty() {
            pass.set_pipeline(&self.pipeline_phong);
            pass.set_bind_group(0, &self.frame_bg_phong, &[]);
            pass.set_vertex_buffer(1, self.instance_lit_buf.slice(..));
            for g in &lit_draws {
                pass.set_vertex_buffer(0, g.vb.slice(..));
                pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                let end = g.first_instance + g.instance_count;
                pass.draw_indexed(0..g.index_count, 0, g.first_instance..end);
            }
        }
    }

    /// Direct access to the non-instanced pipeline + per-node uniforms.
    /// Kept for diagnostics / future features (per-node bind groups
    /// don't survive instancing — anything that varies per-draw rather
    /// than per-instance has to fall back here).
    #[doc(hidden)]
    pub fn _non_instanced_handles(&self) -> (&wgpu::RenderPipeline, &wgpu::BindGroup, &[wgpu::BindGroup]) {
        (&self.pipeline, &self.frame_bg, &self.node_bgs)
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
