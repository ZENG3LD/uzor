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
//! - `UrxWgpu` — **Wave 6 Commit 1** cut this over
//!   (`urx-wave6-autodetect-cutover-design-2026-07-25.md` §3): renders
//!   through `uzor_urx_wgpu::NativeUrxRenderer` (the Wave 1-4 native
//!   pipeline set — quad/line/path/glyph/gradient/image, stencil
//!   rounded-clip, blend layers, full affine) directly into
//!   `surface.target_view`, then blits to the swapchain — the SAME
//!   per-window renderer slot Wave 5's `compose.rs` already added and
//!   uses. Was: adapter pushes Scene into the shared
//!   `InstancedRenderContext`, then `InstancedRenderer` draws it (same
//!   path `Scene2DBackend::InstancedWgpu` still uses today —
//!   `InstancedWgpu` itself is untouched, still legacy, still
//!   manually-selectable).
//! - `UrxHybrid` — CpuBackend rasterises into one region pixmap,
//!   `HybridBackend.upsert_region_pixmap` uploads it as the sole region,
//!   `HybridBackend.composite` blits it to the swapchain. **Unaffected
//!   by Wave 6** — still reachable only via explicit
//!   `.backend(RenderBackend::UrxHybrid)`, never returned by
//!   autodetect.
//! - `UrxWgpuFull` — `WgpuFullBackend.submit` runs the encode → tile_assign
//!   → tile_sort → fine → blit chain. **Unaffected by Wave 6** — same
//!   manually-selectable-only status as `UrxHybrid`.

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
// Wave 6 Commit 1 (`urx-wave6-autodetect-cutover-design-2026-07-25.md`
// §3) cuts this over from the Stage 1a legacy adapter (Scene →
// InstancedRenderContext → InstancedRenderer) to the Wave 1-4 native
// pipeline set (`uzor_urx_wgpu::NativeUrxRenderer`) — the SAME
// `WindowRenderState.urx_native_renderer` slot Wave 5's compose path
// already lazy-inits (`compose.rs::compose_urx_native_into_swap`), so
// a window that already exercised the compose path this session reuses
// the SAME renderer instance (same glyph atlas / gradient LUT / tess
// cache / MSAA target) here — no duplicate GPU resources, no new
// struct field. No synthetic background prepend (unlike compose's
// overlay case): the ordinary `App::ui` 2D path always paints its own
// opaque background as Scene content first, exactly as it does today
// under `VelloGpu`/`TinySkia`/the old adapter — there is no "sometimes
// nothing behind it" gap to paper over here.
//
// `state.instanced_renderer`/`instanced_ctx` are NOT touched by this
// function anymore — they remain lazy-init'd only by
// `RenderBackend::InstancedWgpu`'s own (untouched, still-legacy,
// still-manually-selectable) submit path.

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

    // Cloned `Arc` handles (matches `submit_urx_wgpu_full`'s own
    // established shape in this file) — `NativeUrxRenderer::new` needs
    // owned `Device`/`Queue`, and holding them independently of the
    // `state.surface` borrow lets this function still reach
    // `state.urx_native_renderer` (a disjoint field) afterward.
    let device = gpu_pool.devices[dev_id].device.clone();
    let queue  = gpu_pool.devices[dev_id].queue.clone();

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

    // One encoder for both the native render AND the blit tail — same
    // shape `compose_urx_native_into_swap` (Wave 5a) established, one
    // `queue.submit`, one `present`.
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("uzor-render-hub:urx-wgpu-native"),
    });

    // Lazy-init — the exact same slot Wave 5's compose path already
    // added to `WindowRenderState` (`factory.rs`), fixed at
    // `Rgba8Unorm` (never the swapchain's own, possibly-sRGB format —
    // see `compose_urx_native_into_swap`'s own doc comment for why).
    if state.urx_native_renderer.is_none() {
        state.urx_native_renderer = Some(uzor_urx_wgpu::NativeUrxRenderer::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        ));
    }

    let r2t_t0 = std::time::Instant::now();
    if let Some(renderer) = state.urx_native_renderer.as_mut() {
        // `surface.target_view`/`target_texture` are guaranteed
        // `Rgba8Unorm` + `RENDER_ATTACHMENT` on every GPU-backed
        // surface regardless of active backend
        // (`factory.rs::recreate_target_with_cpu_usage`, unconditional
        // on every `resize_surface`) — the same guarantee Wave 5a
        // leaned on.
        if let Err(e) = renderer.render_into_encoder(
            &scene,
            &mut encoder,
            &surface.target_view,
            uzor_urx_wgpu::Viewport { width, height },
        ) {
            eprintln!("[render-hub] submit_urx_wgpu native render error: {:?}", e);
        }
    }
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    // `params.base_color` is unused here — see this fn's own doc
    // comment: the app's own Scene always paints its own opaque
    // background as content, so there is no clear-color gap to fill
    // (same "handled inside the blit pass" shape
    // `submit_urx_wgpu_full` already documents for its own unused
    // `params`).
    let _ = params;

    // Reuse the existing blit tail — same pattern `submit_urx_cpu`'s
    // own upload-then-blit already uses, just fed by a GPU-rendered
    // `target_view` instead of an uploaded CPU pixmap.
    let present_t0 = std::time::Instant::now();
    surface.blitter.copy(&device, &mut encoder, &surface.target_view, &surface_view);
    queue.submit([encoder.finish()]);
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

// ── Wave 6 Commit 1: submit_urx_wgpu native-cutover wiring proof ─────

#[cfg(test)]
mod tests {
    use uzor_urx_core::math::{Affine, Brush, Color, Gradient};
    use uzor_urx_core::scene::{DrawCommand, Scene};
    use uzor_urx_core::Rect;

