//! Every rendering leg — three CPU legs (always available) plus two GPU
//! legs (gracefully skipped when no adapter is available).
//!
//! The three CPU legs are the same trio `uzor-examples/src/
//! parity_harness.rs`'s own `render_via_urx_cpu`/`render_via_vello_cpu`/
//! `record_via_urx_ctx` already established, promoted here unchanged.
//! The two GPU legs (`render_vello_gpu`/`render_urx_wgpu`) reuse the
//! PROVEN headless-GPU init/readback shape from that same module and
//! from `uzor-urx-wgpu/tests/{common/mod.rs,parity.rs}` — a headless
//! `wgpu::Instance` -> `request_adapter` -> `request_device` (never
//! panicking, `None` on a GPU-less box), an offscreen `Rgba8Unorm`
//! render target, `copy_texture_to_buffer` + de-stride readback. Neither
//! GPU leg is reinvented from scratch.
//!
//! Every leg returns **premultiplied RGBA8**, `width * height * 4` bytes,
//! row-major — the one shared pixel contract [`compare_tight`](crate::compare_tight)
//! and [`write_composite_png`](crate::write_composite_png) both assume.
//!
//! **Both GPU legs also catch a panic from their own rasterisation
//! step**, converting it into a graceful `None` skip (same contract as
//! "no adapter available") rather than aborting the whole test binary.
//! Measured, not theoretical: an investigative raw-NaN fixture
//! (`uzor-figures`'s `gap_policy_multi_backend_divergence_proof`, the
//! deliberately-malformed "before" scene bypassing `GapPolicy` entirely)
//! hard-panics `uzor-urx-wgpu`'s native tessellation
//! (`lyon_path::Path`'s own `assert!(p.x.is_finite())`) — `urx-cpu`
//! degrades to a partial render on the exact same malformed input,
//! `vello-gpu` renders without panicking too, only `urx-gpu`'s
//! tessellation path lacks that same finite-coordinate guard. A real,
//! loud finding (reported in this crate's own divergence report), not
//! swallowed — just not allowed to take the whole proof suite down with
//! it.

use uzor::render::RenderContext;
use uzor_urx_core::scene::Scene;

use crate::urx_degrade_recorder::DegradeRecorder;

/// Best-effort human-readable message from a caught panic payload.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Render `draw` through `tiny-skia` (`uzor-render-tiny-skia`) — the
/// backend every proof render in this workspace currently hardcodes via
/// `uzor-export::render_to_png` (`uzor-export/src/lib.rs:46,162`). tiny-skia
/// is the last-resort fallback family, not a production reference — this
/// leg exists so its own output is compared AGAINST the two production
/// families, never assumed correct by default.
pub fn render_tiny_skia(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut ctx = uzor_render_tiny_skia::TinySkiaCpuRenderContext::new(width, height, 1.0);
    draw(&mut ctx);
    ctx.pixels().to_vec()
}

/// Render `draw` through `vello_cpu` (`uzor-render-vello-cpu`) — headless,
/// no device. Reuses the exact context type production `submit_cpu_vello`
/// drives; output is already premultiplied RGBA8
/// (`VelloCpuRenderContext::render_to_pixmap_rgba8`'s own documented
/// convention), matching every other leg here — no format conversion
/// needed anywhere in this crate.
pub fn render_vello_cpu(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut ctx = uzor_render_vello_cpu::VelloCpuRenderContext::new(1.0);
    ctx.begin_frame(width, height);
    draw(&mut ctx);
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    ctx.render_to_pixmap_rgba8(&mut buf, width as u16, height as u16);
    buf
}

