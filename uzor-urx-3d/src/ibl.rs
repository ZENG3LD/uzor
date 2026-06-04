//! Wave 6b — pre-filtered IBL bake.
//!
//! `IblBaked` is a triple of textures produced from an input env
//! cubemap by `bake_ibl()`:
//!   - irradiance: 32×32 RGBA16F cubemap, diffuse hemisphere convolution
//!   - prefiltered: 128×128 RGBA16F cubemap with N mip levels (N=5 by
//!     default), each mip = GGX-importance-sampled at increasing
//!     roughness
//!   - brdf_lut: 256×256 RG16F 2D texture indexed by (ndotv, roughness)
//!
//! PBR fragment shader replaces the cheap single-tap env sample with:
//!   - irradiance for diffuse IBL
//!   - prefiltered.sampleLevel(refl, roughness * N_levels) for specular
//!   - brdf_lut sample for the F0 / energy-conservation factor
//!
//! Bake is one-shot. Each face of each output texture is rendered via
//! a fullscreen triangle to the matching face layer.

use crate::texture::TextureCube;
use std::sync::Arc;

pub const IRRADIANCE_SIZE: u32 = 32;
pub const PREFILTER_SIZE: u32 = 128;
pub const PREFILTER_MIPS: u32 = 5;
pub const BRDF_LUT_SIZE: u32 = 256;

pub const IBL_HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const BRDF_LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// Pre-filtered IBL artifacts. Hold as `Arc` so multiple Renderer3D
/// instances or render targets can share one bake.
pub struct IblBaked {
    pub irradiance_view: wgpu::TextureView,
    pub irradiance_sampler: wgpu::Sampler,
    pub prefiltered_view: wgpu::TextureView,
    pub prefiltered_sampler: wgpu::Sampler,
    pub brdf_lut_view: wgpu::TextureView,
    pub brdf_lut_sampler: wgpu::Sampler,
    /// Number of mip levels in the prefiltered cubemap (passed to the
    /// PBR shader so it can scale roughness → mip).
    pub prefilter_mips: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BakeParams {
    args: [f32; 4],
}

pub fn bake_ibl(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    env: &TextureCube,
) -> Arc<IblBaked> {
    // ── Common: shader + bake BGL + samplers ──────────────────────
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("urx3d.ibl_bake"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ibl_bake.wgsl").into()),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("urx3d.ibl_bake_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx3d.ibl_bake_params"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let env_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("urx3d.ibl_bake_env_bg"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&env.view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&env.sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: params_buf.as_entire_binding() },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("urx3d.ibl_bake_pipeline_layout"),
        bind_group_layouts: &[&bgl],
        immediate_size: 0,
    });
    let make_pipeline = |label: &str, fs_entry: &str, format: wgpu::TextureFormat| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fs_entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    let pipeline_irr   = make_pipeline("urx3d.ibl_pipeline_irradiance", "fs_irradiance", IBL_HDR_FORMAT);
    let pipeline_pref  = make_pipeline("urx3d.ibl_pipeline_prefilter",  "fs_prefilter",  IBL_HDR_FORMAT);
    let pipeline_brdf  = make_pipeline("urx3d.ibl_pipeline_brdf_lut",   "fs_brdf_lut",   BRDF_LUT_FORMAT);

    // ── Irradiance cubemap (32×32 × 6 faces) ──────────────────────
    let irr_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d.ibl_irradiance"),
        size: wgpu::Extent3d { width: IRRADIANCE_SIZE, height: IRRADIANCE_SIZE, depth_or_array_layers: 6 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: IBL_HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // ── Prefiltered cubemap (128×128 × 6 faces × N mip levels) ────
    let pref_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d.ibl_prefiltered"),
        size: wgpu::Extent3d { width: PREFILTER_SIZE, height: PREFILTER_SIZE, depth_or_array_layers: 6 },
        mip_level_count: PREFILTER_MIPS, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: IBL_HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // ── BRDF LUT (256×256 2D) ─────────────────────────────────────
    let brdf_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d.ibl_brdf_lut"),
        size: wgpu::Extent3d { width: BRDF_LUT_SIZE, height: BRDF_LUT_SIZE, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BRDF_LUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    // Bake irradiance — one pass per face.
    for face in 0..6u32 {
        let p = BakeParams { args: [face as f32, 0.0, 0.0, 0.0] };
        queue.write_buffer(&params_buf, 0, bytemuck::bytes_of(&p));
        let face_view = irr_tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some("urx3d.ibl_irr_face"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: face,
            array_layer_count: Some(1),
            ..Default::default()
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx3d.ibl_bake_irradiance"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &face_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline_irr);
        pass.set_bind_group(0, &env_bg, &[]);
        pass.draw(0..3, 0..1);
        // BakeParams written before pass — but we need to flush between
        // faces. Encoder defers; for cubemap face baking this is OK
        // because we submit at the very end after rewriting params
        // each iteration. The driver replays writes in order.
        // To be safe, submit per-face for the params buffer to flush.
        drop(pass);
        queue.submit(Some(std::mem::replace(
            &mut encoder,
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
        ).finish()));
    }

    // Bake prefilter — per-face per-mip pass.
    for mip in 0..PREFILTER_MIPS {
        let roughness = if PREFILTER_MIPS > 1 {
            mip as f32 / (PREFILTER_MIPS - 1) as f32
        } else { 0.0 };
        let sample_count = 32.0; // moderate quality, fast bake
        for face in 0..6u32 {
            let p = BakeParams { args: [face as f32, roughness, sample_count, 0.0] };
            queue.write_buffer(&params_buf, 0, bytemuck::bytes_of(&p));
            let face_view = pref_tex.create_view(&wgpu::TextureViewDescriptor {
                label: Some("urx3d.ibl_pref_face_mip"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: face,
                array_layer_count: Some(1),
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("urx3d.ibl_bake_prefilter"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &face_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline_pref);
            pass.set_bind_group(0, &env_bg, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            queue.submit(Some(std::mem::replace(
                &mut encoder,
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
            ).finish()));
        }
    }

    // Bake BRDF LUT — single pass.
    {
        let p = BakeParams { args: [0.0; 4] };
        queue.write_buffer(&params_buf, 0, bytemuck::bytes_of(&p));
        let brdf_view = brdf_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx3d.ibl_bake_brdf"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &brdf_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline_brdf);
        pass.set_bind_group(0, &env_bg, &[]); // env unused but layout requires it
        pass.draw(0..3, 0..1);
        drop(pass);
    }
    queue.submit(Some(encoder.finish()));

    let irr_cube_view = irr_tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("urx3d.ibl_irr_cube_view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let pref_cube_view = pref_tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("urx3d.ibl_pref_cube_view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let brdf_view = brdf_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let cube_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("urx3d.ibl_cube_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    let lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("urx3d.ibl_lut_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    Arc::new(IblBaked {
        irradiance_view: irr_cube_view,
        irradiance_sampler: cube_sampler.clone(),
        prefiltered_view: pref_cube_view,
        prefiltered_sampler: cube_sampler,
        brdf_lut_view: brdf_view,
        brdf_lut_sampler: lut_sampler,
        prefilter_mips: PREFILTER_MIPS,
    })
}