    /// Headless wgpu device — same shape as `uzor-urx-wgpu`'s own
    /// `renderer.rs::test_device`/`tests/common/mod.rs::init_device`.
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-render-hub-submit-urx-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    /// Read back a `RENDER_ATTACHMENT | COPY_SRC` texture as a tight
    /// (no row padding) `Vec<u8>` of RGBA8 bytes — same shape as
    /// `uzor-urx-wgpu`'s own `renderer.rs::readback_rgba`.
    fn readback_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
        let aligned_stride = (width * 4 + 255) & !255;
        let buf_size = (aligned_stride * height) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uzor-render-hub-submit-urx-test-readback"),
            size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(aligned_stride), rows_per_image: Some(height) },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        queue.submit(Some(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        rx.recv()
            .expect("map_async callback channel closed before firing")
            .expect("staging buffer map failed");
        let raw = slice.get_mapped_range();
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height as usize {
            let row_start = row * aligned_stride as usize;
            let row_end = row_start + (width * 4) as usize;
            out.extend_from_slice(&raw[row_start..row_end]);
        }
        drop(raw);
        staging.unmap();
        out
    }

    /// Wave 6 Commit 1 wiring proof
    /// (`urx-wave6-autodetect-cutover-design-2026-07-25.md` §3.4): a
    /// Radial gradient — a primitive the OLD `adapt_scene_into`
    /// legacy-adapter path (deleted from `submit_urx_wgpu` this commit)
    /// could not render for real (Wave 1's own capability-gap matrix,
    /// `plan-urx-family-parity-2026-07-24.md` §1) — must come out as a
    /// genuine, position-varying interpolated colour, not a flat
    /// first-stop solid.
    ///
    /// **Disclosed deviation from the design's literal test recipe**:
    /// design asks to drive this "end-to-end through the REAL dispatch"
    /// via `submit_frame` with `state.active = RenderBackend::UrxWgpu`
    /// against a live `SurfaceMode::Gpu` window. `SurfaceMode::Gpu`
    /// wraps a REAL `wgpu::Surface`, which fundamentally requires a
    /// real OS window/canvas presentation target — categorically
    /// different from the headless `(Device, Queue)` `test_device()`
    /// shape design itself cites as the model to follow (that shape is
    /// for OFFSCREEN texture rendering only, no window). No test
    /// anywhere in this whole design-doc family — including
    /// `uzor-urx-wgpu`'s own 27/27 parity suite — constructs a real
    /// swapchain surface; grepped this crate for any existing
    /// `SurfaceMode::Gpu`-in-test precedent, found none. Building that
    /// infrastructure from scratch (a hidden real window + a
    /// `raw-window-handle` presentation target) is substantial new
    /// scope this commit does not need to prove its actual claim.
    /// Instead, this test exercises the EXACT mechanism
    /// `submit_urx_wgpu` now uses internally — the SAME
    /// `NativeUrxRenderer::new(device, queue, Rgba8Unorm)` construction
    /// and the SAME `render_into_encoder` call, into an offscreen
    /// `Rgba8Unorm | RENDER_ATTACHMENT | COPY_SRC` texture standing in
    /// for `surface.target_view` — proving the mechanism this function
    /// now calls genuinely renders the gradient, the substantive claim
    /// this smoke test exists to prove. The DISPATCH wiring itself
    /// (`submit.rs`'s `RenderBackend::UrxWgpu` arm calling this exact
    /// function) is a static, unchanged function reference — confirmed
    /// unmodified by this commit's own diff, needing no runtime proof
    /// beyond that.
    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn native_cutover_renders_a_radial_gradient_not_a_degraded_solid() {
        let Some((device, queue)) = test_device() else { return };
        const SIZE: u32 = 64;
        const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

        // Same construction `submit_urx_wgpu` itself now uses.
        let mut renderer = uzor_urx_wgpu::NativeUrxRenderer::new(device.clone(), queue.clone(), FORMAT);

        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, SIZE as f64, SIZE as f64),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(20, 20, 20, 255)),
            transform: Affine::IDENTITY,
        });
        let gradient = Gradient::new_radial((32.0, 32.0), 30.0).with_stops([
            (0.0f32, Color::from_rgba8(230, 60, 60, 255)),
            (1.0f32, Color::from_rgba8(60, 90, 230, 255)),
        ]);
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(2.0, 2.0, 62.0, 62.0),
            radii: None,
            brush: Brush::Gradient(gradient),
            transform: Affine::IDENTITY,
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor-render-hub-submit-urx-test-target"),
            size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer
            .render_into_encoder(&scene, &mut encoder, &view, uzor_urx_wgpu::Viewport { width: SIZE, height: SIZE })
            .expect("a well-formed gradient scene must not error");
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba(&device, &queue, &texture, SIZE, SIZE);
        // Probe (12, 32): 20px from the gradient center (32,32) against
        // a 30px radius (t ~ 0.667) — a genuine interpolated colour,
        // neither the pure start stop (230,60,60) nor the pure end stop
        // (60,90,230). A silently-degraded "first-stop solid" path
        // would read EXACTLY (230,60,60,255) here instead.
        let idx = ((32u32 * SIZE + 12u32) * 4) as usize;
        let px = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];
        assert_ne!(
            px, [230, 60, 60, 255],
            "must be a real interpolated gradient colour, not a flat first-stop solid (the old adapter's degrade): got {px:?}"
        );
        assert_ne!(px, [20, 20, 20, 255], "must be inside the gradient rect, not reading the background: got {px:?}");
    }
}
