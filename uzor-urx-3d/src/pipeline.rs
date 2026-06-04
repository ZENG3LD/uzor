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
    mesh::{Vertex, VertexLit, VertexPbr, VertexUv},
    mesh_cache::MeshCache,
    scene3d::{NodeMesh, Scene3D},
    texture::{Texture3D, TextureCache},
};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use std::collections::BTreeMap;
use std::sync::Arc;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],       // 64
    eye: [f32; 4],                   // 16 — xyz = camera pos, w = unused
    light_view_proj: [[f32; 4]; 4], // 64 — Wave 7 shadow map projection
    shadow_params: [f32; 4],         // 16 — x = has_shadow (0/1), y..w unused
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

/// Wave 6 per-instance record for the PBR pipeline. PBR vertex has 4
/// attribute slots (pos@0, normal@1, tangent@2, uv@3) so per-instance
/// locations start at 4.
///
/// Layout: model (64) + tint (16) + pbr_params (16) = 96 bytes.
/// pbr_params = [metalness, roughness, ao, has_normal_map]
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct InstancePbrRaw {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
    pbr_params: [f32; 4],
}

impl InstancePbrRaw {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstancePbrRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0,  shader_location: 4 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 5 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 6 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 7 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 8 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 80, shader_location: 9 },
            ],
        }
    }
}

pub struct Renderer3D {
    pipeline: wgpu::RenderPipeline,
    pipeline_instanced: wgpu::RenderPipeline,
    pipeline_phong: wgpu::RenderPipeline,
    pipeline_textured: wgpu::RenderPipeline,
    pipeline_pbr: wgpu::RenderPipeline,
    // Wave 7 — shadow caster pipelines (depth-only)
    pipeline_shadow_lit: wgpu::RenderPipeline,
    pipeline_shadow_pbr: wgpu::RenderPipeline,
    node_bgl: wgpu::BindGroupLayout,
    tex_bgl: wgpu::BindGroupLayout,
    frame_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    frame_bg_inst: wgpu::BindGroup,
    /// Phong path uses a combined BG (frame + lights at bindings 0+1).
    lights_buf: wgpu::Buffer,
    frame_bg_phong: wgpu::BindGroup,
    /// Textured path reuses the same frame+lights BG; needs its own
    /// per-texture BG (texture view + sampler), cached.
    texture_cache: TextureCache,
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
    /// Textured instance buffer (same InstanceLitRaw layout, distinct
    /// storage so phong + textured can both bind their own VBs in one
    /// render pass).
    instance_tex_buf: wgpu::Buffer,
    instance_tex_capacity: u32,
    /// PBR instance buffer.
    instance_pbr_buf: wgpu::Buffer,
    instance_pbr_capacity: u32,
    /// 1×1 flat-blue normal-map stub for PBR nodes without a real
    /// normal map (sampled but multiplied by has_normal_map=0).
    normal_map_stub: Arc<Texture3D>,
    /// Wave 7 — shadow map texture + view + dedicated frame uniform
    /// buffer for the depth pass (carries light_view_proj as view_proj).
    shadow_view: wgpu::TextureView,
    shadow_frame_buf: wgpu::Buffer,
    shadow_frame_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    color_format: wgpu::TextureFormat,
    mesh_cache: MeshCache,
    mesh_lit_cache: crate::mesh_cache::MeshLitCache,
    mesh_uv_cache: crate::mesh_cache::MeshUvCache,
    mesh_pbr_cache: crate::mesh_cache::MeshPbrCache,
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

impl Renderer3D {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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
        // (Old pipeline_phong layout/pipeline removed in Wave 7 — replaced
        // by `pipeline_phong` further down that includes the shadow_bgl
        // bind group. shader_phong now references @group(1) which the
        // OLD layout doesn't provide → wgpu validation fail. Pipeline is
        // constructed once, after shadow_bgl exists.)

