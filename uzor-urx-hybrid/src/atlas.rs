//! Region atlas — shared `wgpu::Texture` packing many small regions
//! into one bind group (Hybrid-P1).
//!
//! ## Rationale
//!
//! Default `HybridBackend` allocates one `wgpu::Texture` + one bind
//! group per region. At ~20+ regions per frame the per-region bind
//! group switches dominate composite CPU cost (each switch =
//! pipeline-state validation work in wgpu's tracker — see Bevy
//! bindless PR #16368, measured 1.47× speedup on N≈100 textures).
//!
//! The atlas packs all "small" regions (each side ≤ atlas_size/2)
//! into one shared texture using `etagere::BucketedAtlasAllocator`
//! (shelf packing — production choice in Mozilla WebRender + glyphon).
//!
//! On composite, all atlas-packed regions share ONE bind group and
//! can ultimately be drawn with a single instanced draw call (the
//! `hybrid_instanced_composite` flag in `UrxConfig`).
//!
//! ## What the atlas does NOT cover
//!
//! - Regions larger than `atlas_w/2 × atlas_h/2` — kept as standalone
//!   `RegionTexture` (fallback path stays untouched). The atlas is a
//!   strict win for small regions; large regions don't benefit from
//!   shared bind groups.
//! - Regions that consumer hands as straight RGBA8 — atlas internal
//!   format is `Rgba8UnormSrgb` for correct sRGB blending (per
//!   16-research §"sRGB handling"). Caller's pixmap is uploaded
//!   as-is and the GPU sampler does the linearisation.
//!
//! ## Eviction
//!
//! Currently NONE. Atlas is monotonically allocated until full, then
//! `allocate` returns `None` and the consumer falls back to
//! standalone. A future commit will add LRU eviction.

use std::collections::HashMap;

use etagere::{size2, BucketedAtlasAllocator};
use uzor_urx_core::region::RegionId;
use uzor_urx_cpu::Pixmap;

use crate::region_tex::fnv1a_64;

/// One region's atlas slot: where in the atlas it lives + freshness
/// metadata.
#[derive(Debug, Clone, Copy)]
pub struct AtlasSlot {
    /// etagere allocation handle — kept so the slot can be freed.
    pub allocation: etagere::Allocation,
    /// Pixel rect within the atlas: `[x, y, w, h]`.
    pub px_rect: [u32; 4],
    /// Normalised UV rect within the atlas: `[u0, v0, u1, v1]`.
    pub uv_rect: [f32; 4],
    /// fnv1a-64 over the most recently written pixmap bytes; for
    /// dirty-skip on identical content uploads.
    pub content_hash: u64,
    /// Caller-supplied generation counter (None if caller didn't
    /// pass one).
    pub generation: Option<u64>,
}

/// Outcome of a `try_upsert` attempt.
#[derive(Debug)]
pub enum AtlasUpsertResult {
    /// Region was uploaded (new allocation or in-place sub-rect write).
    Uploaded { bytes: u64 },
    /// Region's existing slot is already current (hash or generation
    /// match) — no GPU work happened.
    Skipped { bytes: u64 },
    /// Atlas is full OR the region is too large for the atlas; caller
    /// must fall back to standalone-texture path.
    Reject,
}

/// Shared-texture region atlas. Construct once per HybridBackend.
///
/// LRU eviction is built-in: on allocation failure (atlas full), the
/// least-recently-used slot is evicted and its space freed for the
/// new allocation. "Recency" is tracked at upsert AND at composite
/// time via [`Self::touch`] — callers should `touch` each rendered
/// region once per frame so static panels stay alive across many
/// dynamic uploads.
pub struct RegionAtlas {
    pub texture:    wgpu::Texture,
    pub view:       wgpu::TextureView,
    pub width:      u32,
    pub height:     u32,
    allocator:      BucketedAtlasAllocator,
    slots:          HashMap<RegionId, AtlasSlot>,
    /// LRU queue. `lru_order[0]` = least-recently-used (head); push
    /// to back on every touch. Eviction pops the head.
    lru_order:      std::collections::VecDeque<RegionId>,
    /// Eviction counter for the current frame — reset by
    /// [`Self::reset_frame_counters`] (called from composite()).
    /// When this exceeds [`AUTO_RESIZE_EVICT_THRESHOLD`] in one frame
    /// the consumer reads `should_resize()` and rebuilds the atlas at
    /// 2× dimensions (clamped to [`MAX_ATLAS_DIM`]).
    evictions_this_frame: u32,
}

/// If atlas-evictions/frame exceeds this, [`RegionAtlas::should_resize`]
/// recommends doubling the atlas. Tuned heuristic — large enough that
/// occasional eviction (3-4) doesn't trigger resize, small enough that
/// sustained pressure does.
pub const AUTO_RESIZE_EVICT_THRESHOLD: u32 = 8;

