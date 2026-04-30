//! Per-frame GPU/CPU submission, dispatched by the active backend.
//!
//! [`submit_frame`] reads `state.active` and routes to the correct render path.
//!
//! # CPU backends + GPU surface (mlc pattern)
//!
//! When a CPU backend (`VelloCpu` or `TinySkia`) runs on a machine that has a
//! GPU adapter, the CPU pixels are uploaded via `queue.write_texture` into the
//! shared `target_texture`, then the blitter copies to the swapchain — matching
//! the pattern in `mylittlechart` `chart-app-vello/src/render/gpu_submit.rs`
//! lines 235–258.
//!
//! On a GPU-less machine (`SurfaceMode::Software`) the pixels are delivered to
//! the CPU context's softbuffer output.  Full softbuffer wiring requires a
//! `WindowProvider`-level surface; the stub here keeps the frame loop alive.
//!
//! # Backend coverage
//!
//! | Backend | Status |
//! |---------|--------|
//! | `VelloGpu` | Full — scene → render_to_texture → blit → present |
//! | `VelloHybrid` | Full — lazy renderer init + direct render to swapchain |
//! | `InstancedWgpu` | Full — lazy renderer init + direct render to swapchain |
//! | `VelloCpu` | Full (Gpu surface) — write_texture → blit → present; stub (Software) |
//! | `TinySkia` | Full (Gpu surface) — write_texture → blit → present; stub (Software) |

use vello::peniko::color::{AlphaColor, Srgb};
use vello::{wgpu, AaConfig, RenderParams};

use crate::backend::RenderBackend;
use crate::factory::{SurfaceMode, WindowRenderState};
use crate::metrics::RenderMetrics;

/// Inputs to [`submit_frame`].
pub struct SubmitParams {
    /// Background colour for backends that clear the swapchain.
    pub base_color: AlphaColor<Srgb>,
    /// MSAA sample count (`0` = area AA, `8` = MSAA8, `16` = MSAA16).
    /// Other values fall back to MSAA8.
    pub msaa_samples: u8,
}

/// Outcome of [`submit_frame`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SubmitOutcome {
    /// Per-frame timing counters.
    pub metrics: RenderMetrics,
    /// `true` when the wgpu surface is unrecoverable (`OutOfMemory`).
    pub surface_lost: bool,
}

/// Submit one frame and present the swapchain.
///
/// Dispatches to the correct backend path based on `state.active`.
///
/// # Caller responsibilities
///
/// - **VelloGpu**: fill `state.scene_mut()` before calling.
/// - **VelloHybrid**: fill `state.vello_hybrid_ctx_mut()` before calling.
/// - **VelloCpu**: draw into `state.vello_cpu_ctx_mut()` before calling.
/// - **TinySkia**: draw into `state.cpu_ctx_mut()` before calling.
/// - **InstancedWgpu**: no caller setup (renderer drives itself; empty frame
///   clears to `base_color`).
pub fn submit_frame(state: &mut WindowRenderState, params: SubmitParams) -> SubmitOutcome {
    let mut metrics = RenderMetrics {
        backend: Some(state.active),
        ..Default::default()
    };
    let total_t0 = std::time::Instant::now();

    let surface_lost = match state.active {
        RenderBackend::VelloGpu      => submit_vello_gpu(state, &params, &mut metrics),
        RenderBackend::VelloHybrid   => submit_vello_hybrid(state, &params, &mut metrics),
        RenderBackend::InstancedWgpu => submit_instanced(state, &params, &mut metrics),
        RenderBackend::VelloCpu      => submit_cpu_vello(state, &mut metrics),
        RenderBackend::TinySkia      => submit_cpu_tinyskia(state, &mut metrics),
        RenderBackend::Canvas2d      => {
            // DOM canvas auto-presents — all draw calls were issued synchronously
            // by the app's ui() callback via canvas2d_ctx_mut(). Nothing to flush.
            false
        }
    };

    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
    SubmitOutcome { metrics, surface_lost }
}

