//! Concrete [`RenderSurfaceFactory`] implementations for each backend.
//!
//! These factories live here (in the hub) rather than in the individual
//! backend crates to avoid circular dependencies: each backend crate is a
//! dependency of `uzor-render-hub`, so the backends cannot also depend on
//! `uzor-render-hub`.
//!
//! # Available factories
//!
//! | Factory | Backend | Status |
//! |---------|---------|--------|
//! | [`VelloGpuSurfaceFactory`] | `VelloGpu` | Functional |
//! | [`TinySkiaSurfaceFactory`] | `TinySkia` | Functional |
//! | [`VelloCpuSurfaceFactory`] | `VelloCpu` | Functional — pure CPU, no wgpu |
//! | [`VelloHybridSurfaceFactory`] | `VelloHybrid` | Functional — GPU init deferred to first submit |
//! | [`WgpuInstancedSurfaceFactory`] | `InstancedWgpu` | Functional — GPU init deferred to first submit |
//! | [`Canvas2dSurfaceFactory`] | web canvas | wasm32 full impl, native stub |

use uzor::layout::window::RawHandle;

use crate::{RenderBackend, RenderSurfaceFactory, SurfaceError, SurfaceSize, WindowRenderState};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use vello::{AaSupport, Renderer, RendererOptions};
#[cfg(not(target_arch = "wasm32"))]
use crate::factory::GpuDevicePool;
#[cfg(not(target_arch = "wasm32"))]
use vello::wgpu::PresentMode;
#[cfg(not(target_arch = "wasm32"))]
use winit::raw_window_handle::{RawWindowHandle, RawDisplayHandle};
#[cfg(not(target_arch = "wasm32"))]
use uzor::layout::window::SoftwarePresenter;
#[cfg(not(target_arch = "wasm32"))]
use uzor_window_desktop::SendSyncHandlePair;

// ─── Internal surface target helper (desktop only) ───────────────────────────

#[cfg(not(target_arch = "wasm32"))]
/// Minimal `HasWindowHandle + HasDisplayHandle` wrapper around raw handles.
///
/// Allows calling `GpuDevicePool::create_surface` from a copied
/// `(RawWindowHandle, RawDisplayHandle)` pair without holding a live
/// `Arc<Window>`.
///
/// # Safety
///
/// The caller must guarantee that the underlying OS window outlives every
/// `wgpu::Surface` created from this target.
struct WinitSurfaceTarget {
    window: RawWindowHandle,
    display: RawDisplayHandle,
}

// SAFETY: raw handles are plain integer/pointer values.  The underlying OS
// window and display are guaranteed to outlive the factory call (the
// `Arc<Window>` in `WinitWindowProvider` keeps them alive for the entire
// runtime duration).  No thread-local state is accessed during wgpu surface
// creation on desktop platforms (Win32, X11, Wayland).
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Send for WinitSurfaceTarget {}
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Sync for WinitSurfaceTarget {}

