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
//!   that does 2D first, then N×3D, then an optional post-3D 2D
//!   overlay, all in one encoder.
//!
//! ## Post-3D 2D overlay (label/hover-card gap, closed here)
//!
//! `submit_urx_composed`'s Phase 3 2D pass reads exclusively from
//! `state.urx_ctx`/`state.active_urx` — a channel a `Scene3D`-driven
//! consumer (e.g. `uzor-desktop::Manager`'s `run_with_3d` path) never
//! populates (that consumer skips the whole 2D chrome pass on a
//! 3D-active frame — see `uzor-desktop/CLAUDE.md`'s divergence log).
//! There was therefore no way to paint real 2D content (node labels, a
//! hover info card) ON TOP of a composed 3D viewport. The optional
//! `overlay` parameter below closes that gap: an additive Phase 4.5,
//! painted AFTER every [`Compose3DJob`]'s copy into `swap_view`, so it
//! composites over whatever the 3D pass drew, not under it. Wave 5b
//! (`urx-wave5-compose-cutover-design-2026-07-25.md`) cut this over
//! from an entirely separate, self-contained CPU `RenderContext`
//! ([`uzor_render_tiny_skia::TinySkiaCpuRenderContext`]) to
//! [`uzor_render_urx::UrxRenderContext`] — the SAME `RenderContext` ->
//! `Scene` bridge Phase 3's own `Wgpu`/`Auto` 2D channel already uses —
//! rendered through the SAME per-window `NativeUrxRenderer` Phase 3
//! drives (`compose_urx_native_into_swap`, Wave 5a). The overlay
//! records into its OWN separate texture (not `urx_ctx`'s own `Scene`
//! slot — a distinct recording each call), then composites onto
//! `swap_view` via the existing premultiplied-alpha blit
//! ([`blit_overlay_onto`]) — see [`build_overlay_texture_native`]'s own
//! doc comment for why the render pass's transparent-clear behavior is
//! exactly right for an overlay, not a problem to work around.
//!
//! Co-existence rule (doctrine): this path is opt-in. Consumers that
//! want fullscreen 3D keep calling `submit_3d_frame_to_rect`;
//! consumers that want fullscreen 2D keep calling `submit_frame`; this
//! new path is for the COMPOSE case.

use uzor::render::RenderContext;

use crate::factory::{
    Submit3DError, SurfaceMode, UrxComposeOverlayCache, UrxComposeOverlayDynamic, WindowRenderState,
};

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

/// One particle viewport contribution to a composed frame. Mirrors
/// [`Compose3DJob`] but carries a `ParticlesId.raw()` so the per-id
/// system (1.4.11 multi-emitter registry) is selected per job.
#[derive(Debug, Clone)]
pub struct ComposeParticlesJob {
    /// `ParticlesId.raw()` — selects the hub's per-id particle system.
    pub id: u64,
    /// Perspective camera for this viewport.
    pub camera: uzor_urx_3d::PerspectiveCamera,
    pub dst_x: u32,
    pub dst_y: u32,
    pub dst_w: u32,
    pub dst_h: u32,
}

