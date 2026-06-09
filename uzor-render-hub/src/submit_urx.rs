//! URX-family submit paths.
//!
//! Mirrors `submit.rs`'s per-backend dispatch shape: one function per active
//! `RenderBackend` variant. All four URX backends consume the same
//! `urx_core::Scene` produced by `UrxRenderContext` during the frame, so
//! the only difference is which backend rasterises it.
//!
//! **Stage 1a (2026-06-05) — all 4 backends real:**
//! - `UrxCpu` — own scanline rasteriser → pixmap → CPU presenter path
//!   (mirror of `submit_cpu_tinyskia`).
//! - `UrxWgpu` — adapter pushes Scene into the shared
//!   `InstancedRenderContext`, then `InstancedRenderer` draws it onto the
//!   swapchain (same path Scene2DBackend::InstancedWgpu uses).
//! - `UrxHybrid` — CpuBackend rasterises into one region pixmap,
//!   `HybridBackend.upsert_region_pixmap` uploads it as the sole region,
//!   `HybridBackend.composite` blits it to the swapchain.
//! - `UrxWgpuFull` — `WgpuFullBackend.submit` runs the encode → tile_assign
//!   → tile_sort → fine → blit chain.

use uzor_urx_core::Scene;

use crate::factory::{SurfaceMode, WindowRenderState};
use crate::metrics::RenderMetrics;
use crate::submit::SubmitParams;

/// Pull this frame's `Scene` out of the shared URX context. `None` if the
/// consumer never produced anything (e.g. the very first frame before any
/// paint callback fires).
fn take_urx_scene(state: &mut WindowRenderState) -> Option<Scene> {
    state.urx_ctx.as_mut().map(|c| c.take_scene())
}

// ── UrxCpu ──────────────────────────────────────────────────────────────────

pub fn submit_urx_cpu(state: &mut WindowRenderState, metrics: &mut RenderMetrics) -> bool {
    let scene = match take_urx_scene(state) {
        Some(s) => s,
        None => return false,
    };

    // Determine surface size + kind without holding any borrow into state.surface.
    enum SurfaceKind { Gpu, Software, #[cfg(target_arch = "wasm32")] Canvas2d }
    let (width, height, kind) = match &state.surface {
        SurfaceMode::Gpu { surface, .. } => (surface.config.width, surface.config.height, SurfaceKind::Gpu),
        #[cfg(not(target_arch = "wasm32"))]
        SurfaceMode::Software { width, height, .. } => (*width, *height, SurfaceKind::Software),
        #[cfg(target_arch = "wasm32")]
        SurfaceMode::Canvas2d { .. } => return false,
    };
    if width == 0 || height == 0 { return false; }

    // Lazy-init backend + sized pixmap (separate function takes &mut state alone).
    ensure_urx_cpu_resources(state, width, height);

    let r2t_t0 = std::time::Instant::now();

    // Render into the pixmap — borrow backend + pixmap together; backend is
    // immutable so we can hold &state.urx_cpu_backend through pixmap mutate.
    let render_ok = {
        let backend = match state.urx_cpu_backend.as_ref() { Some(b) => b, None => return false };
        let pixmap  = match state.urx_cpu_pixmap.as_mut()   { Some(p) => p, None => return false };
        pixmap.fill([0, 0, 0, 0]);
        match backend.render(&scene, pixmap) {
            Ok(_)  => true,
            Err(e) => { eprintln!("[render-hub] urx-cpu render error: {:?}", e); false }
        }
    };
    if !render_ok { return false; }

    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    // Present (separate borrow scope of surface).
    match kind {
        SurfaceKind::Gpu => {
            // Upload pixels then blit.
            let (pix_ptr, pix_len, cw, ch) = {
                let pixmap = state.urx_cpu_pixmap.as_ref().expect("pixmap inited above");
                let pix = pixmap.pixels();
                (pix.as_ptr(), pix.len(), pixmap.width(), pixmap.height())
            };
            if let SurfaceMode::Gpu { gpu_pool, surface, dev_id } = &mut state.surface {
                let device = &gpu_pool.devices[*dev_id].device;
                let queue  = &gpu_pool.devices[*dev_id].queue;
                // SAFETY: pix_ptr/pix_len describe a borrow of state.urx_cpu_pixmap.
                // We hold &mut state.surface, but pixmap and surface are disjoint
                // fields — the slice is valid for this call.
                let pix: &[u8] = unsafe { std::slice::from_raw_parts(pix_ptr, pix_len) };
                if !pix.is_empty() && cw == width && ch == height {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &surface.target_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        pix,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * cw),
                            rows_per_image: Some(ch),
                        },
                        wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
                    );
                }
                let present_t0 = std::time::Instant::now();
                let lost = crate::submit::blit_and_present_urx(surface, device, queue);
                metrics.present_us = present_t0.elapsed().as_micros() as u64;
                return lost;
            }
            false
        }
        #[cfg(not(target_arch = "wasm32"))]
        SurfaceKind::Software => {
            // Re-borrow disjoint fields.
            let (pix_ptr, pix_len, cw, ch) = {
                let pixmap = state.urx_cpu_pixmap.as_ref().expect("pixmap inited above");
                (pixmap.pixels().as_ptr(), pixmap.pixels().len(), pixmap.width(), pixmap.height())
            };
            if let SurfaceMode::Software { presenter, .. } = &mut state.surface {
                let pix: &[u8] = unsafe { std::slice::from_raw_parts(pix_ptr, pix_len) };
                presenter.present(pix, cw, ch);
            }
            false
        }
        #[cfg(target_arch = "wasm32")]
        SurfaceKind::Canvas2d => false,
    }
}

