//! `UrxEngine` — cross-backend façade.

use std::collections::BTreeMap;
use std::time::Instant;

use uzor_urx_core::config::UrxConfig;
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
    /// URX 1.6 full-GPU compute pipeline (encode → tile_assign →
    /// tile_sort → fine → blit). Wired into `render()` via
    /// [`RenderTarget::FullGpu`]; selected by [`Backend::auto`] when
    /// `WorkloadHint::heavy_compute` is set.
    #[cfg(feature = "full-gpu-backend")]
    FullGpu,
    /// Wave 9 (urx-3d session) — per-region mixed backend dispatch.
    /// Engine routes each region through its own `BackendHint` and
    /// composites the per-region intermediate textures onto the
    /// swapchain in one final pass. See `BackendHint` + [`RegionMixer`]
    /// for the per-region routing rules.
    ///
    /// Construction: `UrxEngine::new(Backend::Mixed, w, h)`. Mixed
    /// backend is currently a SCAFFOLD — `render()` accepts a
    /// [`RenderTarget::Mixed`] but defers actual dispatch to the
    /// consumer-supplied [`RegionMixer`]. The contract + design lives
    /// in `crate::mixer`; consumer must drive the mix loop directly
    /// until Wave 9b ships the in-engine mixer implementation.
    Mixed,
}

/// Per-region backend routing hint (Wave 9). Engine reads this on
/// every `render()` call to a `RenderTarget::Mixed` target; the mixer
/// dispatches each region through the indicated backend.
///
/// This is INTENT — the engine still has to have a concrete
/// [`Backend`] enum value for the global construction. Mixed mode
/// reads the per-region `BackendHint` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendHint {
    /// Force CPU rasterisation for this region. Right for static UI
    /// chrome, borders, button bands, anything cache-friendly that
    /// would waste GPU bandwidth.
    Cpu,
    /// Force GPU compute pipeline (URX 1.6). Right for heavy realtime
    /// regions — charts, particle effects, 5000+ primitive scenes.
    FullGpu,
    /// Force Hybrid (CPU rasterise → GPU composite). Right for
    /// retained-mode regions with many small primitives.
    Hybrid,
    /// Default — engine picks per its global construction backend.
    /// Same as leaving the hint unset.
    Inherit,
}

impl Default for BackendHint {
    fn default() -> Self {
        Self::Inherit
    }
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
    /// Hint that the workload is dense draw-call-heavy (>5000 prims/frame
    /// across all regions, no per-region retained cache). Selects the
    /// URX 1.6 full-GPU compute pipeline when `full-gpu-backend` is on —
    /// it eats large cmd counts cheaper than CPU raster + GPU composite
    /// (13× win at N=10k on RTX 4060 Ti). Requires `gpu_available`.
    pub heavy_compute:    bool,
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
        // Dense draw-call-heavy + GPU → URX 1.6 compute pipeline.
        // 13× win vs CPU at N=10k; cmd count grows linearly so
        // beats per-region raster once the cmd density is high.
        #[cfg(feature = "full-gpu-backend")]
        if hint.heavy_compute {
            return Backend::FullGpu;
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
    /// FullGpu: URX 1.6 compute pipeline. Engine walks dirty regions,
    /// encodes each scene → `Vec<SceneCmd>`, concatenates into one
    /// dispatch, runs assign → sort → fine → blit into `target_view`.
    ///
    /// Caller owns:
    /// - `pipeline` — compiled `TilePipeline` (one per device)
    /// - `blit`     — compiled `BlitPipeline` whose format matches
    ///                `target_view`'s texture format
    /// - `bufs`     — `TileBuffers` sized for the screen; if cmd count
    ///                exceeds `bufs.cmds_buf` capacity the engine
    ///                returns `RenderError::CmdBufferTooSmall` and the
    ///                caller should re-allocate.
    /// - `storage_view` — the rgba8unorm storage texture view that
    ///                `bufs` was created with (via `with_output_texture`)
    /// - `target_view` — final render-attachment-compatible view
    /// - `glyph_atlas_view` — R8Unorm view; pass the value from
    ///                `TilePipeline::dummy_glyph_atlas` when no glyphs
    /// - `image_atlas_view` — RGBA8 view; pass the value from
    ///                `TilePipeline::dummy_image_atlas` when no images
    /// - `src_w`/`src_h` — padded tile-aligned dims (`tile_count_x*16`)
    #[cfg(feature = "full-gpu-backend")]
    FullGpu {
        pipeline:         &'a uzor_urx_wgpu_full::TilePipeline,
        blit:             &'a uzor_urx_wgpu_full::BlitPipeline,
        bufs:             &'a uzor_urx_wgpu_full::TileBuffers,
        device:           &'a wgpu::Device,
        queue:            &'a wgpu::Queue,
        encoder:          &'a mut wgpu::CommandEncoder,
        storage_view:     &'a wgpu::TextureView,
        target_view:      &'a wgpu::TextureView,
        glyph_atlas_view: &'a wgpu::TextureView,
        image_atlas_view: &'a wgpu::TextureView,
        src_w:            u32,
        src_h:            u32,
    },
    /// Wave 9 — Mixed: caller supplies a per-region dispatcher.
    /// Engine iterates regions in RegionId order, hands each
    /// (RegionId, bounds, BackendHint, &Scene) to the dispatcher's
    /// callback. The dispatcher is responsible for invoking the
    /// concrete backend per region and compositing onto the final
    /// swapchain.
    ///
    /// This is intentionally a SCAFFOLD: the engine guarantees stable
    /// iteration order + correct dirty bookkeeping; the dispatcher
    /// owns intermediate-texture management, recipe-per-backend, and
    /// composite. Future Wave 9b will move the recipe + composite
    /// into the engine itself.
    Mixed {
        dispatcher: &'a mut dyn MixDispatcher,
    },
}

/// Callback the consumer provides for `RenderTarget::Mixed`. The
/// engine calls `dispatch` once per region in stable RegionId order;
/// the consumer routes the region to its concrete backend (CPU
/// pixmap / FullGpu / Hybrid / etc.) and accumulates results onto
/// the final swapchain.
pub trait MixDispatcher {
    fn dispatch(
        &mut self,
        id: RegionId,
        bounds: Rect,
        hint: BackendHint,
        scene: &Scene,
    );
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
    /// Process-wide config; stored so callers can read it back and so
    /// the cache budget survives across `invalidate_all()` calls.
    config:     UrxConfig,
}

impl UrxEngine {
    /// Construct an engine with [`UrxConfig::default`] configuration.
    /// Cache budget is set to `config.region_cache_budget_bytes` (64 MiB).
    pub fn new(backend: Backend, width: u32, height: u32) -> Self {
        Self::new_with_config(backend, width, height, UrxConfig::default())
    }