#[cfg(not(target_arch = "wasm32"))]
impl winit::raw_window_handle::HasWindowHandle for WinitSurfaceTarget {
    fn window_handle(
        &self,
    ) -> Result<winit::raw_window_handle::WindowHandle<'_>, winit::raw_window_handle::HandleError>
    {
        // SAFETY: caller guarantees the underlying window is still alive.
        Ok(unsafe { winit::raw_window_handle::WindowHandle::borrow_raw(self.window) })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl winit::raw_window_handle::HasDisplayHandle for WinitSurfaceTarget {
    fn display_handle(
        &self,
    ) -> Result<winit::raw_window_handle::DisplayHandle<'_>, winit::raw_window_handle::HandleError>
    {
        // SAFETY: caller guarantees the underlying display is still alive.
        Ok(unsafe { winit::raw_window_handle::DisplayHandle::borrow_raw(self.display) })
    }
}

// ─── GPU pre-warm (run before HWND exists) ───────────────────────────────────

/// Heavy GPU init state that can be built ahead of the window handle.
///
/// `Renderer::new` (vello pipeline compilation, ~2.7s on a 4060 Ti)
/// only needs a `Device` — no `Surface`, no HWND.  By running this on
/// a background thread *during* winit's HWND construction, we overlap
/// ~3s of startup latency.  The remaining work after the HWND is
/// available (`Instance::create_surface` + `surface.configure` +
/// target_texture) is only ~50ms.
#[cfg(not(target_arch = "wasm32"))]
pub struct GpuPrewarm {
    pub gpu_pool: GpuDevicePool,
    pub dev_id:   usize,
    pub renderer: vello::Renderer,
}

/// Device-only pre-warm: Instance + Adapter + Device.  ~600ms on a
/// 4060 Ti — fast enough that the HWND is barely ready by the time
/// it lands.  Hands off to the caller, who can spin a lightweight
/// wgpu skeleton (CPU-rasterised frame uploaded each tick) on the
/// surface while [`Renderer::new`] continues compiling pipelines on
/// another thread.  See `start_renderer_in_background`.
#[cfg(not(target_arch = "wasm32"))]
pub struct GpuDeviceReady {
    pub gpu_pool: GpuDevicePool,
    pub dev_id:   usize,
}

/// Build a wgpu device pool — Instance + Adapter + Device only.
/// Safe to call before `winit::Window` exists.
#[cfg(not(target_arch = "wasm32"))]
pub fn prewarm_device() -> Result<GpuDeviceReady, SurfaceError> {
    let t_total = std::time::Instant::now();
    let mut gpu_pool = GpuDevicePool::new();
    gpu_pool.instance = build_dcomp_instance();
    eprintln!("[render-hub] prewarm_device: instance: {:?}", t_total.elapsed());

    let t_dev = std::time::Instant::now();
    let dev_id = pollster::block_on(gpu_pool.device(None))
        .ok_or_else(|| SurfaceError::InitFailed("prewarm: no compatible device".into()))?;
    eprintln!("[render-hub] prewarm_device: adapter + device: {:?}", t_dev.elapsed());
    eprintln!("[render-hub] prewarm_device: TOTAL: {:?}", t_total.elapsed());

    Ok(GpuDeviceReady { gpu_pool, dev_id })
}

/// Build a vello `Renderer` against a pre-warmed device.  ~2.7s on a
/// 4060 Ti.  Run this on a background thread alongside skeleton
/// rendering on the main thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_renderer(device: &wgpu::Device) -> Result<vello::Renderer, SurfaceError> {
    let t_renderer = std::time::Instant::now();
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let aa = vello::AaSupport { area: true, msaa8: false, msaa16: false };
    let renderer = vello::Renderer::new(
        device,
        vello::RendererOptions {
            use_cpu: false,
            antialiasing_support: aa,
            num_init_threads: std::num::NonZeroUsize::new(n_threads),
            pipeline_cache: None,
        },
    )
    .map_err(|e| SurfaceError::InitFailed(format!("build_renderer: {e}")))?;
    eprintln!("[render-hub] build_renderer: {:?}", t_renderer.elapsed());
    Ok(renderer)
}

/// Combined helper: device prewarm + renderer build, all-in-one.
/// Used when no skeleton is wanted (legacy path).
#[cfg(not(target_arch = "wasm32"))]
pub fn prewarm_vello_gpu() -> Result<GpuPrewarm, SurfaceError> {
    let ready = prewarm_device()?;
    let renderer = build_renderer(&ready.gpu_pool.devices[ready.dev_id].device)?;
    Ok(GpuPrewarm {
        gpu_pool: ready.gpu_pool,
        dev_id:   ready.dev_id,
        renderer,
    })
}

/// Build a `RenderSurface` from a device-ready pre-warm WITHOUT a
/// `vello::Renderer`.  Used so the caller can paint a CPU-rasterised
/// skeleton through the swapchain while vello compiles its pipelines
/// on a background thread.  When the renderer arrives, call
/// [`attach_renderer_to_render_state`] to slot it into the existing
/// `WindowRenderState`.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_surface_from_device(
    ready: GpuDeviceReady,
    handle: &RawHandle,
    size: SurfaceSize,
) -> Result<(GpuDevicePool, vello::util::RenderSurface<'static>, usize), SurfaceError> {
    let pair = extract_handle_pair(handle, RenderBackend::VelloGpu)?;
    let t_total = std::time::Instant::now();
    let GpuDeviceReady { gpu_pool, dev_id } = ready;
    let target = WinitSurfaceTarget { window: pair.0, display: pair.1 };
    use vello::util::RenderSurface;

    let surface_raw: wgpu::Surface<'_> = gpu_pool
        .instance
        .create_surface(wgpu::SurfaceTarget::from(target))
        .map_err(|e| SurfaceError::InitFailed(format!("build_surface: create_surface: {e}")))?;

    {
        let dh = &gpu_pool.devices[dev_id];
        if !dh.adapter().is_surface_supported(&surface_raw) {
            return Err(SurfaceError::InitFailed(
                "build_surface: pre-warmed adapter does not support this HWND".into(),
            ));
        }
    }

    let (alpha_mode, swap_format) = {
        let dh = &gpu_pool.devices[dev_id];
        let caps = surface_raw.get_capabilities(dh.adapter());
        let am = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        let fmt = caps.formats.iter()
            .find(|f| matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm))
            .copied().unwrap_or(caps.formats[0]);
        (am, fmt)
    };

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        format: swap_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![],
    };
    {
        let device = &gpu_pool.devices[dev_id].device;
        surface_raw.configure(device, &config);
    }

    let (target_texture, target_view) = {
        let device = &gpu_pool.devices[dev_id].device;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_target_texture"),
            size: wgpu::Extent3d {
                width: config.width, height: config.height, depth_or_array_layers: 1,
            },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    };
    let blitter = {
        let device = &gpu_pool.devices[dev_id].device;
        wgpu::util::TextureBlitter::new(device, swap_format)
    };

    let surface_with_lifetime: RenderSurface<'_> = RenderSurface {
        surface: surface_raw, config, dev_id,
        format: swap_format, target_texture, target_view, blitter,
    };
    let surface: RenderSurface<'static> = unsafe { std::mem::transmute(surface_with_lifetime) };

    eprintln!("[render-hub] build_surface_from_device: {:?}", t_total.elapsed());
    Ok((gpu_pool, surface, dev_id))
}