/// Record `draw` into a `Scene` via `uzor_render_urx::UrxRenderContext`
/// (records, doesn't rasterize), then rasterize that `Scene` through the
/// URX family's own CPU backend (`uzor-urx-cpu::CpuBackend` — the URX
/// family's semantic reference, not a GPU backend, matching that crate's
/// own parity-harness convention).
///
/// This crate's own `uzor-urx-cpu` dependency turns on the `glyph`
/// feature (see `Cargo.toml`'s own comment) specifically so this
/// function's output is a genuine rendering of `draw`, not a build
/// silently missing every unrotated `fill_text` call.
pub fn render_urx_cpu(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut rec_ctx = uzor_render_urx::UrxRenderContext::new(1.0);
    rec_ctx.begin_frame(width, height);
    draw(&mut rec_ctx);
    let scene: Scene = rec_ctx.take_scene();

    let mut pixmap = uzor_urx_cpu::Pixmap::new(width, height);
    let backend = uzor_urx_cpu::CpuBackend::new();
    if let Err(e) = backend.render(&scene, &mut pixmap) {
        eprintln!("[uzor-proof-harness] urx-cpu render error: {e:?}");
    }
    pixmap.pixels().to_vec()
}

// ── GPU legs ─────────────────────────────────────────────────────────────

/// Headless wgpu device — `compatible_surface: None`, no window. Returns
/// `None` (never panics) when no adapter is available so callers can
/// skip gracefully on a GPU-less box. `pick_features` receives the
/// adapter so a caller (e.g. [`render_vello_gpu`]) can request only the
/// features the adapter ACTUALLY supports, matching
/// `vello::util::RenderContext::new_device`'s own
/// `adapter.features() & maybe_features` intersection.
fn init_headless_device(pick_features: impl FnOnce(&wgpu::Adapter) -> wgpu::Features) -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let required_features = pick_features(&adapter);
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("uzor-proof-harness-gpu-leg"),
        required_features,
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()
}

/// Read back a `COPY_SRC` texture as a tight (no row padding) `Vec<u8>`
/// of premultiplied RGBA8 bytes — same shape as `uzor-urx-wgpu/tests/
/// common/mod.rs::readback_rgba` / `uzor-examples/src/
/// parity_harness.rs::readback_rgba`, shared here by both GPU legs.
fn readback_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let aligned_stride = (width * 4 + 255) & !255;
    let buf_size = (aligned_stride * height) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor-proof-harness-gpu-readback"),
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
    let map_result = rx.recv().unwrap_or(Err(wgpu::BufferAsyncError));
    if let Err(e) = map_result {
        eprintln!("[uzor-proof-harness] gpu readback: staging buffer map failed: {e}");
        return vec![0u8; (width as usize) * (height as usize) * 4];
    }
    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for row in 0..height as usize {
        let row_start = row * aligned_stride as usize;
        let row_end = row_start + (width * 4) as usize;
        out.extend_from_slice(&raw[row_start..row_end]);
    }
    drop(raw);
    staging.unmap();
    out
}

fn init_vello_gpu_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    init_headless_device(|adapter| adapter.features() & (wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE))
}

fn init_urx_wgpu_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    init_headless_device(|_adapter| wgpu::Features::empty())
}