    /// Construct an engine with explicit configuration. The cache budget
    /// is taken from `config.region_cache_budget_bytes` immediately.
    pub fn new_with_config(backend: Backend, width: u32, height: u32, config: UrxConfig) -> Self {
        let mut cache = CacheStore::new();
        cache.set_budget(config.region_cache_budget_bytes);
        Self {
            backend,
            width,
            height,
            regions: BTreeMap::new(),
            dirty:   DirtyRect::EMPTY,
            cache,
            started: Instant::now(),
            last_cache_hits:   0,
            last_cache_misses: 0,
            config,
        }
    }

    /// Set the cache memory budget in bytes (default 64 MB).
    /// Also updates the stored config so `config()` stays in sync.
    pub fn set_cache_budget(&mut self, bytes: u64) {
        self.config.region_cache_budget_bytes = bytes;
        self.cache.set_budget(bytes);
    }
    pub fn cache_bytes(&self) -> u64 { self.cache.total_bytes() }
    pub fn cache_count(&self) -> usize { self.cache.count() }

    /// Read-only access to the engine's config.
    pub fn config(&self) -> &UrxConfig { &self.config }

    /// Convenience: CPU engine with default config.
    pub fn new_cpu(width: u32, height: u32) -> Self {
        Self::new(Backend::Cpu, width, height)
    }

    /// Convenience: CPU engine with explicit config.
    pub fn new_cpu_with_config(width: u32, height: u32, config: UrxConfig) -> Self {
        Self::new_with_config(Backend::Cpu, width, height, config)
    }

    /// Convenience: WGPU engine with default config.
    #[cfg(feature = "wgpu-backend")]
    pub fn new_wgpu(width: u32, height: u32) -> Self {
        Self::new(Backend::Wgpu, width, height)
    }

    /// Convenience: WGPU engine with explicit config.
    #[cfg(feature = "wgpu-backend")]
    pub fn new_wgpu_with_config(width: u32, height: u32, config: UrxConfig) -> Self {
        Self::new_with_config(Backend::Wgpu, width, height, config)
    }

    /// Convenience: Hybrid engine (CPU raster + GPU composite) with default config.
    #[cfg(feature = "hybrid-backend")]
    pub fn new_hybrid(width: u32, height: u32) -> Self {
        Self::new(Backend::Hybrid, width, height)
    }

    /// Convenience: Hybrid engine with explicit config.
    #[cfg(feature = "hybrid-backend")]
    pub fn new_hybrid_with_config(width: u32, height: u32, config: UrxConfig) -> Self {
        Self::new_with_config(Backend::Hybrid, width, height, config)
    }

    /// Convenience: FullGpu (URX 1.6 compute) engine with default config.
    #[cfg(feature = "full-gpu-backend")]
    pub fn new_full_gpu(width: u32, height: u32) -> Self {
        Self::new(Backend::FullGpu, width, height)
    }

    /// Convenience: FullGpu engine with explicit config.
    #[cfg(feature = "full-gpu-backend")]
    pub fn new_full_gpu_with_config(width: u32, height: u32, config: UrxConfig) -> Self {
        Self::new_with_config(Backend::FullGpu, width, height, config)
    }

