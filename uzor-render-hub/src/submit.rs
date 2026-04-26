//! Per-frame GPU submission, dispatched per active backend.
//!
//! Logic mirrors `mylittlechart/crates/chart-app-vello/src/render/gpu_submit.rs`
//! minus the chart/screenshot/agent specifics — those stay in the caller.

use vello::peniko::color::{AlphaColor, Srgb};
use vello::util::{RenderContext as VelloRenderContext, RenderSurface};
use vello::{wgpu, AaConfig, RenderParams};

use crate::backend::RenderBackend;
use crate::factory::WindowRenderState;
use crate::metrics::RenderMetrics;

/// Inputs to `submit_frame`.
pub struct SubmitParams<'a> {
    /// Vello shared GPU context (devices + queues by `dev_id`).
    pub render_cx: &'a VelloRenderContext,
    /// Window's vello surface (also holds `target_view` + `target_texture`
    /// for off-screen render-to-texture and a blitter for CPU/GPU paths).
    pub surface: &'a RenderSurface<'static>,
    /// Background colour for backends that clear the swapchain.
    pub base_color: AlphaColor<Srgb>,
    /// MSAA sample count (0/8/16). Other values fall back to MSAA8.
    pub msaa_samples: u8,
}

/// Outcome of `submit_frame`. `surface_lost` signals catastrophic surface
/// failure — caller should close the window.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubmitOutcome {
    pub metrics: RenderMetrics,
    pub surface_lost: bool,
}

/// Submit one frame for `state.backend` and present the swapchain.
///
/// Caller responsibility, per backend:
/// - **`VelloGpu`**: fill `state.scene` before calling.
/// - **`VelloHybrid`**: store the per-frame context in `state.hybrid_ctx`.
/// - **`InstancedWgpu`**: fill `state.instanced_commands`.
/// - **`VelloCpu` / `TinySkia`**: write a `width*height*4` RGBA8 buffer into
///   `state.cpu_pixels` and set `state.cpu_dims`.
pub fn submit_frame(state: &mut WindowRenderState, params: SubmitParams<'_>) -> SubmitOutcome {
    let mut metrics = RenderMetrics {
        backend: Some(state.backend),
        ..Default::default()
    };

    let width = params.surface.config.width;
    let height = params.surface.config.height;
    if width == 0 || height == 0 {
        return SubmitOutcome { metrics, surface_lost: false };
    }

    let dev_id = params.surface.dev_id;
    let device = &params.render_cx.devices[dev_id].device;
    let queue = &params.render_cx.devices[dev_id].queue;

    let total_t0 = std::time::Instant::now();
    let r2t_t0 = std::time::Instant::now();

    // ── Phase 1: produce the off-screen / pre-swapchain artifact ─────────────
    match state.backend {
        RenderBackend::VelloGpu => {
            // Render the scene to the off-screen target texture.
            state
                .vello_renderer
                .render_to_texture(
                    device,
                    queue,
                    &state.scene,
                    &params.surface.target_view,
                    &RenderParams {
                        base_color: params.base_color,
                        width,
                        height,
                        antialiasing_method: aa_for(params.msaa_samples),
                    },
                )
                .expect("vello render_to_texture failed");
        }
        RenderBackend::VelloCpu | RenderBackend::TinySkia => {
            // Upload the CPU pixel buffer into the off-screen target texture.
            let (cw, ch) = state.cpu_dims;
            if !state.cpu_pixels.is_empty() && cw > 0 && ch > 0 && cw == width && ch == height {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture:    &params.surface.target_texture,
                        mip_level:  0,
                        origin:     wgpu::Origin3d::ZERO,
                        aspect:     wgpu::TextureAspect::All,
                    },
                    &state.cpu_pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset:         0,
                        bytes_per_row:  Some(4 * cw),
                        rows_per_image: Some(ch),
                    },
                    wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
                );
            }
        }
        RenderBackend::InstancedWgpu | RenderBackend::VelloHybrid => {
            // No off-screen pass — both render directly to the swapchain below.
        }
    }
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;

    // ── Phase 2: acquire swapchain image ─────────────────────────────────────
    let present_t0 = std::time::Instant::now();
    let surface_texture = match params.surface.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::OutOfMemory) => {
            metrics.submit_us = total_t0.elapsed().as_micros() as u64;
            return SubmitOutcome { metrics, surface_lost: true };
        }
        Err(e) => {
            eprintln!("[render-hub] surface error: {e:?}, reconfiguring");
            params.surface.surface.configure(device, &params.surface.config);
            metrics.submit_us = total_t0.elapsed().as_micros() as u64;
            return SubmitOutcome { metrics, surface_lost: false };
        }
    };
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // ── Phase 3: blit / direct-render to the swapchain ───────────────────────
    match state.backend {
        RenderBackend::VelloGpu | RenderBackend::VelloCpu | RenderBackend::TinySkia => {
            // Off-screen target → swapchain blit.
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uzor-render-hub:blit"),
            });
            params.surface.blitter.copy(
                device,
                &mut encoder,
                &params.surface.target_view,
                &surface_view,
            );
            queue.submit([encoder.finish()]);
        }
        RenderBackend::InstancedWgpu => {
            // Lazy renderer creation, then direct render to swapchain.
            if state.instanced_renderer.is_none() {
                state.instanced_renderer = Some(
                    uzor_backend_wgpu_instanced::InstancedRenderer::new(
                        device,
                        queue,
                        surface_texture.texture.format(),
                    ),
                );
            }
            let clear = wgpu::Color {
                r: params.base_color.components[0] as f64,
                g: params.base_color.components[1] as f64,
                b: params.base_color.components[2] as f64,
                a: params.base_color.components[3] as f64,
            };
            if let Some(ref mut inst) = state.instanced_renderer {
                inst.render(
                    device,
                    queue,
                    &surface_view,
                    width,
                    height,
                    &state.instanced_commands,
                    Some(clear),
                    None,
                );
            }
            metrics.draw_calls = state.instanced_commands.len() as u32;
        }
        RenderBackend::VelloHybrid => {
            if let Some(ref hybrid_ctx) = state.hybrid_ctx {
                if state.hybrid_renderer.is_none() {
                    state.hybrid_renderer = Some(vello_hybrid::Renderer::new(
                        device,
                        &vello_hybrid::RenderTargetConfig {
                            format: surface_texture.texture.format(),
                            width,
                            height,
                        },
                    ));
                }
                if let Some(ref mut renderer) = state.hybrid_renderer {
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("uzor-render-hub:vello_hybrid"),
                    });
                    let _ = hybrid_ctx.render(renderer, device, queue, &mut encoder, &surface_view);
                    queue.submit([encoder.finish()]);
                }
            }
        }
    }

    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    metrics.submit_us = total_t0.elapsed().as_micros() as u64;

    SubmitOutcome { metrics, surface_lost: false }
}

fn aa_for(msaa: u8) -> AaConfig {
    match msaa {
        0  => AaConfig::Area,
        8  => AaConfig::Msaa8,
        16 => AaConfig::Msaa16,
        _  => AaConfig::Msaa8,
    }
}
