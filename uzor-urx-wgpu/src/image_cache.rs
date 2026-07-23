//! `NativeImageCache` — per-`ImageId` GPU texture cache, no atlas
//! packing (each resident image is its OWN full `wgpu::Texture` + bind
//! group, evicted whole).
//!
//! Built standalone in URX Wave 4 Commit 2
//! (`docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §4.2) — exercised only by this file's own unit tests at this point,
//! same "isolated, independently reviewable new file" shape as Wave 2
//! Commit 1's `atlas::NativeGlyphAtlas` / this wave's own
//! `gradient_lut::GradientLutAtlas` (see either module's doc comment
//! for the precedent this mirrors). Wired into `NativeUrxRenderer`/
//! `encode.rs` in Commit 3.
//!
//! ## Why no atlas packing (design §4.2)
//!
//! Unlike glyph bitmaps (small, numerous, packed tightly into shared
//! atlas pages) or gradient LUTs (fixed 256x1 rows), registered images
//! can be arbitrarily large — packing them into a shared atlas would
//! need real 2D bin-packing with page growth/fragmentation handling
//! for a shape (large individual textures) that doesn't benefit from
//! it the way small glyphs do. Each resident image gets its own
//! `wgpu::Texture`, sized exactly to its decoded dimensions.
//!
//! ## Upload-timing decision: IMMEDIATE, not a deferred pending-queue
//! (disclosed, per this commit's own explicit ask to "decide and
//! document")
//!
//! `atlas::NativeGlyphAtlas`/`gradient_lut::GradientLutAtlas` both
//! defer their actual `queue.write_texture` calls to a separate
//! `flush_uploads(&queue)` step, called once per frame AFTER the whole
//! scene has been walked. That deferral exists for a REAL reason
//! specific to THEM: both are SHARED, REUSED physical storage (one
//! atlas page, one LUT row) that can be evicted-and-reused for
//! DIFFERENT logical content across frames while `encode_scene` itself
//! has no `wgpu::Device`/`Queue` handle at all — bytes are queued
//! during the scene walk, then actually written once `renderer.rs`
//! regains device/queue access after `encode_scene` returns.
//!
//! Per design §4.2's own note ("an image's bind group is baked into
//! `ImageInstance` at encode time too"), this cache's resolution
//! (`get_or_upload`) is likewise expected to run where `device`/`queue`
//! ARE already in scope (mirroring how `NativeGlyphAtlas::bind_group()`/
//! `GradientLutAtlas::bind_group()` are read at BATCH-REPLAY time, not
//! baked as raw bytes ahead of time) — `get_or_upload` takes
//! `device`/`queue` directly and, on a genuine cache miss, calls
//! `queue.write_texture` SYNCHRONOUSLY, right there, rather than
//! queuing bytes for a later drain. Two reasons this is correct here
//! specifically, where it would NOT be for the atlas/LUT case:
//! 1. Every `ImageId` slot is written AT MOST ONCE in its entire
//!    resident lifetime (images are immutable once registered —
//!    `uzor_urx_image::ImageRegistry::register` always mints a NEW id,
//!    never mutates bytes under an existing one) — there is no
//!    "insert now, might get re-inserted differently before the frame
//!    ends" churn to batch away, unlike a shared atlas row that can be
//!    evicted-and-reused for a DIFFERENT gradient/glyph within the
//!    SAME frame.
//! 2. `wgpu::Queue::write_texture` is itself just a recorded command
//!    against the queue's internal upload path — it does not require
//!    an open `CommandEncoder`/render pass, so calling it immediately
//!    at whatever point `get_or_upload` is invoked is exactly as safe
//!    as calling it later from a dedicated flush step; there is no
//!    ordering hazard a pending-queue would need to protect against.
//!
//! Net effect: no `pending: Vec<_>` field, no `flush_uploads` method on
//! this type at all — `get_or_upload` alone is the complete upload
//! path.
//!
//! ## Never-evict-this-frame invariant (same shape as the other two
//! caches, kept for consistency even though — see the upload-timing
//! note above — an image slot is never MUTATED once uploaded)
//!
//! `begin_frame`/the `tick` field are still present and still enforced
//! identically to `atlas::NativeGlyphAtlas`/`gradient_lut::GradientLutAtlas`:
//! eviction only ever picks a slot with `tick < self.tick` (never one
//! touched this frame), returning `None` when no such candidate exists
//! rather than silently evicting something an already-processed batch
//! in the SAME frame is still relying on.

use std::collections::HashMap;

use uzor_urx_core::scene::ImageId;

/// One resident image's GPU placement. Deliberately does NOT store the
/// raw `wgpu::Texture` handle (a deviation from design §4.2's literal
/// struct sketch, disclosed) — the `bind_group`'s own `TextureView`
/// keeps the underlying GPU resource alive internally (wgpu's
/// Arc-backed resource tracking), the same "build the view, drop the
/// local `Texture` variable, never store it" idiom already established
/// in this crate (`msaa.rs`/`stencil.rs`/Wave 3's `BlendLayerTarget`).
/// Storing an unused `texture` field here would be dead code: unlike
/// the atlas/LUT case, this cache never calls `write_texture` against
/// an EXISTING slot's texture a second time (see the upload-timing
/// doc comment above) — there is nothing left to do with the raw
/// handle after the bind group is built.
pub(crate) struct GpuImageSlot {
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    tick: u64,
}