/// Build a `RenderSurface` on top of a `GpuPrewarm` using the supplied
/// window handle.  Only the surface-bound work runs here (typically
/// 30–80ms): `instance.create_surface`, alpha-mode probe + configure,
/// target_texture + blitter.
#[cfg(not(target_arch = "wasm32"))]
fn finalize_gpu_surface(
    prewarm: GpuPrewarm,
    pair: &SendSyncHandlePair,
    size: SurfaceSize,
) -> Result<(GpuDevicePool, vello::util::RenderSurface<'static>, usize, vello::Renderer), SurfaceError> {
    let t_total = std::time::Instant::now();
    let GpuPrewarm { gpu_pool, dev_id, renderer } = prewarm;

    let target = WinitSurfaceTarget { window: pair.0, display: pair.1 };

    use vello::util::RenderSurface;

    let surface_raw: wgpu::Surface<'_> = gpu_pool
        .instance
        .create_surface(wgpu::SurfaceTarget::from(target))
        .map_err(|e| SurfaceError::InitFailed(format!("finalize: create_surface: {e}")))?;

    // The pre-warmed adapter may not be compatible with this HWND
    // (multi-GPU edge case).  Verify; if mismatched, fall back to
    // synchronous init — we lose the pre-warm savings but stay correct.
    {
        let dh = &gpu_pool.devices[dev_id];
        if !dh.adapter().is_surface_supported(&surface_raw) {
            return Err(SurfaceError::InitFailed(
                "finalize: pre-warmed adapter does not support this HWND surface — fallback path needed".into()
            ));
        }
    }

    let (alpha_mode, swap_format) = {
        let dh = &gpu_pool.devices[dev_id];
        let caps = surface_raw.get_capabilities(dh.adapter());
        let am = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        let fmt = caps.formats.iter()
            .find(|f| matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm))
            .copied().unwrap_or(caps.formats[0]);
        (am, fmt)
    };

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        format: swap_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![],
    };
    {
        let device = &gpu_pool.devices[dev_id].device;
        surface_raw.configure(device, &config);
    }

    let (target_texture, target_view) = {
        let device = &gpu_pool.devices[dev_id].device;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_target_texture"),
            size: wgpu::Extent3d {
                width: config.width, height: config.height, depth_or_array_layers: 1,
            },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    };
    let blitter = {
        let device = &gpu_pool.devices[dev_id].device;
        wgpu::util::TextureBlitter::new(device, swap_format)
    };

    let surface_with_lifetime: RenderSurface<'_> = RenderSurface {
        surface: surface_raw, config, dev_id,
        format: swap_format, target_texture, target_view, blitter,
    };
    let surface: RenderSurface<'static> = unsafe { std::mem::transmute(surface_with_lifetime) };

    eprintln!("[render-hub] finalize_gpu_surface: {:?}", t_total.elapsed());
    Ok((gpu_pool, surface, dev_id, renderer))
}

// ─── Shared GPU init helper (desktop only) ───────────────────────────────────