/// Cache-keyed overlay contribution for composed 3D windows.
///
/// `paint` is invoked only when `key` changes or the surface is resized.
/// Use it for stable chrome such as toolbars and legends; keep animated or
/// selection-dependent content in `submit_urx_composed`'s ordinary overlay.
pub struct CachedOverlayJob {
    pub key: u64,
    pub paint: Box<dyn FnMut(&mut dyn RenderContext)>,
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
///   (caller did `set_active_urx(Some(b))` before paint). The 2D pass is
///   wired for every backend: `Cpu` rasterises the chrome through the
///   CPU URX backend; `Hybrid`/`WgpuFull`/`Auto` (Wave 5a,
///   `urx-wave5-compose-cutover-design-2026-07-25.md`) draw it through
///   the native Wave 1-4 `NativeUrxRenderer` pipeline set instead
///   (`compose_urx_native_into_swap`); `Wgpu` draws it via the
///   instanced renderer (legacy adapter path, untouched — Wave 5 scopes
///   only Hybrid/WgpuFull/Auto). The 3D viewports always render
///   natively via `Renderer3D`.
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
/// 4. If `cached_overlay` is `Some`, reuse its retained texture while
///    its key and surface size match; otherwise paint and upload it once.
///    Then paint `overlay`, when present, into a fresh transparent CPU
///    `RenderContext` and alpha-composite it above the cached layer.
/// 5. One `queue.submit`, one `present`.
///
/// `overlay` is taken BY VALUE (owned `Box`, not a borrowed
/// `&mut dyn FnMut(...)`) — this call is the only place it's ever
/// invoked, so an owned closure sidesteps threading a borrowed trait
/// object's lifetime back out through this function's signature. It
/// receives `&mut dyn RenderContext` in the SAME logical/physical 1:1
/// pixel space as `surf_w`/`surf_h` (no dpr scaling is applied here) —
/// the same space `uzor-graph`'s `GraphEngine3D::visible_labels`/
/// `draw_overlay` already project against via the job's own
/// `camera`/viewport, so a caller building the overlay from that engine
/// needs no coordinate translation.
pub fn submit_urx_composed(
    state:      &mut WindowRenderState,
    base_color: [f32; 4],
    jobs:       &[Compose3DJob],
    overlay: Option<Box<dyn FnMut(&mut dyn RenderContext)>>,
    cached_overlay: Option<CachedOverlayJob>,
) -> Result<ComposedOutcome, Submit3DError> {
    submit_urx_composed_impl(
        state,
        None,
        base_color,
        jobs,
        overlay,
        cached_overlay,
    )
}

/// Composes a frame from a caller-owned or caller-retained scene.
///
/// Unlike [`submit_urx_composed`], this path borrows `scene` directly
/// instead of requiring a move into the hub-owned scene slot. This lets
/// retained [`std::sync::Arc`] scene storage be reused across frames
/// without cloning its node buffers.
pub fn submit_urx_composed_with_scene(
    state:      &mut WindowRenderState,
    scene:      &uzor_urx_3d::Scene3D,
    base_color: [f32; 4],
    jobs:       &[Compose3DJob],
    overlay: Option<Box<dyn FnMut(&mut dyn RenderContext)>>,
    cached_overlay: Option<CachedOverlayJob>,
) -> Result<ComposedOutcome, Submit3DError> {
    submit_urx_composed_impl(
        state,
        Some(scene),
        base_color,
        jobs,
        overlay,
        cached_overlay,
    )
}

fn submit_urx_composed_impl(
    state:      &mut WindowRenderState,
    scene_override: Option<&uzor_urx_3d::Scene3D>,
    base_color: [f32; 4],
    jobs:       &[Compose3DJob],
    mut overlay: Option<Box<dyn FnMut(&mut dyn RenderContext)>>,
    mut cached_overlay: Option<CachedOverlayJob>,
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

    // Screenshot mirror — ensure BEFORE the surface borrow below. The
    // 2D + 3D phases each dup their output into it when present.
    if state.capture_3d_enabled {
        state.ensure_capture_3d(&device, surf_w, surf_h, surface_format);
    }

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
    // encoder). Wgpu/Hybrid/WgpuFull/Auto (Wave 5a + Wave 7 tail
    // 2026-07-24) all drive the Wave 1-4 `NativeUrxRenderer` via
    // `render_into_encoder` into `surface.target_view`, then reuse the
    // SAME blit tail the Cpu arm uses — see
    // `compose_urx_native_into_swap`'s own doc comment. Wgpu's own
    // legacy `InstancedRenderer`-backed compose path
    // (`compose_urx_wgpu_into_swap`, adapter-mediated) was retired in
    // that same pass — the native pipelines already cover this arm at
    // full parity (`uzor-urx-wgpu/tests/parity.rs`, 28/28) and the
    // adapter path added a second, less-capable renderer + a whole
    // extra internal `queue.submit` this compose frame didn't need.
    let urx_backend = state.active_urx.unwrap_or(uzor::UrxBackend::Cpu);
    let backend_resolved = match urx_backend {
        uzor::UrxBackend::Auto => uzor::UrxBackend::Wgpu, // GPU surface → wgpu
        b => b,
    };

    // Dead-pass elimination (perf pass 2026-07-24): when there is NO 2D
    // scene to draw AND some 3D job covers the ENTIRE swapchain, the 2D
    // pass would only produce a full-surface base-color fill that Phase
    // 4's whole-surface `copy_texture_to_texture` then overwrites pixel
    // for pixel. On the CPU arm that fill is real, per-frame work — a
    // full-surface pixmap memset + a full-surface `write_texture` upload
    // + a blit (~10 MB of pure PCIe traffic per frame at 1400×900) for
    // zero visible pixels. `uzor-desktop`'s `Manager` never arms
    // `urx_ctx`, so every composed 3D window hit this every frame. A
    // window with a PARTIAL 3D rect (or any real 2D scene) keeps the 2D
    // pass — it genuinely owns the pixels outside the job rect there.
    let full_cover_3d_job = jobs.iter().any(|job| {
        job.dst_x == 0 && job.dst_y == 0 && job.dst_w >= surf_w && job.dst_h >= surf_h
    });
    let skip_2d_pass = urx_scene_opt.is_none() && full_cover_3d_job;

    if !skip_2d_pass {
        match backend_resolved {
            uzor::UrxBackend::Cpu => {
                compose_urx_cpu_into_swap(
                    state, &device, &queue, &mut encoder, &swap_view,
                    surf_w, surf_h, urx_scene_opt, base_color,
                );
            }
            uzor::UrxBackend::Wgpu | uzor::UrxBackend::Hybrid | uzor::UrxBackend::WgpuFull | uzor::UrxBackend::Auto => {
                // Wave 5a cutover (Hybrid/WgpuFull/Auto) + Wave 7 tail
                // (Wgpu, 2026-07-24): `HybridBackend`/`WgpuFullBackend`
                // themselves are still untouched (their OWN monolithic
                // submit paths do accept an external `&mut encoder`+view,
                // `submit_urx.rs`'s `WgpuFullBackend::submit`/
                // `HybridBackend::composite` — that description was
                // already stale even before Wave 5a). For the COMPOSE
                // case, every one of these arms routes the chrome pass
                // through the already-built Wave 1-4 `NativeUrxRenderer`
                // instead — the crate the SR3 plan always meant here —
                // via `render_into_encoder`. The 3D viewports still
                // render natively via `Renderer3D` in Phase 4, unchanged.
                compose_urx_native_into_swap(
                    state, &device, &queue, &mut encoder, &swap_view,
                    surf_w, surf_h, urx_scene_opt, base_color,
                );
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
            let mut r3d = uzor_urx_3d::Renderer3D::new(&device, &queue, surface_format, (dw, dh), 1024);
            // Owner-ordered live fix ("линии глитчуют") — this is the
            // ONE live compose path a visible 3D window actually renders
            // through (confirmed against `uzor-desktop`'s own divergence
            // log: `Manager` calls `submit_urx_composed`, never the
            // `with_renderer_3d`/`submit_3d_frame`/`submit_3d_frame_to_rect`
            // sibling entry points in `factory.rs`), so arming the
            // owner's own requested default (4x MSAA) here, once, at
            // lazy-init, is the smallest real path that actually
            // improves what's on screen. Falls back to single-sample
            // automatically for any scene `Renderer3D::render_inner`'s
            // own MSAA gate doesn't cover (transparent/textured/pbr
            // content) — never a crash, just no AA for that frame.
            //
            // Graph-strengthening arc item 5: the sample count is now
            // `state.urx_compose_msaa_sample_count`
            // ([`WindowRenderState::set_compose_msaa_sample_count`]) —
            // was a bare literal `4` here. Default is still exactly `4`
            // for any consumer that never calls the setter, so this is a
            // configurability addition, not a behavior change.
            r3d.set_sample_count(&device, state.urx_compose_msaa_sample_count);
            // Same item — an already-`pub` `Renderer3D::set_edge_width_px`
            // that nothing at this lazy-init call site ever invoked.
            // `None` (the default) leaves `Renderer3D::new`'s own default
            // untouched.
            if let Some(px) = state.urx_compose_edge_width_px {
                r3d.set_edge_width_px(px);
            }
            state.urx_renderer_3d = Some(r3d);
        }
        if scene_override.is_none() && state.urx_scene_3d.is_none() {
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
            let scene = match scene_override.or(state.urx_scene_3d.as_ref()) {
                Some(s) => s,
                None => { jobs_skipped += 1; continue; }
            };
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
        // Mirror the same rect into the capture texture (screenshot pipe).
        if let Some(cap) = state.urx_capture_3d.as_ref() {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture:   &off.texture,
                    mip_level: 0,
                    origin:    wgpu::Origin3d::ZERO,
                    aspect:    wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture:   &cap.texture,
                    mip_level: 0,
                    origin:    wgpu::Origin3d { x: dx, y: dy, z: 0 },
                    aspect:    wgpu::TextureAspect::All,
                },
                wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
            );
        }
        jobs_rendered += 1;
    }

    // ── Phase 4.5: optional cached + dynamic 2D overlays over 3D ─────
    // Stable chrome is rendered only on cache miss or resize. Dynamic
    // content is painted after it so labels, selection cards and
    // crosshairs remain on top. One retained blitter handles both
    // layers. Wave 5b (`urx-wave5-compose-cutover-design-2026-07-25.md`
    // §4 5b): both layers now record through
    // `uzor_render_urx::UrxRenderContext` and render through dedicated
    // per-layer `NativeUrxRenderer` instances, instead of rasterising
    // into a CPU `tiny-skia` pixmap and uploading it. Separate renderers
    // are required because Phase 3, cached overlay refresh, and dynamic
    // overlay can all record into this encoder before its one submit;
    // a later renderer upload must not overwrite an earlier pass's
    // still-pending instance buffers.
    if let Some(cached_job) = cached_overlay.as_mut() {
        let needs_refresh = state.urx_compose_overlay_cache.as_ref().is_none_or(|cache| {
            cache.key != cached_job.key || cache.width != surf_w || cache.height != surf_h
        });
        if needs_refresh {
            // Lazy-init an independent buffer arena. Phase 3 and the
            // dynamic overlay can both record later in the same encoder.
            if state.urx_cached_overlay_renderer.is_none() {
                state.urx_cached_overlay_renderer = Some(uzor_urx_wgpu::NativeUrxRenderer::new(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Rgba8Unorm,
                ));
            }
            let uploaded = match state.urx_cached_overlay_renderer.as_mut() {
                Some(renderer) => build_overlay_texture_native(
                    &device,
                    &queue,
                    &mut encoder,
                    renderer,
                    surf_w,
                    surf_h,
                    cached_job.paint.as_mut(),
                ),
                None => None,
            };
            state.urx_compose_overlay_cache = uploaded.map(|uploaded| UrxComposeOverlayCache {
                key: cached_job.key,
                texture: uploaded.texture,
                view: uploaded.view,
                width: surf_w,
                height: surf_h,
            });
        }
        if let Some(cache) = state.urx_compose_overlay_cache.as_ref() {
            blit_overlay_onto(
                &device,
                &mut encoder,
                &cache.view,
                &swap_view,
                surface_format,
                &mut state.urx_compose_overlay_blitter,
            );
            if let Some(cap) = state.urx_capture_3d.as_ref() {
                blit_overlay_onto(
                    &device,
                    &mut encoder,
                    &cache.view,
                    &cap.view,
                    surface_format,
                    &mut state.urx_compose_overlay_blitter,
                );
            }
        }
    }

    if let Some(overlay_fn) = overlay.as_mut() {
        if surf_w > 0 && surf_h > 0 {
            // Lazy-init an independent buffer arena. Neither Phase 3
            // nor the cached-overlay branch is guaranteed to run, but
            // either may already have recorded work into this encoder.
            if state.urx_dynamic_overlay_renderer.is_none() {
                state.urx_dynamic_overlay_renderer = Some(uzor_urx_wgpu::NativeUrxRenderer::new(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Rgba8Unorm,
                ));
            }

            let mut ctx = uzor_render_urx::UrxRenderContext::new(1.0);
            ctx.begin_frame(surf_w, surf_h);
            overlay_fn(&mut ctx);
            let scene = ctx.take_scene();

            // Reuse the persistent dynamic-overlay texture across frames
            // (perf pass 2026-07-24, PRESERVED under this native cutover
            // — a deliberate deviation from the design's literal
            // "wire `build_overlay_texture_native` into both call
            // sites" wording, see this module's own doc comment on
            // `build_overlay_texture_native` for why): the CONTENT
            // re-renders every frame via `render_into_encoder`, but the
            // texture object itself is reused; recreated only on a
            // surface resize, exactly as before — only WHAT populates
            // it changed (a native render pass instead of a tiny-skia
            // raster + `queue.write_texture` upload).
            let needs_new = state
                .urx_compose_overlay_dynamic
                .as_ref()
                .is_none_or(|d| d.width != surf_w || d.height != surf_h);
            if needs_new {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("uzor-render-hub:compose-overlay-dynamic"),
                    size: wgpu::Extent3d { width: surf_w, height: surf_h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: OVERLAY_TEXTURE_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                state.urx_compose_overlay_dynamic =
                    Some(UrxComposeOverlayDynamic { texture, view, width: surf_w, height: surf_h });
            }
            if let Some(dynamic) = state.urx_compose_overlay_dynamic.as_ref() {
                if let Some(renderer) = state.urx_dynamic_overlay_renderer.as_mut() {
                    if let Err(e) = renderer.render_into_encoder(
                        &scene,
                        &mut encoder,
                        &dynamic.view,
                        uzor_urx_wgpu::Viewport { width: surf_w, height: surf_h },
                    ) {
                        eprintln!("[render-hub] compose urx-native dynamic-overlay render error: {:?}", e);
                    }
                }
                blit_overlay_onto(
                    &device,
                    &mut encoder,
                    &dynamic.view,
                    &swap_view,
                    surface_format,
                    &mut state.urx_compose_overlay_blitter,
                );
                if let Some(cap) = state.urx_capture_3d.as_ref() {
                    blit_overlay_onto(
                        &device,
                        &mut encoder,
                        &dynamic.view,
                        &cap.view,
                        surface_format,
                        &mut state.urx_compose_overlay_blitter,
                    );
                }
            }
        }
    }

    // ── Phase 5: one submit, one present ────────────────────────────
    queue.submit([encoder.finish()]);
    frame.present();

    Ok(ComposedOutcome { jobs_rendered, jobs_skipped, surface_lost: false })
}

/// Compose N particle viewports into ONE swapchain frame — single
/// acquire, background clear, per-job particle render, single present.
///
/// Replaces calling [`WindowRenderState::submit_particles_to_rect`] once
/// per emitter: that path did its own `get_current_texture` + `present`
/// per call, so two emitters in a window issued TWO presents on one
/// swapchain frame (undefined → hard flicker) AND each only copied its
/// own sub-rect into an otherwise-uninitialised swapchain texture (the
/// area outside the rect flickered between recycled frames even with a
/// single emitter). This path clears the whole frame to `base_color`
/// first, then composites every emitter's offscreen over it, then
/// presents once — flicker-free for any emitter count.
///
/// Each job's particle system is selected by `job.id` from the per-id
/// registry (1.4.11). Jobs whose rect clips to zero or whose id is
/// uninitialised are skipped.
pub fn submit_particles_composed(
    state:      &mut WindowRenderState,
    base_color: [f32; 4],
    jobs:       &[ComposeParticlesJob],
) -> Result<ComposedOutcome, Submit3DError> {
    let (surf_w, surf_h, surface_format) = match &state.surface {
        SurfaceMode::Gpu { surface, .. } => (
            surface.config.width, surface.config.height, surface.config.format,
        ),
        _ => return Err(Submit3DError::NotGpuSurface),
    };
    if surf_w == 0 || surf_h == 0 { return Err(Submit3DError::ZeroSizedSurface); }

    let (device, queue) = match &state.surface {
        SurfaceMode::Gpu { gpu_pool, dev_id, .. } => (
            gpu_pool.devices[*dev_id].device.clone(),
            gpu_pool.devices[*dev_id].queue.clone(),
        ),
        _ => return Err(Submit3DError::NotGpuSurface),
    };

    if state.capture_3d_enabled {
        state.ensure_capture_3d(&device, surf_w, surf_h, surface_format);
    }

    // Acquire the frame.
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

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("uzor-render-hub:particles-compose"),
    });

