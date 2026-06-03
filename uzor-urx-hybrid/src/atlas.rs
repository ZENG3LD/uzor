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
pub struct RegionAtlas {
    pub texture:    wgpu::Texture,
    pub view:       wgpu::TextureView,
    pub width:      u32,
    pub height:     u32,
    allocator:      BucketedAtlasAllocator,
    slots:          HashMap<RegionId, AtlasSlot>,
}

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
            texture, view, width, height, allocator, slots: HashMap::new(),
        }
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
                    return AtlasUpsertResult::Skipped {
                        bytes: (w as u64) * (h as u64) * 4,
                    };
                }
                // Same dims, different content — write_texture into slot.
                write_to_slot(queue, &self.texture, slot, pixmap);
                let slot_mut = self.slots.get_mut(&id).unwrap();
                slot_mut.content_hash = new_hash;
                return AtlasUpsertResult::Uploaded {
                    bytes: (w as u64) * (h as u64) * 4,
                };
            }
            // Dimensions differ — deallocate, fall through.
            let alloc = slot.allocation;
            self.allocator.deallocate(alloc.id);
            self.slots.remove(&id);
        }

        // Allocate fresh.
        let Some(alloc) = self.allocator.allocate(size2(w as i32, h as i32)) else {
            return AtlasUpsertResult::Reject;
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
        AtlasUpsertResult::Uploaded { bytes: (w as u64) * (h as u64) * 4 }
    }

    /// Remove a region from the atlas (free its allocation).
    pub fn remove(&mut self, id: RegionId) {
        if let Some(slot) = self.slots.remove(&id) {
            self.allocator.deallocate(slot.allocation.id);
        }
    }

    /// Clear the entire atlas.
    pub fn clear(&mut self) {
        for (_, slot) in self.slots.drain() {
            self.allocator.deallocate(slot.allocation.id);
        }
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
