//! `GradientLutAtlas` — a `256 x cap` `Rgba8Unorm` GPU texture holding
//! premultiplied gradient lookup tables, one row per distinct
//! `(stops, extend)` combination, keyed by
//! `uzor_urx_core::gradient_lut::hash_stops`.
//!
//! Built standalone in URX Wave 4 Commit 2
//! (`docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §2.2) — exercised only by this file's own unit tests at this point,
//! same "isolated, independently reviewable new file" shape as Wave 2
//! Commit 1's `atlas::NativeGlyphAtlas` (see that module's own doc
//! comment for the precedent this mirrors). Wired into
//! `NativeUrxRenderer`/`encode.rs` in Commit 3.
//!
//! ## Byte-identical LUTs across backends (design §2.2)
//!
//! `build_lut`/`hash_stops` are NOT reimplemented here — both come
//! straight from `uzor_urx_core::gradient_lut` (the same pure-function
//! extraction `uzor-urx-cpu::gradient` calls, Wave 4 Commit 1). A LUT
//! this atlas builds from a given `(stops, extend)` pair is therefore
//! BYTE-IDENTICAL to the one CPU's own `LutCache` would build from the
//! same inputs — this is what lets the fragment shader's
//! zero-interpolation `textureLoad` sample (design §2.3) reproduce
//! CPU's own zero-interpolation `lut_sample` exactly, not merely
//! approximately.
//!
//! ## Never-evict-this-frame invariant (design §2.2, same shape as
//! `atlas::NativeGlyphAtlas`)
//!
//! `GradientInstance.lut_row` (Commit 3) is baked into the instance
//! buffer at encode time, not re-looked-up at draw time — so an
//! eviction that overwrites a row an EARLIER instance in the same
//! frame already captured a row index for would make that earlier
//! instance sample the WRONG gradient once the pass runs.
//! `get_or_insert` restricts eviction candidates to rows with
//! `tick < self.tick` (not yet touched this frame) — nothing
//! referenced by an already-emitted instance can ever be picked as a
//! victim within the same `encode_scene` call. When every existing row
//! has already been touched this frame and the atlas is still full,
//! `get_or_insert` honestly returns `None` rather than corrupting an
//! earlier instance's sampled colours; the caller (Commit 3) counts a
//! degrade and falls back to a solid colour for that draw.
//!
//! ## Row allocation — simpler than `NativeGlyphAtlas`'s `etagere` packing
//!
//! Every row is IDENTICAL size (`LUT_SIZE` texels wide, 1 texel tall) —
//! there is no 2D rect-packing problem here, unlike glyph bitmaps of
//! varying width/height. Allocation is therefore a plain row COUNTER
//! (`next_free_row`, `0..cap`) for rows never yet used, falling back to
//! LRU eviction (oldest `tick` strictly less than the current one) only
//! once every row has been used at least once — no `etagere` dependency
//! needed for this atlas.
//!
//! ## No sampler — `textureLoad`, not `textureSample` (design §2.3)
//!
//! The group-1 bind group layout carries ONLY a texture binding
//! (`sample_type: Float { filterable: false }`) — CPU's own LUT read
//! (`lut_sample`) is an EXACT rounded-clamped index with zero
//! interpolation between entries; the fragment shader (Commit 3) reads
//! this atlas the same way, via `textureLoad(lut_tex, vec2<i32>(col,
//! row), 0)`, never `textureSample`. A sampler binding would invite
//! accidental bilinear blending between two DIFFERENT gradients'
//! stacked rows (a nonsensical operation, since each row is an
//! independent, unrelated LUT) — omitting the sampler entirely
//! prevents that class of bug at the type level, not just by
//! convention.

use std::collections::HashMap;

use uzor_urx_core::gradient_lut::{build_lut, hash_stops, GradientLut, LUT_SIZE};

/// One resident LUT row's bookkeeping.
struct LutSlot {
    row: u32,
    /// Frame tick this slot was last touched (inserted OR hit) on.
    tick: u64,
}