fn ensure_urx_cpu_resources(state: &mut WindowRenderState, width: u32, height: u32) {
    if state.urx_cpu_backend.is_none() {
        state.urx_cpu_backend = Some(uzor_urx_cpu::CpuBackend::new());
    }
    let need_new = match &state.urx_cpu_pixmap {
        None => true,
        Some(p) => p.width() != width || p.height() != height,
    };
    if need_new {
        state.urx_cpu_pixmap = Some(uzor_urx_cpu::Pixmap::new(width, height));
    }
}

// ── UrxWgpu ────────────────────────────────────────────────────────────────
//
// Stage 1a wires the urx-wgpu adapter (Scene → InstancedRenderContext →
// InstancedRenderer) into the same GPU swapchain path InstancedWgpu uses.
// We share the `state.instanced_renderer` / `state.instanced_ctx` slots —
// each frame the URX channel's Scene is adapted into the ctx's
// `draw_commands`, then the same renderer draws them.

pub fn submit_urx_wgpu(
    state: &mut WindowRenderState,
    params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> bool {
    let scene = match take_urx_scene(state) {
        Some(s) => s,
        None => return false,
    };

    let SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } = state.surface else {
        eprintln!("[render-hub] urx_wgpu requires SurfaceMode::Gpu");
        return false;
    };
    let width  = surface.config.width;
    let height = surface.config.height;
    if width == 0 || height == 0 { return false; }

    let device = &gpu_pool.devices[dev_id].device;
    let queue  = &gpu_pool.devices[dev_id].queue;

    let surface_texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            surface.surface.configure(device, &surface.config);
            return false;
        }
        wgpu::CurrentSurfaceTexture::Validation => return false,
    };
    let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let format = surface_texture.texture.format();

    // Lazy-init InstancedRenderer (shared with the Scene2DBackend::InstancedWgpu path).
    if state.instanced_renderer.is_none() {
        state.instanced_renderer =
            Some(uzor_render_wgpu_instanced::InstancedRenderer::new(device, queue, format));
    }
    // Lazy-init the per-frame ctx (the adapter writes into its draw_commands).
    if state.instanced_ctx.is_none() {
        state.instanced_ctx = Some(uzor_render_wgpu_instanced::InstancedRenderContext::new(
            width as f32, height as f32, 0.0, 0.0,
        ));
    }
    // Run the adapter: Scene → InstancedRenderContext draw_commands.
    let r2t_t0 = std::time::Instant::now();
    if let Some(ctx) = state.instanced_ctx.as_mut() {
        ctx.clear();
        uzor_urx_wgpu::adapt_scene_into(&scene, ctx);
    }
    let cmds: Vec<uzor_render_wgpu_instanced::DrawCmd> = state.instanced_ctx.as_mut()
        .map(|c| std::mem::take(&mut c.draw_commands))
        .unwrap_or_default();

    let clear = wgpu::Color {
        r: params.base_color.components[0] as f64,
        g: params.base_color.components[1] as f64,
        b: params.base_color.components[2] as f64,
        a: params.base_color.components[3] as f64,
    };
    if let Some(ref mut inst) = state.instanced_renderer {
        inst.render(device, queue, &surface_view, width, height, &cmds, Some(clear), None);
    }
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;
    // Return the Vec to the ctx so its capacity is reused.
    if let Some(ctx) = state.instanced_ctx.as_mut() {
        let mut taken = cmds;
        taken.clear();
        ctx.draw_commands = taken;
    }

    let present_t0 = std::time::Instant::now();
    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    false
}