/// Build a wgpu `Instance` wired for DirectComposition on Windows.
///
/// `Dx12SwapchainKind::DxgiFromVisual` is the only path that makes
/// the DX12 backend report `CompositeAlphaMode::PreMultiplied` in
/// `Surface::get_capabilities()` — a plain HWND swapchain reports
/// `Opaque` only, so the swapchain discards the alpha channel even
/// if the HWND has `WS_EX_NOREDIRECTIONBITMAP` and vello renders
/// alpha=0 pixels.  See `docs/research/transparency-dcomp-research.md`.
///
/// On non-Windows platforms returns a plain default `Instance`.
#[cfg(not(target_arch = "wasm32"))]
fn build_dcomp_instance() -> wgpu::Instance {
    #[cfg(target_os = "windows")]
    {
        let backends = wgpu::Backends::DX12;
        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let memory_budget_thresholds = wgpu::MemoryBudgetThresholds::default();
        let backend_options = wgpu::BackendOptions {
            dx12: wgpu::Dx12BackendOptions {
                presentation_system: wgpu::Dx12SwapchainKind::DxgiFromVisual,
                ..Default::default()
            },
            ..Default::default()
        };
        // wgpu 29: Instance::new takes the descriptor by value, the descriptor
        // gained a `display: Option<...>` field (None = no display handle).
        return wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags,
            memory_budget_thresholds,
            backend_options,
            display: None,
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        let backends = wgpu::Backends::from_env().unwrap_or_default();
        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let memory_budget_thresholds = wgpu::MemoryBudgetThresholds::default();
        let backend_options = wgpu::BackendOptions::from_env_or_default();
        // wgpu 29: by-value descriptor + `display` field (matches the windows
        // branch above; this arm was missed when wgpu's API changed).
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags,
            memory_budget_thresholds,
            backend_options,
            display: None,
        })
    }
}

/// Create a `GpuDevicePool` + `RenderSurface` from a `SendSyncHandlePair`.
///
/// Shared by all GPU-backed factories (VelloGpu, VelloHybrid, WgpuInstanced).
#[cfg(not(target_arch = "wasm32"))]
fn init_gpu_surface(
    pair: &SendSyncHandlePair,
    size: SurfaceSize,
    backend: RenderBackend,
) -> Result<(GpuDevicePool, vello::util::RenderSurface<'static>, usize), SurfaceError> {
    let target = WinitSurfaceTarget {
        window: pair.0,
        display: pair.1,
    };

    // Replace vello's default `wgpu::Instance` with one configured for
    // DirectComposition.  vello's `RenderContext` exposes `instance` as
    // a public field and `devices` is empty until the first
    // `create_surface` call — so swapping the instance pre-surface is
    // safe and does not require a vello fork.
    let t_init = std::time::Instant::now();
    let mut gpu_pool = GpuDevicePool::new();
    gpu_pool.instance = build_dcomp_instance();
    eprintln!("[render-hub] build_dcomp_instance: {:?}", t_init.elapsed());

    // We DON'T call `gpu_pool.create_surface` here — that helper calls
    // `surface.configure(..., alpha_mode: Auto)` first, then we would
    // re-configure with PreMultiplied.  On DComp DX12 swapchains the
    // first configure latches the visual into Opaque mode and the
    // second configure does not flip it back, so the final swapchain
    // composites as Opaque even though `surface.config.alpha_mode` says
    // PreMultiplied.  Instead we build the `RenderSurface` manually
    // and configure exactly once with the right alpha mode.

    use vello::util::RenderSurface;

    // Step 1: raw wgpu surface from the HWND.
    let t_surf = std::time::Instant::now();
    let surface_raw: wgpu::Surface<'_> = gpu_pool
        .instance
        .create_surface(wgpu::SurfaceTarget::from(target))
        .map_err(|e| SurfaceError::InitFailed(format!("{backend:?}: create_surface: {e}")))?;
    eprintln!("[render-hub] instance.create_surface: {:?}", t_surf.elapsed());

    // Step 2: acquire a compatible device through vello's helper so the
    // `gpu_pool.devices` Vec gets populated (vello uses dev_id to index
    // it elsewhere).
    let t_dev = std::time::Instant::now();
    let dev_id = pollster::block_on(gpu_pool.device(Some(&surface_raw)))
        .ok_or_else(|| SurfaceError::InitFailed(format!("{backend:?}: no compatible device")))?;
    eprintln!("[render-hub] request_adapter + device: {:?}", t_dev.elapsed());

    let (alpha_mode, swap_format, alpha_modes_log, formats_log, adapter_name) = {
        let dh = &gpu_pool.devices[dev_id];
        let caps = surface_raw.get_capabilities(dh.adapter());
        let am = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        // Match vello's selection — first non-sRGB Rgba8/Bgra8. The blit
        // path uploads sRGB CPU pixmap bytes straight into the swapchain,
        // so a non-sRGB swapchain is gamma-correct (verified: this dev box
        // picks Bgra8Unorm, screenshot bytes match spec exactly). An
        // `*Srgb`-only adapter would double-encode (darker) — but the safe
        // fix there (non-sRGB view_format + blit through it) is unverified
        // for lack of such hardware, so it's left for the SR3 native-wgpu
        // pass which owns the surface config. See backlog 1.4.11b.
        let fmt = caps
            .formats
            .iter()
            .find(|f| matches!(f, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm))
            .copied()
            .unwrap_or(caps.formats[0]);
        (am, fmt, caps.alpha_modes.clone(), caps.formats.clone(), dh.adapter().get_info().name.clone())
    };

    eprintln!(
        "[render-hub] alpha_modes={:?}  formats={:?}  adapter={:?}  picked alpha_mode={:?}  picked format={:?}",
        alpha_modes_log, formats_log, adapter_name, alpha_mode, swap_format,
    );

    // Step 3: build SurfaceConfiguration with the chosen alpha mode and
    // configure exactly once.
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        format: swap_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![],
    };
    {
        let device = &gpu_pool.devices[dev_id].device;
        surface_raw.configure(device, &config);
    }
    eprintln!("[render-hub] surface configured ONCE with alpha_mode={:?}", alpha_mode);
    use std::io::Write;
    let _ = std::io::stderr().flush();
    let t_blitter = std::time::Instant::now();

    // Step 4: build the vello target_texture + blitter with our usage flags
    // (so we don't have to pay for a second create+drop).
    let (target_texture, target_view) = {
        let device = &gpu_pool.devices[dev_id].device;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_target_texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    };
    let blitter = {
        let device = &gpu_pool.devices[dev_id].device;
        wgpu::util::TextureBlitter::new(device, swap_format)
    };
    eprintln!("[render-hub] target_texture + blitter: {:?}", t_blitter.elapsed());
    let _ = std::io::stderr().flush();

    // Step 5: assemble the `RenderSurface` from public fields.
    let surface_with_lifetime: RenderSurface<'_> = RenderSurface {
        surface: surface_raw,
        config,
        dev_id,
        format: swap_format,
        target_texture,
        target_view,
        blitter,
    };

    // SAFETY: see the lifetime note above — the underlying winit Window
    // outlives every consumer of this surface.
    let surface: RenderSurface<'static> =
        unsafe { std::mem::transmute(surface_with_lifetime) };

    Ok((gpu_pool, surface, dev_id))
}

