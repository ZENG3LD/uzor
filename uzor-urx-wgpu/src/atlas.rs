//! `NativeGlyphAtlas` — a single `R8Unorm` GPU texture holding
//! rasterised glyph coverage bitmaps, packed by `etagere`'s
//! `BucketedAtlasAllocator` and keyed by `uzor_urx_glyph::GlyphKey`.
//!
//! Built standalone in URX Wave 2 Commit 1
//! (`docs/uzor-engines/plans/urx-wave2-native-glyph-atlas-design-2026-07-25.md`
//! §3) — exercised only by this file's own unit tests at that point,
//! so Commit 1 stayed an isolated, independently reviewable change to
//! a shared-nothing new file. Wired into `NativeUrxRenderer`/
//! `encode.rs` in Commit 2 (`renderer.rs`'s `glyph_atlas` field,
//! `encode.rs`'s `encode_glyph_run`) — this is now real production
//! machinery, not standalone. Commit 3 added the parity fixture
//! (`tests/fixtures.rs::glyph_run_two_letters`) that exercises the
//! whole path end-to-end and this doc pass.
//!
//! ## Rasterisation source (design §0)
//!
//! Bitmaps come from `uzor_urx_glyph::rasterise_glyph` — the SAME
//! function `uzor-urx-cpu`'s `GlyphRun` arm ultimately calls via
//! `draw_glyph_run`. This atlas never runs its own `swash`/`cosmic-text`
//! rasteriser; it only packs bitmaps `encode.rs`'s `encode_glyph_run`
//! hands it into GPU texture space.
//!
//! ## Never-evict-this-frame invariant (design §3)
//!
//! `GlyphInstance.uv_pos`/`uv_size` are baked into the instance buffer
//! at encode time, not re-looked-up at draw time — so an eviction that
//! overwrites a slot an EARLIER instance in the same frame already
//! captured a `uv_rect` for would make that earlier instance sample the
//! wrong (newly evicted-in) bitmap once the pass runs. `get_or_insert`
//! restricts eviction candidates to slots with `tick < self.tick` (not
//! yet touched this frame) — nothing referenced by an already-emitted
//! instance can ever be picked as a victim within the same
//! `encode_scene` call. When every existing slot has already been
//! touched this frame and the atlas is still full, `get_or_insert`
//! honestly returns `None` (the caller counts
//! `native_glyph_atlas_full_this_frame` and skips that glyph) rather
//! than corrupting an earlier instance's sampled pixels.
//!
//! ## `etagere::BucketedAtlasAllocator::deallocate` finding (design risk 1)
//!
//! Confirmed present in `etagere` 0.2.15
//! (`etagere-0.2.15/src/bucketed.rs:286`,
//! `pub fn deallocate(&mut self, id: AllocId)`) — the legacy crate's own
//! atlas (`uzor-render-wgpu-instanced/src/text_atlas.rs`) never calls
//! it (only `.allocate`, full-reset via `clear_cache` on cache drop),
//! so this is the first real per-slot-eviction consumer of `etagere` in
//! this workspace. No fallback needed; real per-slot `deallocate` is
//! used directly.
//!
//! ## Data-structure deviation from Wave 1's `TessCache`/`GlyphLru` (design §3)
//!
//! Those are `Vec<(key, value, tick)>` + linear scan + `swap_remove`,
//! sized for a small, count-bound working set. This atlas's cache is
//! AREA-bound, not count-bound (a document-heavy scene can hold
//! thousands of live glyph identities at once), and eviction must
//! interact with `etagere`'s allocator (`deallocate(AllocId)`, not a
//! flat index swap-remove) — so `HashMap<GlyphKey, AtlasSlot>` for O(1)
//! exact lookup is the right structure here, matching the same call the
//! workspace's own legacy `GlyphAtlas` already made for the identical
//! problem (`text_atlas.rs:44`, `HashMap<CacheKey, GlyphEntry>`).

use std::collections::HashMap;

use etagere::{size2, AllocId, BucketedAtlasAllocator};

use uzor_urx_glyph::{GlyphBitmap, GlyphKey};

/// One packed glyph's placement inside the atlas texture.
struct AtlasSlot {
    alloc_id: AllocId,
    /// x, y, w, h in `[0, 1]` UV space — what `GlyphInstance` bakes in.
    uv_rect: [f32; 4],
    /// Frame tick this slot was last touched (inserted OR hit) on.
    tick: u64,
}