// ── UrxHybrid ──────────────────────────────────────────────────────────────
//
// HybridBackend = CPU strip raster + GPU atlas + quad compositor. Stage 1a
// runs it in single-region mode (one region = whole window). The whole
// Scene is CPU-rasterised into one Pixmap (via CpuBackend, since the URX
// hybrid backend doesn't yet own a CPU rasteriser of its own — that's a
// Stage 3 refactor when regions land), uploaded as the sole region
// texture, then composited.

// ── UrxRegions (U2 Wave A, 2026-06-10) ─────────────────────────────────────
//
// Retained-mode submit: drives the `UrxEngine`'s dirty-region pipeline
// instead of consuming the immediate `urx_ctx` scene. The consumer
// (tessera-window region walker) has already `upsert_region`'d this
// frame's changed regions; this fn:
//   1. `engine.needs_paint()` — `None` → return `skipped` WITHOUT
//      acquiring the swapchain. That early-out is the whole point of
//      retained mode: a fully-static window costs ~nothing.
//   2. `engine.render(RenderTarget::Cpu(&mut pixmap))` — re-rasterises
//      ONLY dirty regions into the PERSISTENT pixmap (the same pixmap
//      lives across frames, so clean regions keep their pixels).
//   3. upload + blit + present (same plumbing as `submit_urx_cpu`).
//
// Wave A rasterises regions on CPU regardless of `active_urx` — the
// pixmap-persistence model is what makes dirty-skip correct. Engine
// Wgpu/Hybrid render-targets are a follow-up (they need clean-region
// re-emit semantics verified first).

/// Outcome of [`submit_urx_regions`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RegionSubmitOutcome {
    /// `true` when `needs_paint()` returned `None` — nothing dirty,
    /// no GPU work was done, the swapchain was NOT touched.
    pub skipped: bool,
    /// `true` when the surface was lost (caller should retry next frame).
    pub surface_lost: bool,
    /// Regions re-rasterised this call (0 when skipped).
    pub dirty_regions: u32,
}