// ── VelloGpu ──────────────────────────────────────────────────────────────────

fn submit_vello_gpu(
    state: &mut WindowRenderState,
    params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> bool {
    let SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } = state.surface else {
        eprintln!("[render-hub] VelloGpu requires SurfaceMode::Gpu");
        return false;
    };

    let width = surface.config.width;
    let height = surface.config.height;
    if width == 0 || height == 0 {
        return false;
    }

    let device = &gpu_pool.devices[dev_id].device;
    let queue = &gpu_pool.devices[dev_id].queue;

    let Some(ref mut renderer) = state.vello_gpu_renderer else {
        eprintln!("[render-hub] VelloGpu renderer slot is None — call new_gpu()");
        return false;
    };

    // Phase 1: render scene to off-screen target texture.
    let r2t_t0 = std::time::Instant::now();
    renderer
        .render_to_texture(
            device,
            queue,
            &state.scene,
            &surface.target_view,
            &RenderParams {
                base_color: params.base_color,
                width,
                height,
                antialiasing_method: aa_for(params.msaa_samples),
            },
        )
        .unwrap_or_else(|e| eprintln!("[render-hub] vello render_to_texture: {e}"));
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    // Phase 2: acquire swapchain + blit + present.
    let present_t0 = std::time::Instant::now();
    let lost = blit_and_present(surface, device, queue);
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    lost
}

// ── VelloHybrid ───────────────────────────────────────────────────────────────

fn submit_vello_hybrid(
    state: &mut WindowRenderState,
    _params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> bool {
    let SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } = state.surface else {
        eprintln!("[render-hub] VelloHybrid requires SurfaceMode::Gpu");
        return false;
    };

    let width = surface.config.width;
    let height = surface.config.height;
    if width == 0 || height == 0 {
        return false;
    }

    let device = &gpu_pool.devices[dev_id].device;
    let queue = &gpu_pool.devices[dev_id].queue;

    let present_t0 = std::time::Instant::now();

    let surface_texture = match surface.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::OutOfMemory) => return true,
        Err(e) => {
            eprintln!("[render-hub] vello-hybrid surface error: {e:?}, reconfiguring");
            surface.surface.configure(device, &surface.config);
            return false;
        }
    };
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let format = surface_texture.texture.format();

    // Lazy-init the vello_hybrid renderer.
    if state.vello_hybrid_renderer.is_none() {
        state.vello_hybrid_renderer = Some(vello_hybrid::Renderer::new(
            device,
            &vello_hybrid::RenderTargetConfig { format, width, height },
        ));
    }

    if let Some(ref mut hybrid_renderer) = state.vello_hybrid_renderer {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uzor-render-hub:vello-hybrid"),
        });
        state
            .vello_hybrid_ctx
            .render(hybrid_renderer, device, queue, &mut encoder, &surface_view)
            .unwrap_or_else(|e| eprintln!("[render-hub] vello-hybrid render: {e:?}"));
        queue.submit([encoder.finish()]);
    }

    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    false
}

// ── InstancedWgpu ─────────────────────────────────────────────────────────────

fn submit_instanced(
    state: &mut WindowRenderState,
    params: &SubmitParams,
    metrics: &mut RenderMetrics,
) -> bool {
    let SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } = state.surface else {
        eprintln!("[render-hub] InstancedWgpu requires SurfaceMode::Gpu");
        return false;
    };

    let width = surface.config.width;
    let height = surface.config.height;
    if width == 0 || height == 0 {
        return false;
    }

    let device = &gpu_pool.devices[dev_id].device;
    let queue = &gpu_pool.devices[dev_id].queue;

    let present_t0 = std::time::Instant::now();

    let surface_texture = match surface.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::OutOfMemory) => return true,
        Err(e) => {
            eprintln!("[render-hub] instanced surface error: {e:?}, reconfiguring");
            surface.surface.configure(device, &surface.config);
            return false;
        }
    };
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let format = surface_texture.texture.format();

    // Lazy-init the InstancedRenderer.
    if state.instanced_renderer.is_none() {
        state.instanced_renderer =
            Some(uzor_render_wgpu_instanced::InstancedRenderer::new(device, queue, format));
    }

    let clear = wgpu::Color {
        r: params.base_color.components[0] as f64,
        g: params.base_color.components[1] as f64,
        b: params.base_color.components[2] as f64,
        a: params.base_color.components[3] as f64,
    };

    if let Some(ref mut inst) = state.instanced_renderer {
        inst.render(device, queue, &surface_view, width, height, &[], Some(clear), None);
    }

    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    false
}