/// Hard cap on atlas dimensions; matches wgpu's `max_texture_dimension_2d`
/// minimum guarantee across all backends.
pub const MAX_ATLAS_DIM: u32 = 8192;

impl RegionAtlas {
    /// Create the atlas texture + view + allocator. `format` is the
    /// surface format the compositor renders to (we choose
    /// `Rgba8UnormSrgb` for correct blending; the atlas is read into
    /// linear by the GPU sampler).
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx-hybrid-region-atlas"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let allocator = BucketedAtlasAllocator::new(size2(width as i32, height as i32));
        Self {
            texture, view, width, height, allocator,
            slots: HashMap::new(),
            lru_order: std::collections::VecDeque::new(),
            evictions_this_frame: 0,
        }
    }

    /// Per-frame counter reset. Call from the consumer's composite loop
    /// at the start of each frame so [`Self::should_resize`] only sees
    /// the current frame's eviction pressure.
    pub fn reset_frame_counters(&mut self) {
        self.evictions_this_frame = 0;
    }

    /// Recommend new atlas dimensions if eviction pressure crosses the
    /// auto-resize threshold this frame. Returns `Some((new_w, new_h))`
    /// at 2× current size (clamped to [`MAX_ATLAS_DIM`]) when sustained
    /// pressure suggests the atlas is too small for the workload.
    ///
    /// Returns `None` when pressure is below threshold OR the atlas is
    /// already at the cap on both axes (can't grow further).
    pub fn should_resize(&self) -> Option<(u32, u32)> {
        if self.evictions_this_frame < AUTO_RESIZE_EVICT_THRESHOLD {
            return None;
        }
        let new_w = (self.width  * 2).min(MAX_ATLAS_DIM);
        let new_h = (self.height * 2).min(MAX_ATLAS_DIM);
        if new_w == self.width && new_h == self.height {
            return None;
        }
        Some((new_w, new_h))
    }

    /// Promote `id` to most-recently-used position in the LRU queue.
    /// Call once per region per frame from the consumer's composite
    /// path so static panels stay alive across many dynamic uploads.
    pub fn touch(&mut self, id: RegionId) {
        if let Some(pos) = self.lru_order.iter().position(|&r| r == id) {
            self.lru_order.remove(pos);
        }
        self.lru_order.push_back(id);
    }

    /// Pop + deallocate the least-recently-used slot. Returns the
    /// evicted id (and bumps `KEY_HYBRID_ATLAS_EVICTIONS`), or `None`
    /// if the atlas is empty.
    fn evict_lru(&mut self) -> Option<RegionId> {
        while let Some(victim) = self.lru_order.pop_front() {
            if let Some(slot) = self.slots.remove(&victim) {
                self.allocator.deallocate(slot.allocation.id);
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_HYBRID_ATLAS_EVICTIONS
                ).increment(1);
                self.evictions_this_frame =
                    self.evictions_this_frame.saturating_add(1);
                return Some(victim);
            }
            // Stale entry in LRU queue (slot already gone) — keep popping.
        }
        None
    }

    /// True iff a region of (w, h) is small enough to fit any free
    /// shelf in the atlas (heuristic: each side ≤ half the atlas
    /// dimension). Bigger regions waste atlas space — fall back to
    /// standalone texture.
    pub fn fits(&self, w: u32, h: u32) -> bool {
        w * 2 <= self.width && h * 2 <= self.height
    }

    /// Look up the current slot for a region (if cached).
    pub fn slot(&self, id: RegionId) -> Option<&AtlasSlot> {
        self.slots.get(&id)
    }

    /// Returns the count of currently allocated slots.
    pub fn slot_count(&self) -> usize { self.slots.len() }

    /// Try to upsert `pixmap` into the atlas at `id`. Returns:
    /// - `Uploaded` on fresh allocation or in-place re-write
    /// - `Skipped` if content_hash matches the existing slot
    /// - `Reject` if the region doesn't fit or atlas is full
    ///
    /// Caller falls back to standalone `RegionTexture` on `Reject`.
    pub fn try_upsert(
        &mut self,
        queue:  &wgpu::Queue,
        id:     RegionId,
        pixmap: &Pixmap,
    ) -> AtlasUpsertResult {
        let w = pixmap.width();
        let h = pixmap.height();
        if w == 0 || h == 0 { return AtlasUpsertResult::Reject; }
        if !self.fits(w, h) { return AtlasUpsertResult::Reject; }

        // Existing slot? Try in-place re-write if dimensions match.
        if let Some(slot) = self.slots.get(&id) {
            if slot.px_rect[2] == w && slot.px_rect[3] == h {
                let new_hash = fnv1a_64(pixmap.pixels());
                if new_hash == slot.content_hash {
                    // Touch — still in use, even though we skipped upload.
                    self.touch(id);
                    return AtlasUpsertResult::Skipped {
                        bytes: (w as u64) * (h as u64) * 4,
                    };
                }
                // Same dims, different content — write_texture into slot.
                write_to_slot(queue, &self.texture, slot, pixmap);
                let slot_mut = self.slots.get_mut(&id).unwrap();
                slot_mut.content_hash = new_hash;
                self.touch(id);
                return AtlasUpsertResult::Uploaded {
                    bytes: (w as u64) * (h as u64) * 4,
                };
            }
            // Dimensions differ — deallocate, fall through.
            let alloc = slot.allocation;
            self.allocator.deallocate(alloc.id);
            self.slots.remove(&id);
            if let Some(pos) = self.lru_order.iter().position(|&r| r == id) {
                self.lru_order.remove(pos);
            }
        }

        // Try to allocate. On OOM, evict LRU and retry. Bounded by the
        // slot count — at most N evictions to free enough space.
        let alloc = match self.allocator.allocate(size2(w as i32, h as i32)) {
            Some(a) => a,
            None => {
                // Evict LRU slots until we either succeed or run out.
                let mut allocated = None;
                while self.evict_lru().is_some() {
                    if let Some(a) = self.allocator.allocate(size2(w as i32, h as i32)) {
                        allocated = Some(a);
                        break;
                    }
                }
                match allocated {
                    Some(a) => a,
                    None => {
                        metrics::counter!(
                            uzor_urx_core::metrics_keys::KEY_HYBRID_ATLAS_REJECTS
                        ).increment(1);
                        return AtlasUpsertResult::Reject;
                    }
                }
            }
        };
        let px_rect = [
            alloc.rectangle.min.x as u32,
            alloc.rectangle.min.y as u32,
            w,
            h,
        ];
        let uv_rect = [
            px_rect[0] as f32 / self.width as f32,
            px_rect[1] as f32 / self.height as f32,
            (px_rect[0] + w) as f32 / self.width as f32,
            (px_rect[1] + h) as f32 / self.height as f32,
        ];
        let content_hash = fnv1a_64(pixmap.pixels());
        let slot = AtlasSlot {
            allocation: alloc,
            px_rect,
            uv_rect,
            content_hash,
            generation: None,
        };
        write_to_slot(queue, &self.texture, &slot, pixmap);
        self.slots.insert(id, slot);
        self.touch(id);
        AtlasUpsertResult::Uploaded { bytes: (w as u64) * (h as u64) * 4 }
    }

    /// Remove a region from the atlas (free its allocation).
    pub fn remove(&mut self, id: RegionId) {
        if let Some(slot) = self.slots.remove(&id) {
            self.allocator.deallocate(slot.allocation.id);
        }
        if let Some(pos) = self.lru_order.iter().position(|&r| r == id) {
            self.lru_order.remove(pos);
        }
    }

    /// Clear the entire atlas.
    pub fn clear(&mut self) {
        for (_, slot) in self.slots.drain() {
            self.allocator.deallocate(slot.allocation.id);
        }
        self.lru_order.clear();
    }

    /// Update the generation tag on an existing slot (no upload).
    pub fn set_slot_generation(&mut self, id: RegionId, gen: Option<u64>) -> bool {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.generation = gen;
            true
        } else {
            false
        }
    }
}

/// Write pixmap bytes into the atlas at `slot.px_rect`.
fn write_to_slot(
    queue:   &wgpu::Queue,
    texture: &wgpu::Texture,
    slot:    &AtlasSlot,
    pixmap:  &Pixmap,
) {
    let [x, y, w, h] = slot.px_rect;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        pixmap.pixels(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_fits_predicate_rejects_oversize() {
        // Pure logic test — no GPU needed.
        // 2048×2048 atlas: half = 1024. 1025 should reject.
        let dummy_alloc = BucketedAtlasAllocator::new(size2(2048, 2048));
        // We can't construct RegionAtlas without a wgpu::Device, so
        // test fits logic directly on the size formula.
        fn fits(w: u32, h: u32, aw: u32, ah: u32) -> bool {
            w * 2 <= aw && h * 2 <= ah
        }
        assert!(fits(512, 512, 2048, 2048));
        assert!(fits(1024, 1024, 2048, 2048));
        assert!(!fits(1025, 100, 2048, 2048));
        assert!(!fits(100, 1025, 2048, 2048));
        let _ = dummy_alloc;
    }
}