/// Render `draw` through the real GPU compute pipeline `vello::Renderer`
/// drives — `uzor-render-vello-gpu`'s own `VelloGpuRenderContext` records
/// into a `vello::Scene` (the exact recording step production's
/// `submit_vello_gpu` performs, `uzor-render-hub/src/submit.rs`), then
/// `vello::Renderer::render_to_texture` rasterizes it into a headless
/// offscreen `Rgba8Unorm | STORAGE_BINDING | TEXTURE_BINDING | COPY_SRC`
/// texture (the exact usage flags `vello::util::RenderContext`'s own
/// `create_targets` uses, `COPY_SRC` added so this fn can read the result
/// back), read back to premultiplied RGBA8.
///
/// Returns `None` (never panics) when no GPU/software adapter is
/// available, or when `vello::Renderer::new` itself fails to compile its
/// pipelines — same graceful-skip convention `uzor-urx-wgpu/tests/
/// parity.rs`'s own `render_native` already establishes for this exact
/// scenario, so the harness keeps running (minus this one leg) on a
/// GPU-less box.
pub fn render_vello_gpu(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Option<Vec<u8>> {
    let (device, queue) = init_vello_gpu_device()?;

    let mut renderer = match vello::Renderer::new(
        &device,
        vello::RendererOptions {
            use_cpu: false,
            antialiasing_support: vello::AaSupport::area_only(),
            num_init_threads: std::num::NonZeroUsize::new(1),
            pipeline_cache: None,
        },
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[uzor-proof-harness] vello::Renderer::new failed: {e}; skipping vello-gpu leg");
            return None;
        }
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut scene = vello::Scene::new();
        {
            let mut ctx = uzor_render_vello_gpu::VelloGpuRenderContext::new(&mut scene, 0.0, 0.0);
            let ctx_dyn: &mut dyn RenderContext = &mut ctx;
            draw(ctx_dyn);
        }

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor-proof-harness-vello-gpu-target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        if let Err(e) = renderer.render_to_texture(
            &device,
            &queue,
            &scene,
            &view,
            &vello::RenderParams {
                base_color: vello::peniko::Color::TRANSPARENT,
                width,
                height,
                antialiasing_method: vello::AaConfig::Area,
            },
        ) {
            eprintln!("[uzor-proof-harness] vello render_to_texture error: {e}");
        }

        readback_rgba(&device, &queue, &target, width, height)
    }));

    match result {
        Ok(pixels) => Some(pixels),
        Err(payload) => {
            eprintln!("[uzor-proof-harness] vello-gpu leg panicked: {}; skipping vello-gpu leg", panic_message(payload.as_ref()));
            None
        }
    }
}

/// Result of [`render_urx_wgpu`] — the rasterised pixels plus every URX
/// native-pipeline degrade counter that fired while producing them.
pub struct UrxWgpuRender {
    pub pixels: Vec<u8>,
    /// `(degrade kind, fired count)` — every counter under
    /// `uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES` that actually
    /// incremented while rasterising THIS scene (see
    /// `uzor-urx-wgpu/src/encode.rs::degrade`), captured via a scoped
    /// `metrics::with_local_recorder` around this one render call —
    /// never touches or is touched by any process-global recorder.
    /// Empty on an ordinary, defect-free scene; any entry here is a
    /// finding (the native pipeline silently fell back to an
    /// approximation), not routine telemetry noise.
    pub degrades: Vec<(String, u64)>,
}

/// Record `draw` into a `Scene` via `uzor_render_urx::UrxRenderContext`
/// (the SAME recording step [`render_urx_cpu`] uses — a divergence
/// between this leg and that one is purely rasterisation, never a
/// recording difference), then rasterize that Scene through
/// `uzor-urx-wgpu::NativeUrxRenderer::render_into_encoder` — the exact
/// native-pipeline path `uzor-urx-wgpu/tests/parity.rs`'s own pixel-
/// parity harness already drives — into a headless offscreen
/// `Rgba8Unorm | RENDER_ATTACHMENT | COPY_SRC` texture, read back to
/// premultiplied RGBA8.
///
/// Returns `None` (never panics — even if the native pipeline's own
/// rasterisation step panics internally, see this module's own doc
/// comment) when no GPU/software adapter is available — same
/// graceful-skip convention as [`render_vello_gpu`].
pub fn render_urx_wgpu(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Option<UrxWgpuRender> {
    let mut rec_ctx = uzor_render_urx::UrxRenderContext::new(1.0);
    rec_ctx.begin_frame(width, height);
    draw(&mut rec_ctx);
    let scene: Scene = rec_ctx.take_scene();

    let (device, queue) = init_urx_wgpu_device()?;
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = uzor_urx_wgpu::NativeUrxRenderer::new(device.clone(), queue.clone(), FORMAT);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-proof-harness-urx-wgpu-target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let recorder = DegradeRecorder::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        metrics::with_local_recorder(&recorder, || {
            if let Err(e) = renderer.render_into_encoder(&scene, &mut encoder, &view, uzor_urx_wgpu::Viewport { width, height }) {
                eprintln!("[uzor-proof-harness] urx-wgpu render error: {e:?}");
            }
        });
        queue.submit(Some(encoder.finish()));
        readback_rgba(&device, &queue, &texture, width, height)
    }));

    match result {
        Ok(pixels) => Some(UrxWgpuRender { pixels, degrades: recorder.fired() }),
        Err(payload) => {
            eprintln!("[uzor-proof-harness] urx-gpu leg panicked: {}; skipping urx-gpu leg", panic_message(payload.as_ref()));
            None
        }
    }
}