// ── VelloCpu ─────────────────────────────────────────────────────────────────

fn submit_cpu_vello(state: &mut WindowRenderState, metrics: &mut RenderMetrics) -> bool {
    match state.surface {
        SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } => {
            let width = surface.config.width;
            let height = surface.config.height;
            if width == 0 || height == 0 {
                return false;
            }

            let device = &gpu_pool.devices[dev_id].device;
            let queue = &gpu_pool.devices[dev_id].queue;

            let r2t_t0 = std::time::Instant::now();

            // Render vello-cpu to a temporary RGBA8 buffer, then upload.
            if let Some(ref mut cpu_ctx) = state.vello_cpu_ctx {
                let pixel_count = (width * height) as usize;
                let mut rgba8 = vec![0u8; pixel_count * 4];
                cpu_ctx.render_to_pixmap_rgba8(&mut rgba8, width as u16, height as u16);

                // mlc gpu_submit.rs lines 243–256 verbatim pattern.
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &surface.target_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba8,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );
            }

            metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

            let present_t0 = std::time::Instant::now();
            let lost = blit_and_present(surface, device, queue);
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
            lost
        }

        #[cfg(not(target_arch = "wasm32"))]
        SurfaceMode::Software { ref mut presenter, ref mut width, ref mut height } => {
            if let Some(ref mut cpu_ctx) = state.vello_cpu_ctx {
                let w = *width;
                let h = *height;
                if w > 0 && h > 0 {
                    let pixel_count = (w as usize).saturating_mul(h as usize);
                    let mut rgba8 = vec![0u8; pixel_count * 4];
                    cpu_ctx.render_to_pixmap_rgba8(
                        &mut rgba8,
                        w.min(u16::MAX as u32) as u16,
                        h.min(u16::MAX as u32) as u16,
                    );
                    presenter.present(&rgba8, w, h);
                }
            }
            false
        }

        // Canvas2d surface does not use CPU render paths.
        #[cfg(target_arch = "wasm32")]
        SurfaceMode::Canvas2d { .. } => false,
    }
}

// ── TinySkia ──────────────────────────────────────────────────────────────────

fn submit_cpu_tinyskia(state: &mut WindowRenderState, metrics: &mut RenderMetrics) -> bool {
    match state.surface {
        SurfaceMode::Gpu { ref gpu_pool, ref mut surface, dev_id } => {
            let width = surface.config.width;
            let height = surface.config.height;
            if width == 0 || height == 0 {
                return false;
            }

            let device = &gpu_pool.devices[dev_id].device;
            let queue = &gpu_pool.devices[dev_id].queue;

            let r2t_t0 = std::time::Instant::now();

            if let Some(ref tiny_ctx) = state.tiny_skia_ctx {
                let pix = tiny_ctx.pixels();
                let cw = tiny_ctx.width();
                let ch = tiny_ctx.height();

                if !pix.is_empty() && cw > 0 && ch > 0 && cw == width && ch == height {
                    // mlc gpu_submit.rs lines 243–256 verbatim pattern.
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
            }

            metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

            let present_t0 = std::time::Instant::now();
            let lost = blit_and_present(surface, device, queue);
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
            lost
        }

        #[cfg(not(target_arch = "wasm32"))]
        SurfaceMode::Software { ref mut presenter, width, height } => {
            if let Some(ref tiny_ctx) = state.tiny_skia_ctx {
                let pix = tiny_ctx.pixels();
                let cw  = tiny_ctx.width();
                let ch  = tiny_ctx.height();

                if !pix.is_empty() && cw > 0 && ch > 0 && cw == width && ch == height {
                    presenter.present(pix, cw, ch);
                }
            }
            false
        }

        // Canvas2d surface does not use CPU render paths.
        #[cfg(target_arch = "wasm32")]
        SurfaceMode::Canvas2d { .. } => false,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Acquire swapchain texture, blit `target_view` → swapchain, present.
///
/// Returns `true` on `OutOfMemory` (surface lost), `false` on success or
/// recoverable errors (reconfigured inline).
fn blit_and_present(
    surface: &mut vello::util::RenderSurface<'static>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> bool {
    let surface_texture = match surface.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::OutOfMemory) => return true,
        Err(e) => {
            eprintln!("[render-hub] surface error: {e:?}, reconfiguring");
            surface.surface.configure(device, &surface.config);
            return false;
        }
    };

    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("uzor-render-hub:blit"),
    });
    surface.blitter.copy(device, &mut encoder, &surface.target_view, &surface_view);
    queue.submit([encoder.finish()]);
    surface_texture.present();
    false
}