/// Extract a `SendSyncHandlePair` from a `RawHandle::RawWindowHandle`.
#[cfg(not(target_arch = "wasm32"))]
fn extract_handle_pair<'a>(
    handle: &'a RawHandle,
    backend: RenderBackend,
) -> Result<&'a SendSyncHandlePair, SurfaceError> {
    let RawHandle::RawWindowHandle(any) = handle else {
        return Err(SurfaceError::HandleMismatch(backend));
    };

    any.downcast_ref::<SendSyncHandlePair>().ok_or_else(|| {
        SurfaceError::InitFailed(
            "expected SendSyncHandlePair inside RawHandle — \
             use WinitWindowProvider to obtain the handle"
                .into(),
        )
    })
}

// ─── Desktop-only factories ───────────────────────────────────────────────────
// VelloGpuSurfaceFactory, TinySkiaSurfaceFactory, VelloCpuSurfaceFactory,
// VelloHybridSurfaceFactory, and WgpuInstancedSurfaceFactory all require a
// native OS window and wgpu / softbuffer. They are compiled out on wasm32.

// ─── VelloGpuSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloGpu`] path.
///
/// On [`create_render_state`](RenderSurfaceFactory::create_render_state) it:
///
/// 1. Creates a `GpuDevicePool` (wgpu device + queue).
/// 2. Creates a `RenderSurface` bound to the OS window handle.
/// 3. Creates a vello `Renderer`.
/// 4. Moves **all three** into [`WindowRenderState::Gpu`].
#[cfg(not(target_arch = "wasm32"))]
pub struct VelloGpuSurfaceFactory;

#[cfg(not(target_arch = "wasm32"))]
impl VelloGpuSurfaceFactory {
    /// Create a new factory.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl VelloGpuSurfaceFactory {
    /// Fast path: build the per-window `WindowRenderState` from a
    /// pre-warmed `GpuPrewarm` (Instance + Adapter + Device + vello
    /// Renderer already ready).  Only the surface-bound steps run
    /// here.  Caller produces the prewarm on a background thread
    /// during winit HWND creation; see `prewarm_vello_gpu`.
    pub fn create_render_state_with_prewarm(
        &self,
        prewarm: GpuPrewarm,
        handle: &RawHandle,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        let pair = extract_handle_pair(handle, RenderBackend::VelloGpu)?;
        let (gpu_pool, surface, dev_id, renderer) = finalize_gpu_surface(prewarm, pair, size)?;
        Ok(WindowRenderState::new_gpu(gpu_pool, surface, renderer, dev_id))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for VelloGpuSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderSurfaceFactory for VelloGpuSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::VelloGpu) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;

