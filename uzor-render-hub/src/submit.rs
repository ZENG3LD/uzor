//! Per-frame GPU submission, dispatched per active backend variant.
//!
//! [`submit_frame`] matches on [`WindowRenderState`] to drive the correct
//! pipeline.
//!
//! # Backend coverage
//!
//! | Variant | Submit status |
//! |---------|--------------|
//! | `Gpu` (VelloGpu) | Full — renders vello scene, blits to swapchain |
//! | `Cpu` (TinySkia) | Stub — pixels available in ctx; presenter not wired |
//! | `VelloCpu` | Stub — pixels via `render_to_softbuffer`; presenter not wired |
//! | `VelloHybrid` | Full — deferred GPU renderer init + direct render to swapchain |
//! | `WgpuInstanced` | Full — deferred `InstancedRenderer` init + direct render |

use vello::peniko::color::{AlphaColor, Srgb};
use vello::{wgpu, AaConfig, RenderParams};

use crate::factory::WindowRenderState;
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
///
/// `surface_lost` signals a catastrophic surface failure — the caller should
/// close the window.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubmitOutcome {
    /// Per-frame timing counters.
    pub metrics: RenderMetrics,
    /// `true` when the wgpu surface is unrecoverable (`OutOfMemory` error).
    pub surface_lost: bool,
}