pub fn submit_urx_regions(
    state: &mut WindowRenderState,
    _params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> RegionSubmitOutcome {
    // Engine must exist (the region walker lazy-inits it via
    // `with_urx_engine` before calling us).
    let Some(engine) = state.urx_engine.as_mut() else {
        return RegionSubmitOutcome::default();
    };

    // 1. Frame skip — the retained-mode payoff.
    if engine.needs_paint().is_none() {
        return RegionSubmitOutcome { skipped: true, ..Default::default() };
    }

    // Resolve surface size.
    let (width, height) = match &state.surface {
        SurfaceMode::Gpu { surface, .. } => (surface.config.width, surface.config.height),
        #[cfg(not(target_arch = "wasm32"))]
        SurfaceMode::Software { width, height, .. } => (*width, *height),
        #[cfg(target_arch = "wasm32")]
        SurfaceMode::Canvas2d { .. } => return RegionSubmitOutcome::default(),
    };
    if width == 0 || height == 0 { return RegionSubmitOutcome::default(); }

    // 2. Persistent pixmap — clean regions keep their pixels across
    // frames; a size change forces a full re-rasterise.
    let need_new = match &state.urx_cpu_pixmap {
        None => true,
        Some(p) => p.width() != width || p.height() != height,
    };
    if need_new {
        state.urx_cpu_pixmap = Some(uzor_urx_cpu::Pixmap::new(width, height));
        if let Some(engine) = state.urx_engine.as_mut() {
            engine.invalidate_all();
        }
    }

    let r2t_t0 = std::time::Instant::now();
    let dirty_regions = {
        let engine = state.urx_engine.as_mut().expect("checked above");
        let pixmap = state.urx_cpu_pixmap.as_mut().expect("inited above");
        match engine.render(uzor_urx_engine::RenderTarget::Cpu(pixmap)) {
            Ok(stats) => stats.regions_dirty,
            Err(e) => {
                eprintln!("[render-hub] urx region render error: {:?}", e);
                return RegionSubmitOutcome::default();
            }
        }
    };
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    // 3. Present — same upload/blit path as submit_urx_cpu.
    let (pix_ptr, pix_len, cw, ch) = {
        let pixmap = state.urx_cpu_pixmap.as_ref().expect("inited above");
        let pix = pixmap.pixels();
        (pix.as_ptr(), pix.len(), pixmap.width(), pixmap.height())
    };
    match &mut state.surface {
        SurfaceMode::Gpu { gpu_pool, surface, dev_id } => {
            let device = &gpu_pool.devices[*dev_id].device;
            let queue  = &gpu_pool.devices[*dev_id].queue;
            // SAFETY: disjoint fields — pixmap outlives this borrow of surface.
            let pix: &[u8] = unsafe { std::slice::from_raw_parts(pix_ptr, pix_len) };
            if !pix.is_empty() && cw == width && ch == height {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &surface.target_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    pix,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * cw),
                        rows_per_image: Some(ch),
                    },
                    wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
                );
            }
            let present_t0 = std::time::Instant::now();
            let lost = crate::submit::blit_and_present_urx(surface, device, queue);
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
            RegionSubmitOutcome { skipped: false, surface_lost: lost, dirty_regions }
        }
        #[cfg(not(target_arch = "wasm32"))]
        SurfaceMode::Software { presenter, .. } => {
            let pix: &[u8] = unsafe { std::slice::from_raw_parts(pix_ptr, pix_len) };
            presenter.present(pix, cw, ch);
            RegionSubmitOutcome { skipped: false, surface_lost: false, dirty_regions }
        }
        #[cfg(target_arch = "wasm32")]
        SurfaceMode::Canvas2d { .. } => RegionSubmitOutcome::default(),
    }
}

