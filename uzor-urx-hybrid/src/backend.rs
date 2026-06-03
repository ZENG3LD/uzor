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

use crate::atlas::{AtlasUpsertResult, RegionAtlas};
use crate::composite::{QuadInstance, ScreenUniform, COMPOSITE_SHADER};
use crate::region_tex::{RegionTexture, ResizeNeeded};

pub struct HybridBackend {
    region_textures: BTreeMap<RegionId, RegionTexture>,
    /// Lazy-init compositor pipeline. Created on first composite().
    pipeline:        Option<CompositorPipeline>,
    /// Dirty-skip + atlas tuning policy.
    config:          UrxConfig,
    /// Count of `upsert_*` calls (any upload OR skipped-by-dirty)
    /// since the previous `composite()`. Reset on every composite().
    /// When 0 after a composite, the frame was transform-only — see
    /// `KEY_HYBRID_COMPOSITE_TRANSFORM_ONLY_FRAMES`.
    uploads_since_last_composite: u32,
    /// Optional pipeline cache (Vulkan only — no-op elsewhere). When
    /// set BEFORE the first `composite()` call, the compositor
    /// pipeline is built using this cache, saving ~50-500 ms cold
    /// start on first launch after a driver update / fresh install.
    /// Use [`uzor_urx_core::pipeline_cache::load_or_create`] +
    /// [`Self::set_pipeline_cache`] before first composite.
    pipeline_cache:  Option<wgpu::PipelineCache>,
    /// Atlas for small-region packing (Hybrid-P1). Lazily created on
    /// first eligible upsert when `config.hybrid_atlas_enabled` is
    /// `true`. Regions ≤ `(atlas_w/2, atlas_h/2)` go into the atlas;
    /// larger regions stay in `region_textures` as standalone
    /// `RegionTexture`. Both paths coexist — `composite()` checks
    /// each id and picks the right bind group.
    atlas:           Option<RegionAtlas>,
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
            uploads_since_last_composite: 0,
            pipeline_cache: None,
            atlas: None,
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
            uploads_since_last_composite: 0,
            pipeline_cache: None,
            atlas: None,
        }
    }

    pub fn config(&self) -> &UrxConfig { &self.config }

    /// Install a `wgpu::PipelineCache` to be used when the compositor
    /// pipeline is built (on first `composite()` call). Must be set
    /// BEFORE the first composite — afterwards has no effect, since
    /// the pipeline is already created.
    ///
    /// Construct the cache via
    /// [`uzor_urx_core::pipeline_cache::load_or_create`] which reads
    /// the on-disk blob (or creates an empty fallback).
    pub fn set_pipeline_cache(&mut self, cache: Option<wgpu::PipelineCache>) {
        self.pipeline_cache = cache;
    }

    /// Returns a reference to the installed pipeline cache, if any.
    /// Useful for persisting back to disk on shutdown via
    /// [`uzor_urx_core::pipeline_cache::save_to_disk`].
    pub fn pipeline_cache(&self) -> Option<&wgpu::PipelineCache> {
        self.pipeline_cache.as_ref()
    }

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

        // Atlas path (Hybrid-P1) — only if explicitly enabled and the
        // region's dimensions fit. Existing standalone region for the
        // same id (rare — happens when consumer enables atlas mid-run)
        // is dropped so the atlas takes over.
        if self.config.hybrid_atlas_enabled {
            // Lazy-init atlas on first attempt.
            if self.atlas.is_none() {
                self.atlas = Some(RegionAtlas::new(
                    device,
                    self.config.hybrid_atlas_w,
                    self.config.hybrid_atlas_h,
                ));
            }
            if let Some(atlas) = self.atlas.as_mut() {
                if atlas.fits(pixmap.width(), pixmap.height()) {
                    // Drop any standalone version of this id first.
                    self.region_textures.remove(&region_id);
                    match atlas.try_upsert(queue, region_id, pixmap) {
                        AtlasUpsertResult::Uploaded { bytes } => {
                            metrics::counter!(KEY_HYBRID_UPLOAD_BYTES).increment(bytes);
                            self.uploads_since_last_composite =
                                self.uploads_since_last_composite.saturating_add(1);
                            return;
                        }
                        AtlasUpsertResult::Skipped { bytes } => {
                            metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                            metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES)
                                .increment(bytes);
                            return;
                        }
                        AtlasUpsertResult::Reject => {
                            // Atlas full or region too large — fall through
                            // to standalone path.
                        }
                    }
                }
                // Region doesn't fit atlas — fall through.
            }
        }

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
                    self.uploads_since_last_composite =
                        self.uploads_since_last_composite.saturating_add(1);
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
        self.uploads_since_last_composite =
            self.uploads_since_last_composite.saturating_add(1);
    }

    /// Returns the generation counter recorded for this region's
    /// uploaded texture (i.e. the gen of the last successful upload),
    /// or `None` if the region isn't cached. Used for consumer-side
    /// "do I need to rebuild the pixmap?" checks.
    ///
    /// Checks both the atlas (if enabled) and the standalone-texture
    /// map; whichever path holds the region wins.
    pub fn last_uploaded_generation(&self, id: RegionId) -> Option<u64> {
        if let Some(atlas) = self.atlas.as_ref() {
            if let Some(slot) = atlas.slot(id) {
                return slot.generation;
            }
        }
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
        if let Some(atlas) = self.atlas.as_mut() {
            if atlas.set_slot_generation(id, Some(generation)) {
                return true;
            }
        }
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
        // Atlas slot check first (when enabled).
        if let Some(atlas) = self.atlas.as_ref() {
            if let Some(slot) = atlas.slot(region_id) {
                if slot.generation == Some(generation) {
                    let bytes = (slot.px_rect[2] as u64) * (slot.px_rect[3] as u64) * 4;
                    metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                    metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES).increment(bytes);
                    return false;
                }
            }
        }
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
        // Atlas slot generation match → cheapest skip.
        if let Some(atlas) = self.atlas.as_ref() {
            if let Some(slot) = atlas.slot(region_id) {
                if slot.generation == Some(generation) {
                    let bytes = (slot.px_rect[2] as u64) * (slot.px_rect[3] as u64) * 4;
                    metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                    metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES).increment(bytes);
                    return;
                }
            }
        }
        if let Some(existing) = self.region_textures.get_mut(&region_id) {
            if existing.generation == Some(generation) {
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED).increment(1);
                metrics::counter!(KEY_HYBRID_UPLOAD_SKIPPED_BYTES)
                    .increment(existing.bytes);
                return;
            }
        }
        self.upsert_region_pixmap(device, queue, region_id, pixmap);
        // Tag the region (whichever path took it).
        if let Some(atlas) = self.atlas.as_mut() {
            if atlas.set_slot_generation(region_id, Some(generation)) {
                return;
            }
        }
        if let Some(tex) = self.region_textures.get_mut(&region_id) {
            tex.set_generation(Some(generation));
        }
    }

    pub fn remove_region(&mut self, region_id: RegionId) {
        self.region_textures.remove(&region_id);
        if let Some(atlas) = self.atlas.as_mut() {
            atlas.remove(region_id);
        }
    }

    pub fn clear(&mut self) {
        self.region_textures.clear();
        if let Some(atlas) = self.atlas.as_mut() {
            atlas.clear();
        }
    }

    /// Returns the atlas slot for `id` if the region is currently
    /// atlas-packed (UV rect + content_hash + generation). Returns
    /// `None` if the region is standalone (or absent).
    pub fn atlas_slot(&self, id: RegionId) -> Option<&crate::atlas::AtlasSlot> {
        self.atlas.as_ref().and_then(|a| a.slot(id))
    }

    /// Returns the atlas backing texture view + atlas dimensions when
    /// the atlas exists, otherwise None. Used by callers wanting to
    /// composite directly via the atlas bind group (Hybrid-P2).
    pub fn atlas_view(&self) -> Option<(&wgpu::TextureView, u32, u32)> {
        self.atlas.as_ref().map(|a| (&a.view, a.width, a.height))
    }

    /// True if the region is cached in the atlas (vs standalone).
    pub fn is_atlas_packed(&self, id: RegionId) -> bool {
        self.atlas.as_ref().is_some_and(|a| a.slot(id).is_some())
    }

    /// Auto-resize the atlas if eviction pressure crossed threshold this
    /// frame. Call this from the consumer's frame loop AFTER `composite()`
    /// — it reads the eviction counter set by composite()'s reset+evict
    /// path, drops all current atlas slots (next-frame upserts re-populate),
    /// and re-creates the atlas texture at 2× dims (clamped to MAX_ATLAS_DIM).
    ///
    /// Returns `Some((new_w, new_h))` if a resize happened, `None` otherwise.
    ///
    /// The atlas is recreated empty — all existing slots are dropped. The
    /// consumer's next-frame upserts repopulate naturally; for one frame the
    /// fallback (standalone region textures) covers regions that haven't
    /// re-uploaded yet. Acceptable trade-off for the "atlas was too small
    /// for this workload" case.
    pub fn maybe_resize_atlas(&mut self, device: &wgpu::Device) -> Option<(u32, u32)> {
        let (new_w, new_h) = self.atlas.as_ref()?.should_resize()?;
        // Drop the old atlas and rebuild fresh at the new size. Slots
        // (and their GPU memory residency) are gone; consumer must
        // re-upload.
        self.atlas = Some(crate::atlas::RegionAtlas::new(device, new_w, new_h));
        // Update config so future logic (fits() checks etc.) sees the
        // new dimensions consistently.
        self.config.hybrid_atlas_w = new_w;
        self.config.hybrid_atlas_h = new_h;
        Some((new_w, new_h))
    }

    fn ensure_pipeline(
        &mut self,
        device:        &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> &mut CompositorPipeline {
        if self.pipeline.is_none() {
            self.pipeline = Some(build_pipeline(
                device, surface_format, self.pipeline_cache.as_ref(),
            ));
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

        // Reset per-frame counters on the atlas so should_resize() only
        // sees this frame's eviction pressure.
        if let Some(atlas) = self.atlas.as_mut() {
            atlas.reset_frame_counters();
        }

        // Pre-resolve per-instance texture view + UV remap. Atlas-resident
        // regions get the atlas view + remapped UV from their slot;
        // standalone regions get their own texture view + the caller's UV.
        //
        // We also tag each instance with whether it's atlas-resident — used
        // below (with `hybrid_instanced_composite`) to coalesce contiguous
        // atlas runs into a single `pass.draw(start..end)` call instead of
        // N per-instance draws. Standalone instances always get their own
        // bind group + draw (no atlas, no shared texture).
        //
        // Painter's order is preserved: runs are contiguous in the input
        // sequence, so two adjacent atlas instances batch into one draw,
        // an interrupting standalone breaks the batch but keeps z-order.
        let mut texture_views: Vec<wgpu::TextureView> = Vec::with_capacity(instances.len());
        let mut inst_data: Vec<QuadInstance> = Vec::with_capacity(instances.len());
        let mut atlas_mask: Vec<bool> = Vec::with_capacity(instances.len());
        let mut atlas_touched: Vec<RegionId> = Vec::new();
        for (id, qi) in instances {
            // Atlas path?
            if let Some(atlas) = self.atlas.as_ref() {
                if let Some(slot) = atlas.slot(*id) {
                    texture_views.push(atlas.view.clone());
                    // Compose caller's UV (sub-rect within the region)
                    // with the atlas slot's UV rect. For default
                    // [0,0,1,1] UVs the result is just slot.uv_rect.
                    let su = slot.uv_rect;
                    let cu = qi.uv;
                    let span_u = su[2] - su[0];
                    let span_v = su[3] - su[1];
                    let composed_uv = [
                        su[0] + cu[0] * span_u,
                        su[1] + cu[1] * span_v,
                        su[0] + cu[2] * span_u,
                        su[1] + cu[3] * span_v,
                    ];
                    inst_data.push(QuadInstance {
                        dst:  qi.dst,
                        uv:   composed_uv,
                        tint: qi.tint,
                    });
                    atlas_mask.push(true);
                    atlas_touched.push(*id);
                    continue;
                }
            }
            // Standalone path.
            let tex = self.region_textures.get(id)
                .expect("region texture must be uploaded before composite");
            texture_views.push(tex.view.clone());
            inst_data.push(*qi);
            atlas_mask.push(false);
        }

        // LRU bookkeeping: mark all atlas-resident regions composited
        // this frame as MRU so static panels survive across many
        // dynamic uploads. Cheap O(N) on a small VecDeque.
        if let Some(atlas) = self.atlas.as_mut() {
            for id in atlas_touched.drain(..) {
                atlas.touch(id);
            }
        }

        let coalesce = self.config.hybrid_instanced_composite;
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

        // Write instance data (with atlas UV remapping baked in above).
        queue.write_buffer(&pipeline.instance_buf, 0, bytemuck::cast_slice(&inst_data));

        // Build the bind group list. With coalesce enabled, contiguous
        // atlas-resident instances share one bind group (the atlas view);
        // each standalone instance still gets its own. We compute the
        // run boundaries here and the draw loop below walks them.
        //
        // Without coalesce: every instance gets its own bind group, even
        // if many adjacent ones would share the atlas — preserves the
        // pre-B7 byte-exact behaviour.
        struct DrawRun {
            bind_group: wgpu::BindGroup,
            // [start, end) range into the instance buffer.
            range: std::ops::Range<u32>,
        }
        let mut runs: Vec<DrawRun> = Vec::with_capacity(instances.len());
        if coalesce {
            // Walk the mask, coalesce contiguous true-runs into one
            // DrawRun pointing at the atlas view; emit per-instance runs
            // for standalone (false) entries.
            let mut i = 0usize;
            while i < instances.len() {
                if atlas_mask[i] {
                    // Find end of atlas run.
                    let start = i;
                    while i < instances.len() && atlas_mask[i] { i += 1; }
                    let end = i;
                    // All atlas runs share the same view (texture_views[start]).
                    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("urx-hybrid-atlas-bg"),
                        layout: &pipeline.bind_group_layout_1,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture_views[start]) },
                        ],
                    });
                    runs.push(DrawRun { bind_group: bg, range: (start as u32)..(end as u32) });
                } else {
                    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("urx-hybrid-region-bg"),
                        layout: &pipeline.bind_group_layout_1,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture_views[i]) },
                        ],
                    });
                    runs.push(DrawRun { bind_group: bg, range: (i as u32)..((i + 1) as u32) });
                    i += 1;
                }
            }
        } else {
            // Per-region bind group (pre-B7 behaviour).
            for (i, view) in texture_views.iter().enumerate() {
                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("urx-hybrid-region-bg"),
                    layout: &pipeline.bind_group_layout_1,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    ],
                });
                runs.push(DrawRun { bind_group: bg, range: (i as u32)..((i + 1) as u32) });
            }
        }

        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx-hybrid-composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
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
        for run in &runs {
            rp.set_bind_group(1, &run.bind_group, &[]);
            // 6 verts per quad × N instances per draw. With B7 coalesce
            // on, N can be the full atlas-resident subset → ONE draw
            // for the entire static-panel dashboard.
            rp.draw(0..6, run.range.clone());
        }

        metrics::counter!(uzor_urx_core::metrics_keys::KEY_HYBRID_COMPOSITE_DRAWS)
            .increment(instances.len() as u64);
        metrics::counter!(uzor_urx_core::metrics_keys::KEY_HYBRID_COMPOSITE_CALLS)
            .increment(1);
        metrics::counter!(uzor_urx_core::metrics_keys::KEY_HYBRID_COMPOSITE_PASS_DRAWS)
            .increment(runs.len() as u64);
        // Transform-only frame proxy: composite happened, zero uploads
        // since last composite. Reset the counter for the next frame.
        if self.uploads_since_last_composite == 0 {
            metrics::counter!(
                uzor_urx_core::metrics_keys::KEY_HYBRID_COMPOSITE_TRANSFORM_ONLY_FRAMES
            ).increment(1);
        }
        self.uploads_since_last_composite = 0;
    }

    /// Returns how many region uploads (or skipped-by-dirty calls)
    /// happened since the previous `composite()`. Reset on every
    /// composite call. Used by tests to verify the transform-only
    /// path; also useful for live diagnostics.
    pub fn uploads_since_last_composite(&self) -> u32 {
        self.uploads_since_last_composite
    }
}

fn build_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    cache: Option<&wgpu::PipelineCache>,
) -> CompositorPipeline {
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
        cache,
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