/// A built LUT queued for upload — drained by `flush_uploads`.
struct PendingUpload {
    row: u32,
    lut: GradientLut,
}

/// Read-only atlas telemetry — hit/miss/eviction/entry counts, same
/// shape family as `atlas::AtlasStats`. `pub` (re-exported from
/// `lib.rs`, same convention as `AtlasStats`/`TessCacheStats`) —
/// `NativeUrxRenderer::gradient_lut_stats()` (Wave 4 Commit 3) returns
/// this, so it must be at least as visible as that `pub fn`.
#[derive(Debug, Clone, Copy, Default)]
pub struct GradientLutAtlasStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// Owns the `256 x cap` `Rgba8Unorm` LUT texture, its group-1 bind
/// group (texture only, no sampler — see this module's doc comment),
/// and the `hash_stops`-keyed row table.
pub(crate) struct GradientLutAtlas {
    texture: wgpu::Texture,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    rows: HashMap<u64, LutSlot>,
    pending: Vec<PendingUpload>,
    /// Rows `0..next_free_row` have been allocated at least once;
    /// `next_free_row..cap` are still virgin (no eviction needed to use
    /// them). Once `next_free_row == cap`, every further `get_or_insert`
    /// miss must evict.
    next_free_row: u32,
    tick: u64,
    cap: u32,
    stats: GradientLutAtlasStats,
}

impl GradientLutAtlas {
    /// Build a `LUT_SIZE x cap` `Rgba8Unorm` atlas texture + its group-1
    /// bind group layout (`binding 0`: texture only — design §2.3's
    /// `textureLoad`-only contract, no sampler entry at all).
    pub(crate) fn new(device: &wgpu::Device, cap: u32) -> Self {
        let cap = cap.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_urx_wgpu.native_gradient_lut"),
            size: wgpu::Extent3d { width: LUT_SIZE as u32, height: cap, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uzor_urx_wgpu.native_gradient_lut_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    // Non-filterable — this atlas is read exclusively
                    // via `textureLoad` (exact integer-coordinate
                    // fetch), never `textureSample`; no sampler binding
                    // exists in this bind group at all (see module doc).
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uzor_urx_wgpu.native_gradient_lut_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });

        Self {
            texture,
            bind_group_layout,
            bind_group,
            rows: HashMap::new(),
            pending: Vec::new(),
            next_free_row: 0,
            tick: 0,
            cap,
            stats: GradientLutAtlasStats::default(),
        }
    }

    /// Bind group layout for `GradientPipeline::new`'s group-1 pipeline
    /// layout entry (Commit 3).
    pub(crate) fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Bound at group 1 during the `BatchKind::Gradient` batch-replay
    /// arm (Commit 3).
    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Advance the per-frame tick. Call exactly once per `encode_scene`
    /// call, before walking the scene — mirrors
    /// `atlas::NativeGlyphAtlas::begin_frame` exactly.
    pub(crate) fn begin_frame(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Get-or-build a LUT row for `(stops, extend)` — hashed via the
    /// SAME `hash_stops` CPU uses (design §2.2), so a GPU row and a CPU
    /// `LutCache` entry built from identical stops always agree on
    /// "same gradient." On a hit, stamps the row's tick (protecting it
    /// from this-frame eviction) and returns its row index. On a miss,
    /// allocates a virgin row if any remain, else evicts the row with
    /// the OLDEST tick strictly less than `self.tick` (never a row
    /// touched THIS frame — see module doc). Returns `None` only when
    /// no eviction candidate exists (every row already touched this
    /// frame — genuine within-frame oversubscription).
    pub(crate) fn get_or_insert(&mut self, stops: &peniko::ColorStops, extend: peniko::Extend) -> Option<u32> {
        let key = hash_stops(stops, extend);
        if let Some(slot) = self.rows.get_mut(&key) {
            slot.tick = self.tick;
            self.stats.hits += 1;
            return Some(slot.row);
        }
        self.stats.misses += 1;

        let row = if self.next_free_row < self.cap {
            let r = self.next_free_row;
            self.next_free_row += 1;
            r
        } else {
            let victim = self
                .rows
                .iter()
                .filter(|(_, slot)| slot.tick < self.tick)
                .min_by_key(|(_, slot)| slot.tick)
                .map(|(k, slot)| (*k, slot.row));
            let (victim_key, victim_row) = victim?;
            self.rows.remove(&victim_key);
            self.stats.evictions += 1;
            victim_row
        };

        let lut = build_lut(stops);
        self.pending.push(PendingUpload { row, lut });
        self.rows.insert(key, LutSlot { row, tick: self.tick });
        self.stats.entries = self.rows.len();

        Some(row)
    }

    /// Drain every queued LUT upload via `queue.write_texture`, one
    /// call per row. Called once per frame from `render_into_encoder`
    /// (Commit 3), after `encode_scene` returns and before the render
    /// pass opens — same timing convention as
    /// `atlas::NativeGlyphAtlas::flush_uploads`.
    pub(crate) fn flush_uploads(&mut self, queue: &wgpu::Queue) {
        for upload in self.pending.drain(..) {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: upload.row, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::bytes_of(&upload.lut),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(LUT_SIZE as u32 * 4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d { width: LUT_SIZE as u32, height: 1, depth_or_array_layers: 1 },
            );
        }
    }

    pub(crate) fn stats(&self) -> GradientLutAtlasStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless wgpu device — mirrors `atlas.rs`'s own `test_device`
    /// helper (same wgpu-29 shape, same graceful-skip-if-no-adapter
    /// policy). Every test using this is `#[ignore]`-gated, matching
    /// this crate's parity-test policy.
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-urx-wgpu-gradient-lut-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    fn stops(c0: [u8; 4], c1: [u8; 4]) -> peniko::ColorStops {
        use peniko::ColorStop;
        // `ColorStops` has no `FromIterator<ColorStop>` on the pinned
        // `peniko = "0.6"` — only `From<&[ColorStop]>` (same finding as
        // `uzor-urx-cpu/tests/gradient_wave4_fixes.rs`).
        let v = vec![
            ColorStop { offset: 0.0, color: peniko::Color::from_rgba8(c0[0], c0[1], c0[2], c0[3]).into() },
            ColorStop { offset: 1.0, color: peniko::Color::from_rgba8(c1[0], c1[1], c1[2], c1[3]).into() },
        ];
        peniko::ColorStops::from(&v[..])
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn repeat_key_is_a_cache_hit_with_the_same_row() {
        let Some((device, _queue)) = test_device() else { return };
        let mut atlas = GradientLutAtlas::new(&device, 4);
        atlas.begin_frame();

        let s = stops([255, 0, 0, 255], [0, 0, 255, 255]);
        let first = atlas.get_or_insert(&s, peniko::Extend::Pad).expect("first insert must succeed");
        let second = atlas.get_or_insert(&s, peniko::Extend::Pad).expect("repeat key must hit, not fail");
        assert_eq!(first, second, "repeat key returns the SAME row");

        let stats = atlas.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn built_row_is_byte_identical_to_cpu_build_lut() {
        // Proves design §2.2's core claim directly: the LUT this atlas
        // queues for upload is byte-for-byte what
        // `uzor_urx_core::gradient_lut::build_lut` (the SAME function
        // `uzor-urx-cpu::gradient` calls) produces from identical
        // stops — not just "close," not re-derived.
        let Some((device, _queue)) = test_device() else { return };
        let mut atlas = GradientLutAtlas::new(&device, 4);
        atlas.begin_frame();

        let s = stops([10, 20, 30, 255], [200, 150, 100, 255]);
        let _ = atlas.get_or_insert(&s, peniko::Extend::Pad).expect("insert must succeed");

        assert_eq!(atlas.pending.len(), 1);
        let expected = build_lut(&s);
        assert_eq!(atlas.pending[0].lut, expected, "atlas-built LUT must be byte-identical to CPU's own build_lut");
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn different_extend_mode_is_a_distinct_row() {
        let Some((device, _queue)) = test_device() else { return };
        let mut atlas = GradientLutAtlas::new(&device, 4);
        atlas.begin_frame();

        let s = stops([255, 0, 0, 255], [0, 0, 255, 255]);
        let pad_row = atlas.get_or_insert(&s, peniko::Extend::Pad).unwrap();
        let repeat_row = atlas.get_or_insert(&s, peniko::Extend::Repeat).unwrap();
        assert_ne!(pad_row, repeat_row, "same stops but different extend mode must hash to a DIFFERENT row");
        assert_eq!(atlas.stats().entries, 2);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn tiny_atlas_forced_eviction_never_touches_this_frame_rows() {
        let Some((device, _queue)) = test_device() else { return };
        // cap = 1 makes "is it full" deterministic on the very next
        // distinct key — same reasoning as `atlas.rs`'s single-slot
        // 8x8 forced-eviction test.
        let mut atlas = GradientLutAtlas::new(&device, 1);
        atlas.begin_frame();

        let a = stops([255, 0, 0, 255], [0, 255, 0, 255]);
        let row_a = atlas.get_or_insert(&a, peniko::Extend::Pad).expect("one gradient must fit a cap=1 atlas");

        // A second, distinct gradient, same frame: no room left AND the
        // only row (`a`) was touched THIS frame — must return None,
        // never silently evict `a`.
        let b = stops([0, 0, 255, 255], [255, 255, 0, 255]);
        let second = atlas.get_or_insert(&b, peniko::Extend::Pad);
        assert!(second.is_none(), "no eviction candidate exists this frame — must report atlas-full, not corrupt a");
        assert_eq!(atlas.stats().evictions, 0);

        // `a` must still resolve to its original row.
        let a_again = atlas.get_or_insert(&a, peniko::Extend::Pad).expect("a must still be a hit");
        assert_eq!(a_again, row_a);

        // Next frame: `a` is now stale, so `b` CAN evict it.
        atlas.begin_frame();
        let second_attempt = atlas.get_or_insert(&b, peniko::Extend::Pad);
        assert!(second_attempt.is_some(), "next frame, a is eviction-eligible — b must now fit");
        assert_eq!(atlas.stats().evictions, 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn flush_uploads_drains_exactly_the_queued_rows() {
        let Some((device, queue)) = test_device() else { return };
        let mut atlas = GradientLutAtlas::new(&device, 8);
        atlas.begin_frame();

        let _ = atlas.get_or_insert(&stops([1, 1, 1, 255], [2, 2, 2, 255]), peniko::Extend::Pad);
        let _ = atlas.get_or_insert(&stops([3, 3, 3, 255], [4, 4, 4, 255]), peniko::Extend::Pad);
        assert_eq!(atlas.pending.len(), 2);

        atlas.flush_uploads(&queue);
        assert!(atlas.pending.is_empty(), "flush_uploads must drain every queued row");

        // A second flush with nothing pending must be a harmless no-op.
        atlas.flush_uploads(&queue);
        assert!(atlas.pending.is_empty());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn bind_group_layout_is_usable_in_a_pipeline_layout() {
        let Some((device, _queue)) = test_device() else { return };
        let atlas = GradientLutAtlas::new(&device, 4);

        // Proves `bind_group_layout()` returns a layout Commit 3's
        // `GradientPipeline::new` can actually build a
        // `wgpu::PipelineLayout` from (group 1, alongside the shared
        // uniform group 0).
        let _layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gradient_lut_bgl_smoke_test_layout"),
            bind_group_layouts: &[Some(atlas.bind_group_layout())],
            immediate_size: 0,
        });

        assert!(std::ptr::eq(atlas.bind_group(), atlas.bind_group()));
    }

    #[test]
    fn stats_default_is_all_zero() {
        let stats = GradientLutAtlasStats::default();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.evictions, 0);
    }
}
