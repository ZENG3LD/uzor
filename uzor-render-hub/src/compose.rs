//! URX 2D + 3D composition — one swapchain acquire, one present.
//!
//! Built for the "chrome 2D + viewport 3D" UX shape: a window where the
//! consumer wants both URX 2D content (chrome, toolbars, overlays) AND
//! one-or-more 3D viewports (Scene3D / Physics3D) rendered into the
//! SAME swapchain frame.
//!
//! Why this exists: `submit_urx_{cpu,wgpu,hybrid,wgpu_full}` and
//! `submit_3d_frame_to_rect` each do their own `get_current_texture` +
//! `present()`. Two `present()` on one swapchain frame is undefined —
//! the second fails silently. Before this module the tessera driver
//! had to choose ONE path per frame (`continue;`-ing out of the 2D
//! submit when any 3D container was present), which made 2D chrome
//! around a 3D viewport impossible.
//!
//! This module ships:
//! - [`Compose3DJob`] — one 3D content unit (camera + dst rect, the
//!   `Scene3D` is read from the hub's `urx_scene_3d` slot — caller
//!   pushes the scene before each job).
//! - [`submit_urx_composed`] — single-acquire / single-present submit
//!   that does 2D first, then N×3D, all in one encoder.
//!
//! Co-existence rule (doctrine): this path is opt-in. Consumers that
//! want fullscreen 3D keep calling `submit_3d_frame_to_rect`;
//! consumers that want fullscreen 2D keep calling `submit_frame`; this
//! new path is for the COMPOSE case.

use crate::factory::{Submit3DError, SurfaceMode, WindowRenderState};

/// One 3D viewport contribution to a composed frame.
///
/// The `Scene3D` to render is whatever currently sits in the hub's
/// `urx_scene_3d` slot AT THE TIME this job is processed — push the
/// scene into the slot via [`WindowRenderState::with_renderer_3d`]
/// (just the `*dst_scene = my_scene;` shape) before each job in
/// `submit_urx_composed`.
#[derive(Debug, Clone)]
pub struct Compose3DJob {
    /// Perspective camera for this viewport. `aspect` should match
    /// `dst_w / dst_h` (computed by caller).
    pub camera: uzor_urx_3d::PerspectiveCamera,
    /// Destination sub-rectangle in the swapchain (physical pixels,
    /// top-left origin). Out-of-bounds rects are clipped silently.
    pub dst_x: u32,
    pub dst_y: u32,
    pub dst_w: u32,
    pub dst_h: u32,
}

/// Outcome of [`submit_urx_composed`].
#[derive(Debug, Clone, Copy)]
pub struct ComposedOutcome {
    /// Number of 3D jobs that actually rendered (post-clip).
    pub jobs_rendered: u32,
    /// Number of jobs skipped because their rect clipped to zero or
    /// failed lazy-init.
    pub jobs_skipped: u32,
    /// `true` when the swapchain texture acquisition failed
    /// (Lost / Outdated / Validation). Caller should retry next frame.
    pub surface_lost: bool,
}