pub fn submit_urx_hybrid(
    state: &mut WindowRenderState,
    params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> bool {
    let scene = match take_urx_scene(state) {
        Some(s) => s,
        None => return false,
    };

    let SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } = state.surface else {
        eprintln!("[render-hub] urx_hybrid requires SurfaceMode::Gpu");
        return false;
    };
    let width  = surface.config.width;
    let height = surface.config.height;
    if width == 0 || height == 0 { return false; }

    let device = &gpu_pool.devices[dev_id].device;
    let queue  = &gpu_pool.devices[dev_id].queue;

    // Lazy-init backends.
    if state.urx_cpu_backend.is_none() {
        state.urx_cpu_backend = Some(uzor_urx_cpu::CpuBackend::new());
    }
    if state.urx_hybrid_backend.is_none() {
        state.urx_hybrid_backend = Some(uzor_urx_hybrid::HybridBackend::new());
    }
    let need_new_pixmap = match &state.urx_cpu_pixmap {
        None => true,
        Some(p) => p.width() != width || p.height() != height,
    };
    if need_new_pixmap {
        state.urx_cpu_pixmap = Some(uzor_urx_cpu::Pixmap::new(width, height));
    }

    // Step 1: CPU rasterise the Scene into the shared pixmap.
    let r2t_t0 = std::time::Instant::now();
    let render_ok = {
        let backend = state.urx_cpu_backend.as_ref().expect("inited above");
        let pixmap  = state.urx_cpu_pixmap.as_mut().expect("inited above");
        pixmap.fill([0, 0, 0, 0]);
        backend.render(&scene, pixmap).is_ok()
    };
    if !render_ok { return false; }

    // Step 2: upsert the pixmap into the hybrid backend as one region.
    let region_id = uzor_urx_core::region::RegionId(0);
    let pixmap_clone = state.urx_cpu_pixmap.as_ref().expect("inited above").clone();
    if let Some(ref mut hb) = state.urx_hybrid_backend {
        hb.upsert_region_pixmap(device, queue, region_id, &pixmap_clone);
    }
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    // Step 3: composite the region onto the swapchain.
    let surface_texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            surface.surface.configure(device, &surface.config);
            return false;
        }
        wgpu::CurrentSurfaceTexture::Validation => return false,
    };
    let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let format = surface_texture.texture.format();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("urx-hybrid-encoder"),
    });
    let _ = params;
    let present_t0 = std::time::Instant::now();

    // Single-region composite: whole window as one quad sampling the texture
    // (full UV [0..1, 0..1], neutral tint).
    let instances: [(uzor_urx_core::region::RegionId, uzor_urx_hybrid::QuadInstance); 1] = [
        (region_id, uzor_urx_hybrid::QuadInstance::new(
            0.0, 0.0, width as f32, height as f32,
        )),
    ];
    if let Some(ref mut hb) = state.urx_hybrid_backend {
        hb.composite(
            device, queue, &mut encoder, &surface_view, format,
            width, height, &instances,
        );
    }
    queue.submit(std::iter::once(encoder.finish()));
    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    false
}

// ── UrxWgpuFull ────────────────────────────────────────────────────────────
//
// Stage 1a wires the WgpuFullBackend wrapper from uzor-urx-wgpu-full.
// We lazy-init the backend on first frame with the surface's device/queue/
// format, resize it on every frame (cheap if dims unchanged), then call
// `submit(scene, encoder, view)` and present.

pub fn submit_urx_wgpu_full(
    state: &mut WindowRenderState,
    params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> bool {
    let scene = match take_urx_scene(state) {
        Some(s) => s,
        None => return false,
    };

    let SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } = state.surface else {
        eprintln!("[render-hub] urx_wgpu_full requires SurfaceMode::Gpu");
        return false;
    };
    let width  = surface.config.width;
    let height = surface.config.height;
    if width == 0 || height == 0 { return false; }

    let device = gpu_pool.devices[dev_id].device.clone();
    let queue  = gpu_pool.devices[dev_id].queue.clone();

    // Lazy-init.
    if state.urx_wgpu_full_backend.is_none() {
        let surface_texture_format = surface.config.format;
        state.urx_wgpu_full_backend = Some(uzor_urx_wgpu_full::WgpuFullBackend::new(
            device.clone(), queue.clone(), surface_texture_format,
        ));
    }
    if let Some(ref mut backend) = state.urx_wgpu_full_backend {
        backend.resize(width, height);
    }

    let surface_texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            surface.surface.configure(&device, &surface.config);
            return false;
        }
        wgpu::CurrentSurfaceTexture::Validation => return false,
    };
    let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("urx-wgpu-full-encoder"),
    });
    let _ = params; // base_color handled inside the blit pass (clear=TRANSPARENT) currently.

    let r2t_t0 = std::time::Instant::now();
    if let Some(ref mut backend) = state.urx_wgpu_full_backend {
        if let Err(e) = backend.submit(&scene, &mut encoder, &surface_view) {
            eprintln!("[render-hub] urx_wgpu_full submit error: {:?}", e);
        }
    }
    queue.submit(std::iter::once(encoder.finish()));
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    let present_t0 = std::time::Instant::now();
    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    false
}
