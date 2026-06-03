//! `UrxEngine` — cross-backend façade.

use std::collections::BTreeMap;
use std::time::Instant;

use uzor_urx_core::dirty::{DirtyRect, DirtyState};
use uzor_urx_core::math::{Affine, Rect};
use uzor_urx_core::region::{CacheKey, RegionId};
use uzor_urx_core::scene::Scene;
use uzor_urx_cpu::{CpuBackend, Pixmap};

use crate::cache::{blit_cached, CacheStore};
use crate::cadence::RenderCadence;
use crate::region_state::RegionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    #[cfg(feature = "wgpu-backend")]
    Wgpu,
    #[cfg(feature = "hybrid-backend")]
    Hybrid,
}

/// Hint about the consumer's expected workload — feeds [`Backend::auto`].
///
/// All fields are best-effort estimates; the heuristic stays robust
/// even when callers pass approximations. Construct with
/// `WorkloadHint::default()` + `..` updates; extending the struct
/// later means consumers using `..Default::default()` keep compiling.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkloadHint {
    /// Number of regions the engine will hold steady state. Many small
    /// regions (>32) lean toward hybrid; few large ones (<8) lean
    /// toward whichever backend renders the region fastest.
    pub region_count:     u32,
    /// Total pixel count across all regions (sum of `w × h`). Larger
    /// totals lean toward GPU.
    pub total_pixels:     u64,
    /// True if the consumer expects most regions to be re-painted every
    /// frame (high-frequency animation). High-Hz reads-from-GPU lean
    /// toward CPU + small-region cache or full hybrid.
    pub high_hz:          bool,
    /// True if the consumer expects rendering to be retained-mode
    /// (most regions are static or low-Hz). Retained mode is the
    /// ideal case for hybrid (upload once, composite many times).
    pub retained:         bool,
    /// True if the running process has wgpu adapter available. Without
    /// this, GPU/hybrid paths are off-limits. Defaults `false`.
    pub gpu_available:    bool,
    /// True if the host is on Apple Silicon / unified memory. Lowers
    /// the cost of CPU↔GPU transfer so hybrid wins more workloads.
    pub unified_memory:   bool,
}

impl Backend {
    /// Heuristic chooser. Pure function (no GPU init, no IO); callers
    /// pass a [`WorkloadHint`] and get back a recommended backend.
    ///
    /// Algorithm (from `13-arch-plan-2026-06-03.md` §"Decision rules"):
    ///
    /// 1. No GPU → `Cpu`.
    /// 2. <500 draws + LowPower hint → `Cpu` (overhead-bound).
    /// 3. Many regions retained-mode + GPU → `Hybrid` (cache once,
    ///    composite N).
    /// 4. High-Hz huge area + GPU → `Wgpu`.
    /// 5. Anything else with GPU → `Hybrid` (the safe default — CPU
    ///    raster + GPU composite handles every primitive type with
    ///    bounded cost on either side).
    ///
    /// Returns the same value when feature gating disables a path
    /// (e.g. `hybrid-backend` off → falls through to `Wgpu`/`Cpu`).
    pub fn auto(hint: WorkloadHint) -> Self {
        if !hint.gpu_available {
            return Backend::Cpu;
        }
        // Small workloads: GPU overhead dominates. Stay on CPU.
        if hint.region_count < 4 && hint.total_pixels < 100_000 {
            return Backend::Cpu;
        }
        // Retained-mode + GPU = hybrid sweet spot.
        #[cfg(feature = "hybrid-backend")]
        if hint.retained && !hint.high_hz {
            return Backend::Hybrid;
        }
        // Apple-Silicon-style unified memory: hybrid wins everywhere
        // (zero-copy upload makes it dominate even high-Hz scenes).
        #[cfg(feature = "hybrid-backend")]
        if hint.unified_memory {
            return Backend::Hybrid;
        }
        // Heavy high-Hz GPU-direct = wgpu instanced.
        #[cfg(feature = "wgpu-backend")]
        if hint.high_hz && hint.total_pixels > 1_000_000 {
            return Backend::Wgpu;
        }
        // Default with GPU available: hybrid (safest general-purpose).
        #[cfg(feature = "hybrid-backend")]
        { return Backend::Hybrid; }
        #[cfg(all(feature = "wgpu-backend", not(feature = "hybrid-backend")))]
        { return Backend::Wgpu; }
        #[cfg(not(any(feature = "wgpu-backend", feature = "hybrid-backend")))]
        { Backend::Cpu }
    }
}

