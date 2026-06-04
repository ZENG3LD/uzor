//! Wave 9b extension — GPU per-region compositor.
//!
//! Owns one intermediate `wgpu::Texture` per region (sized to that
//! region's bounds) so consumers can render each region into its own
//! sub-texture with whatever backend they like, then call
//! `composite_to()` to blit all regions into a final framebuffer in
//! the engine's RegionId iteration order.
//!
//! Workflow:
//!   1. `GpuCompositor::ensure(device, region, w, h)` to (re)allocate
//!      the intermediate when a region's bounds change.
//!   2. Consumer renders into `compositor.region_view(region)` via
//!      whatever pipeline matches the region's BackendHint (e.g.
//!      urx-3d Renderer3D for FullGpu hints, or a CPU rasteriser
//!      whose pixels are uploaded via queue.write_texture).
//!   3. After all regions are rendered:
//!      `compositor.composite_to(encoder, &mixer, target_view, target_size)`
//!      walks `mixer.records()` in order and blits each region into
//!      `target_view` at its bounds.
//!
//! The compositor is INDEPENDENT of which backend painted each region
//! — it just blits textures. Consumers wire backend → texture themselves
//! inside the `RegionMixer` callbacks.

use std::collections::HashMap;
use uzor_urx_core::math::Rect;
use uzor_urx_core::region::RegionId;

use crate::RegionMixer;

pub const COMPOSITOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

struct RegionSurface {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

pub struct GpuCompositor {
    surfaces: HashMap<RegionId, RegionSurface>,
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
    target_format: wgpu::TextureFormat,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BlitRect {
    rect: [f32; 4],
}

impl GpuCompositor {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("urx-region-mixer.blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx-region-mixer.blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("urx-region-mixer.blit_layout"),
            bind_group_layouts: &[&bgl],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("urx-region-mixer.blit_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("urx-region-mixer.blit_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-region-mixer.blit_params"),
            size: std::mem::size_of::<BlitRect>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            surfaces: HashMap::new(),
            sampler,
            bgl,
            pipeline,
            params_buf,
            target_format,
        }
    }

    pub fn target_format(&self) -> wgpu::TextureFormat { self.target_format }

    /// Allocate (or re-allocate) the intermediate texture for a region.
    /// Call this any time the region's bounds change. Texture is
    /// `Rgba8UnormSrgb` with `RENDER_ATTACHMENT + TEXTURE_BINDING +
    /// COPY_DST` so backends can either render or upload pixels.
    pub fn ensure(&mut self, device: &wgpu::Device, region: RegionId, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if let Some(s) = self.surfaces.get(&region) {
            if s.width == width && s.height == height { return; }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx-region-mixer.region_tex"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COMPOSITOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.surfaces.insert(region, RegionSurface { texture, view, width, height });
    }

    pub fn region_view(&self, region: RegionId) -> Option<&wgpu::TextureView> {
        self.surfaces.get(&region).map(|s| &s.view)
    }
    pub fn region_texture(&self, region: RegionId) -> Option<&wgpu::Texture> {
        self.surfaces.get(&region).map(|s| &s.texture)
    }
    pub fn region_size(&self, region: RegionId) -> Option<(u32, u32)> {
        self.surfaces.get(&region).map(|s| (s.width, s.height))
    }
    pub fn drop_region(&mut self, region: RegionId) {
        self.surfaces.remove(&region);
    }

    /// Composite all rendered regions into `target_view` at their
    /// recorded bounds. Walks `mixer.records()` in dispatch order so
    /// the painter algorithm is engine-stable.
    ///
    /// `target_size` = pixel dimensions of `target_view`; used to map
    /// each region's pixel-space bounds → NDC.
    pub fn composite_to(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        mixer: &RegionMixer,
        target_view: &wgpu::TextureView,
        target_size: (u32, u32),
    ) {
        // Clear the target first.
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("urx-region-mixer.composite_clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
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
        }

        // One blit per region, in dispatch order. We allocate a FRESH
        // params buffer + bind group per region so all blits in this
        // frame see their own correct NDC rect (a shared uniform
        // buffer with per-iteration writes would race because all
        // bind groups would read the LAST written rect).
        for rec in mixer.records() {
            let surface = match self.surfaces.get(&rec.region) {
                Some(s) => s,
                None => continue,
            };
            let rect = rect_to_ndc(rec.bounds, target_size);
            let params = BlitRect { rect };
            let p_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("urx-region-mixer.blit_params_per_region"),
                size: std::mem::size_of::<BlitRect>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&p_buf, 0, bytemuck::bytes_of(&params));
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("urx-region-mixer.region_blit_bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: p_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&surface.view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("urx-region-mixer.composite_blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.draw(0..4, 0..1);
            drop(pass);
        }
        let _ = device;
    }
}

fn rect_to_ndc(r: Rect, target: (u32, u32)) -> [f32; 4] {
    let tw = target.0.max(1) as f64;
    let th = target.1.max(1) as f64;
    let x0 = ((r.x0 / tw) * 2.0 - 1.0) as f32;
    let x1 = ((r.x1 / tw) * 2.0 - 1.0) as f32;
    // wgpu NDC y points UP; our pixel coords have y pointing DOWN.
    // Flip y so a region whose y0=0 (top) lands at NDC y=+1.
    let y0 = (1.0 - (r.y0 / th) * 2.0) as f32;
    let y1 = (1.0 - (r.y1 / th) * 2.0) as f32;
    // Ensure min < max ordering as the vertex shader expects
    // (rect.x..z = min..max, rect.y..w = min..max — but with the y
    // flip the original y0 corresponds to MAX, not MIN). Sort here.
    let xmin = x0.min(x1);
    let xmax = x0.max(x1);
    let ymin = y0.min(y1);
    let ymax = y0.max(y1);
    [xmin, ymin, xmax, ymax]
}