        let device = &gpu_pool.devices[dev_id].device;

        let t_renderer = std::time::Instant::now();
        // Use all available cores to parallelise vello pipeline init —
        // the default of 1 takes ~14 s on a 4060 Ti, the parallel
        // path drops it to under 2 s.  AaSupport::all() compiles three
        // antialiasing variants (Area + MSAA8 + MSAA16); restrict to
        // just `area` if we ever want to cut it further.
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // AaSupport::all() compiles three antialiasing variants (Area
        // + MSAA8 + MSAA16) which roughly triples vello's pipeline
        // count.  Most workloads use only `Area`; MSAA is opt-in per
        // submit and we currently never request it from the dashboard.
        // Restricting to `area_only` cuts the cold start by ~2/3.
        let aa = AaSupport { area: true, msaa8: false, msaa16: false };
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: aa,
                num_init_threads: std::num::NonZeroUsize::new(n_threads),
                pipeline_cache: None,
            },
        )
        .map_err(|e| SurfaceError::InitFailed(e.to_string()))?;
        eprintln!("[render-hub] vello Renderer::new ({} threads, area-only AA): {:?}", n_threads, t_renderer.elapsed());

        Ok(WindowRenderState::new_gpu(gpu_pool, surface, renderer, dev_id))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloGpu)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── TinySkiaSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::TinySkia`] CPU software path.
///
/// Constructs a [`WindowRenderState`] backed by a `TinySkiaCpuRenderContext`
/// plus a [`SoftwarePresenter`] for OS-window presentation without a GPU.
///
/// Build via [`TinySkiaSurfaceFactory::with_presenter`] when a software surface
/// is needed, or [`TinySkiaSurfaceFactory::new`] when the presenter will be
/// supplied separately.
#[cfg(not(target_arch = "wasm32"))]
pub struct TinySkiaSurfaceFactory {
    presenter: Mutex<Option<Box<dyn SoftwarePresenter>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TinySkiaSurfaceFactory {
    /// Create the factory without a presenter.
    ///
    /// Callers that need a software surface must call
    /// [`with_presenter`](Self::with_presenter) instead; using this constructor
    /// and then calling `create_render_state` will return a
    /// [`SurfaceError::HandleUnavailable`] error.
    pub fn new() -> Self {
        Self { presenter: Mutex::new(None) }
    }

    /// Create the factory with a pre-built software presenter.
    ///
    /// Obtain the presenter from
    /// [`WindowProvider::create_software_presenter`](uzor::layout::window::WindowProvider::create_software_presenter).
    /// The presenter is moved into the factory and transferred to the
    /// [`WindowRenderState`] on the first call to [`create_render_state`].
    pub fn with_presenter(presenter: Box<dyn SoftwarePresenter>) -> Self {
        Self { presenter: Mutex::new(Some(presenter)) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for TinySkiaSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderSurfaceFactory for TinySkiaSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::TinySkia) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        // Software-presenter path (legacy / headless tests).
        if let Some(presenter) = self
            .presenter
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            return Ok(WindowRenderState::new_cpu(size.width, size.height, presenter));
        }

        // Default path: render into a tiny-skia pixmap, upload as a
        // texture, blit through the wgpu swapchain.  Mirrors the
        // proven mlc submit path; identical for every spawned window.
        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;
        Ok(WindowRenderState::new_tiny_skia_gpu(gpu_pool, surface, dev_id))
    }

    fn supports(&self, _handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::TinySkia)
    }
}

// ─── VelloCpuSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloCpu`] path.
///
/// Constructs a [`WindowRenderState`] backed by a `VelloCpuRenderContext`
/// plus a [`SoftwarePresenter`] for OS-window presentation without a GPU.
///
/// Build via [`VelloCpuSurfaceFactory::with_presenter`] when a software surface
/// is needed.
#[cfg(not(target_arch = "wasm32"))]
pub struct VelloCpuSurfaceFactory {
    /// Device pixel ratio.  Defaults to `1.0`.
    pub dpr: f64,
    presenter: Mutex<Option<Box<dyn SoftwarePresenter>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl VelloCpuSurfaceFactory {
    /// Create the factory with the given device pixel ratio but no presenter.
    ///
    /// Callers that need a software surface must call
    /// [`with_presenter`](Self::with_presenter) instead.
    pub fn new(dpr: f64) -> Self {
        Self { dpr, presenter: Mutex::new(None) }
    }

    /// Create the factory with a device pixel ratio and a software presenter.
    ///
    /// Obtain the presenter from
    /// [`WindowProvider::create_software_presenter`](uzor::layout::window::WindowProvider::create_software_presenter).
    pub fn with_presenter(dpr: f64, presenter: Box<dyn SoftwarePresenter>) -> Self {
        Self { dpr, presenter: Mutex::new(Some(presenter)) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for VelloCpuSurfaceFactory {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderSurfaceFactory for VelloCpuSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::VelloCpu) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        // Software-presenter path (kept for headless tests).
        if let Some(presenter) = self
            .presenter
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            return Ok(WindowRenderState::new_vello_cpu(self.dpr, presenter));
        }

        // Default path: render into a vello-cpu pixmap, upload as a
        // texture, blit through the wgpu swapchain.
        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;
        Ok(WindowRenderState::new_vello_cpu_gpu(gpu_pool, surface, dev_id, self.dpr))
    }

    fn supports(&self, _handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloCpu)
    }
}

