//! `HybridBackend` — region-texture cache + composite pipeline.
//!
//! Phase 7 shell. Owns:
//!   - `region_textures: HashMap<RegionId, RegionTexture>` — wgpu
//!     texture per cached region (CPU rasterised, uploaded once)
//!   - Lazy-initialised compositor pipeline (vertex + fragment + bind
//!     group layouts + sampler)
//!
//! Public API:
//!   - `upsert_region_pixmap(id, pixmap)` — CPU rasteriser hands a
//!     fresh pixmap; backend uploads (or reuploads, or recreates).
//!   - `remove_region(id)` — drop the cached texture.
//!   - `composite(...)` — emit a render pass with N textured quads
//!     to the swap chain target.

use std::collections::BTreeMap;

use uzor_urx_core::region::RegionId;
use uzor_urx_core::config::{DirtyStrategy, UrxConfig};
use uzor_urx_cpu::Pixmap;

use crate::composite::{QuadInstance, ScreenUniform, COMPOSITE_SHADER};
use crate::region_tex::{RegionTexture, ResizeNeeded};

pub struct HybridBackend {
    region_textures: BTreeMap<RegionId, RegionTexture>,
    /// Lazy-init compositor pipeline. Created on first composite().
    pipeline:        Option<CompositorPipeline>,
    /// Dirty-skip + atlas tuning policy.
    config:          UrxConfig,
}

struct CompositorPipeline {
    pipeline:       wgpu::RenderPipeline,
    bind_group_0:   wgpu::BindGroup,             // screen uniform
    bind_group_layout_1: wgpu::BindGroupLayout,  // per-texture
    screen_buffer:  wgpu::Buffer,
    sampler:        wgpu::Sampler,
    instance_cap:   u32,
    instance_buf:   wgpu::Buffer,
}

impl HybridBackend {
    pub fn new() -> Self {
        Self {
            region_textures: BTreeMap::new(),
            pipeline: None,
            config: UrxConfig::default(),
        }
    }

    /// Construct with consumer-tuned [`UrxConfig`]. The
    /// `hybrid_dirty_strategy` + `hybrid_atlas_*` fields are the ones
    /// hybrid actually reads.
    pub fn with_config(cfg: UrxConfig) -> Self {
        cfg.validate().expect("invalid UrxConfig");
        Self {
            region_textures: BTreeMap::new(),
            pipeline: None,
            config: cfg,
        }
    }

    pub fn config(&self) -> &UrxConfig { &self.config }

    pub fn region_count(&self) -> usize { self.region_textures.len() }
    pub fn region_bytes(&self) -> u64 {
        self.region_textures.values().map(|r| r.bytes).sum()
    }