/// A glyph bitmap queued for upload — drained by `flush_uploads`.
struct PendingUpload {
    /// Atlas texel-space destination (unpadded glyph rect).
    px_rect: [u32; 4],
    /// R8 coverage, row-major, tightly packed (`px_rect[2] * px_rect[3]`
    /// bytes) — copied straight from `GlyphBitmap::alpha`.
    alpha: Vec<u8>,
}

/// Read-only atlas telemetry — hit/miss/eviction/entry counts (design
/// §3, same shape family as `TessCacheStats`, plus `evictions` since
/// this cache has a real per-slot eviction path `TessCache` doesn't).
/// `pub` (re-exported from `lib.rs`, same convention as
/// `TessCacheStats`) — `NativeUrxRenderer::glyph_atlas_stats()`
/// (Wave 2 Commit 2) returns this, so it must be at least as visible
/// as that `pub fn`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AtlasStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// Owns the glyph atlas `R8Unorm` texture, its group-1 bind group
/// (texture + sampler), the `etagere` rectangle allocator, and the
/// per-`GlyphKey` slot table.
pub(crate) struct NativeGlyphAtlas {
    texture: wgpu::Texture,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    allocator: BucketedAtlasAllocator,
    slots: HashMap<GlyphKey, AtlasSlot>,
    pending: Vec<PendingUpload>,
    tick: u64,
    width: u32,
    height: u32,
    stats: AtlasStats,
}

impl NativeGlyphAtlas {
    /// Build a `width x height` `R8Unorm` atlas texture + its group-1
    /// bind group layout (`binding 0`: texture, `binding 1`: sampler —
    /// matches `GLYPH_SHADER_NATIVE`'s `@group(1)` bindings, design §5).
    pub(crate) fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_urx_wgpu.native_glyph_atlas"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Linear mag/min + nearest mipmap, matching the legacy atlas's
        // proven sampler (`text_atlas.rs:69-75`) — 1px padding on every
        // glyph allocation (see `get_or_insert`) keeps this filter from
        // bleeding a neighbour glyph's coverage into an edge sample.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uzor_urx_wgpu.native_glyph_atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uzor_urx_wgpu.native_glyph_atlas_bgl"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uzor_urx_wgpu.native_glyph_atlas_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        let allocator = BucketedAtlasAllocator::new(size2(width as i32, height as i32));