// ── Combined capture ─────────────────────────────────────────────────────

/// One proof scene, rendered through every available leg — the three
/// CPU legs (always present) plus the two GPU legs
/// (`vello_gpu`/`urx_gpu`, `None` on a GPU-less box).
pub struct MultiLegRender {
    pub width: u32,
    pub height: u32,
    pub tiny_skia: Vec<u8>,
    pub vello_cpu: Vec<u8>,
    pub vello_gpu: Option<Vec<u8>>,
    pub urx_cpu: Vec<u8>,
    pub urx_gpu: Option<UrxWgpuRender>,
}

impl MultiLegRender {
    /// Render `draw` through every leg at `width x height`.
    ///
    /// `draw` takes `&impl Fn` (re-callable), not `FnOnce` — every real
    /// `uzor-figures` proof draw closure already has this shape (e.g.
    /// `|ctx| figure.render(ctx, rect, &theme)`, capturing only shared
    /// borrows), and driving five separate render passes from ONE
    /// closure value (rather than requiring the caller to construct it
    /// five times) is the whole point of this entry point.
    ///
    /// A skipped GPU leg prints why (via that leg's own `eprintln!` on
    /// the failure it hit, plus one summary line here) and simply leaves
    /// its field `None` — the suite keeps running on a GPU-less machine.
    pub fn capture(width: u32, height: u32, draw: impl Fn(&mut dyn RenderContext)) -> Self {
        let vello_gpu = render_vello_gpu(width, height, &draw);
        if vello_gpu.is_none() {
            eprintln!("[uzor-proof-harness] vello-gpu leg: no GPU/software adapter available (or renderer init failed); skipping");
        }
        let urx_gpu = render_urx_wgpu(width, height, &draw);
        if urx_gpu.is_none() {
            eprintln!("[uzor-proof-harness] urx-gpu leg: no GPU/software adapter available; skipping");
        }
        Self {
            width,
            height,
            tiny_skia: render_tiny_skia(width, height, &draw),
            vello_cpu: render_vello_cpu(width, height, &draw),
            vello_gpu,
            urx_cpu: render_urx_cpu(width, height, &draw),
            urx_gpu,
        }
    }