/// Convert MSAA count to vello's `AaConfig`.
fn aa_for(msaa: u8) -> AaConfig {
    match msaa {
        0 => AaConfig::Area,
        8 => AaConfig::Msaa8,
        16 => AaConfig::Msaa16,
        _ => AaConfig::Msaa8,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_window_hub::SoftwarePresenter;

    // ── MockPresenter ────────────────────────────────────────────────────────

    /// Test double for [`SoftwarePresenter`] — records every call to `present`.
    struct MockPresenter {
        calls: Vec<(u32, u32, Vec<u8>)>,
        resize_calls: Vec<(u32, u32)>,
    }

    impl MockPresenter {
        fn new() -> Self {
            Self { calls: Vec::new(), resize_calls: Vec::new() }
        }
    }

    impl SoftwarePresenter for MockPresenter {
        fn present(&mut self, pixels: &[u8], width: u32, height: u32) {
            self.calls.push((width, height, pixels.to_vec()));
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.resize_calls.push((width, height));
        }
    }

    // ── TinySkia software submit ─────────────────────────────────────────────

    /// `submit_frame` with `TinySkia` + `Software` surface calls `presenter.present`
    /// with the correct pixel dimensions and a non-empty buffer.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tinyskia_software_present_called() {
        use crate::factory::{SurfaceMode, WindowRenderState};
        use crate::backend::RenderBackend;
        use uzor_render_tiny_skia::TinySkiaCpuRenderContext;
        use vello::peniko::color::{AlphaColor, Srgb};

        let width = 16u32;
        let height = 16u32;

        let mock = Box::new(MockPresenter::new());
        let mock_ptr: *mut MockPresenter = &mut *(Box::leak(Box::new(MockPresenter::new())));

        // Build a WindowRenderState manually with a forwarding presenter that
        // records calls into mock_ptr.
        struct FwdPresenter(*mut MockPresenter);
        // SAFETY: test is single-threaded.
        unsafe impl Send for FwdPresenter {}

        impl SoftwarePresenter for FwdPresenter {
            fn present(&mut self, pixels: &[u8], w: u32, h: u32) {
                // SAFETY: test is single-threaded, mock_ptr is valid.
                unsafe { (*self.0).present(pixels, w, h) };
            }
            fn resize(&mut self, w: u32, h: u32) {
                unsafe { (*self.0).resize(w, h) };
            }
        }

        let _ = mock; // suppress unused warning

        let mut tiny_ctx = TinySkiaCpuRenderContext::new(width, height, 1.0);
        // Paint the entire pixmap with a known solid color using the public API.
        // TinySkiaCpuRenderContext implements uzor's RenderContext so fill_rect works.
        {
            use uzor::render::RenderContext as _;
            tiny_ctx.set_fill_color("#deadbe");
            tiny_ctx.fill_rect(0.0, 0.0, width as f64, height as f64);
        }

        let mut state = WindowRenderState {
            surface: SurfaceMode::Software {
                presenter: Box::new(FwdPresenter(mock_ptr)),
                width,
                height,
            },
            vello_gpu_renderer:   None,
            vello_hybrid_renderer: None,
            instanced_renderer:   None,
            vello_cpu_ctx:        None,
            tiny_skia_ctx:        Some(tiny_ctx),
            scene:                vello::Scene::new(),
            vello_hybrid_ctx:     uzor_render_vello_hybrid::VelloHybridRenderContext::new(1.0),
            active:               RenderBackend::TinySkia,
        };

        let outcome = submit_frame(
            &mut state,
            SubmitParams {
                base_color: AlphaColor::<Srgb>::new([0.0, 0.0, 0.0, 1.0]),
                msaa_samples: 8,
            },
        );
        assert!(!outcome.surface_lost);

        // SAFETY: test is single-threaded, present has been called synchronously.
        let mock = unsafe { &*mock_ptr };
        assert_eq!(mock.calls.len(), 1, "presenter.present must be called once per frame");
        let (pw, ph, ref buf) = mock.calls[0];
        assert_eq!(pw, width);
        assert_eq!(ph, height);
        assert_eq!(buf.len(), (width * height * 4) as usize);
        // First pixel R channel should be 0xDE (tiny-skia stores premultiplied RGBA).
        assert_eq!(buf[0], 0xDE, "R channel of first pixel mismatch");

        // Cleanup the leaked box.
        drop(unsafe { Box::from_raw(mock_ptr) });
    }

    // ── VelloCpu software submit ─────────────────────────────────────────────

    /// `submit_frame` with `VelloCpu` + `Software` surface calls `presenter.present`
    /// with a properly sized buffer.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn vello_cpu_software_present_called() {
        use crate::factory::{SurfaceMode, WindowRenderState};
        use crate::backend::RenderBackend;
        use uzor_render_vello_cpu::VelloCpuRenderContext;
        use vello::peniko::color::{AlphaColor, Srgb};

        let width = 16u32;
        let height = 16u32;

        let mock_ptr: *mut MockPresenter = Box::into_raw(Box::new(MockPresenter::new()));

        struct FwdPresenter(*mut MockPresenter);
        unsafe impl Send for FwdPresenter {}
        impl SoftwarePresenter for FwdPresenter {
            fn present(&mut self, pixels: &[u8], w: u32, h: u32) {
                unsafe { (*self.0).present(pixels, w, h) };
            }
            fn resize(&mut self, w: u32, h: u32) {
                unsafe { (*self.0).resize(w, h) };
            }
        }

        let mut vello_ctx = VelloCpuRenderContext::new(1.0);
        vello_ctx.begin_frame(width, height);

        let mut state = WindowRenderState {
            surface: SurfaceMode::Software {
                presenter: Box::new(FwdPresenter(mock_ptr)),
                width,
                height,
            },
            vello_gpu_renderer:    None,
            vello_hybrid_renderer: None,
            instanced_renderer:    None,
            vello_cpu_ctx:         Some(vello_ctx),
            tiny_skia_ctx:         None,
            scene:                 vello::Scene::new(),
            vello_hybrid_ctx:      uzor_render_vello_hybrid::VelloHybridRenderContext::new(1.0),
            active:                RenderBackend::VelloCpu,
        };

        let outcome = submit_frame(
            &mut state,
            SubmitParams {
                base_color: AlphaColor::<Srgb>::new([0.0, 0.0, 0.0, 1.0]),
                msaa_samples: 8,
            },
        );
        assert!(!outcome.surface_lost);

        let mock = unsafe { &*mock_ptr };
        assert_eq!(mock.calls.len(), 1, "presenter.present must be called once per frame");
        let (pw, ph, ref buf) = mock.calls[0];
        assert_eq!(pw, width);
        assert_eq!(ph, height);
        assert_eq!(buf.len(), (width * height * 4) as usize);

        drop(unsafe { Box::from_raw(mock_ptr) });
    }
}