    /// Upload (or reuse) a region texture. If the existing texture's
    /// dimensions match, reuses it (cheap write_texture); otherwise
    /// drops + creates new.
    ///
    /// Honours `UrxConfig::hybrid_dirty_strategy`:
    /// - `GenerationOnly`: never hashes; uploads every call (caller
    ///   should use `upsert_region_with_generation`)
    /// - `HashBytes`: always hashes; skips upload if hash matches
    /// - `Both` (default): hashes and skips if equal
    ///
    /// For the generation-tracked path, see
    /// [`Self::upsert_region_with_generation`].
    pub fn upsert_region_pixmap(
        &mut self,
        device:    &wgpu::Device,
        queue:     &wgpu::Queue,
        region_id: RegionId,
        pixmap:    &Pixmap,
    ) {
        use uzor_urx_core::metrics_keys::{
            KEY_HYBRID_UPLOAD_BYTES, KEY_HYBRID_UPLOAD_SKIPPED,
            KEY_HYBRID_UPLOAD_SKIPPED_BYTES,
        };
        if let Some(existing) = self.region_textures.get_mut(&region_id) {
            // Hash-based skip — strategy may opt-out.
            let do_hash = matches!(
                self.config.hybrid_dirty_strategy,
                DirtyStrategy::HashBytes | DirtyStrategy::Both,
            );
            if do_hash && !existing.is_dirty_by_hash(pixmap) {
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES)
                    .increment(existing.bytes);
                return;
            }
            match existing.replace_contents(queue, pixmap) {
                Ok(()) => {
                    metrics::counter!(KEY_HYBRID_UPLOAD_BYTES)
                        .increment(existing.bytes);
                    return;
                }
                Err(ResizeNeeded) => {
                    // Fall through — drop + recreate below.
                }
            }
        }
        let tex = RegionTexture::new(device, queue, region_id, pixmap);
        metrics::counter!(KEY_HYBRID_UPLOAD_BYTES).increment(tex.bytes);
        self.region_textures.insert(region_id, tex);
    }

    /// Returns the generation counter recorded for this region's
    /// uploaded texture (i.e. the gen of the last successful upload),
    /// or `None` if the region isn't cached. Used for consumer-side
    /// "do I need to rebuild the pixmap?" checks.
    pub fn last_uploaded_generation(&self, id: RegionId) -> Option<u64> {
        self.region_textures.get(&id).and_then(|t| t.generation)
    }

    /// Returns `true` if the cached region is up-to-date at the given
    /// generation — caller doesn't need to re-rasterise or upload.
    /// Equivalent to `last_uploaded_generation(id) == Some(gen)`.
    pub fn is_region_clean_at(&self, id: RegionId, gen: u64) -> bool {
        matches!(self.last_uploaded_generation(id), Some(g) if g == gen)
    }

    /// "Mark the region's existing upload as still current at this
    /// generation." Bumps no counter, allocates nothing — just
    /// updates the cached generation tag. Useful when the consumer
    /// knows a region didn't change but wants the backend's
    /// generation tag to advance for diagnostic alignment.
    ///
    /// Returns `true` if the region exists (and was tagged); `false`
    /// if it wasn't cached yet (caller must upload first).
    pub fn mark_clean_with_generation(
        &mut self,
        id:         RegionId,
        generation: u64,
    ) -> bool {
        if let Some(tex) = self.region_textures.get_mut(&id) {
            tex.set_generation(Some(generation));
            true
        } else {
            false
        }
    }

    /// Generation-first lazy upsert: if the cached texture's
    /// generation matches `gen`, NO work happens — no CPU raster,
    /// no hash, no upload. Otherwise the consumer's `raster_fn` is
    /// invoked to produce the fresh pixmap, then the regular
    /// `upsert_region_with_generation` path runs.
    ///
    /// This is the cheapest path the hybrid backend exposes:
    /// up-to-date region → ~3 ns (HashMap lookup + u64 compare). It
    /// also saves the consumer's CPU rasterisation cost on the
    /// clean path — a 512×512 region staying unchanged at 60 Hz
    /// stops costing 1-2 ms/frame.
    ///
    /// Returns `true` if the upload happened (region was stale);
    /// `false` if it was skipped (region was clean at this gen).
    pub fn upload_if_dirty<F>(
        &mut self,
        device:     &wgpu::Device,
        queue:      &wgpu::Queue,
        region_id:  RegionId,
        generation: u64,
        raster_fn:  F,
    ) -> bool
    where
        F: FnOnce() -> Pixmap,
    {
        use uzor_urx_core::metrics_keys::{
            KEY_HYBRID_UPLOAD_SKIPPED, KEY_HYBRID_UPLOAD_SKIPPED_BYTES,
        };
        if let Some(existing) = self.region_textures.get(&region_id) {
            if existing.generation == Some(generation) {
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES)
                    .increment(existing.bytes);
                return false;
            }
        }
        // Stale or missing — invoke raster_fn ONCE and upload.
        let pixmap = raster_fn();
        self.upsert_region_with_generation(device, queue, region_id, &pixmap, generation);
        true
    }

    /// Upsert with a caller-supplied generation counter. When the
    /// generation matches the cached one, skips the upload AND the
    /// byte hash (cheapest path). Mismatch falls through to the
    /// `upsert_region_pixmap` logic (hash check, then upload).
    ///
    /// Use this when the consumer already tracks region dirtiness in
    /// its own data model — saves the per-byte fnv pass.
    pub fn upsert_region_with_generation(
        &mut self,
        device:     &wgpu::Device,
        queue:      &wgpu::Queue,
        region_id:  RegionId,
        pixmap:     &Pixmap,
        generation: u64,
    ) {
        use uzor_urx_core::metrics_keys::{
            KEY_HYBRID_UPLOAD_SKIPPED, KEY_HYBRID_UPLOAD_SKIPPED_BYTES,
        };
        if let Some(existing) = self.region_textures.get_mut(&region_id) {
            if existing.generation == Some(generation) {
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES)
                    .increment(existing.bytes);
                return;
            }
        }
        self.upsert_region_pixmap(device, queue, region_id, pixmap);
        if let Some(tex) = self.region_textures.get_mut(&region_id) {
            tex.set_generation(Some(generation));
        }
    }

    pub fn remove_region(&mut self, region_id: RegionId) {
        self.region_textures.remove(&region_id);
    }

    pub fn clear(&mut self) {
        self.region_textures.clear();
    }

    fn ensure_pipeline(
        &mut self,
        device:        &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> &mut CompositorPipeline {
        if self.pipeline.is_none() {
            self.pipeline = Some(build_pipeline(device, surface_format));
        }
        self.pipeline.as_mut().unwrap()
    }

    /// Emit a composite pass — one textured quad per (region, instance)
    /// pair the caller passes in.
    ///
    /// Caller is responsible for ordering the instances in painter's
    /// order; backend just dispatches in array order.
    pub fn composite(
        &mut self,
        device:    &wgpu::Device,
        queue:     &wgpu::Queue,
        encoder:   &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        screen_w: u32,
        screen_h: u32,
        instances: &[(RegionId, QuadInstance)],
    ) {
        if instances.is_empty() { return; }

        // Pre-resolve texture views BEFORE borrowing pipeline mutably.
        let mut texture_views: Vec<wgpu::TextureView> = Vec::with_capacity(instances.len());
        for (id, _) in instances {
            let tex = self.region_textures.get(id).expect("region texture must be uploaded before composite");
            texture_views.push(tex.view.clone());
        }

        let pipeline = self.ensure_pipeline(device, surface_format);

        // Upload screen uniform.
        let su = ScreenUniform { w: screen_w as f32, h: screen_h as f32, _pad: [0.0; 2] };
        queue.write_buffer(&pipeline.screen_buffer, 0, bytemuck::bytes_of(&su));

        // Grow instance buffer if needed.
        if (instances.len() as u32) > pipeline.instance_cap {
            let new_cap = (instances.len() as u32).next_power_of_two().max(64);
            pipeline.instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("urx-hybrid-instance"),
                size: (new_cap as u64) * (std::mem::size_of::<QuadInstance>() as u64),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            pipeline.instance_cap = new_cap;
        }

        // Write instance data.
        let inst_data: Vec<QuadInstance> = instances.iter().map(|(_, q)| *q).collect();
        queue.write_buffer(&pipeline.instance_buf, 0, bytemuck::cast_slice(&inst_data));

        // Build per-region bind groups from the pre-resolved views.
        let mut per_region_bgs: Vec<wgpu::BindGroup> = Vec::with_capacity(instances.len());
        for view in &texture_views {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("urx-hybrid-region-bg"),
                layout: &pipeline.bind_group_layout_1,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                ],
            });
            per_region_bgs.push(bg);
        }

        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx-hybrid-composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Caller may have already cleared via a previous
                    // pass; we preserve. To force clear, call
                    // `composite_with_clear` instead.
                    load:  wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&pipeline.pipeline);
        rp.set_bind_group(0, &pipeline.bind_group_0, &[]);
        rp.set_vertex_buffer(0, pipeline.instance_buf.slice(..));
        for (i, bg) in per_region_bgs.iter().enumerate() {
            rp.set_bind_group(1, bg, &[]);
            // 6 verts per quad, 1 instance per draw (we batch the
            // VERTEX buffer but switch bind group per region, so 1
            // draw per region — the per-region bind group switch IS
            // the cost). Future: pack regions into an atlas + single
            // bindless draw.
            rp.draw(0 .. 6, (i as u32) .. (i as u32 + 1));
        }

        metrics::counter!(uzor_urx_core::metrics_keys::KEY_HYBRID_COMPOSITE_DRAWS)
            .increment(instances.len() as u64);
        metrics::counter!(uzor_urx_core::metrics_keys::KEY_HYBRID_COMPOSITE_CALLS)
            .increment(1);
    }
}

