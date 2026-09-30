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

// `color` is the crate vello re-exports as `vello::peniko::color`, so
// `AlphaColor<Srgb>` here is the same type with or without the GPU stack.
use color::{AlphaColor, Srgb};
#[cfg(feature = "gpu")]
use vello::wgpu;
#[cfg(feature = "vello-gpu")]
use vello::{AaConfig, RenderParams};

use crate::backend::RenderBackend;
#[cfg(any(feature = "tiny-skia", feature = "vello-cpu", feature = "vello-gpu", feature = "vello-hybrid", feature = "wgpu-instanced"))]
use crate::factory::SurfaceMode;
use crate::factory::WindowRenderState;
use crate::metrics::RenderMetrics;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg(feature = "vello-gpu")]
struct VelloSceneStats {
    paths: u32,
    path_segments: u32,
    clips: u32,
    open_clips: u32,
    path_tags: usize,
    path_data_words: usize,
    draw_tags: usize,
    draw_data_words: usize,
    transforms: usize,
    styles: usize,
    glyphs: usize,
    glyph_runs: usize,
    encoded_bytes: usize,
    reserved_bytes: usize,
}

#[cfg(feature = "vello-gpu")]
impl VelloSceneStats {
    fn capture(scene: &vello::Scene) -> Self {
        let encoding = scene.encoding();
        let encoded_bytes = vec_len_bytes(&encoding.path_tags)
            .saturating_add(vec_len_bytes(&encoding.path_data))
            .saturating_add(vec_len_bytes(&encoding.draw_tags))
            .saturating_add(vec_len_bytes(&encoding.draw_data))
            .saturating_add(vec_len_bytes(&encoding.transforms))
            .saturating_add(vec_len_bytes(&encoding.styles))
            .saturating_add(vec_len_bytes(&encoding.resources.patches))
            .saturating_add(vec_len_bytes(&encoding.resources.color_stops))
            .saturating_add(vec_len_bytes(&encoding.resources.glyphs))
            .saturating_add(vec_len_bytes(&encoding.resources.glyph_runs))
            .saturating_add(vec_len_bytes(&encoding.resources.normalized_coords));
        let reserved_bytes = vec_capacity_bytes(&encoding.path_tags)
            .saturating_add(vec_capacity_bytes(&encoding.path_data))
            .saturating_add(vec_capacity_bytes(&encoding.draw_tags))
            .saturating_add(vec_capacity_bytes(&encoding.draw_data))
            .saturating_add(vec_capacity_bytes(&encoding.transforms))
            .saturating_add(vec_capacity_bytes(&encoding.styles))
            .saturating_add(vec_capacity_bytes(&encoding.resources.patches))
            .saturating_add(vec_capacity_bytes(&encoding.resources.color_stops))
            .saturating_add(vec_capacity_bytes(&encoding.resources.glyphs))
            .saturating_add(vec_capacity_bytes(&encoding.resources.glyph_runs))
            .saturating_add(vec_capacity_bytes(&encoding.resources.normalized_coords));
        Self {
            paths: encoding.n_paths,
            path_segments: encoding.n_path_segments,
            clips: encoding.n_clips,
            open_clips: encoding.n_open_clips,
            path_tags: encoding.path_tags.len(),
            path_data_words: encoding.path_data.len(),
            draw_tags: encoding.draw_tags.len(),
            draw_data_words: encoding.draw_data.len(),
            transforms: encoding.transforms.len(),
            styles: encoding.styles.len(),
            glyphs: encoding.resources.glyphs.len(),
            glyph_runs: encoding.resources.glyph_runs.len(),
            encoded_bytes,
            reserved_bytes,
        }
    }
}

#[cfg(feature = "vello-gpu")]
fn vec_len_bytes<T>(values: &Vec<T>) -> usize {
    values.len().saturating_mul(std::mem::size_of::<T>())
}

#[cfg(feature = "vello-gpu")]
fn vec_capacity_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(std::mem::size_of::<T>())
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "vello-gpu")]
struct VelloGpuErrorScopes {
    out_of_memory: wgpu::ErrorScopeGuard,
    internal: wgpu::ErrorScopeGuard,
    validation: wgpu::ErrorScopeGuard,
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "vello-gpu")]
impl VelloGpuErrorScopes {
    fn push(device: &wgpu::Device) -> Self {
        Self {
            out_of_memory: device.push_error_scope(wgpu::ErrorFilter::OutOfMemory),
            internal: device.push_error_scope(wgpu::ErrorFilter::Internal),
            validation: device.push_error_scope(wgpu::ErrorFilter::Validation),
        }
    }