/// Caller-supplied render target. Backend-specific — caller picks
/// which one matches the engine's `Backend`.
pub enum RenderTarget<'a> {
    /// CPU pixmap. Engine writes into it via `CpuBackend`.
    Cpu(&'a mut Pixmap),
    /// WGPU instanced render context. Engine writes draw commands
    /// into it via the urx-wgpu adapter; caller must then run
    /// `InstancedRenderer::render(commands)` + submit_frame from hub.
    #[cfg(feature = "wgpu-backend")]
    Wgpu(&'a mut uzor_urx_wgpu::InstancedRenderContext),
    /// Hybrid: CPU rasterises every dirty region into scratch pixmaps,
    /// uploads them to `HybridBackend` as GPU textures, then composites
    /// them onto `target_view` via the hybrid pipeline. Caller supplies
    /// the wgpu device/queue/encoder + the target view + screen size.
    #[cfg(feature = "hybrid-backend")]
    Hybrid {
        backend:        &'a mut uzor_urx_hybrid::HybridBackend,
        device:         &'a wgpu::Device,
        queue:          &'a wgpu::Queue,
        encoder:        &'a mut wgpu::CommandEncoder,
        target_view:    &'a wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        screen_w:       u32,
        screen_h:       u32,
    },
}

/// The URX engine. ONE per window. Owns region state + dirty tracking;
/// dispatches to a backend on `render()`.
pub struct UrxEngine {
    backend:    Backend,
    width:      u32,
    height:     u32,
    /// BTreeMap so iteration order is stable (RegionId-sorted painter
    /// order). Consumers can override Z by reading and re-upserting
    /// regions in the order they want.
    regions:    BTreeMap<RegionId, RegionState>,
    /// Union of all regions' bounds whose DirtyState != Clean, since
    /// the last successful render(). Driver reads via needs_paint().
    dirty:      DirtyRect,
    /// Retained-mode cache (Phase 6). Holds CPU pixmaps for regions
    /// whose cadence `allows_cache()`. Cleared on DPR change. Subject
    /// to LRU eviction when total bytes > budget.
    cache:      CacheStore,
    /// Monotonic clock anchor for LRU timestamps. Engines that prefer
    /// a wall-clock source can override via `set_clock_anchor`.
    started:    Instant,
    /// Counters surfaced via RenderStats per call.
    last_cache_hits:   u32,
    last_cache_misses: u32,
}

impl UrxEngine {
    /// Construct an engine bound to a window size + backend.
    pub fn new(backend: Backend, width: u32, height: u32) -> Self {
        Self {
            backend,
            width,
            height,
            regions: BTreeMap::new(),
            dirty:   DirtyRect::EMPTY,
            cache:   CacheStore::new(),
            started: Instant::now(),
            last_cache_hits:   0,
            last_cache_misses: 0,
        }
    }

    /// Set the cache memory budget in bytes (default 64 MB).
    pub fn set_cache_budget(&mut self, bytes: u64) { self.cache.set_budget(bytes); }
    pub fn cache_bytes(&self) -> u64 { self.cache.total_bytes() }
    pub fn cache_count(&self) -> usize { self.cache.count() }

    /// Convenience: CPU engine.
    pub fn new_cpu(width: u32, height: u32) -> Self {
        Self::new(Backend::Cpu, width, height)
    }

    /// Convenience: WGPU engine.
    #[cfg(feature = "wgpu-backend")]
    pub fn new_wgpu(width: u32, height: u32) -> Self {
        Self::new(Backend::Wgpu, width, height)
    }

    /// Convenience: Hybrid engine (CPU raster + GPU composite).
    #[cfg(feature = "hybrid-backend")]
    pub fn new_hybrid(width: u32, height: u32) -> Self {
        Self::new(Backend::Hybrid, width, height)
    }

    pub fn backend(&self) -> Backend { self.backend }
    pub fn region_count(&self) -> usize { self.regions.len() }

    /// Insert or replace a region. Marks `Content` dirty so the next
    /// render touches it. If only the scene's logical bounds (not its
    /// content) changed, you can call `set_bounds()` instead — the
    /// engine then marks `TransformOnly` and skips re-raster.
    pub fn upsert_region(
        &mut self,
        id:      RegionId,
        scene:   Scene,
        bounds:  Rect,
        cadence: RenderCadence,
    ) {
        // Add the old bounds (if any) and new bounds both to dirty
        // union so the previous region's footprint is also re-cleared.
        if let Some(old) = self.regions.get(&id) {
            self.dirty.add(old.bounds);
        }
        self.dirty.add(bounds);
        self.regions.insert(id, RegionState::new(scene, bounds, cadence));
    }

    /// Remove a region. Marks the region's bounds dirty so the next
    /// render clears its footprint.
    pub fn remove_region(&mut self, id: RegionId) {
        if let Some(old) = self.regions.remove(&id) {
            self.dirty.add(old.bounds);
        }
        self.cache.remove(id);
    }

    /// Mark a region as `Content`-dirty (re-rasterise needed).
    pub fn mark_dirty(&mut self, id: RegionId) {
        if let Some(rs) = self.regions.get_mut(&id) {
            rs.dirty.promote_to_content();
            self.dirty.add(rs.bounds);
        }
    }

    /// Mark a region as `TransformOnly`-dirty (composite at new
    /// transform, NO re-raster).
    pub fn mark_transform_only(&mut self, id: RegionId, new_transform: Affine) {
        if let Some(rs) = self.regions.get_mut(&id) {
            rs.transform = new_transform;
            rs.dirty.promote_to_transform();
            self.dirty.add(rs.bounds);
        }
    }

    /// Driver-pollable cadence hook. Returns `Some(union)` when at
    /// least one region is dirty since the last successful render;
    /// returns `None` for a fully-clean frame (driver SHOULD skip).
    pub fn needs_paint(&self) -> Option<DirtyRect> {
        if self.dirty.is_empty() { None } else { Some(self.dirty) }
    }

    /// Forcibly mark every region dirty (e.g. on DPR change or window
    /// resize). Caller then needs_paint() returns the whole window.
    /// ALSO clears the retained cache — cached pixels were rasterised
    /// at the old DPR/size and would be stale.
    pub fn invalidate_all(&mut self) {
        for rs in self.regions.values_mut() {
            rs.dirty.promote_to_content();
        }
        self.dirty.add(Rect::new(0.0, 0.0, self.width as f64, self.height as f64));
        self.cache.clear();
    }

    /// Render every dirty region into the supplied target.
    ///
    /// Backend must match: pass `RenderTarget::Cpu(pixmap)` for a CPU
    /// engine, `RenderTarget::Wgpu(ctx)` for a WGPU engine. Mismatch
    /// returns an error.
    pub fn render(&mut self, target: RenderTarget<'_>) -> Result<RenderStats, RenderError> {
        use uzor_urx_core::metrics_keys::{
            KEY_REGION_DIRTY_COUNT, KEY_REGION_CLEAN_COUNT, KEY_REGION_TRANSFORM_ONLY,
            KEY_TICK_REGION_PASSES_US, KEY_TICK_TOTAL_US, KEY_TICK_FRAMES,
            KEY_TICK_WINDOWS_PAINTED,
        };

        let t_total = std::time::Instant::now();

        let mut dirty_n = 0u32;
        let mut clean_n = 0u32;
        let mut xform_n = 0u32;
        for rs in self.regions.values() {
            match rs.dirty {
                DirtyState::Clean         => clean_n += 1,
                DirtyState::TransformOnly => xform_n += 1,
                DirtyState::Content       => dirty_n += 1,
            }
        }
        metrics::gauge!(KEY_REGION_DIRTY_COUNT).set(dirty_n as f64);
        metrics::gauge!(KEY_REGION_CLEAN_COUNT).set(clean_n as f64);
        metrics::gauge!(KEY_REGION_TRANSFORM_ONLY).set(xform_n as f64);

        let now_us = self.started.elapsed().as_micros() as u64;
        let mut cache_hits   = 0u32;
        let mut cache_misses = 0u32;

        match (target, self.backend) {
            (RenderTarget::Cpu(pixmap), Backend::Cpu) => {
                let cpu = CpuBackend::new();
                for (id, rs) in self.regions.iter_mut() {
                    if rs.dirty == DirtyState::Clean { continue; }

                    let allows_cache = rs.cadence.allows_cache();
                    let needs_raster = rs.dirty.needs_raster();

                    // Cache hit path: TransformOnly + cached + cacheable
                    // → blit the cached pixmap into the target at the
                    // region's translate offset. Cheap as one memcpy.
                    if !needs_raster && allows_cache {
                        if let Some(entry) = self.cache.get(*id) {
                            let (ox, oy) = decompose_translate(&rs.transform);
                            let dst_x = (rs.bounds.x0 + ox).round() as i64;
                            let dst_y = (rs.bounds.y0 + oy).round() as i64;
                            blit_cached(&entry.pixmap, pixmap, dst_x, dst_y);
                            self.cache.touch(*id, now_us);
                            cache_hits += 1;
                            rs.dirty = DirtyState::Clean;
                            metrics::counter!(uzor_urx_core::metrics_keys::KEY_CACHE_HIT).increment(1);
                            continue;
                        }
                    }

                    // Cache miss / re-raster: walk the scene. If
                    // cacheable, rasterise into a scratch pixmap of
                    // region-bounds size, blit to target, then store
                    // the scratch pixmap as the new cache entry. If
                    // NOT cacheable, raster straight into the target.
                    if allows_cache {
                        let rw = rs.bounds.width().max(1.0).ceil() as u32;
                        let rh = rs.bounds.height().max(1.0).ceil() as u32;
                        let mut scratch = Pixmap::new(rw, rh);
                        // Translate scene by -bounds.x0 / -bounds.y0
                        // so it renders at (0,0) of the scratch.
                        let mut shifted = rs.scene.clone();
                        shift_scene_origin(&mut shifted, -rs.bounds.x0, -rs.bounds.y0);
                        cpu.render(&shifted, &mut scratch)
                            .map_err(|_| RenderError::BackendFailed)?;
                        // Blit scratch into the real target.
                        let (ox, oy) = decompose_translate(&rs.transform);
                        let dst_x = (rs.bounds.x0 + ox).round() as i64;
                        let dst_y = (rs.bounds.y0 + oy).round() as i64;
                        blit_cached(&scratch, pixmap, dst_x, dst_y);
                        // Store as cache.
                        let key = CacheKey {
                            region:    *id,
                            dpr_x1k:   1000,
                            logical_w: rw,
                            logical_h: rh,
                        };
                        self.cache.insert(*id, key, scratch, now_us);
                        cache_misses += 1;
                        metrics::counter!(uzor_urx_core::metrics_keys::KEY_CACHE_MISS).increment(1);
                    } else {
                        // HighHz / forced → direct raster, no cache.
                        cpu.render(&rs.scene, pixmap)
                            .map_err(|_| RenderError::BackendFailed)?;
                    }
                    rs.dirty = DirtyState::Clean;
                }
            }
            #[cfg(feature = "wgpu-backend")]
            (RenderTarget::Wgpu(ctx), Backend::Wgpu) => {
                for rs in self.regions.values_mut() {
                    if rs.dirty == DirtyState::Clean { continue; }
                    uzor_urx_wgpu::adapt_scene_into(&rs.scene, ctx);
                    rs.dirty = DirtyState::Clean;
                }
            }
            #[cfg(feature = "hybrid-backend")]
            (RenderTarget::Hybrid {
                backend, device, queue, encoder, target_view,
                surface_format, screen_w, screen_h,
            }, Backend::Hybrid) => {
                let cpu = CpuBackend::new();
                let mut instances: Vec<(RegionId, uzor_urx_hybrid::QuadInstance)>
                    = Vec::with_capacity(self.regions.len());
                for (id, rs) in self.regions.iter_mut() {
                    if rs.dirty == DirtyState::Clean {
                        // Still need to composite previously-cached
                        // regions, but only if their texture is up to
                        // date. Push their QuadInstance.
                        let (ox, oy) = decompose_translate(&rs.transform);
                        let dst_x = rs.bounds.x0 + ox;
                        let dst_y = rs.bounds.y0 + oy;
                        let rw = rs.bounds.width()  as f32;
                        let rh = rs.bounds.height() as f32;
                        instances.push((*id, uzor_urx_hybrid::QuadInstance::new(
                            dst_x as f32, dst_y as f32, rw, rh,
                        )));
                        continue;
                    }
                    let needs_raster = rs.dirty.needs_raster();
                    let rw = rs.bounds.width().max(1.0).ceil() as u32;
                    let rh = rs.bounds.height().max(1.0).ceil() as u32;
                    if needs_raster {
                        let mut scratch = Pixmap::new(rw, rh);
                        let mut shifted = rs.scene.clone();
                        shift_scene_origin(&mut shifted, -rs.bounds.x0, -rs.bounds.y0);
                        cpu.render(&shifted, &mut scratch)
                            .map_err(|_| RenderError::BackendFailed)?;
                        backend.upsert_region_pixmap(device, queue, *id, &scratch);
                        cache_misses += 1;
                    } else {
                        // TransformOnly: GPU texture already correct,
                        // just composite at new offset.
                        cache_hits += 1;
                    }
                    let (ox, oy) = decompose_translate(&rs.transform);
                    let dst_x = rs.bounds.x0 + ox;
                    let dst_y = rs.bounds.y0 + oy;
                    instances.push((*id, uzor_urx_hybrid::QuadInstance::new(
                        dst_x as f32, dst_y as f32, rw as f32, rh as f32,
                    )));
                    rs.dirty = DirtyState::Clean;
                }
                if !instances.is_empty() {
                    backend.composite(
                        device, queue, encoder, target_view,
                        surface_format, screen_w, screen_h, &instances,
                    );
                }
            }
            _ => return Err(RenderError::BackendMismatch),
        }

        // Successful render → clear the union.
        let stats = RenderStats {
            regions_total: self.regions.len() as u32,
            regions_dirty: dirty_n,
            regions_clean: clean_n,
            regions_transform_only: xform_n,
            cache_hits,
            cache_misses,
            elapsed_us: t_total.elapsed().as_micros() as u64,
        };
        self.last_cache_hits   = cache_hits;
        self.last_cache_misses = cache_misses;
        self.dirty.reset();
        metrics::histogram!(KEY_TICK_REGION_PASSES_US).record(stats.elapsed_us as f64);
        metrics::histogram!(KEY_TICK_TOTAL_US).record(stats.elapsed_us as f64);
        metrics::counter!(KEY_TICK_FRAMES).increment(1);
        if dirty_n > 0 || xform_n > 0 {
            metrics::counter!(KEY_TICK_WINDOWS_PAINTED).increment(1);
        }
        Ok(stats)
    }
}

/// Pull the translation component out of an Affine for cached blit
/// offsetting. Returns (tx, ty). Scale/rotation are ignored — the
/// cache layer only handles translation in Phase 6; non-translation
/// transforms degrade to direct re-raster.
#[inline]
fn decompose_translate(a: &Affine) -> (f64, f64) {
    let c = a.as_coeffs();
    (c[4], c[5])
}

/// Shift every primitive in a scene by (dx, dy). Used when rasterising
/// a region into a region-bounds-sized scratch pixmap — the original
/// scene is in window-space, but the scratch is region-local.
fn shift_scene_origin(scene: &mut Scene, dx: f64, dy: f64) {
    use uzor_urx_core::scene::DrawCommand;
    use uzor_urx_core::math::{Rect, RoundedRect, Vec2};
    let shift = uzor_urx_core::math::Affine::translate((dx, dy));
    for cmd in scene.commands.iter_mut() {
        match cmd {
            DrawCommand::FillRect   { rect, .. } |
            DrawCommand::StrokeRect { rect, .. } => {
                *rect = Rect::new(rect.x0 + dx, rect.y0 + dy, rect.x1 + dx, rect.y1 + dy);
            }
            DrawCommand::Line { from, to, .. } => {
                *from = Vec2 { x: from.x + dx, y: from.y + dy };
                *to   = Vec2 { x: to.x   + dx, y: to.y   + dy };
            }
            DrawCommand::FillPath { transform, .. } |
            DrawCommand::StrokePath { transform, .. } => {
                // Premultiply the shift into the path's transform —
                // the path coordinates stay as the consumer wrote them,
                // but the effective screen-position drops by (dx, dy).
                *transform = shift * *transform;
            }
            DrawCommand::PushClipRect { rect, .. } => {
                *rect = Rect::new(rect.x0 + dx, rect.y0 + dy, rect.x1 + dx, rect.y1 + dy);
            }
            DrawCommand::PushClipRoundedRect { rect, .. } => {
                let r = rect.rect();
                *rect = RoundedRect::new(r.x0 + dx, r.y0 + dy, r.x1 + dx, r.y1 + dy, rect.radii());
            }
            DrawCommand::GlyphRun { transform, .. } | DrawCommand::Image { transform, .. } => {
                *transform = shift * *transform;
            }
            DrawCommand::PopClip => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderStats {
    pub regions_total:          u32,
    pub regions_dirty:          u32,
    pub regions_clean:          u32,
    pub regions_transform_only: u32,
    pub cache_hits:             u32,
    pub cache_misses:           u32,
    pub elapsed_us:             u64,
}

#[derive(Debug, Clone, Copy)]
pub enum RenderError {
    BackendMismatch,
    BackendFailed,
}
