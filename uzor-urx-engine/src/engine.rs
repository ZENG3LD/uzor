//! `UrxEngine` — cross-backend façade.

use std::collections::BTreeMap;

use uzor_urx_core::dirty::{DirtyRect, DirtyState};
use uzor_urx_core::math::{Affine, Rect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_cpu::{CpuBackend, Pixmap};

use crate::cadence::RenderCadence;
use crate::region_state::RegionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    #[cfg(feature = "wgpu")]
    Wgpu,
}

/// Caller-supplied render target. Backend-specific — caller picks
/// which one matches the engine's `Backend`.
pub enum RenderTarget<'a> {
    /// CPU pixmap. Engine writes into it via `CpuBackend`.
    Cpu(&'a mut Pixmap),
    /// WGPU instanced render context. Engine writes draw commands
    /// into it via the urx-wgpu adapter; caller must then run
    /// `InstancedRenderer::render(commands)` + submit_frame from hub.
    #[cfg(feature = "wgpu")]
    Wgpu(&'a mut uzor_urx_wgpu::InstancedRenderContext),
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
        }
    }

    /// Convenience: CPU engine.
    pub fn new_cpu(width: u32, height: u32) -> Self {
        Self::new(Backend::Cpu, width, height)
    }

    /// Convenience: WGPU engine.
    #[cfg(feature = "wgpu")]
    pub fn new_wgpu(width: u32, height: u32) -> Self {
        Self::new(Backend::Wgpu, width, height)
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
    pub fn invalidate_all(&mut self) {
        for rs in self.regions.values_mut() {
            rs.dirty.promote_to_content();
        }
        self.dirty.add(Rect::new(0.0, 0.0, self.width as f64, self.height as f64));
    }

    /// Render every dirty region into the supplied target.
    ///
    /// Backend must match: pass `RenderTarget::Cpu(pixmap)` for a CPU
    /// engine, `RenderTarget::Wgpu(ctx)` for a WGPU engine. Mismatch
    /// returns an error.
    pub fn render(&mut self, target: RenderTarget<'_>) -> Result<RenderStats, RenderError> {
        use uzor_urx_core::metrics_keys::{
            KEY_REGION_DIRTY_COUNT, KEY_REGION_CLEAN_COUNT, KEY_REGION_TRANSFORM_ONLY,
            KEY_TICK_REGION_PASSES_US,
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

        match (target, self.backend) {
            (RenderTarget::Cpu(pixmap), Backend::Cpu) => {
                let cpu = CpuBackend::new();
                for rs in self.regions.values_mut() {
                    if rs.dirty == DirtyState::Clean { continue; }
                    // Content OR TransformOnly: for CPU we always
                    // re-raster (cached-region texture composite is
                    // a Phase 6 optimisation). Walk the scene as-is.
                    cpu.render(&rs.scene, pixmap)
                        .map_err(|_| RenderError::BackendFailed)?;
                    rs.dirty = DirtyState::Clean;
                }
            }
            #[cfg(feature = "wgpu")]
            (RenderTarget::Wgpu(ctx), Backend::Wgpu) => {
                for rs in self.regions.values_mut() {
                    if rs.dirty == DirtyState::Clean { continue; }
                    uzor_urx_wgpu::adapt_scene_into(&rs.scene, ctx);
                    rs.dirty = DirtyState::Clean;
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
            elapsed_us: t_total.elapsed().as_micros() as u64,
        };
        self.dirty.reset();
        metrics::histogram!(KEY_TICK_REGION_PASSES_US).record(stats.elapsed_us as f64);
        Ok(stats)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderStats {
    pub regions_total:          u32,
    pub regions_dirty:          u32,
    pub regions_clean:          u32,
    pub regions_transform_only: u32,
    pub elapsed_us:             u64,
}

#[derive(Debug, Clone, Copy)]
pub enum RenderError {
    BackendMismatch,
    BackendFailed,
}