/// Read-only cache telemetry — hit/miss/eviction/entry counts, same
/// shape family as `atlas::AtlasStats`/`gradient_lut::GradientLutAtlasStats`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NativeImageCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// Owns every resident image's GPU texture + bind group, the shared
/// group-1 bind group layout (texture + Linear sampler, matching CPU's
/// bilinear default) all of them build against, and the
/// `ImageId`-keyed slot table.
pub(crate) struct NativeImageCache {
    slots: HashMap<ImageId, GpuImageSlot>,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    tick: u64,
    cap: usize,
    stats: NativeImageCacheStats,
}

impl NativeImageCache {
    /// `cap` is `UrxConfig::wgpu_image_cache_cap` — count-bound (not
    /// area-bound), unvalidated at the config layer (a `cap` of `0` is
    /// a legitimate, if extreme, "never actually cache anything"
    /// configuration for this data structure, unlike an atlas texture
    /// dimension which cannot be zero at all — see this module's own
    /// `get_or_upload` doc comment) — handled gracefully here rather
    /// than clamped.
    pub(crate) fn new(device: &wgpu::Device, cap: usize) -> Self {
        // Linear mag/min, matching CPU's default bilinear sampling path
        // (`image_draw.rs::sample_bilinear`, design §4.2) — the 1:1
        // fast path is NOT special-cased on GPU (design §4.3): bilinear
        // sampling at an exact integer-aligned 1:1 mapping mathematically
        // degenerates to the same value a direct texel copy would
        // produce, so one sampler/shader path serves both of CPU's two
        // branches.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uzor_urx_wgpu.native_image_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uzor_urx_wgpu.native_image_bgl"),
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

        Self {
            slots: HashMap::new(),
            bind_group_layout,
            sampler,
            tick: 0,
            cap,
            stats: NativeImageCacheStats::default(),
        }
    }

    /// Bind group layout for `ImagePipeline::new`'s group-1 pipeline
    /// layout entry (Commit 3).
    pub(crate) fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Advance the per-frame tick. Call exactly once per frame, before
    /// walking the scene — mirrors `atlas::NativeGlyphAtlas::begin_frame`/
    /// `gradient_lut::GradientLutAtlas::begin_frame` exactly.
    pub(crate) fn begin_frame(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Look up `id`'s GPU slot, uploading a fresh texture on a genuine
    /// cache miss (resolved via `uzor_urx_image::lookup_image`).
    /// Returns `None` in exactly two cases, both honest, both the
    /// caller's responsibility to count (this cache never emits its
    /// own metrics — same convention as the atlas/LUT caches, which
    /// also just return `Option` and let the caller decide what a
    /// `None` means):
    /// - `id` was never registered (or has since been unregistered) in
    ///   `uzor_urx_image`'s global registry — mirrors CPU's own
    ///   `image_id_unknown` miss (design §4.4).
    /// - the cache is full AND every existing slot has already been
    ///   touched this frame (no eviction candidate) — the
    ///   never-evict-this-frame invariant.
    pub(crate) fn get_or_upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: ImageId) -> Option<&GpuImageSlot> {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.tick = self.tick;
            self.stats.hits += 1;
            // Reborrow immutably for the return — a function can't
            // hand back the `&mut` just used to stamp the tick as a
            // `&`, so this is one more (cheap, cold-path) lookup.
            return self.slots.get(&id);
        }

        let data = uzor_urx_image::lookup_image(id)?;
        self.stats.misses += 1;