    /// Convenience: Mixed engine (Wave 9 — per-region BackendHint
    /// routing).
    pub fn new_mixed(width: u32, height: u32) -> Self {
        Self::new(Backend::Mixed, width, height)
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

    /// Wave 9 — same as `upsert_region` plus a per-region backend
    /// routing hint. Read by [`RenderTarget::Mixed`] for per-region
    /// dispatch.
    pub fn upsert_region_with_hint(
        &mut self,
        id:      RegionId,
        scene:   Scene,
        bounds:  Rect,
        cadence: RenderCadence,
        hint:    BackendHint,
    ) {
        if let Some(old) = self.regions.get(&id) {
            self.dirty.add(old.bounds);
        }
        self.dirty.add(bounds);
        let mut rs = RegionState::new(scene, bounds, cadence);
        rs.backend_hint = hint;
        self.regions.insert(id, rs);
    }

    /// Set just the backend hint for an existing region. No-op if the
    /// region doesn't exist.
    pub fn set_region_backend_hint(&mut self, id: RegionId, hint: BackendHint) {
        if let Some(rs) = self.regions.get_mut(&id) {
            rs.backend_hint = hint;
        }
    }

    /// Read a region's backend hint, falling back to `Inherit` when the
    /// region doesn't exist.
    pub fn region_backend_hint(&self, id: RegionId) -> BackendHint {
        self.regions
            .get(&id)
            .map(|r| r.backend_hint)
            .unwrap_or(BackendHint::Inherit)
    }

    /// Iterator over (id, bounds, hint, dirty) for all regions, sorted
    /// by RegionId (matches the engine's painter order). Used by
    /// Wave 9 mixers to drive per-region dispatch from outside.
    pub fn regions_for_mix(&self) -> impl Iterator<Item = (RegionId, Rect, BackendHint, &Scene)> + '_ {
        self.regions
            .iter()
            .map(|(&id, rs)| (id, rs.bounds, rs.backend_hint, &rs.scene))
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
            #[cfg(feature = "full-gpu-backend")]
            (RenderTarget::FullGpu {
                pipeline, blit, bufs, device, queue, encoder,
                storage_view, target_view, glyph_atlas_view,
                image_atlas_view,
                src_w, src_h,
            }, Backend::FullGpu) => {
                // FullGpu is a write-only-into-storage path: we walk ALL
                // regions every frame (not just dirty) because the output
                // texture is cleared each blit. There's no per-region
                // cache yet — total cost dominates by the encode/dispatch
                // for the cmd list, not by tracking per-region dirty.
                //
                // Path cmds (kind=6) reference a per-frame `path_points`
                // buffer; the encoder returns both lists from each
                // region. We accumulate them with a running base offset
                // so each Path cmd points at the right slice.
                let mut all_cmds:   Vec<uzor_urx_wgpu_full::SceneCmd> = Vec::new();
                let mut all_points: Vec<[f32; 2]>                     = Vec::new();
                for rs in self.regions.values_mut() {
                    let mut shifted = rs.scene.clone();
                    shift_scene_origin(&mut shifted, rs.bounds.x0, rs.bounds.y0);
                    let base = all_points.len() as u32;
                    let (mut cmds, mut points) =
                        uzor_urx_wgpu_full::encode_scene_with_paths(&shifted, base);
                    all_cmds.append(&mut cmds);
                    all_points.append(&mut points);
                    rs.dirty = DirtyState::Clean;
                }

                let needed = all_cmds.len() as u32;
                let available = (bufs.cmds_buf.size() / 32) as u32;
                if needed > available {
                    return Err(RenderError::CmdBufferTooSmall { needed, available });
                }
                if all_points.len() as u32 > bufs.path_points_cap {
                    return Err(RenderError::PathPointsBufferTooSmall {
                        needed:    all_points.len() as u32,
                        available: bufs.path_points_cap,
                    });
                }

                pipeline.render_to_target(
                    device, queue, encoder, bufs, &all_cmds, &all_points,
                    storage_view, blit, target_view,
                    src_w, src_h, glyph_atlas_view, image_atlas_view,
                );
                cache_misses = self.regions.len() as u32;
            }
            (RenderTarget::Mixed { dispatcher }, Backend::Mixed) => {
                // Wave 9 dispatch — engine guarantees stable RegionId
                // order + dirty bookkeeping; consumer's MixDispatcher
                // owns concrete backend dispatch + composite.
                for (id, rs) in self.regions.iter_mut() {
                    dispatcher.dispatch(*id, rs.bounds, rs.backend_hint, &rs.scene);
                    rs.dirty = DirtyState::Clean;
                }
                cache_misses = self.regions.len() as u32;
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
            DrawCommand::PushBlendLayer { transform, .. } => {
                *transform = shift * *transform;
            }
            DrawCommand::PopBlendLayer => {}
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
    /// FullGpu only: the supplied `TileBuffers::cmds_buf` is too small
    /// for the concatenated cmd list. Caller should re-allocate
    /// `TileBuffers` with `cmds_n >= reported size` and retry.
    CmdBufferTooSmall { needed: u32, available: u32 },
    /// FullGpu only: the scene's flattened paths produced more vertices
    /// than `TileBuffers::path_points_cap`. Re-allocate via
    /// `TileBuffers::allocate_with(..., path_points_cap = N)` and retry.
    PathPointsBufferTooSmall { needed: u32, available: u32 },
}