    // Phase: clear the whole frame so areas outside every viewport are
    // a stable background instead of a recycled swapchain texture.
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particles-compose:clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &swap_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: base_color[0] as f64, g: base_color[1] as f64,
                        b: base_color[2] as f64, a: base_color[3] as f64,
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
    }
    // Mirror clear into the capture texture too (so areas outside the
    // viewports aren't stale in screenshots).
    if let Some(cap) = state.urx_capture_3d.as_ref() {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particles-compose:clear-capture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &cap.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: base_color[0] as f64, g: base_color[1] as f64,
                        b: base_color[2] as f64, a: base_color[3] as f64,
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
    }

    // Lazy renderer + empty scene (particles need a Renderer3D + a scene
    // to draw "over"; the scene stays empty for pure-particle viewports).
    if state.urx_renderer_3d.is_none() {
        state.urx_renderer_3d = Some(uzor_urx_3d::Renderer3D::new(
            &device, &queue, surface_format, (surf_w.max(1), surf_h.max(1)), 1024,
        ));
    }
    if state.urx_scene_3d.is_none() {
        state.urx_scene_3d = Some(uzor_urx_3d::Scene3D::new());
    }

    let mut jobs_rendered = 0u32;
    let mut jobs_skipped  = 0u32;

    for job in jobs {
        let dx = job.dst_x.min(surf_w.saturating_sub(1));
        let dy = job.dst_y.min(surf_h.saturating_sub(1));
        let dw = job.dst_w.min(surf_w.saturating_sub(dx));
        let dh = job.dst_h.min(surf_h.saturating_sub(dy));
        if dw == 0 || dh == 0 { jobs_skipped += 1; continue; }
        if !state.urx_particles.contains_key(&job.id) { jobs_skipped += 1; continue; }

        // Resize the shared offscreen to this job's rect.
        let need_new = match &state.urx_offscreen_3d {
            None => true,
            Some(o) => o.width != dw || o.height != dh || o.format != surface_format,
        };
        if need_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("urx-particles-offscreen-compose"),
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

        // Render this id's particles into the offscreen (over the empty scene).
        {
            let r3d   = match state.urx_renderer_3d.as_mut()   { Some(r) => r, None => { jobs_skipped += 1; continue; } };
            let scene = match state.urx_scene_3d.as_ref()       { Some(s) => s, None => { jobs_skipped += 1; continue; } };
            let parts = match state.urx_particles.get(&job.id)  { Some(p) => p, None => { jobs_skipped += 1; continue; } };
            let off   = match state.urx_offscreen_3d.as_ref()   { Some(o) => o, None => { jobs_skipped += 1; continue; } };
            r3d.render_with_particles(&device, &queue, &mut encoder, &off.view, &job.camera, scene, parts);
        }

        // Copy offscreen → swapchain at (dx,dy), and into the mirror.
        let off = state.urx_offscreen_3d.as_ref().unwrap();
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo { texture: &off.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyTextureInfo { texture: &frame.texture, mip_level: 0, origin: wgpu::Origin3d { x: dx, y: dy, z: 0 }, aspect: wgpu::TextureAspect::All },
            wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
        );
        if let Some(cap) = state.urx_capture_3d.as_ref() {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo { texture: &off.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyTextureInfo { texture: &cap.texture, mip_level: 0, origin: wgpu::Origin3d { x: dx, y: dy, z: 0 }, aspect: wgpu::TextureAspect::All },
                wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
            );
        }
        jobs_rendered += 1;
    }

    queue.submit([encoder.finish()]);
    frame.present();
    Ok(ComposedOutcome { jobs_rendered, jobs_skipped, surface_lost: false })
}