// ─── VelloHybridSurfaceFactory ────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloHybrid`] path.
///
/// Constructs a [`WindowRenderState::VelloHybrid`].  GPU surface and device
/// pool are initialised eagerly; the `vello_hybrid::Renderer` itself is
/// deferred to the first frame (requires the swapchain texture format, which
/// only becomes available when the first `get_current_texture` call is made).
#[cfg(not(target_arch = "wasm32"))]
pub struct VelloHybridSurfaceFactory {
    /// Device pixel ratio passed to the `VelloHybridRenderContext`.
    pub dpr: f64,
}

#[cfg(not(target_arch = "wasm32"))]
impl VelloHybridSurfaceFactory {
    /// Create the factory.
    pub fn new(dpr: f64) -> Self {
        Self { dpr }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for VelloHybridSurfaceFactory {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderSurfaceFactory for VelloHybridSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::VelloHybrid) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;

        Ok(WindowRenderState::new_vello_hybrid(gpu_pool, surface, dev_id, self.dpr))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloHybrid)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── WgpuInstancedSurfaceFactory ─────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::InstancedWgpu`] path.
///
/// Constructs a [`WindowRenderState::WgpuInstanced`].  GPU surface and device
/// pool are initialised eagerly; the `InstancedRenderer` itself is deferred
/// to the first frame (requires the swapchain texture format).
#[cfg(not(target_arch = "wasm32"))]
pub struct WgpuInstancedSurfaceFactory;

#[cfg(not(target_arch = "wasm32"))]
impl WgpuInstancedSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for WgpuInstancedSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderSurfaceFactory for WgpuInstancedSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::InstancedWgpu) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;

        Ok(WindowRenderState::new_wgpu_instanced(gpu_pool, surface, dev_id))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::InstancedWgpu)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── Canvas2dSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the HTML Canvas 2D backend (wasm32 only).
///
/// On native targets this always returns [`SurfaceError::UnsupportedBackend`].
/// On `wasm32` targets it downcasts the [`RawHandle::Canvas`] payload to a
/// `web_sys::HtmlCanvasElement`, calls `getContext("2d")`, reads the device
/// pixel ratio from `window.devicePixelRatio`, and returns a fully initialized
/// [`WindowRenderState`] for DOM canvas rendering.
pub struct Canvas2dSurfaceFactory;

impl Canvas2dSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Canvas2dSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
impl RenderSurfaceFactory for Canvas2dSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        _size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::Canvas2d) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let RawHandle::Canvas(any) = handle else {
            return Err(SurfaceError::HandleMismatch(backend));
        };

        // The RawHandle::Canvas payload is a SendSyncCanvas (from uzor-window-web).
        let canvas = any
            .downcast_ref::<uzor_window_web::SendSyncCanvas>()
            .ok_or_else(|| {
                SurfaceError::InitFailed(
                    "expected SendSyncCanvas in RawHandle::Canvas — use WebWindowProvider".into(),
                )
            })?
            .0
            .clone();

        let raw_ctx = canvas
            .get_context("2d")
            .map_err(|e| {
                SurfaceError::InitFailed(format!("canvas.getContext(\"2d\") failed: {e:?}"))
            })?
            .ok_or_else(|| SurfaceError::InitFailed("canvas.getContext(\"2d\") returned null".into()))?;

        use wasm_bindgen::JsCast as _;
        let ctx2d = raw_ctx
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| {
                SurfaceError::InitFailed(
                    "getContext(\"2d\") object is not CanvasRenderingContext2d".into(),
                )
            })?;

        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);

        let render_ctx = uzor_render_canvas2d::Canvas2dRenderContext::new(ctx2d, dpr);

        Ok(WindowRenderState::new_canvas2d(canvas, render_ctx))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::Canvas2d) && matches!(handle, RawHandle::Canvas(_))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderSurfaceFactory for Canvas2dSurfaceFactory {
    fn create_render_state(
        &self,
        _handle: &RawHandle,
        backend: RenderBackend,
        _size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        Err(SurfaceError::UnsupportedBackend(backend))
    }

    fn supports(&self, _handle: &RawHandle, _backend: RenderBackend) -> bool {
        false
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    // Helper: a dummy handle that is not a RawWindowHandle (for mismatch tests).
    fn canvas_handle() -> RawHandle {
        RawHandle::Canvas(Box::new(42u32))
    }

    fn raw_window_handle_dummy() -> RawHandle {
        // We can't construct a real SendSyncHandlePair in unit tests, but we
        // can verify the discriminant check at the `supports` level, which
        // only looks at the handle variant and backend, not the contents.
        RawHandle::Canvas(Box::new(99u32))
    }

    // ── VelloGpuSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn vello_gpu_supports_correct_pair() {
        let f = VelloGpuSurfaceFactory::new();
        // `supports` only checks the discriminant, not whether the inner Any
        // can be downcast to SendSyncHandlePair — so we can test with a dummy
        // RawWindowHandle variant.
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(f.supports(&handle, RenderBackend::VelloGpu));
    }

    #[test]
    fn vello_gpu_rejects_wrong_backend() {
        let f = VelloGpuSurfaceFactory::new();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::TinySkia));
        assert!(!f.supports(&handle, RenderBackend::VelloCpu));
    }

    #[test]
    fn vello_gpu_rejects_canvas_handle() {
        let f = VelloGpuSurfaceFactory::new();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
    }

    // ── TinySkiaSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn tiny_skia_supports_any_handle() {
        let f = TinySkiaSurfaceFactory::new();
        // TinySkia doesn't care about the handle type.
        assert!(f.supports(&canvas_handle(), RenderBackend::TinySkia));
        assert!(f.supports(&raw_window_handle_dummy(), RenderBackend::TinySkia));
    }

    #[test]
    fn tiny_skia_rejects_wrong_backend() {
        let f = TinySkiaSurfaceFactory::new();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloCpu));
    }

    // ── VelloCpuSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn vello_cpu_supports_any_handle() {
        let f = VelloCpuSurfaceFactory::default();
        assert!(f.supports(&canvas_handle(), RenderBackend::VelloCpu));
        assert!(f.supports(&raw_window_handle_dummy(), RenderBackend::VelloCpu));
    }

    #[test]
    fn vello_cpu_rejects_wrong_backend() {
        let f = VelloCpuSurfaceFactory::default();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
        assert!(!f.supports(&canvas_handle(), RenderBackend::TinySkia));
    }

    // ── VelloHybridSurfaceFactory ─────────────────────────────────────────────

    #[test]
    fn vello_hybrid_supports_raw_window_handle() {
        let f = VelloHybridSurfaceFactory::default();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(f.supports(&handle, RenderBackend::VelloHybrid));
    }

    #[test]
    fn vello_hybrid_rejects_canvas_handle() {
        let f = VelloHybridSurfaceFactory::default();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloHybrid));
    }

    #[test]
    fn vello_hybrid_rejects_wrong_backend() {
        let f = VelloHybridSurfaceFactory::default();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::VelloGpu));
        assert!(!f.supports(&handle, RenderBackend::TinySkia));
    }

    // ── WgpuInstancedSurfaceFactory ───────────────────────────────────────────

    #[test]
    fn wgpu_instanced_supports_raw_window_handle() {
        let f = WgpuInstancedSurfaceFactory::new();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(f.supports(&handle, RenderBackend::InstancedWgpu));
    }

    #[test]
    fn wgpu_instanced_rejects_canvas_handle() {
        let f = WgpuInstancedSurfaceFactory::new();
        assert!(!f.supports(&canvas_handle(), RenderBackend::InstancedWgpu));
    }

    #[test]
    fn wgpu_instanced_rejects_wrong_backend() {
        let f = WgpuInstancedSurfaceFactory::new();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::VelloGpu));
        assert!(!f.supports(&handle, RenderBackend::TinySkia));
    }

    // ── Canvas2dSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn canvas2d_never_supports() {
        let f = Canvas2dSurfaceFactory::new();
        // Canvas2d factory always returns false on native — no matching backend.
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
        assert!(!f.supports(&canvas_handle(), RenderBackend::TinySkia));
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::VelloGpu));
    }
}