    /// `(label, premultiplied RGBA8 pixels)` for every leg that actually
    /// rendered, in the FIXED display order this crate's composite uses
    /// (tiny-skia | vello-cpu | vello-gpu | urx-cpu | urx-gpu) — a
    /// skipped GPU leg is simply absent from this list, never a
    /// placeholder.
    pub fn legs(&self) -> Vec<(&'static str, &[u8])> {
        let mut out = vec![("tiny-skia", self.tiny_skia.as_slice()), ("vello-cpu", self.vello_cpu.as_slice())];
        if let Some(ref px) = self.vello_gpu {
            out.push(("vello-gpu", px.as_slice()));
        }
        out.push(("urx-cpu", self.urx_cpu.as_slice()));
        if let Some(ref r) = self.urx_gpu {
            out.push(("urx-gpu", r.pixels.as_slice()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_red_rect(ctx: &mut dyn RenderContext) {
        ctx.set_fill_color("#ff0000");
        ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    }

    /// All three CPU legs must produce a non-empty, correctly-sized,
    /// opaque-red buffer for a trivial solid fill with no rasterizer-
    /// specific behavior to diverge on — a plumbing sanity check, not a
    /// divergence test (this crate's real proof tests own that).
    #[test]
    fn all_three_cpu_legs_render_a_solid_fill_correctly_sized_and_opaque_red() {
        let render = MultiLegRender::capture(4, 4, solid_red_rect);
        let center = ((2 * 4 + 2) * 4) as usize;
        for (label, pixels) in [("tiny-skia", &render.tiny_skia), ("vello-cpu", &render.vello_cpu), ("urx-cpu", &render.urx_cpu)] {
            assert_eq!(pixels.len(), 4 * 4 * 4, "{label}: unexpected buffer length");
            assert_eq!(&pixels[center..center + 4], &[255, 0, 0, 255], "{label}: center pixel should be opaque red");
        }
    }

    /// `MultiLegRender::legs` always lists the three CPU legs regardless
    /// of GPU availability, and lists a GPU leg if and only if that
    /// leg's own field is `Some` — the exact invariant the composite
    /// writer depends on.
    #[test]
    fn legs_lists_cpu_legs_always_and_gpu_legs_iff_present() {
        let render = MultiLegRender::capture(4, 4, solid_red_rect);
        let labels: Vec<&str> = render.legs().into_iter().map(|(label, _)| label).collect();
        assert!(labels.contains(&"tiny-skia"));
        assert!(labels.contains(&"vello-cpu"));
        assert!(labels.contains(&"urx-cpu"));
        assert_eq!(labels.contains(&"vello-gpu"), render.vello_gpu.is_some());
        assert_eq!(labels.contains(&"urx-gpu"), render.urx_gpu.is_some());
    }

    /// Both GPU legs must produce a correctly-sized, opaque-red buffer
    /// for the same trivial solid fill the CPU legs already proved —
    /// same plumbing sanity check, extended to the GPU path. `#[ignore]`-
    /// gated (needs a real GPU/software adapter to assert anything, same
    /// convention `uzor-urx-wgpu/tests/parity.rs`'s own GPU-dependent
    /// tests already use) — run with `--ignored` on a box with a GPU.
    ///
    /// Uses a 64x64 canvas (not the CPU legs' own 4x4) and probes a
    /// pixel comfortably deep in the interior (32px from every edge) —
    /// same "interior probe, never near an edge" discipline
    /// `uzor-urx-wgpu/tests/parity.rs`'s own `ParityCase::interior_probes`
    /// already establishes. A 4x4 canvas measured a real, if minor, AA
    /// softening on `urx-gpu` even at its own geometric center (only
    /// 1.5px from the fill's own edge on a canvas that tiny — always-on
    /// 4x MSAA's coverage falloff reaching further than one might expect
    /// at that scale); this fixture is deliberately sized so ordinary AA
    /// softness at a rect's edge can never reach the probe, keeping this
    /// a pure plumbing sanity check, not a rasteriser-precision one.
    #[test]
    #[ignore = "needs a GPU/software adapter; run with --ignored"]
    fn gpu_legs_render_a_solid_fill_correctly_sized_and_opaque_red_when_available() {
        const SIZE: u32 = 64;
        let render = MultiLegRender::capture(SIZE, SIZE, |ctx| {
            ctx.set_fill_color("#ff0000");
            ctx.fill_rect(0.0, 0.0, SIZE as f64, SIZE as f64);
        });
        let center = (((SIZE / 2) * SIZE + SIZE / 2) * 4) as usize;
        let Some(ref vello_gpu) = render.vello_gpu else {
            eprintln!("gpu_legs_render_a_solid_fill_correctly_sized_and_opaque_red_when_available: vello-gpu skipped, no adapter");
            return;
        };
        assert_eq!(vello_gpu.len(), (SIZE * SIZE * 4) as usize, "vello-gpu: unexpected buffer length");
        assert_eq!(&vello_gpu[center..center + 4], &[255, 0, 0, 255], "vello-gpu: center pixel should be opaque red");

        let Some(ref urx_gpu) = render.urx_gpu else {
            eprintln!("gpu_legs_render_a_solid_fill_correctly_sized_and_opaque_red_when_available: urx-gpu skipped, no adapter");
            return;
        };
        assert_eq!(urx_gpu.pixels.len(), (SIZE * SIZE * 4) as usize, "urx-gpu: unexpected buffer length");
        assert_eq!(&urx_gpu.pixels[center..center + 4], &[255, 0, 0, 255], "urx-gpu: center pixel should be opaque red");
        assert!(urx_gpu.degrades.is_empty(), "urx-gpu: a bare solid fill must not trip any degrade counter: {:?}", urx_gpu.degrades);
    }
}