    fn pop(self, device: &wgpu::Device) -> VelloGpuScopedErrors {
        VelloGpuScopedErrors {
            validation: vello::util::block_on_wgpu(device, self.validation.pop()),
            internal: vello::util::block_on_wgpu(device, self.internal.pop()),
            out_of_memory: vello::util::block_on_wgpu(device, self.out_of_memory.pop()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
#[cfg(feature = "vello-gpu")]
struct VelloGpuScopedErrors {
    validation: Option<wgpu::Error>,
    internal: Option<wgpu::Error>,
    out_of_memory: Option<wgpu::Error>,
}

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
    /// `true` when the wgpu surface/device is unrecoverable
    /// (`OutOfMemory` or surface validation failure).
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
    let mut frame_metrics = RenderMetrics {
        backend: Some(state.active),
        ..Default::default()
    };
    let total_t0 = std::time::Instant::now();

    // ── URX engine channel takes priority when armed ────────────────────
    // Dual-channel dispatch (2026-06-09 owner doctrine, prefix scheme):
    // - `engine3d_*` consumer calls `set_active_urx(Some(b))` →
    //   `state.active_urx` is `Some`. Paint walker writes into
    //   `state.urx_ctx` via `with_urx_engine`. Submit MUST go through
    //   the URX backend matching `active_urx`, not the Scene2D `active`.
    // - `canvas2d_*` consumer leaves `active_urx == None`. Submit
    //   follows the Scene2D channel as before.
    //
    // Channels co-exist; this dispatch picks which one paints the
    // swapchain THIS frame. The other channel's last buffered content
    // is preserved but not presented.
    #[cfg(feature = "urx")]
    if let Some(urx) = state.active_urx {
        let surface_lost = match urx {
            uzor::UrxBackend::Cpu      => crate::submit_urx::submit_urx_cpu(state, &mut frame_metrics),
            uzor::UrxBackend::Wgpu     => crate::submit_urx::submit_urx_wgpu(state, &params, &mut frame_metrics),
            uzor::UrxBackend::Hybrid   => crate::submit_urx::submit_urx_hybrid(state, &params, &mut frame_metrics),
            uzor::UrxBackend::WgpuFull => crate::submit_urx::submit_urx_wgpu_full(state, &params, &mut frame_metrics),
            uzor::UrxBackend::Auto     => {
                // Auto resolves at `with_urx_engine` time via WorkloadHint.
                // For submit we mirror that choice — the engine remembers
                // what it picked. Until the auto-pick is surfaced through
                // a backend accessor, fall through to UrxWgpu for GPU
                // surfaces / UrxCpu for software.
                let gpu = matches!(state.surface, crate::factory::SurfaceMode::Gpu { .. });
                if gpu {
                    crate::submit_urx::submit_urx_wgpu(state, &params, &mut frame_metrics)
                } else {
                    crate::submit_urx::submit_urx_cpu(state, &mut frame_metrics)
                }
            }
        };
        frame_metrics.submit_us = total_t0.elapsed().as_micros() as u64;
        return SubmitOutcome {
            metrics: frame_metrics,
            surface_lost,
        };
    }

    let surface_lost = match state.active {
        #[cfg(feature = "vello-gpu")]
        RenderBackend::VelloGpu      => submit_vello_gpu(state, &params, &mut frame_metrics, total_t0),
        #[cfg(feature = "vello-hybrid")]
        RenderBackend::VelloHybrid   => submit_vello_hybrid(state, &params, &mut frame_metrics),
        #[cfg(feature = "wgpu-instanced")]
        RenderBackend::InstancedWgpu => submit_instanced(state, &params, &mut frame_metrics),
        #[cfg(feature = "vello-cpu")]
        RenderBackend::VelloCpu      => submit_cpu_vello(state, &mut frame_metrics),
        #[cfg(feature = "tiny-skia")]
        RenderBackend::TinySkia      => submit_cpu_tinyskia(state, &mut frame_metrics),
        RenderBackend::Canvas2d      => {
            // DOM canvas auto-presents — all draw calls were issued synchronously
            // by the app's ui() callback via canvas2d_ctx_mut(). Nothing to flush.
            false
        }

        // ── URX family via Scene2D channel ─────────────────────────────────
        // Reached via `canvas2d_urx_*` labels — consumer used the legacy
        // Canvas2D vocabulary with a URX rasterizer underneath. Same
        // submit functions as the dedicated URX channel above.
        #[cfg(feature = "urx")]
        RenderBackend::UrxCpu        => crate::submit_urx::submit_urx_cpu(state, &mut frame_metrics),
        #[cfg(feature = "urx")]
        RenderBackend::UrxWgpu       => crate::submit_urx::submit_urx_wgpu(state, &params, &mut frame_metrics),
        #[cfg(feature = "urx")]
        RenderBackend::UrxHybrid     => crate::submit_urx::submit_urx_hybrid(state, &params, &mut frame_metrics),
        #[cfg(feature = "urx")]
        RenderBackend::UrxWgpuFull   => crate::submit_urx::submit_urx_wgpu_full(state, &params, &mut frame_metrics),

        // Backend not compiled into this build: nothing can paint it, so
        // skip the frame (not a lost surface).
        #[allow(unreachable_patterns)]
        _ => {
            let _ = (&params, total_t0);
            warn_backend_not_compiled(state.active);
            false
        }
    };

    frame_metrics.submit_us = total_t0.elapsed().as_micros() as u64;

    // ── URX metrics fanout ──────────────────────────────────────────────────
    // Emit through the `metrics` crate facade so any installed
    // recorder (UrxRecorder + downstream dashboards) sees per-frame
    // timing. Per-backend keys so runtime backend switch produces
    // side-by-side histograms.
    let backend_label = state.active.as_str();
    use uzor_urx_core::metrics_keys::{
        render_submit_us_key, render_submit_count_key,
        KEY_TICK_SUBMIT_US, KEY_TICK_FRAMES,
        KEY_RENDER_R2T_US, KEY_RENDER_PRESENT_US,
    };
    metrics::histogram!(KEY_TICK_SUBMIT_US).record(frame_metrics.submit_us as f64);
    metrics::counter!(KEY_TICK_FRAMES).increment(1);
    metrics::histogram!(render_submit_us_key(backend_label)).record(frame_metrics.submit_us as f64);
    metrics::counter!(render_submit_count_key(backend_label)).increment(1);
    if frame_metrics.render_to_texture_us > 0 {
        metrics::histogram!(KEY_RENDER_R2T_US).record(frame_metrics.render_to_texture_us as f64);
    }
    if frame_metrics.present_us > 0 {
        metrics::histogram!(KEY_RENDER_PRESENT_US).record(frame_metrics.present_us as f64);
    }

    SubmitOutcome { metrics: frame_metrics, surface_lost }
}

// ── VelloGpu ──────────────────────────────────────────────────────────────────

#[cfg(feature = "vello-gpu")]
fn submit_vello_gpu(
    state: &mut WindowRenderState,
    params: &SubmitParams,
    metrics: &mut RenderMetrics,
    submit_started: std::time::Instant,
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

    let profile_enabled = uzor::diagnostics::profile_enabled();
    let profile_frame_id = profile_enabled
        .then(uzor::diagnostics::current_frame_id)
        .flatten();
    let scene_snapshot_t0 = std::time::Instant::now();
    let scene_stats = profile_enabled.then(|| VelloSceneStats::capture(&state.scene));
    let scene_snapshot_us = scene_snapshot_t0.elapsed().as_micros() as u64;
    if let Some(scene_stats) = scene_stats {
        let adapter_info = gpu_pool.devices[dev_id].adapter().get_info();
        let limits = device.limits();
        let target_bytes = u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(4);
        uzor::diagnostics::stage(
            "vello_gpu",
            "submit_begin",
            format_args!(
             "pid={} correlated_frame_id={:?} backend=vello_gpu target={}x{} \
             target_rgba8_bytes={} surface_format={:?} msaa={} \
             device_index={} adapter_name={:?} adapter_backend={:?} \
             adapter_device_type={:?} adapter_driver={:?} adapter_driver_info={:?} \
             vendor={} device={} max_buffer_size={} max_storage_buffer_binding_size={} \
             device_features={:?} paths={} path_segments={} clips={} open_clips={} \
             path_tags={} path_data_words={} draw_tags={} draw_data_words={} \
             transforms={} styles={} glyphs={} glyph_runs={} scene_encoded_bytes={} \
             scene_reserved_bytes={} scene_snapshot_us={scene_snapshot_us}",
            std::process::id(),
            profile_frame_id,
            width,
            height,
            target_bytes,
            surface.config.format,
            params.msaa_samples,
            dev_id,
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type,
            adapter_info.driver,
            adapter_info.driver_info,
            adapter_info.vendor,
            adapter_info.device,
            limits.max_buffer_size,
            limits.max_storage_buffer_binding_size,
            device.features(),
            scene_stats.paths,
            scene_stats.path_segments,
            scene_stats.clips,
            scene_stats.open_clips,
            scene_stats.path_tags,
            scene_stats.path_data_words,
            scene_stats.draw_tags,
            scene_stats.draw_data_words,
            scene_stats.transforms,
            scene_stats.styles,
            scene_stats.glyphs,
            scene_stats.glyph_runs,
            scene_stats.encoded_bytes,
            scene_stats.reserved_bytes,
            ),
        );
    }

    let Some(ref mut renderer) = state.vello_gpu_renderer else {
        eprintln!("[render-hub] VelloGpu renderer slot is None — call new_gpu()");
        return false;
    };

    #[cfg(not(target_arch = "wasm32"))]
    let gpu_error_scopes = profile_enabled.then(|| VelloGpuErrorScopes::push(device));

    // Phase 1: render scene to off-screen target texture.
    let r2t_t0 = std::time::Instant::now();
    let render_result = renderer.render_to_texture(
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
    );
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;
    if let Err(error) = &render_result {
        eprintln!("[render-hub] vello render_to_texture: {error}");
    }

    // Phase 2: acquire swapchain + blit + present.
    let present_t0 = std::time::Instant::now();
    let lost = blit_and_present(surface, device, queue);
    metrics.present_us = present_t0.elapsed().as_micros() as u64;

    #[cfg(not(target_arch = "wasm32"))]
    let gpu_errors = gpu_error_scopes.map(|scopes| scopes.pop(device));

    if profile_enabled {
        #[cfg(not(target_arch = "wasm32"))]
        let gpu_error_summary = gpu_errors
            .map(|errors| format!(
                "validation={:?} internal={:?} out_of_memory={:?}",
                errors.validation,
                errors.internal,
                errors.out_of_memory,
            ))
            .unwrap_or_else(|| {
                "validation=not_scoped internal=not_scoped out_of_memory=not_scoped".to_owned()
            });
        #[cfg(target_arch = "wasm32")]
        let gpu_error_summary =
            "validation=unsupported_wasm internal=unsupported_wasm out_of_memory=unsupported_wasm"
                .to_owned();
        uzor::diagnostics::stage(
            "vello_gpu",
            "submit_end",
            format_args!(
                "pid={} correlated_frame_id={:?} scene_snapshot_us={} \
                 render_to_texture_us={} present_us={} submit_elapsed_us={} \
                 surface_lost={lost} render_result={:?} gpu_scope={}",
                std::process::id(),
                profile_frame_id,
                scene_snapshot_us,
                metrics.render_to_texture_us,
                metrics.present_us,
                submit_started.elapsed().as_micros(),
                render_result.as_ref().err(),
                gpu_error_summary,
            ),
        );
    }
    lost
}

// ── VelloHybrid ───────────────────────────────────────────────────────────────

#[cfg(feature = "vello-hybrid")]
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

    // wgpu 29: get_current_texture returns CurrentSurfaceTexture enum, not Result.
    let surface_texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            eprintln!("[render-hub] vello-hybrid surface outdated/lost, reconfiguring");
            surface.surface.configure(device, &surface.config);
            return false;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            eprintln!("[render-hub] vello-hybrid surface validation error");
            return true;
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

#[cfg(feature = "wgpu-instanced")]
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

    // wgpu 29: get_current_texture returns CurrentSurfaceTexture enum, not Result.
    let surface_texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            eprintln!("[render-hub] instanced surface outdated/lost, reconfiguring");
            surface.surface.configure(device, &surface.config);
            return false;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            eprintln!("[render-hub] instanced surface validation error");
            return true;
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

    // Pull this frame's draw commands from the context the walker
    // populated via `with_render_context`. Before this wiring the
    // hub passed `&[]` and the renderer rendered nothing — a
    // clear-only no-op pipeline.
    //
    // Take the commands out so the next frame's `with_render_context`
    // call sees a fresh empty Vec (the `clear()` there is now a
    // no-op anyway, but keeping it preserves the contract).
    let r2t_t0 = std::time::Instant::now();
    let commands_taken: Vec<uzor_render_wgpu_instanced::DrawCmd> =
        state.instanced_ctx.as_mut()
            .map(|c| std::mem::take(&mut c.draw_commands))
            .unwrap_or_default();
    let cmd_count = commands_taken.len();
    if let Some(ref mut inst) = state.instanced_renderer {
        inst.render(
            device, queue, &surface_view, width, height,
            &commands_taken,
            Some(clear),
            None,
        );
    }
    metrics.render_to_texture_us = r2t_t0.elapsed().as_micros() as u64;
    // Hand the Vec back so its capacity is reused across frames
    // (the walker pushes into the same allocation next frame).
    if let Some(ctx) = state.instanced_ctx.as_mut() {
        let mut taken = commands_taken;
        taken.clear();
        ctx.draw_commands = taken;
    }
    let _ = cmd_count;

    surface_texture.present();
    metrics.present_us = present_t0.elapsed().as_micros() as u64;
    false
}

// ── VelloCpu ─────────────────────────────────────────────────────────────────

#[cfg(feature = "vello-cpu")]
fn submit_cpu_vello(state: &mut WindowRenderState, metrics: &mut RenderMetrics) -> bool {
    // Timing counters are only filled on the GPU-swapchain path.
    #[cfg(not(feature = "gpu"))]
    let _ = &metrics;
    match state.surface {
        #[cfg(feature = "gpu")]
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

#[cfg(feature = "tiny-skia")]
fn submit_cpu_tinyskia(state: &mut WindowRenderState, metrics: &mut RenderMetrics) -> bool {
    // Timing counters are only filled on the GPU-swapchain path.
    #[cfg(not(feature = "gpu"))]
    let _ = &metrics;
    match state.surface {
        #[cfg(feature = "gpu")]
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

/// One stderr line per backend per process when `submit_frame` is asked to
/// present a backend whose Cargo feature is off. Never reached with default
/// features (every native variant has its own arm there).
fn warn_backend_not_compiled(backend: RenderBackend) {
    use std::sync::Mutex;
    static WARNED: Mutex<Vec<RenderBackend>> = Mutex::new(Vec::new());
    let mut warned = WARNED.lock().unwrap_or_else(|p| p.into_inner());
    if !warned.contains(&backend) {
        warned.push(backend);
        eprintln!(
            "[render-hub] {backend:?} is not compiled into this build of uzor-render-hub \
             (its Cargo feature is off); skipping frame"
        );
    }
}

/// Acquire swapchain texture, blit `target_view` → swapchain, present.
///
/// Returns `true` on an unrecoverable surface/device failure, `false`
/// on success or recoverable errors (reconfigured inline).
/// Crate-public alias of `blit_and_present` — used by `submit_urx::submit_urx_cpu`
/// (CPU rasteriser → swapchain) to share the same blit + present path as
/// the other CPU backends.
#[cfg(feature = "gpu")]
pub(crate) fn blit_and_present_urx(
    surface: &mut vello::util::RenderSurface<'static>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> bool {
    blit_and_present(surface, device, queue)
}

#[cfg(feature = "gpu")]
fn blit_and_present(
    surface: &mut vello::util::RenderSurface<'static>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> bool {
    // wgpu 29: get_current_texture returns CurrentSurfaceTexture enum, not Result.
    let surface_texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return false,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            eprintln!("[render-hub] vello-gpu surface outdated/lost, reconfiguring");
            surface.surface.configure(device, &surface.config);
            return false;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            eprintln!("[render-hub] vello-gpu surface validation error");
            return true;
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
///
/// Unknown counts fall back to `Area`, not `Msaa8`: the default vello
/// factory compiles area-only shaders, so a stray value (e.g. the
/// framework `AppConfig` default of `1`) must never select an MSAA mode
/// the shaders were not built for — that is an instant vello panic.
#[cfg(feature = "vello-gpu")]
fn aa_for(msaa: u8) -> AaConfig {
    match msaa {
        8 => AaConfig::Msaa8,
        16 => AaConfig::Msaa16,
        _ => AaConfig::Area,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "vello-gpu", feature = "tiny-skia", feature = "vello-cpu"))]
    use super::*;
    #[cfg(any(feature = "tiny-skia", feature = "vello-cpu"))]
    use uzor::layout::window::SoftwarePresenter;

    #[test]
    #[cfg(feature = "vello-gpu")]
    fn vello_scene_stats_reports_stream_counts_and_memory_proxies() {
        let mut scene = vello::Scene::new();
        let encoding = scene.encoding_mut();
        encoding.n_paths = 2;
        encoding.n_path_segments = 5;
        encoding.path_data.extend([1, 2, 3]);
        encoding.draw_data.extend([4, 5]);

        let stats = VelloSceneStats::capture(&scene);
        assert_eq!(stats.paths, 2);
        assert_eq!(stats.path_segments, 5);
        assert_eq!(stats.path_data_words, 3);
        assert_eq!(stats.draw_data_words, 2);
        assert_eq!(stats.encoded_bytes, 5 * std::mem::size_of::<u32>());
        assert!(stats.reserved_bytes >= stats.encoded_bytes);
    }

    // ── MockPresenter ────────────────────────────────────────────────────────

    /// Test double for [`SoftwarePresenter`] — records every call to `present`.
    #[cfg(any(feature = "tiny-skia", feature = "vello-cpu"))]
    struct MockPresenter {
        calls: Vec<(u32, u32, Vec<u8>)>,
        resize_calls: Vec<(u32, u32)>,
    }

    #[cfg(any(feature = "tiny-skia", feature = "vello-cpu"))]
    impl MockPresenter {
        fn new() -> Self {
            Self { calls: Vec::new(), resize_calls: Vec::new() }
        }
    }

    #[cfg(any(feature = "tiny-skia", feature = "vello-cpu"))]
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
    #[cfg(feature = "tiny-skia")]
    fn tinyskia_software_present_called() {
        use crate::factory::{SurfaceMode, WindowRenderState};
        use crate::backend::RenderBackend;
        use uzor_render_tiny_skia::TinySkiaCpuRenderContext;
        use color::{AlphaColor, Srgb};

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
            use uzor::render::{Painter as _, ShapeHelpers as _};
            tiny_ctx.set_fill_color("#deadbe");
            tiny_ctx.fill_rect(0.0, 0.0, width as f64, height as f64);
        }

        let mut state = WindowRenderState {
            surface: SurfaceMode::Software {
                presenter: Box::new(FwdPresenter(mock_ptr)),
                width,
                height,
            },
            #[cfg(feature = "vello-gpu")]
            vello_gpu_renderer:   None,
            #[cfg(feature = "vello-hybrid")]
            vello_hybrid_renderer: None,
            #[cfg(feature = "wgpu-instanced")]
            instanced_renderer:   None,
            #[cfg(feature = "wgpu-instanced")]
            instanced_ctx:        None,
            #[cfg(feature = "vello-cpu")]
            vello_cpu_ctx:        None,
            #[cfg(feature = "tiny-skia")]
            tiny_skia_ctx:        Some(tiny_ctx),
            #[cfg(feature = "urx")]
            urx_ctx:              None,
            #[cfg(feature = "urx")]
            urx_cpu_backend:      None,
            #[cfg(feature = "urx")]
            urx_cpu_pixmap:       None,
            #[cfg(feature = "urx")]
            urx_hybrid_backend:   None,
            #[cfg(feature = "urx")]
            urx_wgpu_full_backend: None,
            #[cfg(feature = "urx")]
            urx_engine:           None,
            #[cfg(feature = "urx-3d")]
            urx_renderer_3d:      None,
            #[cfg(feature = "urx-3d")]
            urx_scene_3d:         None,
            #[cfg(feature = "urx-3d")]
            urx_physics:          None,
            #[cfg(feature = "urx-3d")]
            urx_particles:        std::collections::HashMap::new(),
            #[cfg(feature = "gpu")]
            scene:                vello::Scene::new(),
            #[cfg(feature = "vello-hybrid")]
            vello_hybrid_ctx:     uzor_render_vello_hybrid::VelloHybridRenderContext::new(1.0),
            active:               RenderBackend::TinySkia,
            active_urx:           None,
            #[cfg(feature = "urx")]
            urx_unified_memory:   None,
            #[cfg(feature = "urx")]
            urx_retained_hint:    true,
            #[cfg(feature = "urx")]
            urx_high_hz_hint:     false,
            #[cfg(feature = "urx-3d")]
            urx_offscreen_3d:     None,
            #[cfg(feature = "urx-3d")]
            urx_capture_3d:       None,
            #[cfg(feature = "urx-3d")]
            urx_compose_overlay_cache: None,
            #[cfg(feature = "urx-3d")]
            urx_compose_overlay_dynamic: None,
            #[cfg(feature = "urx-3d")]
            urx_compose_overlay_blitter: None,
            #[cfg(feature = "urx")]
            urx_native_renderer: None,
            #[cfg(feature = "urx-3d")]
            urx_cached_overlay_renderer: None,
            #[cfg(feature = "urx-3d")]
            urx_dynamic_overlay_renderer: None,
            #[cfg(feature = "urx-3d")]
            capture_3d_enabled:   false,
            #[cfg(feature = "urx-3d")]
            urx_compose_msaa_sample_count: 4,
            #[cfg(feature = "urx-3d")]
            urx_compose_edge_width_px: None,
            retained_cache:       crate::retained::RetainedCache::new(),
            #[cfg(feature = "vello-gpu")]
            vello_fragment_store: Default::default(),
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
    #[cfg(feature = "vello-cpu")]
    fn vello_cpu_software_present_called() {
        use crate::factory::{SurfaceMode, WindowRenderState};
        use crate::backend::RenderBackend;
        use uzor_render_vello_cpu::VelloCpuRenderContext;
        use color::{AlphaColor, Srgb};

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
            #[cfg(feature = "vello-gpu")]
            vello_gpu_renderer:    None,
            #[cfg(feature = "vello-hybrid")]
            vello_hybrid_renderer: None,
            #[cfg(feature = "wgpu-instanced")]
            instanced_renderer:    None,
            #[cfg(feature = "wgpu-instanced")]
            instanced_ctx:         None,
            #[cfg(feature = "vello-cpu")]
            vello_cpu_ctx:         Some(vello_ctx),
            #[cfg(feature = "tiny-skia")]
            tiny_skia_ctx:         None,
            #[cfg(feature = "urx")]
            urx_ctx:               None,
            #[cfg(feature = "urx")]
            urx_cpu_backend:       None,
            #[cfg(feature = "urx")]
            urx_cpu_pixmap:        None,
            #[cfg(feature = "urx")]
            urx_hybrid_backend:    None,
            #[cfg(feature = "urx")]
            urx_wgpu_full_backend: None,
            #[cfg(feature = "urx")]
            urx_engine:           None,
            #[cfg(feature = "urx-3d")]
            urx_renderer_3d:      None,
            #[cfg(feature = "urx-3d")]
            urx_scene_3d:         None,
            #[cfg(feature = "urx-3d")]
            urx_physics:          None,
            #[cfg(feature = "urx-3d")]
            urx_particles:        std::collections::HashMap::new(),
            #[cfg(feature = "gpu")]
            scene:                 vello::Scene::new(),
            #[cfg(feature = "vello-hybrid")]
            vello_hybrid_ctx:      uzor_render_vello_hybrid::VelloHybridRenderContext::new(1.0),
            active:                RenderBackend::VelloCpu,
            active_urx:            None,
            #[cfg(feature = "urx")]
            urx_unified_memory:    None,
            #[cfg(feature = "urx")]
            urx_retained_hint:     true,
            #[cfg(feature = "urx")]
            urx_high_hz_hint:      false,
            #[cfg(feature = "urx-3d")]
            urx_offscreen_3d:      None,
            #[cfg(feature = "urx-3d")]
            urx_capture_3d:        None,
            #[cfg(feature = "urx-3d")]
            urx_compose_overlay_cache: None,
            #[cfg(feature = "urx-3d")]
            urx_compose_overlay_dynamic: None,
            #[cfg(feature = "urx-3d")]
            urx_compose_overlay_blitter: None,
            #[cfg(feature = "urx")]
            urx_native_renderer: None,
            #[cfg(feature = "urx-3d")]
            urx_cached_overlay_renderer: None,
            #[cfg(feature = "urx-3d")]
            urx_dynamic_overlay_renderer: None,
            #[cfg(feature = "urx-3d")]
            capture_3d_enabled:    false,
            #[cfg(feature = "urx-3d")]
            urx_compose_msaa_sample_count: 4,
            #[cfg(feature = "urx-3d")]
            urx_compose_edge_width_px: None,
            retained_cache:        crate::retained::RetainedCache::new(),
            #[cfg(feature = "vello-gpu")]
            vello_fragment_store:  Default::default(),
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