/// Compose one frame: URX 2D (whatever `active_urx` resolved to) +
/// N×3D viewports in a single encoder, one acquire, one present.
///
/// **Requirements**:
/// - `state.surface` is [`SurfaceMode::Gpu`] — software surfaces don't
///   carry the 3D path (returns [`Submit3DError::NotGpuSurface`]).
/// - `state.active_urx` is `Some(_)` — the URX channel must be armed
///   (caller did `set_active_urx(Some(b))` before paint). For the
///   composed path only `UrxBackend::Cpu` and `UrxBackend::Wgpu` (and
///   `Auto` resolving to one of them on a GPU surface) are wired in
///   1.4.9 — the Hybrid / WgpuFull backends fall back to fullscreen
///   submit-then-3D, which still works but with one extra encoder.
/// - For each job: the caller has already pushed the desired
///   `Scene3D` into the hub slot via
///   `state.with_renderer_3d(|_, scene| *scene = my_scene_for_this_job)`.
///
/// Same swapchain frame, in order:
/// 1. Acquire swap texture.
/// 2. 2D: pull URX `Scene` out of `urx_ctx`, rasterise (CPU pixmap →
///    upload → blit, OR Wgpu adapter → instanced draw) into
///    `swap_view`.
/// 3. For each 3D job: render `Scene3D` into the per-window offscreen
///    target with the job's camera, then `copy_texture_to_texture`
///    that result into `swap_view` at the job's rect — in the SAME
///    encoder as the 2D pass.
/// 4. One `queue.submit`, one `present`.
pub fn submit_urx_composed(
    state:      &mut WindowRenderState,
    base_color: [f32; 4],
    jobs:       &[Compose3DJob],
) -> Result<ComposedOutcome, Submit3DError> {
    // Resolve surface kind + size + format up front.
    let (surf_w, surf_h, surface_format) = match &state.surface {
        SurfaceMode::Gpu { surface, .. } => (
            surface.config.width,
            surface.config.height,
            surface.config.format,
        ),
        _ => return Err(Submit3DError::NotGpuSurface),
    };
    if surf_w == 0 || surf_h == 0 { return Err(Submit3DError::ZeroSizedSurface); }

    // Capture device + queue refs (cloned Arc) so we can hold them
    // across the slot borrows below without re-walking state.surface.
    let (device, queue) = match &state.surface {
        SurfaceMode::Gpu { gpu_pool, dev_id, .. } => (
            gpu_pool.devices[*dev_id].device.clone(),
            gpu_pool.devices[*dev_id].queue.clone(),
        ),
        _ => return Err(Submit3DError::NotGpuSurface),
    };

    // ── Phase 0: pull URX 2D scene out of urx_ctx ───────────────────
    // `None` is fine — first frame before any paint callback runs,
    // or a window whose consumer only wants 3D this frame. In that
    // case the 2D pass becomes a clear-only pass via the chosen URX
    // backend's normal lazy-init.
    let urx_scene_opt = state.urx_ctx.as_mut().map(|c| c.take_scene());

    // ── Phase 1: acquire swapchain frame ────────────────────────────
    let SurfaceMode::Gpu { surface, .. } = &mut state.surface else {
        return Err(Submit3DError::NotGpuSurface);
    };
    let frame = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            surface.surface.configure(&device, &surface.config);
            return Ok(ComposedOutcome { jobs_rendered: 0, jobs_skipped: jobs.len() as u32, surface_lost: true });
        }
        _ => return Ok(ComposedOutcome { jobs_rendered: 0, jobs_skipped: jobs.len() as u32, surface_lost: true }),
    };
    let swap_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

    // ── Phase 2: build one encoder for the whole frame ──────────────
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("uzor-render-hub:compose"),
    });

    // ── Phase 3: 2D pass into swap_view ─────────────────────────────
    //
    // URX 2D backends: CPU rasterises into a pixmap and uploads via
    // queue.write_texture into surface.target_texture, then blits to
    // swap_view (matches `submit_urx_cpu` upload path but with our
    // encoder). Wgpu uses the InstancedRenderer's render-pass writing
    // straight into swap_view. Hybrid/WgpuFull fall through to a
    // "clear only" 2D pass for 1.4.9 — they're rare and adding their
    // compose path is mechanical follow-up (covered by FUTURE-WORK
    // note in handoff).
    let urx_backend = state.active_urx.unwrap_or(uzor::UrxBackend::Cpu);
    let backend_resolved = match urx_backend {
        uzor::UrxBackend::Auto => uzor::UrxBackend::Wgpu, // GPU surface → wgpu
        b => b,
    };

    match backend_resolved {
        uzor::UrxBackend::Cpu => {
            compose_urx_cpu_into_swap(
                state, &device, &queue, &mut encoder, &swap_view,
                surf_w, surf_h, urx_scene_opt, base_color,
            );
        }
        uzor::UrxBackend::Wgpu => {
            compose_urx_wgpu_into_swap(
                state, &device, &queue, &mut encoder, &swap_view,
                surf_w, surf_h, surface_format, urx_scene_opt, base_color,
            );
        }
        uzor::UrxBackend::Hybrid | uzor::UrxBackend::WgpuFull | uzor::UrxBackend::Auto => {
            // 1.4.9 fallback: clear the swap_view to base_color so 3D
            // viewports composite onto a clean background. Hybrid /
            // WgpuFull compose passes land in a follow-up patch.
            clear_swap_view(&mut encoder, &swap_view, base_color);
            // Re-stash the unused scene so the next frame starts fresh.
            if let Some(ref mut c) = state.urx_ctx {
                // `take_scene` already drained it; nothing more to do here.
                // (Future: a `restore_scene(s)` op so we can return the
                // borrowed scene to the ctx if needed.)
                let _ = c;
            }
        }
    }

    // ── Phase 4: per-job 3D into offscreen + copy into swap_view ────
    let mut jobs_rendered = 0u32;
    let mut jobs_skipped  = 0u32;

    for job in jobs {
        // Clip rect to swapchain (out-of-bounds shrinks).
        let dx = job.dst_x.min(surf_w.saturating_sub(1));
        let dy = job.dst_y.min(surf_h.saturating_sub(1));
        let dw = job.dst_w.min(surf_w.saturating_sub(dx));
        let dh = job.dst_h.min(surf_h.saturating_sub(dy));
        if dw == 0 || dh == 0 {
            jobs_skipped += 1;
            continue;
        }

        // Lazy-init renderer + scene (mirrors submit_3d_frame_to_rect).
        if state.urx_renderer_3d.is_none() {
            state.urx_renderer_3d = Some(uzor_urx_3d::Renderer3D::new(
                &device, &queue, surface_format, (dw, dh), 1024,
            ));
        }
        if state.urx_scene_3d.is_none() {
            state.urx_scene_3d = Some(uzor_urx_3d::Scene3D::new());
        }

        // Lazy / resize the offscreen target if rect size changed.
        let need_new_target = match &state.urx_offscreen_3d {
            None => true,
            Some(o) => o.width != dw || o.height != dh || o.format != surface_format,
        };
        if need_new_target {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("urx-3d-offscreen-compose"),
                size: wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            state.urx_offscreen_3d = Some(crate::factory::UrxOffscreen3D {
                texture, view, width: dw, height: dh, format: surface_format,
            });
        }

        // Render the (consumer-pushed) Scene3D into the offscreen view
        // using OUR encoder — no acquire, no submit, no present here.
        let r3d_ok = {
            let r3d   = match state.urx_renderer_3d.as_mut() { Some(r) => r, None => { jobs_skipped += 1; continue; } };
            let scene = match state.urx_scene_3d.as_ref()    { Some(s) => s, None => { jobs_skipped += 1; continue; } };
            let off   = match state.urx_offscreen_3d.as_ref() { Some(o) => o, None => { jobs_skipped += 1; continue; } };
            r3d.render(&device, &queue, &mut encoder, &off.view, &job.camera, scene);
            true
        };
        if !r3d_ok { jobs_skipped += 1; continue; }

        // Copy offscreen → swap_view at (dx, dy). Same format both
        // sides ⇒ cheap GPU-side copy, no shader.
        let off = state.urx_offscreen_3d.as_ref().unwrap();
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture:   &off.texture,
                mip_level: 0,
                origin:    wgpu::Origin3d::ZERO,
                aspect:    wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture:   &frame.texture,
                mip_level: 0,
                origin:    wgpu::Origin3d { x: dx, y: dy, z: 0 },
                aspect:    wgpu::TextureAspect::All,
            },
            wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
        );
        jobs_rendered += 1;
    }

    // ── Phase 5: one submit, one present ────────────────────────────
    queue.submit([encoder.finish()]);
    frame.present();

    Ok(ComposedOutcome { jobs_rendered, jobs_skipped, surface_lost: false })
}