fn build_pipeline(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> CompositorPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("urx-hybrid-composite-shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
    });

    // Group 0: screen uniform
    let layout_0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("urx-hybrid-bgl-0-screen"),
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
    // Group 1: per-region sampler + texture
    let layout_1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("urx-hybrid-bgl-1-tex"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("urx-hybrid-pl"),
        bind_group_layouts: &[&layout_0, &layout_1],
        immediate_size: 0,
    });

    let screen_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx-hybrid-screen-ub"),
        size: std::mem::size_of::<ScreenUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("urx-hybrid-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("urx-hybrid-bg-0"),
        layout: &layout_0,
        entries: &[wgpu::BindGroupEntry {
            binding: 0, resource: screen_buffer.as_entire_binding(),
        }],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("urx-hybrid-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x4,  // dst
                    1 => Float32x4,  // uv
                    2 => Float32x4,  // tint
                ],
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                // Premultiplied source-over: out = src + dst * (1 - src.a)
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation:  wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation:  wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    let instance_cap = 64;
    let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx-hybrid-instance"),
        size: (instance_cap as u64) * (std::mem::size_of::<QuadInstance>() as u64),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    CompositorPipeline {
        pipeline,
        bind_group_0,
        bind_group_layout_1: layout_1,
        screen_buffer,
        sampler,
        instance_cap,
        instance_buf,
    }
}