        if self.slots.len() >= self.cap {
            let victim = self
                .slots
                .iter()
                .filter(|(_, slot)| slot.tick < self.tick)
                .min_by_key(|(_, slot)| slot.tick)
                .map(|(k, _)| *k);
            let victim_id = victim?;
            self.slots.remove(&victim_id);
            self.stats.evictions += 1;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_urx_wgpu.native_image"),
            size: wgpu::Extent3d { width: data.width, height: data.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Immediate upload — see this module's own doc comment for why
        // that's correct here (unlike the atlas/LUT caches' deferred
        // `flush_uploads`).
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data.bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(data.width * 4),
                rows_per_image: Some(data.height),
            },
            wgpu::Extent3d { width: data.width, height: data.height, depth_or_array_layers: 1 },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uzor_urx_wgpu.native_image_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });

        self.slots.insert(id, GpuImageSlot { bind_group, width: data.width, height: data.height, tick: self.tick });
        self.stats.entries = self.slots.len();

        self.slots.get(&id)
    }

    /// This slot's bind group — group-1 resource `BatchKind::Image`'s
    /// batch-replay arm binds at draw time (Commit 3).
    pub(crate) fn bind_group_of(slot: &GpuImageSlot) -> &wgpu::BindGroup {
        &slot.bind_group
    }

    /// Decoded pixel dimensions this slot's texture was built at — the
    /// SAME `(width, height)` `uzor_urx_image::ImageData` reported at
    /// upload time, kept on the slot so a caller resolving through the
    /// cache doesn't need a SEPARATE `lookup_image` call just to
    /// recover them.
    pub(crate) fn dims_of(slot: &GpuImageSlot) -> (u32, u32) {
        (slot.width, slot.height)
    }

    pub(crate) fn stats(&self) -> NativeImageCacheStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless wgpu device — mirrors `atlas.rs`/`gradient_lut.rs`'s
    /// own `test_device` helper. Every test using this is
    /// `#[ignore]`-gated, matching this crate's parity-test policy.
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-urx-wgpu-image-cache-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    fn register_sample(w: u32, h: u32, fill: [u8; 4]) -> ImageId {
        let mut bytes = vec![0u8; (w * h * 4) as usize];
        for px in bytes.chunks_exact_mut(4) {
            px.copy_from_slice(&fill);
        }
        let data = uzor_urx_image::ImageData::from_raw_premul(w, h, bytes).expect("size matches by construction");
        uzor_urx_image::register_image(data)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn repeat_lookup_is_a_cache_hit() {
        let Some((device, queue)) = test_device() else { return };
        let id = register_sample(4, 4, [10, 20, 30, 255]);
        let mut cache = NativeImageCache::new(&device, 4);
        cache.begin_frame();

        let first = &cache.get_or_upload(&device, &queue, id).expect("first lookup must succeed").bind_group as *const _;
        let second = &cache.get_or_upload(&device, &queue, id).expect("repeat lookup must hit, not fail").bind_group as *const _;
        assert!(std::ptr::eq(first, second), "repeat lookup returns the SAME bind group, not a re-upload");

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
        uzor_urx_image::unregister_image(id);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn dims_of_reports_the_registered_size() {
        let Some((device, queue)) = test_device() else { return };
        let id = register_sample(8, 5, [1, 2, 3, 255]);
        let mut cache = NativeImageCache::new(&device, 4);
        cache.begin_frame();
        let slot = cache.get_or_upload(&device, &queue, id).expect("lookup must succeed");
        assert_eq!(NativeImageCache::dims_of(slot), (8, 5));
        uzor_urx_image::unregister_image(id);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn unregistered_id_is_a_clean_miss_not_a_panic() {
        let Some((device, queue)) = test_device() else { return };
        let mut cache = NativeImageCache::new(&device, 4);
        cache.begin_frame();
        // A very unlikely-to-collide id — never registered by this test.
        assert!(cache.get_or_upload(&device, &queue, ImageId(u64::MAX)).is_none());
        assert_eq!(cache.stats().misses, 0, "an unregistered id is not counted as a CACHE miss (nothing to evict/insert)");
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn tiny_cache_forced_eviction_never_touches_this_frame_slots() {
        let Some((device, queue)) = test_device() else { return };
        let mut cache = NativeImageCache::new(&device, 1);
        cache.begin_frame();

        let a = register_sample(2, 2, [10, 0, 0, 255]);
        let b = register_sample(2, 2, [0, 10, 0, 255]);

        cache.get_or_upload(&device, &queue, a).expect("one image must fit a cap=1 cache");

        // Second, distinct image, same frame: no room left AND the only
        // slot (`a`) was touched THIS frame — must miss, never evict `a`.
        assert!(cache.get_or_upload(&device, &queue, b).is_none());
        assert_eq!(cache.stats().evictions, 0);

        // `a` must still resolve.
        assert!(cache.get_or_upload(&device, &queue, a).is_some());

        // Next frame: `a` is now stale, so `b` CAN evict it.
        cache.begin_frame();
        assert!(cache.get_or_upload(&device, &queue, b).is_some(), "next frame, a is eviction-eligible — b must now fit");
        assert_eq!(cache.stats().evictions, 1);

        uzor_urx_image::unregister_image(a);
        uzor_urx_image::unregister_image(b);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn bind_group_layout_is_usable_in_a_pipeline_layout() {
        let Some((device, _queue)) = test_device() else { return };
        let cache = NativeImageCache::new(&device, 4);
        let _layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("image_bgl_smoke_test_layout"),
            bind_group_layouts: &[Some(cache.bind_group_layout())],
            immediate_size: 0,
        });
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn bind_group_of_returns_the_slots_own_bind_group() {
        let Some((device, queue)) = test_device() else { return };
        let id = register_sample(3, 3, [5, 6, 7, 255]);
        let mut cache = NativeImageCache::new(&device, 4);
        cache.begin_frame();
        let slot = cache.get_or_upload(&device, &queue, id).expect("lookup must succeed");
        // Just proves the accessor compiles + returns a usable
        // `&wgpu::BindGroup` reference (identity, not content — bind
        // groups have no public equality/inspection API).
        let _bg: &wgpu::BindGroup = NativeImageCache::bind_group_of(slot);
        uzor_urx_image::unregister_image(id);
    }

    #[test]
    fn stats_default_is_all_zero() {
        let stats = NativeImageCacheStats::default();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.evictions, 0);
    }
}