// ── 2D pass helpers ─────────────────────────────────────────────────

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
    // Mirror the 2D layer into the capture texture (screenshot pipe).
    // Disjoint field borrow — `surface` is state.surface, `cap` is
    // state.urx_capture_3d.
    if let Some(cap) = state.urx_capture_3d.as_ref() {
        surface.blitter.copy(_device, encoder, &surface.target_view, &cap.view);
    }
    let _ = base_color;
}

/// Pure, GPU-free — builds the synthetic-background-prepended `Scene`
/// [`compose_urx_native_into_swap`] renders (Wave 5a,
/// `urx-wave5-compose-cutover-design-2026-07-25.md` §4 step 2). Split
/// out so the "does this compose the right command order" logic is
/// unit-testable without a device (see this module's own
/// `#[cfg(test)] mod tests` below).
///
/// `NativeUrxRenderer::render_into_encoder`'s root pass ALWAYS clears
/// its own color attachment to `wgpu::Color::TRANSPARENT` and the
/// eventual MSAA resolve (or direct `Store` at `sample_count == 1`)
/// OVERWRITES the destination view — it never blends with whatever
/// `target_view` held before. A caller-supplied clear color isn't part
/// of `render_into_encoder`'s signature (deliberately — it's a pure
/// `Scene -> pixels` entry point, no side-channel state). The CPU arm
/// ([`compose_urx_cpu_into_swap`]) gets its background "for free" by
/// filling the pixmap directly; this fn gets the identical effect by
/// making the background CONTENT — a `FillRect` first in painter's
/// order, same IR every other opaque background in this codebase
/// already uses, same `bg_rgba` u8-quantization
/// [`compose_urx_cpu_into_swap`] already uses.
fn scene_with_opaque_background(
    surf_w: u32,
    surf_h: u32,
    base_color: [f32; 4],
    scene_opt: Option<uzor_urx_core::Scene>,
) -> uzor_urx_core::Scene {
    let bg_rgba = [
        (base_color[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (base_color[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (base_color[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (base_color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ];

    let mut scene = uzor_urx_core::Scene::new();
    scene.push(uzor_urx_core::DrawCommand::FillRect {
        rect: uzor_urx_core::Rect::new(0.0, 0.0, surf_w as f64, surf_h as f64),
        radii: None,
        brush: uzor_urx_core::math::Brush::Solid(uzor_urx_core::math::Color::from_rgba8(
            bg_rgba[0], bg_rgba[1], bg_rgba[2], bg_rgba[3],
        )),
        transform: uzor_urx_core::Affine::IDENTITY,
    });
    scene.commands.extend(scene_opt.map(|s| s.commands).unwrap_or_default());
    scene
}

/// Native-GPU URX 2D pass (Wave 5a) — drives the already-built Wave 1-4
/// `NativeUrxRenderer` (`uzor-urx-wgpu`) directly into the existing
/// intermediate `surface.target_view`, then reuses the EXISTING
/// blitter tail verbatim (byte-identical to
/// [`compose_urx_cpu_into_swap`]'s own tail, just fed by a
/// GPU-rendered `target_view` instead of an uploaded CPU pixmap).
///
/// `NativeUrxRenderer` is built at a FIXED
/// `wgpu::TextureFormat::Rgba8Unorm` — NEVER the swapchain's own,
/// possibly-sRGB format (design §1: the fragment shaders assume
/// linear-arithmetic-on-Unorm-bytes — premultiplication math, gradient
/// LUT sampling, glyph coverage — an sRGB-tagged target would apply an
/// implicit gamma-encode on store, double-processing colors the
/// shaders already computed correctly). `surface.target_texture`/
/// `target_view` are unconditionally recreated at exactly
/// `Rgba8Unorm` with `RENDER_ATTACHMENT` usage by
/// `recreate_target_with_cpu_usage` on every GPU surface regardless of
/// active backend, so this is a format match by construction, not a
/// coincidence — see risk 1 in the design doc for the one theoretical
/// pre-first-resize edge case (a typed, loggable `FormatMismatch`
/// error, never a panic).
fn compose_urx_native_into_swap(
    state:      &mut WindowRenderState,
    device:     &wgpu::Device,
    queue:      &wgpu::Queue,
    encoder:    &mut wgpu::CommandEncoder,
    swap_view:  &wgpu::TextureView,
    surf_w:     u32,
    surf_h:     u32,
    scene_opt:  Option<uzor_urx_core::Scene>,
    base_color: [f32; 4],
) {
    // The ordinary 2D submit path shares this renderer slot and may have
    // resolved its own SubmitParams to 1x. Compose has an independent
    // owner-configured AA setting, so rebuild only when the resolved native
    // sample count differs; otherwise the first path used by a window would
    // accidentally choose AA for every later path.
    let sample_count = if state.urx_compose_msaa_sample_count > 1 { 4 } else { 1 };
    if state
        .urx_native_renderer
        .as_ref()
        .is_none_or(|renderer| renderer.sample_count() != sample_count)
    {
        state.urx_native_renderer = Some(uzor_urx_wgpu::NativeUrxRenderer::with_sample_count(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
            sample_count,
        ));
    }

    let scene = scene_with_opaque_background(surf_w, surf_h, base_color, scene_opt);

    let SurfaceMode::Gpu { surface, .. } = &mut state.surface else { return };
    if let Some(renderer) = state.urx_native_renderer.as_mut() {
        if let Err(e) = renderer.render_into_encoder(
            &scene,
            encoder,
            &surface.target_view,
            uzor_urx_wgpu::Viewport { width: surf_w, height: surf_h },
        ) {
            eprintln!("[render-hub] compose urx-native render error: {:?}", e);
        }
    }

    // Blit target_texture → swap_view in our encoder — SAME tail as
    // compose_urx_cpu_into_swap, just fed by the native renderer's
    // GPU-written target_view instead of an uploaded CPU pixmap.
    surface.blitter.copy(device, encoder, &surface.target_view, swap_view);
    // Mirror the 2D layer into the capture texture (screenshot pipe).
    // Disjoint field borrow — `surface` is state.surface, `cap` is
    // state.urx_capture_3d (same pattern `compose_urx_cpu_into_swap`
    // already uses).
    if let Some(cap) = state.urx_capture_3d.as_ref() {
        surface.blitter.copy(device, encoder, &surface.target_view, &cap.view);
    }
}

// ── Post-3D 2D overlay pass ──────────────────────────────────────────

/// Overlay texture format — must equal `NativeUrxRenderer`'s own fixed
/// format (`wgpu::TextureFormat::Rgba8Unorm`, design §1) since Wave 5b:
/// `render_into_encoder` rejects a target view whose texture format
/// doesn't match the renderer it was built with
/// (`NativeRenderError::FormatMismatch`). Also doesn't need to match
/// the swapchain format for the SEPARATE reason
/// [`wgpu::util::TextureBlitter`]'s fragment shader just samples it as
/// `texture_2d<f32>` — only the BLIT TARGET has to match the format the
/// blitter's pipeline was built for (see [`blit_overlay_onto`]).
const OVERLAY_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct UploadedOverlay {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// Native-GPU overlay builder (Wave 5b,
/// `urx-wave5-compose-cutover-design-2026-07-25.md` §4 5b step 2) —
/// records `overlay_fn`'s draw calls through
/// [`uzor_render_urx::UrxRenderContext`] (the SAME `RenderContext` ->
/// `Scene` bridge Phase 3's own `Wgpu`/`Auto` 2D channel already uses)
/// instead of rasterising into a CPU `tiny-skia` pixmap, then renders
/// the resulting `Scene` through `renderer` (the dedicated retained-
/// overlay `NativeUrxRenderer`) into a FRESH sampleable texture.
/// Returns `None` for a zero-sized surface (nothing to paint) — this is
/// the cached-overlay call site's exact drop-in replacement for the
/// prior `build_overlay_texture` (`needs_refresh`-gated, so a fresh
/// allocation per actual refresh is cheap in aggregate); the dynamic
/// (per-frame) overlay call site does NOT route through this fn — see
/// that call site's own comment for why it keeps a persistent texture
/// instead.
///
/// **No synthetic background needed here** (unlike Phase 3's
/// `scene_with_opaque_background`): `render_into_encoder`'s own root
/// pass ALWAYS clears its target to `wgpu::Color::TRANSPARENT` — for an
/// overlay that must let the 3D content underneath show through, that
/// is EXACTLY the right behavior, the mirror image of Phase 3's own
/// problem. This texture is a SEPARATE render target from `swap_view`/
/// the 3D content already drawn there — clearing IT to transparent
/// never erases anything; the actual "composite over 3D" step happens
/// afterward, in the caller, via the EXISTING [`blit_overlay_onto`]
/// (`LoadOp::Load` + premultiplied blend against whatever `swap_view`
/// already holds from Phase 4). `NativeUrxRenderer`'s fragment shaders
/// already output premultiplied color (Wave 1 design §7 "the actual
/// fix"), so [`blit_overlay_onto`]'s existing `PREMULTIPLIED_ALPHA_BLENDING`
/// state is already correct for this native-sourced content too — not
/// just the CPU-sourced content it was originally built for.
fn build_overlay_texture_native(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    renderer: &mut uzor_urx_wgpu::NativeUrxRenderer,
    surf_w: u32,
    surf_h: u32,
    overlay_fn: &mut dyn FnMut(&mut dyn RenderContext),
) -> Option<UploadedOverlay> {
    if surf_w == 0 || surf_h == 0 {
        return None;
    }

    let mut ctx = uzor_render_urx::UrxRenderContext::new(1.0);
    ctx.begin_frame(surf_w, surf_h);
    overlay_fn(&mut ctx);
    let scene = ctx.take_scene();

    let overlay_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-render-hub:compose-overlay"),
        size: wgpu::Extent3d { width: surf_w, height: surf_h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OVERLAY_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = overlay_tex.create_view(&wgpu::TextureViewDescriptor::default());

    if let Err(e) = renderer.render_into_encoder(
        &scene,
        encoder,
        &view,
        uzor_urx_wgpu::Viewport { width: surf_w, height: surf_h },
    ) {
        eprintln!("[render-hub] compose urx-native overlay render error: {:?}", e);
    }

    Some(UploadedOverlay { texture: overlay_tex, view })
}

/// Alpha-composite `overlay_view` ON TOP of whatever is already in
/// `target_view` (`wgpu::util::TextureBlitter::copy`'s own render pass
/// uses `LoadOp::Load`, never `Clear` — confirmed by direct read of
/// `wgpu::util::texture_blitter`) — this is what makes the overlay a
/// true OVER composite against the already-drawn 3D result rather than
/// an opaque overwrite.
///
/// `PREMULTIPLIED_ALPHA_BLENDING`, not the default straight-alpha
/// blend: the overlay's source is `NativeUrxRenderer`'s own
/// premultiplied fragment-shader output (Wave 5b — was tiny-skia's own
/// premultiplied RGBA8 pixmap before), and the blit shader is a plain
/// `textureSample`-and-output pass-through (no unpremultiply step) —
/// straight-alpha blending against already-premultiplied source data
/// would double-apply the alpha and darken every translucent pixel.
///
/// The blitter and its render pipeline are retained per window and rebuilt
/// only when the target format changes.
fn blit_overlay_onto(
    device:       &wgpu::Device,
    encoder:      &mut wgpu::CommandEncoder,
    overlay_view: &wgpu::TextureView,
    target_view:  &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    cached_blitter: &mut Option<(wgpu::TextureFormat, wgpu::util::TextureBlitter)>,
) {
    let needs_rebuild = cached_blitter.as_ref().is_none_or(|(format, _)| *format != target_format);
    if needs_rebuild {
        let blitter = wgpu::util::TextureBlitterBuilder::new(device, target_format)
            .blend_state(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
            .build();
        *cached_blitter = Some((target_format, blitter));
    }
    if let Some((_, blitter)) = cached_blitter.as_ref() {
        blitter.copy(device, encoder, overlay_view, target_view);
    }
}

// ── Wave 5a: `scene_with_opaque_background` unit test ────────────────
//
// The one piece of NEW logic Wave 5a introduces — everything
// downstream of it (`NativeUrxRenderer::render_into_encoder` itself)
// is already proven by `uzor-urx-wgpu`'s 27/27 parity suite. No GPU
// adapter needed.
#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-render-hub-compose-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    fn rgba_texture(
        device: &wgpu::Device,
        label: &'static str,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn readback_pixel(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let aligned_stride = (width * 4 + 255) & !255;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uzor-render-hub-compose-test-readback"),
            size: (aligned_stride * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_stride),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        rx.recv()
            .expect("map_async callback channel closed before firing")
            .expect("staging buffer map failed");
        let mapped = slice.get_mapped_range();
        let offset = (y * aligned_stride + x * 4) as usize;
        let pixel = [mapped[offset], mapped[offset + 1], mapped[offset + 2], mapped[offset + 3]];
        drop(mapped);
        staging.unmap();
        pixel
    }

    fn solid_scene(width: u32, height: u32, color: [u8; 4]) -> uzor_urx_core::Scene {
        let mut scene = uzor_urx_core::Scene::new();
        scene.push(uzor_urx_core::DrawCommand::FillRect {
            rect: uzor_urx_core::Rect::new(0.0, 0.0, width as f64, height as f64),
            radii: None,
            brush: uzor_urx_core::math::Brush::Solid(
                uzor_urx_core::math::Color::from_rgba8(color[0], color[1], color[2], color[3]),
            ),
            transform: uzor_urx_core::Affine::IDENTITY,
        });
        scene
    }

    fn probe_scene() -> uzor_urx_core::Scene {
        let mut scene = uzor_urx_core::Scene::new();
        scene.push(uzor_urx_core::DrawCommand::FillRect {
            rect: uzor_urx_core::Rect::new(1.0, 2.0, 3.0, 4.0),
            radii: None,
            brush: uzor_urx_core::math::Brush::Solid(uzor_urx_core::math::Color::from_rgba8(9, 8, 7, 6)),
            transform: uzor_urx_core::Affine::IDENTITY,
        });
        scene
    }

    #[test]
    fn opaque_background_is_first_and_consumer_commands_follow_in_order() {
        let scene = scene_with_opaque_background(64, 32, [0.5, 0.25, 0.75, 1.0], Some(probe_scene()));
        assert_eq!(scene.commands.len(), 2, "background + the one consumer command");

        match &scene.commands[0] {
            uzor_urx_core::DrawCommand::FillRect { rect, radii, brush, transform } => {
                assert_eq!(*rect, uzor_urx_core::Rect::new(0.0, 0.0, 64.0, 32.0), "background must cover the full surface");
                assert!(radii.is_none(), "background is a plain rect, no radii");
                assert_eq!(*transform, uzor_urx_core::Affine::IDENTITY);
                match brush {
                    uzor_urx_core::math::Brush::Solid(c) => {
                        let rgba = c.to_rgba8();
                        // round(0.5*255)=128, round(0.25*255)=64,
                        // round(0.75*255)=191, round(1.0*255)=255 —
                        // same u8-quantization `compose_urx_cpu_into_swap`'s
                        // own `bg_rgba` already uses.
                        assert_eq!([rgba.r, rgba.g, rgba.b, rgba.a], [128, 64, 191, 255]);
                    }
                    other => panic!("background brush must be Solid, got {other:?}"),
                }
            }
            other => panic!("first command must be the background FillRect, got {other:?}"),
        }

        match &scene.commands[1] {
            uzor_urx_core::DrawCommand::FillRect { rect, .. } => {
                assert_eq!(*rect, uzor_urx_core::Rect::new(1.0, 2.0, 3.0, 4.0), "consumer command must follow unchanged, in order");
            }
            other => panic!("second command must be the consumer's own FillRect, got {other:?}"),
        }
    }

    #[test]
    fn none_scene_emits_only_the_background_command() {
        let scene = scene_with_opaque_background(10, 20, [1.0, 1.0, 1.0, 1.0], None);
        assert_eq!(scene.commands.len(), 1, "no consumer scene -> just the background");
        assert!(matches!(&scene.commands[0], uzor_urx_core::DrawCommand::FillRect { .. }));
    }

    #[test]
    fn background_rect_tracks_the_supplied_surface_size() {
        let scene = scene_with_opaque_background(1920, 1080, [0.0, 0.0, 0.0, 1.0], None);
        match &scene.commands[0] {
            uzor_urx_core::DrawCommand::FillRect { rect, .. } => {
                assert_eq!(*rect, uzor_urx_core::Rect::new(0.0, 0.0, 1920.0, 1080.0));
            }
            _ => unreachable!("checked above"),
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn independent_native_renderers_preserve_cached_and_dynamic_overlay_payloads_before_single_submit() {
        let Some((device, queue)) = test_device() else { return };
        const SIZE: u32 = 32;
        let (cached_texture, cached_view) =
            rgba_texture(&device, "uzor-render-hub-compose-test-cached", SIZE, SIZE);
        let (dynamic_texture, dynamic_view) =
            rgba_texture(&device, "uzor-render-hub-compose-test-dynamic", SIZE, SIZE);
        let mut cached_renderer = uzor_urx_wgpu::NativeUrxRenderer::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let mut dynamic_renderer = uzor_urx_wgpu::NativeUrxRenderer::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let cached_scene = solid_scene(SIZE, SIZE, [210, 35, 45, 255]);
        let dynamic_scene = solid_scene(SIZE, SIZE, [35, 210, 80, 255]);
        let viewport = uzor_urx_wgpu::Viewport { width: SIZE, height: SIZE };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uzor-render-hub-compose-test-shared-encoder"),
        });

        cached_renderer
            .render_into_encoder(&cached_scene, &mut encoder, &cached_view, viewport)
            .expect("cached overlay scene must record");
        dynamic_renderer
            .render_into_encoder(&dynamic_scene, &mut encoder, &dynamic_view, viewport)
            .expect("dynamic overlay scene must record");
        queue.submit(Some(encoder.finish()));

        assert_eq!(
            readback_pixel(&device, &queue, &cached_texture, SIZE, SIZE, 16, 16),
            [210, 35, 45, 255],
            "the dynamic upload must not replace the cached overlay payload",
        );
        assert_eq!(
            readback_pixel(&device, &queue, &dynamic_texture, SIZE, SIZE, 16, 16),
            [35, 210, 80, 255],
            "the dynamic overlay must retain its own payload",
        );
    }
}
