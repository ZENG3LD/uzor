//! Per-frame GPU submission, dispatched per active backend variant.
//!
//! [`submit_frame`] matches on [`WindowRenderState`] to drive the correct
//! pipeline.  For the `Gpu` variant it acquires the next swapchain texture,
//! renders the vello scene, blits the off-screen target, and presents.  For
//! the `Cpu` variant it calls into `softbuffer` (not yet wired) or simply
//! clears to the background colour.

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
pub fn submit_frame(state: &mut WindowRenderState, params: SubmitParams) -> SubmitOutcome {
    let mut metrics = RenderMetrics {
        backend: Some(state.backend()),
        ..Default::default()
    };

    let total_t0 = std::time::Instant::now();

    match state {
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

            // ── Phase 1: render scene to the off-screen target texture ────────
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

            // ── Phase 2: acquire swapchain image ──────────────────────────────
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

            // ── Phase 3: blit off-screen target → swapchain ───────────────────
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uzor-render-hub:blit"),
            });
            surface.blitter.copy(device, &mut encoder, &surface.target_view, &surface_view);
            queue.submit([encoder.finish()]);

            surface_texture.present();
            metrics.present_us = present_t0.elapsed().as_micros() as u64;
        }

        WindowRenderState::Cpu { ctx: _ } => {
            // CPU path: the tiny-skia pixmap is ready in `ctx`.
            // Presentation to the OS window requires a software back-buffer
            // (e.g. `softbuffer`).  That wiring is not yet implemented; this
            // is a no-op placeholder so the frame loop can run without
            // panicking.
            //
            // TODO: wire softbuffer or a wgpu blit to present CPU pixels.
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