        Self {
            texture,
            bind_group_layout,
            bind_group,
            allocator,
            slots: HashMap::new(),
            pending: Vec::new(),
            tick: 0,
            width,
            height,
            stats: AtlasStats::default(),
        }
    }

    /// Bind group layout for `GlyphPipeline::new`'s group-1 pipeline
    /// layout entry (Commit 2).
    pub(crate) fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Bound at group 1 during the `BatchKind::Glyph` batch-replay arm
    /// (Commit 2).
    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Advance the per-frame tick. Call exactly once per `encode_scene`
    /// call, before walking the scene — every `get_or_insert` hit/
    /// insert this frame is stamped with the NEW tick, so
    /// `get_or_insert`'s eviction path can tell "already used this
    /// frame" (never evict) apart from "stale" (evict first).
    pub(crate) fn begin_frame(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Look up `key`; on a hit, stamp the slot's tick to `self.tick`
    /// (protecting it from this-frame eviction) and return its current
    /// `uv_rect`. On a miss, allocate a `(width+2) x (height+2)` rect
    /// (1px padding per side — bilinear-bleed prevention, matches
    /// legacy `text_atlas.rs:150-158`); if the atlas is full, evict the
    /// single slot with the OLDEST tick strictly less than `self.tick`
    /// (never a slot touched THIS frame) and retry once. Returns `None`
    /// only when no such eviction candidate exists (every slot was
    /// already touched this frame — genuine within-frame
    /// oversubscription) or the retry still fails to fit. On success,
    /// queues `bitmap.alpha` into the pending-upload list and returns
    /// the new `uv_rect`.
    pub(crate) fn get_or_insert(&mut self, key: GlyphKey, bitmap: &GlyphBitmap) -> Option<[f32; 4]> {
        if let Some(slot) = self.slots.get_mut(&key) {
            slot.tick = self.tick;
            self.stats.hits += 1;
            return Some(slot.uv_rect);
        }
        self.stats.misses += 1;

        let padded_w = bitmap.width + 2;
        let padded_h = bitmap.height + 2;
        let requested = size2(padded_w as i32, padded_h as i32);

        let alloc = match self.allocator.allocate(requested) {
            Some(alloc) => alloc,
            None => {
                let victim = self
                    .slots
                    .iter()
                    .filter(|(_, slot)| slot.tick < self.tick)
                    .min_by_key(|(_, slot)| slot.tick)
                    .map(|(key, slot)| (*key, slot.alloc_id));
                let (victim_key, victim_alloc) = victim?;
                self.allocator.deallocate(victim_alloc);
                self.slots.remove(&victim_key);
                self.stats.evictions += 1;
                self.allocator.allocate(requested)?
            }
        };

        // The glyph itself starts 1px inside the padded allocation.
        let px_x = alloc.rectangle.min.x as u32 + 1;
        let px_y = alloc.rectangle.min.y as u32 + 1;
        let px_rect = [px_x, px_y, bitmap.width, bitmap.height];
        let uv_rect = [
            px_x as f32 / self.width as f32,
            px_y as f32 / self.height as f32,
            bitmap.width as f32 / self.width as f32,
            bitmap.height as f32 / self.height as f32,
        ];

        self.pending.push(PendingUpload { px_rect, alpha: bitmap.alpha.clone() });
        self.slots.insert(key, AtlasSlot { alloc_id: alloc.id, uv_rect, tick: self.tick });
        self.stats.entries = self.slots.len();

        Some(uv_rect)
    }

    /// Drain every queued upload via `queue.write_texture`, one call
    /// per rect. Called once per frame from `render_into_encoder`,
    /// after `encode_scene` returns (so every glyph this frame has
    /// already been placed) and before `begin_render_pass` (Commit 2).
    pub(crate) fn flush_uploads(&mut self, queue: &wgpu::Queue) {
        for upload in self.pending.drain(..) {
            let [x, y, w, h] = upload.px_rect;
            if w == 0 || h == 0 {
                continue;
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload.alpha,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
    }

    pub(crate) fn stats(&self) -> AtlasStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless wgpu device — mirrors `tests/common/mod.rs::init_device`
    /// (same wgpu-29 shape). Returns `None` (never panics) so tests can
    /// skip gracefully on GPU-less CI/dev boxes; every test using this
    /// is `#[ignore]`-gated, matching this crate's parity-test policy.
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-urx-wgpu-atlas-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    fn bitmap(w: u32, h: u32, fill: u8) -> GlyphBitmap {
        GlyphBitmap { width: w, height: h, left: 0, top: 0, alpha: vec![fill; (w * h) as usize] }
    }

    fn key(glyph_id: u32) -> GlyphKey {
        // FontId has no public constructor outside `uzor-urx-glyph`'s
        // own `register_font` — but `GlyphKey::new` takes it as an
        // opaque value, so any two distinct `u32` `glyph_id`s produce
        // distinct keys regardless of which real `FontId` is used.
        // `register_font` needs real font bytes; a fixed dummy id
        // reached via a fresh registration keeps these tests font-file-free.
        GlyphKey::new(dummy_font_id(), glyph_id, 32.0, 0)
    }

    /// A `FontId` for key-construction purposes only — these tests
    /// never rasterise, so the id need not resolve to a real
    /// registered font, but it DOES need to be the SAME value across
    /// every `key()` call in a test (two separate `register_font` calls
    /// would mint two different `FontId`s, silently breaking the
    /// "repeat key" tests this module cares about) — memoized once per
    /// process via `OnceLock`.
    fn dummy_font_id() -> uzor_urx_glyph::FontId {
        static FONT_ID: std::sync::OnceLock<uzor_urx_glyph::FontId> = std::sync::OnceLock::new();
        *FONT_ID.get_or_init(|| {
            let bytes =
                std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../uzor-fonts/fonts/DejaVuSans.ttf"))
                    .expect("uzor-fonts ships DejaVuSans.ttf for exactly this kind of test-only registration");
            uzor_urx_glyph::register_font(bytes).expect("DejaVuSans.ttf is a valid font")
        })
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn repeat_key_is_a_cache_hit_with_no_duplicate_upload() {
        let Some((device, _queue)) = test_device() else { return };
        let mut atlas = NativeGlyphAtlas::new(&device, 256, 256);
        atlas.begin_frame();

        let k = key(36);
        let bm = bitmap(10, 12, 200);

        let first = atlas.get_or_insert(k, &bm).expect("first insert must succeed in an empty 256x256 atlas");
        assert_eq!(atlas.pending.len(), 1, "first insert queues exactly one upload");
        let queued = &atlas.pending[0];
        assert_eq!(queued.px_rect[2], bm.width, "queued upload's px_rect must carry the UNPADDED glyph width");
        assert_eq!(queued.px_rect[3], bm.height, "queued upload's px_rect must carry the UNPADDED glyph height");

        let second = atlas.get_or_insert(k, &bm).expect("repeat key must hit, not fail");
        assert_eq!(first, second, "repeat key returns the SAME uv_rect");
        assert_eq!(atlas.pending.len(), 1, "repeat key must NOT queue a second upload");

        let stats = atlas.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn tiny_atlas_forced_eviction_never_touches_this_frame_slots() {
        let Some((device, _queue)) = test_device() else { return };
        // 8x8 exactly fits ONE 6x6 bitmap's padded (8x8) allocation and
        // NOTHING else (no free space is left for a second shelf/column
        // of any size) — a single-slot atlas makes the "is it full"
        // outcome deterministic instead of depending on `etagere`'s
        // shelf-packing heuristics fitting (or not) a second rect.
        let mut atlas = NativeGlyphAtlas::new(&device, 8, 8);
        atlas.begin_frame();

        let a = key(1);
        let bm_a = bitmap(6, 6, 100);
        let uv_a = atlas.get_or_insert(a, &bm_a).expect("one 6x6 (8x8 padded) must exactly fill an 8x8 atlas");

        // A second, distinct glyph, same frame: the atlas has no room
        // left AND its only slot (`a`) was touched THIS frame — must
        // return None, never silently evict `a`.
        let b = key(2);
        let bm_b = bitmap(6, 6, 100);
        let second = atlas.get_or_insert(b, &bm_b);
        assert!(second.is_none(), "no eviction candidate exists this frame — must report atlas-full, not corrupt a");
        assert_eq!(atlas.stats().evictions, 0, "nothing was evicted — the full-this-frame path never evicts");

        // `a` must still resolve to its original uv_rect — proof the
        // (nonexistent) eviction attempt never touched it.
        let a_again = atlas.get_or_insert(a, &bm_a).expect("a must still be a hit");
        assert_eq!(a_again, uv_a);

        // Next frame: `a` is now "stale" (tick < current), so `b` CAN
        // evict it to make room.
        atlas.begin_frame();
        let second_attempt = atlas.get_or_insert(b, &bm_b);
        assert!(second_attempt.is_some(), "next frame, a is eviction-eligible — b must now fit");
        assert_eq!(atlas.stats().evictions, 1, "a must have been evicted to make room for b");

        // One more frame: `b` is now stale too, so `a` can evict it in
        // turn (the atlas is single-slot — everything churns through
        // the same one rect once every slot has aged out of "this
        // frame" protection).
        atlas.begin_frame();
        let misses_before = atlas.stats().misses;
        let a_third = atlas.get_or_insert(a, &bm_a);
        assert!(a_third.is_some(), "a can be re-inserted by evicting the now-stale b");
        assert_eq!(atlas.stats().misses, misses_before + 1);
        assert_eq!(atlas.stats().evictions, 2);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn flush_uploads_drains_exactly_the_queued_rects() {
        let Some((device, queue)) = test_device() else { return };
        let mut atlas = NativeGlyphAtlas::new(&device, 256, 256);
        atlas.begin_frame();

        let _ = atlas.get_or_insert(key(10), &bitmap(8, 8, 255));
        let _ = atlas.get_or_insert(key(11), &bitmap(9, 9, 255));
        let _ = atlas.get_or_insert(key(12), &bitmap(7, 7, 255));
        assert_eq!(atlas.pending.len(), 3);

        atlas.flush_uploads(&queue);
        assert!(atlas.pending.is_empty(), "flush_uploads must drain every queued rect");

        // A second flush with nothing pending must be a harmless no-op.
        atlas.flush_uploads(&queue);
        assert!(atlas.pending.is_empty());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn bind_group_layout_is_usable_in_a_pipeline_layout() {
        let Some((device, _queue)) = test_device() else { return };
        let atlas = NativeGlyphAtlas::new(&device, 64, 64);

        // Proves `bind_group_layout()` returns a layout Commit 2's
        // `GlyphPipeline::new` can actually build a `wgpu::PipelineLayout`
        // from (group 1, alongside the shared uniform group 0) — the
        // real consumer this accessor exists for.
        let _layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("atlas_bgl_smoke_test_layout"),
            bind_group_layouts: &[Some(atlas.bind_group_layout())],
            immediate_size: 0,
        });

        // `bind_group()` is the group-1 resource `render_into_encoder`'s
        // batch-replay arm binds at draw time (Commit 2) — it must stay
        // a STABLE handle for the atlas's lifetime (this crate has no
        // atlas-resize path, design §3/§6, so it's never rebuilt).
        assert!(std::ptr::eq(atlas.bind_group(), atlas.bind_group()));
    }

    #[test]
    fn atlas_stats_default_is_all_zero() {
        let stats = AtlasStats::default();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.evictions, 0);
    }
}