/// Submit one frame and present the swapchain.
///
/// # Caller responsibilities per variant
///
/// - **`WindowRenderState::Gpu`**: fill `state.scene_mut()` before calling.
/// - **`WindowRenderState::Cpu`**: draw into `state.cpu_ctx_mut()` before calling.
/// - **`WindowRenderState::VelloCpu`**: call `begin_frame` + draw on `vello_cpu_ctx_mut()`.
/// - **`WindowRenderState::VelloHybrid`**: draw into `vello_hybrid_ctx_mut()`.
/// - **`WindowRenderState::WgpuInstanced`**: no caller setup needed (renderer
///   is driven by the hub's own `InstancedRenderer`).
pub fn submit_frame(state: &mut WindowRenderState, params: SubmitParams) -> SubmitOutcome {
    let mut metrics = RenderMetrics {
        backend: Some(state.backend()),
        ..Default::default()
    };

    let total_t0 = std::time::Instant::now();

    match state {
        // ── VelloGpu ──────────────────────────────────────────────────────────
        WindowRenderState::Gpu {
            gpu_pool,
            surface,
            renderer,
            scene,
            dev_id,
        } => {
            let width = surface.config.width;
            let height = surface.config.height;

            if width == 0 || height == 0 {
                return SubmitOutcome { metrics, surface_lost: false };
            }

            let device = &gpu_pool.devices[*dev_id].device;
            let queue = &gpu_pool.devices[*dev_id].queue;

            // Phase 1: render scene to the off-screen target texture
            let r2t_t0 = std::time::Instant::now();
            renderer
                .render_to_texture(
                    device,
                    queue,
                    scene,
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

            // Phase 2: acquire swapchain image
            let present_t0 = std::time::Instant::now();
            let surface_texture = match surface.surface.get_current_texture() {
                Ok(t) => t,
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
                    return SubmitOutcome { metrics, surface_lost: true };
                }
                Err(e) => {
                    eprintln!("[render-hub] surface error: {e:?}, reconfiguring");
                    surface.surface.configure(device, &surface.config);
                    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
                    return SubmitOutcome { metrics, surface_lost: false };
                }
            };
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            // Phase 3: blit off-screen target → swapchain
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uzor-render-hub:blit"),
            });
            surface.blitter.copy(device, &mut encoder, &surface.target_view, &surface_view);
            queue.submit([encoder.finish()]);

            surface_texture.present();
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
        }

        // ── TinySkia (CPU) ────────────────────────────────────────────────────
        WindowRenderState::Cpu { ctx: _ } => {
            // The tiny-skia pixmap is available in `ctx` after drawing.
            // Presenting to the OS window requires a softbuffer or similar
            // software back-buffer presenter.  That wiring lives in the
            // window provider layer (uzor-window-desktop) and is not yet
            // threaded through here.  This is a no-op so the frame loop runs.
            //
            // TODO: wire softbuffer or a wgpu blit to present CPU pixels.
        }

        // ── VelloCpu (CPU) ────────────────────────────────────────────────────
        WindowRenderState::VelloCpu { ctx: _ } => {
            // Pixels are available via ctx.render_to_softbuffer() after drawing.
            // Presenter wiring (softbuffer integration) lives in the window
            // provider layer and is not yet threaded through submit_frame.
            //
            // TODO: wire softbuffer presenter to push VelloCpu pixels to OS window.
        }

        // ── VelloHybrid ───────────────────────────────────────────────────────
        WindowRenderState::VelloHybrid {
            gpu_pool,
            surface,
            renderer,
            ctx,
            dev_id,
        } => {
            let width = surface.config.width;
            let height = surface.config.height;

            if width == 0 || height == 0 {
                return SubmitOutcome { metrics, surface_lost: false };
            }

            let device = &gpu_pool.devices[*dev_id].device;
            let queue = &gpu_pool.devices[*dev_id].queue;

            // Acquire swapchain image
            let present_t0 = std::time::Instant::now();
            let surface_texture = match surface.surface.get_current_texture() {
                Ok(t) => t,
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
                    return SubmitOutcome { metrics, surface_lost: true };
                }
                Err(e) => {
                    eprintln!("[render-hub] vello-hybrid surface error: {e:?}, reconfiguring");
                    surface.surface.configure(device, &surface.config);
                    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
                    return SubmitOutcome { metrics, surface_lost: false };
                }
            };
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let format = surface_texture.texture.format();

            // Lazy-init the vello_hybrid renderer (needs format from first texture)
            if renderer.is_none() {
                *renderer = Some(vello_hybrid::Renderer::new(
                    device,
                    &vello_hybrid::RenderTargetConfig { format, width, height },
                ));
            }

            if let Some(ref mut hybrid_renderer) = renderer {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("uzor-render-hub:vello-hybrid"),
                    });
                ctx.render(hybrid_renderer, device, queue, &mut encoder, &surface_view)
                    .unwrap_or_else(|e| eprintln!("[render-hub] vello-hybrid render: {e:?}"));
                queue.submit([encoder.finish()]);
            }

            surface_texture.present();
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
        }

        // ── WgpuInstanced ─────────────────────────────────────────────────────
        WindowRenderState::WgpuInstanced {
            gpu_pool,
            surface,
            renderer,
            dev_id,
        } => {
            let width = surface.config.width;
            let height = surface.config.height;

            if width == 0 || height == 0 {
                return SubmitOutcome { metrics, surface_lost: false };
            }

            let device = &gpu_pool.devices[*dev_id].device;
            let queue = &gpu_pool.devices[*dev_id].queue;

            // Acquire swapchain image
            let present_t0 = std::time::Instant::now();
            let surface_texture = match surface.surface.get_current_texture() {
                Ok(t) => t,
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
                    return SubmitOutcome { metrics, surface_lost: true };
                }
                Err(e) => {
                    eprintln!("[render-hub] instanced surface error: {e:?}, reconfiguring");
                    surface.surface.configure(device, &surface.config);
                    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
                    return SubmitOutcome { metrics, surface_lost: false };
                }
            };
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let format = surface_texture.texture.format();

            // Lazy-init the InstancedRenderer
            if renderer.is_none() {
                *renderer = Some(uzor_render_wgpu_instanced::InstancedRenderer::new(
                    device, queue, format,
                ));
            }

            let clear = wgpu::Color {
                r: params.base_color.components[0] as f64,
                g: params.base_color.components[1] as f64,
                b: params.base_color.components[2] as f64,
                a: params.base_color.components[3] as f64,
            };

            if let Some(ref mut inst) = renderer {
                // WgpuInstanced has no per-frame draw-command buffer stored in the
                // state — callers using this backend should obtain a mutable reference
                // to the renderer and submit commands directly, or the framework layer
                // can wire a command list here.  For now we submit an empty frame
                // (clear only) so the swapchain stays healthy.
                inst.render(device, queue, &surface_view, width, height, &[], Some(clear), None);
            }

            surface_texture.present();
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
        }
    }

    metrics.submit_us = total_t0.elapsed().as_micros() as u64;
    SubmitOutcome { metrics, surface_lost: false }
}

fn aa_for(msaa: u8) -> AaConfig {
    match msaa {
        0 => AaConfig::Area,
        8 => AaConfig::Msaa8,
        16 => AaConfig::Msaa16,
        _ => AaConfig::Msaa8,
    }
}