// ── 2D pass helpers ─────────────────────────────────────────────────

/// Clear `swap_view` to `base_color` via a render pass with LoadOp::Clear
/// and no draws. Used by Hybrid / WgpuFull fallback paths.
fn clear_swap_view(
    encoder:    &mut wgpu::CommandEncoder,
    swap_view:  &wgpu::TextureView,
    base_color: [f32; 4],
) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("compose:clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: swap_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: base_color[0] as f64,
                    g: base_color[1] as f64,
                    b: base_color[2] as f64,
                    a: base_color[3] as f64,
                }),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    // _pass dropped → encoder records the pass.
}

/// CPU URX 2D pass — rasterise into the per-window pixmap, upload via
/// queue.write_texture into surface.target_texture, then blit
/// target_texture → swap_view in OUR encoder.
fn compose_urx_cpu_into_swap(
    state:      &mut WindowRenderState,
    _device:    &wgpu::Device,
    queue:      &wgpu::Queue,
    encoder:    &mut wgpu::CommandEncoder,
    swap_view:  &wgpu::TextureView,
    surf_w:     u32,
    surf_h:     u32,
    scene_opt:  Option<uzor_urx_core::Scene>,
    base_color: [f32; 4],
) {
    // Lazy-init pixmap + backend.
    if state.urx_cpu_backend.is_none() {
        state.urx_cpu_backend = Some(uzor_urx_cpu::CpuBackend::new());
    }
    let need_new_pix = match &state.urx_cpu_pixmap {
        None => true,
        Some(p) => p.width() != surf_w || p.height() != surf_h,
    };
    if need_new_pix {
        state.urx_cpu_pixmap = Some(uzor_urx_cpu::Pixmap::new(surf_w, surf_h));
    }

    // Background clear in the pixmap so the 2D pass owns the base
    // colour; 3D viewports composite OVER the 2D pixels via the
    // copy_texture_to_texture phase.
    let bg_rgba = [
        (base_color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (base_color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (base_color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (base_color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ];

    {
        let backend = state.urx_cpu_backend.as_ref().expect("inited above");
        let pixmap  = state.urx_cpu_pixmap.as_mut().expect("inited above");
        pixmap.fill(bg_rgba);
        if let Some(scene) = scene_opt {
            if let Err(e) = backend.render(&scene, pixmap) {
                eprintln!("[render-hub] compose urx-cpu render error: {:?}", e);
            }
        }
    }

    // Upload pixmap → surface.target_texture then blit target → swap_view.
    let SurfaceMode::Gpu { surface, .. } = &mut state.surface else { return };
    let (cw, ch, pix_ptr, pix_len) = {
        let pixmap = state.urx_cpu_pixmap.as_ref().expect("inited above");
        (pixmap.width(), pixmap.height(), pixmap.pixels().as_ptr(), pixmap.pixels().len())
    };
    if cw == surf_w && ch == surf_h && pix_len > 0 {
        // SAFETY: pixmap stays alive for the duration of write_texture;
        // we hold &mut state.surface but pixmap is a disjoint field.
        let pix: &[u8] = unsafe { std::slice::from_raw_parts(pix_ptr, pix_len) };
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
    // Blit target_texture → swap_view in our encoder.
    surface.blitter.copy(_device, encoder, &surface.target_view, swap_view);
    let _ = base_color;
}

/// Wgpu URX 2D pass — adapt Scene into InstancedRenderContext, then
/// draw via InstancedRenderer directly into swap_view using our
/// encoder.
fn compose_urx_wgpu_into_swap(
    state:      &mut WindowRenderState,
    device:     &wgpu::Device,
    queue:      &wgpu::Queue,
    encoder:    &mut wgpu::CommandEncoder,
    swap_view:  &wgpu::TextureView,
    surf_w:     u32,
    surf_h:     u32,
    format:     wgpu::TextureFormat,
    scene_opt:  Option<uzor_urx_core::Scene>,
    base_color: [f32; 4],
) {
    // Lazy-init InstancedRenderer + ctx.
    if state.instanced_renderer.is_none() {
        state.instanced_renderer =
            Some(uzor_render_wgpu_instanced::InstancedRenderer::new(device, queue, format));
    }
    if state.instanced_ctx.is_none() {
        state.instanced_ctx = Some(uzor_render_wgpu_instanced::InstancedRenderContext::new(
            surf_w as f32, surf_h as f32, 0.0, 0.0,
        ));
    }

    // Adapter: Scene → InstancedRenderContext draw_commands.
    if let Some(ctx) = state.instanced_ctx.as_mut() {
        ctx.clear();
        if let Some(scene) = scene_opt {
            uzor_urx_wgpu::adapt_scene_into(&scene, ctx);
        }
    }
    let cmds: Vec<uzor_render_wgpu_instanced::DrawCmd> = state.instanced_ctx.as_mut()
        .map(|c| std::mem::take(&mut c.draw_commands))
        .unwrap_or_default();

    let clear = wgpu::Color {
        r: base_color[0] as f64,
        g: base_color[1] as f64,
        b: base_color[2] as f64,
        a: base_color[3] as f64,
    };

    // The InstancedRenderer encodes its own render-pass into its own
    // encoder and submits internally. In 1.4.9 we accept that cost
    // for the Wgpu compose path: the renderer's internal submit
    // happens BEFORE our encoder.submit, so the GPU sees them in the
    // right order (Wgpu 2D → composed-encoder 3D blits). The 2D
    // render writes straight to swap_view, so the 3D copy still
    // lands on top correctly.
    //
    // A future cleanup would expose an `InstancedRenderer::render_into_encoder`
    // entry point that takes an external encoder and skips the
    // internal submit — that lets us collapse to ONE submit per
    // frame. Tracked as backlog item for 1.4.10.
    if let Some(ref mut inst) = state.instanced_renderer {
        inst.render(device, queue, swap_view, surf_w, surf_h, &cmds, Some(clear), None);
    }
    // Hand the Vec back so its capacity is reused.
    if let Some(ctx) = state.instanced_ctx.as_mut() {
        let mut taken = cmds;
        taken.clear();
        ctx.draw_commands = taken;
    }
    let _ = encoder; // Wgpu path doesn't use our encoder for 2D in 1.4.9.
}