        let instance_lit_capacity = node_capacity.max(64);
        let instance_lit_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_lit_buf"),
            size: std::mem::size_of::<InstanceLitRaw>() as u64 * instance_lit_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_tex_capacity = node_capacity.max(64);
        let instance_tex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_tex_buf"),
            size: std::mem::size_of::<InstanceLitRaw>() as u64 * instance_tex_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Textured (Wave 5) ──────────────────────────────────────────
        let shader_tex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.textured_instanced"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/textured_instanced.wgsl").into(),
            ),
        });
        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.tex_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout_tex =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout_textured"),
                bind_group_layouts: &[&frame_bgl_phong, &tex_bgl],
                immediate_size: 0,
            });
        let pipeline_textured = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline_textured"),
            layout: Some(&pipeline_layout_tex),
            vertex: wgpu::VertexState {
                module: &shader_tex,
                entry_point: Some("vs_main"),
                buffers: &[
                    VertexUv::vertex_buffer_layout(),
                    InstanceLitRaw::vertex_buffer_layout(),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_tex,
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

        // ── PBR (Wave 6) ───────────────────────────────────────────────
        let shader_pbr = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.pbr_instanced"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/pbr_instanced.wgsl").into()),
        });
        // PBR uses three bind groups: 0 frame+lights, 1 albedo, 2 normal map.
        // Layouts 0 + 1 already exist (frame_bgl_phong + tex_bgl); the
        // normal map layout is the SAME shape as tex_bgl so we can reuse
        // it (texture + sampler).
        // (Old pipeline_pbr — same reason as old pipeline_phong above —
        // replaced by `pipeline_pbr` after shadow_bgl is in scope.)
        let instance_pbr_capacity = node_capacity.max(64);
        let instance_pbr_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_pbr_buf"),
            size: std::mem::size_of::<InstancePbrRaw>() as u64 * instance_pbr_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // 1×1 flat-blue normal map = tangent-space (0,0,1) packed to
        // [0.5,0.5,1.0]. Sampled but multiplied by has_normal_map=0.
        let normal_map_stub = Arc::new(Texture3D::from_rgba8(
            device, queue, 1, 1, &[128, 128, 255, 255],
        ));

        // ── Shadow pass (Wave 7) ──────────────────────────────────────
        let shadow_depth_size = 2048u32;
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx3d.shadow_map"),
            size: wgpu::Extent3d {
                width: shadow_depth_size,
                height: shadow_depth_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.shadow_frame_buf"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // shadow_frame_bgl is identical SHAPE to frame_bgl (one uniform).
        let shadow_frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.shadow_frame_bgl"),
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
        let shadow_frame_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("urx3d.shadow_frame_bg"),
            layout: &shadow_frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_frame_buf.as_entire_binding(),
            }],
        });

        // shadow_bgl = depth texture + compare sampler (used by phong + pbr passes)
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx3d.shadow_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("urx3d.shadow_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("urx3d.shadow_bg"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });

        let shader_shadow = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx3d.shadow_pass"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow_pass.wgsl").into()),
        });

        // Re-build phong + pbr pipelines INCLUDING shadow group at bind group 1 / 3
        // To keep the diff minimal we add shadow_bgl as an extra group on
        // the pipeline_layout for these two pipelines. The original
        // pipeline_layouts are already constructed above with the OLD
        // layouts — so we redefine pipeline_phong + pipeline_pbr below
        // with the right layouts.
        //
        // Sleight of hand: the existing pipelines (variables `pipeline_phong`
        // and `pipeline_pbr`) are about to be shadowed by new bindings
        // BEFORE they reach `Self {}`, so any earlier reference doesn't leak.
        let pipeline_layout_phong_v2 =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout_phong_v2"),
                bind_group_layouts: &[&frame_bgl_phong, &shadow_bgl],
                immediate_size: 0,
            });
        let pipeline_phong = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline_phong_v2"),
            layout: Some(&pipeline_layout_phong_v2),
            vertex: wgpu::VertexState {
                module: &shader_phong,
                entry_point: Some("vs_main"),
                buffers: &[VertexLit::vertex_buffer_layout(), InstanceLitRaw::vertex_buffer_layout()],
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

        let pipeline_layout_pbr_v2 =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout_pbr_v2"),
                bind_group_layouts: &[&frame_bgl_phong, &tex_bgl, &tex_bgl, &shadow_bgl],
                immediate_size: 0,
            });
        let pipeline_pbr = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx3d.pipeline_pbr_v2"),
            layout: Some(&pipeline_layout_pbr_v2),
            vertex: wgpu::VertexState {
                module: &shader_pbr,
                entry_point: Some("vs_main"),
                buffers: &[VertexPbr::vertex_buffer_layout(), InstancePbrRaw::vertex_buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_pbr,
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

        // Shadow-caster pipelines (depth-only, no color target)
        let pipeline_layout_shadow =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("urx3d.pipeline_layout_shadow"),
                bind_group_layouts: &[&shadow_frame_bgl],
                immediate_size: 0,
            });
        let make_shadow_pipeline = |label: &str, vs_entry: &str, vb_layouts: &[wgpu::VertexBufferLayout]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout_shadow),
                vertex: wgpu::VertexState {
                    module: &shader_shadow,
                    entry_point: Some(vs_entry),
                    buffers: vb_layouts,
                    compilation_options: Default::default(),
                },
                fragment: None,
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
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let lit_vb = [VertexLit::vertex_buffer_layout(), InstanceLitRaw::vertex_buffer_layout()];
        let pbr_vb = [VertexPbr::vertex_buffer_layout(), InstancePbrRaw::vertex_buffer_layout()];
        let pipeline_shadow_lit = make_shadow_pipeline("urx3d.pipeline_shadow_lit", "vs_lit", &lit_vb);
        let pipeline_shadow_pbr = make_shadow_pipeline("urx3d.pipeline_shadow_pbr", "vs_pbr", &pbr_vb);

        let depth_view = Self::create_depth(device, initial_size);

        Self {
            pipeline,
            pipeline_instanced,
            pipeline_phong,
            pipeline_textured,
            pipeline_pbr,
            pipeline_shadow_lit,
            pipeline_shadow_pbr,
            node_bgl,
            tex_bgl,
            frame_buf,
            frame_bg,
            frame_bg_inst,
            lights_buf,
            frame_bg_phong,
            texture_cache: TextureCache::new(),
            node_buf,
            node_bgs,
            node_capacity,
            instance_buf,
            instance_capacity,
            instance_lit_buf,
            instance_lit_capacity,
            instance_tex_buf,
            instance_tex_capacity,
            instance_pbr_buf,
            instance_pbr_capacity,
            normal_map_stub,
            shadow_view,
            shadow_frame_buf,
            shadow_frame_bg,
            shadow_bg,
            depth_view,
            depth_size: initial_size,
            color_format,
            mesh_cache: MeshCache::new(),
            mesh_lit_cache: crate::mesh_cache::MeshLitCache::new(),
            mesh_uv_cache: crate::mesh_cache::MeshUvCache::new(),
            mesh_pbr_cache: crate::mesh_cache::MeshPbrCache::new(),
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

    fn grow_instance_tex_buf(&mut self, device: &wgpu::Device, needed: u32) {
        if needed <= self.instance_tex_capacity {
            return;
        }
        let new_cap = needed.next_power_of_two().max(64);
        self.instance_tex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_tex_buf"),
            size: std::mem::size_of::<InstanceLitRaw>() as u64 * new_cap as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_tex_capacity = new_cap;
    }

    fn grow_instance_pbr_buf(&mut self, device: &wgpu::Device, needed: u32) {
        if needed <= self.instance_pbr_capacity {
            return;
        }
        let new_cap = needed.next_power_of_two().max(64);
        self.instance_pbr_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx3d.instance_pbr_buf"),
            size: std::mem::size_of::<InstancePbrRaw>() as u64 * new_cap as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_pbr_capacity = new_cap;
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
        // 1a. Pick the first DIRECTIONAL light as the shadow caster (if any).
        let mut shadow_light_dir: Option<glam::Vec3> = None;
        for l in &scene.lights {
            if let crate::light::Light::Directional { direction, .. } = l {
                shadow_light_dir = Some(*direction);
                break;
            }
        }
        let has_shadow = shadow_light_dir.is_some();
        // 1b. Build a light_view_proj from the first directional light.
        //     Orthographic projection covers an axis-aligned cube around
        //     the scene origin sized to a fixed half-extent. Good enough
        //     for cube/plane Wave 7 demos; mlc/tessera-scale scenes can
        //     pass a custom shadow camera in a future API rev.
        let light_view_proj = if let Some(dir) = shadow_light_dir {
            let dir_n = dir.normalize_or_zero();
            let half_extent = 8.0_f32;
            let light_eye = -dir_n * 12.0;
            // Pick an up vector that is NOT parallel to the view dir.
            let up = if dir_n.y.abs() > 0.99 {
                glam::Vec3::Z
            } else {
                glam::Vec3::Y
            };
            let view = glam::Mat4::look_at_rh(light_eye, glam::Vec3::ZERO, up);
            let proj = glam::Mat4::orthographic_rh(
                -half_extent, half_extent,
                -half_extent, half_extent,
                0.5, 30.0,
            );
            proj * view
        } else {
            Mat4::IDENTITY
        };

        // 1c. Frame uniform — view_proj + eye + light_view_proj + shadow_params
        let frame = FrameUniform {
            view_proj: camera.view_proj().to_cols_array_2d(),
            eye: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
            light_view_proj: light_view_proj.to_cols_array_2d(),
            shadow_params: [if has_shadow { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));

        // 1d. Shadow-pass frame uniform: view_proj REPLACED with
        //     light_view_proj so the vertex shader projects from the
        //     light's POV. Other fields don't matter for depth-only.
        let shadow_frame = FrameUniform {
            view_proj: light_view_proj.to_cols_array_2d(),
            eye: [0.0; 4],
            light_view_proj: light_view_proj.to_cols_array_2d(),
            shadow_params: [0.0; 4],
        };
        queue.write_buffer(&self.shadow_frame_buf, 0, bytemuck::bytes_of(&shadow_frame));

        // 2. Light uniform — uploaded once per frame; phong shader uses
        //    it, unlit shader doesn't bind binding=1.
        let lights = LightArrayRaw::from_lights(&scene.lights, scene.ambient);
        queue.write_buffer(&self.lights_buf, 0, bytemuck::bytes_of(&lights));

        // 3. Refresh mesh caches.
        self.mesh_cache.begin_frame();
        self.mesh_lit_cache.begin_frame();
        self.mesh_uv_cache.begin_frame();
        self.mesh_pbr_cache.begin_frame();
        self.texture_cache.begin_frame();

        // 4. Split nodes by material into four groupings, all keyed on
        //    Arc<*> identity for instancing.
        let mut groups_unlit: BTreeMap<usize, (Arc<crate::mesh::Mesh>, Vec<usize>)> =
            BTreeMap::new();
        let mut groups_lit: BTreeMap<usize, (Arc<crate::mesh::MeshLit>, Vec<usize>)> =
            BTreeMap::new();
        let mut groups_tex: BTreeMap<
            (usize, usize),
            (Arc<crate::mesh::MeshUv>, Arc<Texture3D>, Vec<usize>),
        > = BTreeMap::new();
        // PBR groups key on (mesh, albedo, normal_map). normal_map can be
        // None → key uses 0 sentinel; the shader's has_normal_map flag
        // distinguishes.
        let mut groups_pbr: BTreeMap<
            (usize, usize, usize),
            (
                Arc<crate::mesh::MeshPbr>,
                Arc<Texture3D>,
                Option<Arc<Texture3D>>,
                Vec<usize>,
            ),
        > = BTreeMap::new();
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
                NodeMesh::Textured(m, t) => {
                    let k = (Arc::as_ptr(m) as usize, Arc::as_ptr(t) as usize);
                    groups_tex
                        .entry(k)
                        .or_insert_with(|| (m.clone(), t.clone(), Vec::new()))
                        .2
                        .push(i);
                }
                NodeMesh::Pbr(m, mat) => {
                    let nm_ptr = mat.normal_map.as_ref().map_or(0, |a| Arc::as_ptr(a) as usize);
                    let k = (
                        Arc::as_ptr(m) as usize,
                        Arc::as_ptr(&mat.albedo) as usize,
                        nm_ptr,
                    );
                    groups_pbr
                        .entry(k)
                        .or_insert_with(|| (m.clone(), mat.albedo.clone(), mat.normal_map.clone(), Vec::new()))
                        .3
                        .push(i);
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

        // 6b. Build textured instance buffer + per-group BG.
        let total_tex: u32 = groups_tex.values().map(|(_, _, v)| v.len() as u32).sum();
        self.grow_instance_tex_buf(device, total_tex.max(1));
        let mut tex_instances: Vec<InstanceLitRaw> = Vec::with_capacity(total_tex as usize);
        struct TexGroupDraw {
            vb: wgpu::Buffer,
            ib: wgpu::Buffer,
            index_count: u32,
            first_instance: u32,
            instance_count: u32,
            bg: wgpu::BindGroup,
        }
        let mut tex_draws: Vec<TexGroupDraw> = Vec::with_capacity(groups_tex.len());
        for (_k, (mesh, tex, node_indices)) in groups_tex.iter() {
            let entry = self.mesh_uv_cache.get_or_upload(device, mesh);
            let first = tex_instances.len() as u32;
            for &idx in node_indices {
                let n = &scene.nodes[idx];
                let mat = n.material;
                tex_instances.push(InstanceLitRaw {
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
            let count = tex_instances.len() as u32 - first;

            // Build per-texture BG inline — TextureCache holds the
            // resulting BindGroup for next-frame reuse keyed on Arc.
            // We need the BG by value (clone-safe — BindGroup is
            // Arc-internal in wgpu).
            let bg = {
                let layout = &self.tex_bgl;
                let cached = self.texture_cache.get_or_create(tex, |t| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("urx3d.tex_bg"),
                        layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&t.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&t.sampler),
                            },
                        ],
                    })
                });
                cached.clone()
            };

            tex_draws.push(TexGroupDraw {
                vb: entry.vb.clone(),
                ib: entry.ib.clone(),
                index_count: entry.index_count,
                first_instance: first,
                instance_count: count,
                bg,
            });
        }
        if !tex_instances.is_empty() {
            queue.write_buffer(
                &self.instance_tex_buf,
                0,
                bytemuck::cast_slice(&tex_instances),
            );
        }

        // 6c. Build PBR instance buffer + per-(albedo,normal) BGs.
        let total_pbr: u32 = groups_pbr.values().map(|(_, _, _, v)| v.len() as u32).sum();
        self.grow_instance_pbr_buf(device, total_pbr.max(1));
        let mut pbr_instances: Vec<InstancePbrRaw> = Vec::with_capacity(total_pbr as usize);
        struct PbrGroupDraw {
            vb: wgpu::Buffer,
            ib: wgpu::Buffer,
            index_count: u32,
            first_instance: u32,
            instance_count: u32,
            bg_albedo: wgpu::BindGroup,
            bg_normal: wgpu::BindGroup,
        }
        let mut pbr_draws: Vec<PbrGroupDraw> = Vec::with_capacity(groups_pbr.len());
        for (_k, (mesh, albedo, nm_opt, node_indices)) in groups_pbr.iter() {
            let entry = self.mesh_pbr_cache.get_or_upload(device, mesh);
            let first = pbr_instances.len() as u32;
            for &idx in node_indices {
                let n = &scene.nodes[idx];
                let mat = match &n.geometry {
                    NodeMesh::Pbr(_, m) => m,
                    _ => unreachable!(),
                };
                let has_nm: f32 = if mat.normal_map.is_some() { 1.0 } else { 0.0 };
                pbr_instances.push(InstancePbrRaw {
                    model: n.model_matrix().to_cols_array_2d(),
                    tint: n.color_tint,
                    pbr_params: [mat.metalness, mat.roughness, mat.ao, has_nm],
                });
            }
            let count = pbr_instances.len() as u32 - first;

            // Albedo BG
            let bg_albedo = {
                let layout = &self.tex_bgl;
                let cached = self.texture_cache.get_or_create(albedo, |t| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("urx3d.tex_bg.albedo"),
                        layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&t.view) },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&t.sampler) },
                        ],
                    })
                });
                cached.clone()
            };
            // Normal map BG (stub when None)
            let nm_arc = nm_opt.clone().unwrap_or_else(|| self.normal_map_stub.clone());
            let bg_normal = {
                let layout = &self.tex_bgl;
                let cached = self.texture_cache.get_or_create(&nm_arc, |t| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("urx3d.tex_bg.normal"),
                        layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&t.view) },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&t.sampler) },
                        ],
                    })
                });
                cached.clone()
            };

            pbr_draws.push(PbrGroupDraw {
                vb: entry.vb.clone(),
                ib: entry.ib.clone(),
                index_count: entry.index_count,
                first_instance: first,
                instance_count: count,
                bg_albedo,
                bg_normal,
            });
        }
        if !pbr_instances.is_empty() {
            queue.write_buffer(
                &self.instance_pbr_buf,
                0,
                bytemuck::cast_slice(&pbr_instances),
            );
        }

        // 6d. Shadow pass (Wave 7) — depth-only pre-pass into shadow_tex.
        //     Casters = lit groups + pbr groups (other materials skip).
        if has_shadow && (!lit_draws.is_empty() || !pbr_draws.is_empty()) {
            let mut spass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("urx3d.shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
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

            if !lit_draws.is_empty() {
                spass.set_pipeline(&self.pipeline_shadow_lit);
                spass.set_bind_group(0, &self.shadow_frame_bg, &[]);
                spass.set_vertex_buffer(1, self.instance_lit_buf.slice(..));
                for g in &lit_draws {
                    spass.set_vertex_buffer(0, g.vb.slice(..));
                    spass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                    let end = g.first_instance + g.instance_count;
                    spass.draw_indexed(0..g.index_count, 0, g.first_instance..end);
                }
            }
            if !pbr_draws.is_empty() {
                spass.set_pipeline(&self.pipeline_shadow_pbr);
                spass.set_bind_group(0, &self.shadow_frame_bg, &[]);
                spass.set_vertex_buffer(1, self.instance_pbr_buf.slice(..));
                for g in &pbr_draws {
                    spass.set_vertex_buffer(0, g.vb.slice(..));
                    spass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                    let end = g.first_instance + g.instance_count;
                    spass.draw_indexed(0..g.index_count, 0, g.first_instance..end);
                }
            }
        }

        // 7. One render pass — four pipelines.
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
            pass.set_bind_group(1, &self.shadow_bg, &[]);
            pass.set_vertex_buffer(1, self.instance_lit_buf.slice(..));
            for g in &lit_draws {
                pass.set_vertex_buffer(0, g.vb.slice(..));
                pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                let end = g.first_instance + g.instance_count;
                pass.draw_indexed(0..g.index_count, 0, g.first_instance..end);
            }
        }

        // Textured (Wave 5) pass
        if !tex_draws.is_empty() {
            pass.set_pipeline(&self.pipeline_textured);
            pass.set_bind_group(0, &self.frame_bg_phong, &[]);
            pass.set_vertex_buffer(1, self.instance_tex_buf.slice(..));
            for g in &tex_draws {
                pass.set_bind_group(1, &g.bg, &[]);
                pass.set_vertex_buffer(0, g.vb.slice(..));
                pass.set_index_buffer(g.ib.slice(..), wgpu::IndexFormat::Uint32);
                let end = g.first_instance + g.instance_count;
                pass.draw_indexed(0..g.index_count, 0, g.first_instance..end);
            }
        }

        // PBR (Wave 6) pass — with Wave 7 shadow group at slot 3
        if !pbr_draws.is_empty() {
            pass.set_pipeline(&self.pipeline_pbr);
            pass.set_bind_group(0, &self.frame_bg_phong, &[]);
            pass.set_bind_group(3, &self.shadow_bg, &[]);
            pass.set_vertex_buffer(1, self.instance_pbr_buf.slice(..));
            for g in &pbr_draws {
                pass.set_bind_group(1, &g.bg_albedo, &[]);
                pass.set_bind_group(2, &g.bg_normal, &[]);
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

    /// Wave 8 — render the scene to an `OffscreenTarget` (a Texture3D
    /// with `RENDER_ATTACHMENT + TEXTURE_BINDING`). The same texture
    /// can then be passed to a `Node::new_textured` / `Node::new_pbr`
    /// to embed the 3D output inside another 3D scene, or be bound as
    /// a URX 2D `SceneCmd::image` source.
    ///
    /// The renderer's color_format must match the target's
    /// (`Rgba8UnormSrgb`). If you constructed the renderer for a
    /// different swapchain format, re-create it with `Rgba8UnormSrgb`
    /// for offscreen rendering or split into two renderers.
    pub fn render_to_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &Texture3D,
        camera: &PerspectiveCamera,
        scene: &Scene3D,
    ) {
        self.resize(device, (target.width, target.height));
        self.render(device, queue, encoder, &target.view, camera, scene);
    }
}

/// Build a model matrix from translation/rotation/scale.
#[inline]
pub fn trs(t: glam::Vec3, r: glam::Quat, s: glam::Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(s, r, t)
}
