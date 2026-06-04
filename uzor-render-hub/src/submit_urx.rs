//! URX-family submit paths.
//!
//! Mirrors `submit.rs`'s per-backend dispatch shape: one function per active
//! `RenderBackend` variant. All four URX backends consume the same
//! `urx_core::Scene` produced by `UrxRenderContext` during the frame, so
//! the only difference is which backend rasterises it.
//!
//! **Phase A scope:**
//! - `UrxCpu` — fully wired: own scanline rasteriser → pixmap → existing CPU
//!   presenter path (mirror of `submit_cpu_tinyskia`).
//! - `UrxWgpu` / `UrxHybrid` / `UrxWgpuFull` — not yet wired. The Scene is
//!   produced and dropped; the swapchain is cleared. Logged once per
//!   process so callers see backend selection is honoured but no pixels
//!   are produced yet. To be filled in once the convenience surface APIs
//!   of those backends settle (each one currently has a different submit
//!   shape: WgpuBackend builds DrawCmds → uzor-render-wgpu-instanced
//!   submit; HybridBackend has its own dispatch; WgpuFullBackend encodes
//!   → uploads → compute dispatches → blits).

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

// ── UrxWgpu / UrxHybrid / UrxWgpuFull (Phase A skeletons) ───────────────────

pub fn submit_urx_wgpu(
    state: &mut WindowRenderState,
    _params: &SubmitParams,
    _metrics: &mut RenderMetrics,
) -> bool {
    let _ = take_urx_scene(state); // drain so the Scene isn't carried forward
    log_phase_a_skeleton("urx_wgpu");
    false
}

pub fn submit_urx_hybrid(
    state: &mut WindowRenderState,
    _params: &SubmitParams,
    _metrics: &mut RenderMetrics,
) -> bool {
    let _ = take_urx_scene(state);
    log_phase_a_skeleton("urx_hybrid");
    false
}

pub fn submit_urx_wgpu_full(
    state: &mut WindowRenderState,
    _params: &SubmitParams,
    _metrics: &mut RenderMetrics,
) -> bool {
    let _ = take_urx_scene(state);
    log_phase_a_skeleton("urx_wgpu_full");
    false
}

fn log_phase_a_skeleton(name: &str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;
    static LOGGED: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    let lock = LOGGED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
    let mut g = match lock.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if g.insert(name.to_string()) {
        eprintln!(
            "[render-hub] {} backend is wired into the catalog but not yet \
             producing pixels (Phase A skeleton). Use urx_cpu for now.",
            name
        );
    }
    // `AtomicBool` referenced just to silence the dead-import lint if rust-analyser
    // decides nothing else uses it.
    let _ = AtomicBool::new(false).load(Ordering::Relaxed);
}
