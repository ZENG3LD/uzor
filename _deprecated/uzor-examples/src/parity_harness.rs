//! Wave 6 app-level screenshot-diff harness
//! (`docs/uzor-engines/plans/urx-wave6-autodetect-cutover-design-2026-07-25.md`
//! §4) — the comparator/rendering glue shared by every reference
//! fixture's own `#[cfg(test)] mod screenshot_diff` (colocated inside
//! each demo's own bin file, design §4.1 — this module owns only the
//! generic rendering + comparison utilities, not any fixture-specific
//! driving code).
//!
//! Two tolerance tiers (design §4.4), matching the brief's own split
//! exactly:
//! - **urx-native vs urx-cpu — byte-tight, automated pass/fail.**
//!   [`compare_tight`] reuses the SAME proven tolerance constants
//!   already validated per-primitive across Waves 1-4
//!   (`uzor-urx-wgpu/tests/parity.rs`'s `CHANNEL_TOLERANCE_INTERIOR`/
//!   `CHANNEL_TOLERANCE_EDGE`/`MAX_DIFFERING_FRACTION` and their
//!   per-feature-class overrides) rather than reinventing new numbers.
//!   A real app fixture composing those same primitives landing OUTSIDE
//!   that envelope is a genuine compose-level bug (batching order,
//!   state bleed between draw calls) the primitive suite structurally
//!   cannot see. **Hard gate.**
//! - **urx-native vs vello — visual, human-reviewed, not a hard
//!   number.** Different rasterisers, different AA algorithms,
//!   different color-management defaults; byte parity was never
//!   vello's promise. [`dump_comparison_pngs`] writes both renders
//!   side-by-side plus a heat-diff for EVERY fixture unconditionally;
//!   the gate is a human looking at all of them before a flip commit
//!   lands, not a computed threshold.

use uzor::docking::panels::DockPanel;
use uzor::layout::window::{RawHandle, WindowKey, WindowProvider};
use uzor::layout::LayoutManager;
use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_urx_core::scene::Scene;

/// Render `scene` through the CPU rasteriser — the family's semantic
/// reference (`docs/uzor-engines/plan-urx-family-parity-2026-07-24.md`
/// §4: "the CPU backend is the semantics reference, not vello"). No
/// GPU, no device, never `None`.
pub fn render_via_urx_cpu(scene: &Scene, width: u32, height: u32) -> Vec<u8> {
    let mut pixmap = uzor_urx_cpu::Pixmap::new(width, height);
    let backend = uzor_urx_cpu::CpuBackend::new();
    if let Err(e) = backend.render(scene, &mut pixmap) {
        eprintln!("[parity-harness] urx-cpu render error: {:?}", e);
    }
    pixmap.pixels().to_vec()
}

/// Headless wgpu device — `compatible_surface: None`, no window. Same
/// shape as `uzor-urx-wgpu/tests/common/mod.rs::init_device` (design
/// §4.2's own cited precedent). Returns `None` (never panics) when no
/// adapter is available so callers can skip gracefully on a GPU-less
/// box.
fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("uzor-examples-parity-harness"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()
}

/// Read back a `RENDER_ATTACHMENT | COPY_SRC` texture as a tight (no
/// row padding) `Vec<u8>` of premultiplied RGBA8 bytes — same shape as
/// `uzor-urx-wgpu/tests/common/mod.rs::readback_rgba`.
fn readback_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let aligned_stride = (width * 4 + 255) & !255;
    let buf_size = (aligned_stride * height) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor-examples-parity-harness-readback"),
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
    rx.recv()
        .expect("map_async callback channel closed before firing")
        .expect("staging buffer map failed");
    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height as usize {
        let row_start = row * aligned_stride as usize;
        let row_end = row_start + (width * 4) as usize;
        out.extend_from_slice(&raw[row_start..row_end]);
    }
    drop(raw);
    staging.unmap();
    out
}

/// Render `scene` through `NativeUrxRenderer::render_into_encoder` into
/// an offscreen `Rgba8Unorm | RENDER_ATTACHMENT | COPY_SRC` texture,
/// headless. Returns `None` (never panics) on a GPU-less box — callers
/// skip gracefully, matching the crate-level parity suite's own
/// convention.
pub fn render_via_urx_native(scene: &Scene, width: u32, height: u32) -> Option<Vec<u8>> {
    let (device, queue) = init_device()?;
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = uzor_urx_wgpu::NativeUrxRenderer::new(device.clone(), queue.clone(), FORMAT);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-examples-parity-harness-target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    if let Err(e) = renderer.render_into_encoder(scene, &mut encoder, &view, uzor_urx_wgpu::Viewport { width, height }) {
        eprintln!("[parity-harness] urx-native render error: {:?}", e);
    }
    queue.submit(Some(encoder.finish()));

    Some(readback_rgba(&device, &queue, &texture, width, height))
}

/// Record a `RenderContext`-driven closure into a `Scene` via
/// `uzor_render_urx::UrxRenderContext` (records, doesn't rasterise) —
/// lets a fixture written as `fn(&mut dyn RenderContext)` (every demo's
/// existing draw functions) feed BOTH [`render_via_urx_cpu`] and
/// [`render_via_urx_native`] from one call.
pub fn record_via_urx_ctx(width: u32, height: u32, f: impl FnOnce(&mut dyn RenderContext)) -> Scene {
    let mut ctx = uzor_render_urx::UrxRenderContext::new(1.0);
    ctx.begin_frame(width, height);
    f(&mut ctx);
    ctx.take_scene()
}

/// Render the SAME closure through `VelloCpuRenderContext` — headless,
/// no device, the loose visual-tolerance reference leg. Reuses the
/// exact context `submit_cpu_vello` drives in production. Output is
/// premultiplied RGBA8 (`VelloCpuRenderContext::render_to_pixmap_rgba8`'s
/// own documented convention), matching every other leg here — no
/// format conversion needed anywhere in this module.
pub fn render_via_vello_cpu(width: u32, height: u32, f: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut ctx = uzor_render_vello_cpu::VelloCpuRenderContext::new(1.0);
    ctx.begin_frame(width, height);
    f(&mut ctx);
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    ctx.render_to_pixmap_rgba8(&mut buf, width as u16, height as u16);
    buf
}

/// Headless stand-in for a platform `WindowProvider` (winit / web /
/// mobile) — `LayoutManager`'s flat API (`solve`/`rect_for_edge_slot`/
/// etc) requires a "current window" attached before any flat-API call
/// (its own panic message: "no current_window — platform layer must
/// call set_current_window"); this lets a fixture satisfy that
/// requirement without a real OS window. Every method beyond
/// `window_rect` is dead weight for a single non-interactive paint
/// pass — trait defaults would do, but the trait requires them
/// explicitly.
struct HeadlessWindowProvider {
    rect: Rect,
}

impl WindowProvider for HeadlessWindowProvider {
    fn poll_events(&mut self) -> Vec<uzor::input::PlatformEvent> {
        Vec::new()
    }
    fn window_rect(&self) -> Rect {
        self.rect
    }
    fn scale_factor(&self) -> f64 {
        1.0
    }
    fn request_redraw(&mut self) {}
    fn should_close(&self) -> bool {
        false
    }
    fn raw_window_handle(&self) -> Option<RawHandle> {
        None
    }
}

/// Attach a fresh headless window (key `"parity-fixture"`) to `layout`,
/// make it current, and solve it once at `(width, height)` — the
/// one-time setup every `LayoutManager`-driven fixture (any `DockPanel`
/// type `P` — `l3-dashboard`'s `DemoPanel`, `force-graph-demo`'s
/// `NoPanel`, ...) needs before calling app code that uses the flat API
/// (`App::ui`, `draw_l3_frame`, etc). Shared here (not duplicated per
/// fixture) since `WindowProvider` itself has no `P` parameter — one
/// implementation covers every panel type.
pub fn attach_headless_window<P: DockPanel>(layout: &mut LayoutManager<P>, width: u32, height: u32) {
    let key = WindowKey::new("parity-fixture");
    let rect = Rect::new(0.0, 0.0, width as f64, height as f64);
    layout.attach_window(key.clone(), Box::new(HeadlessWindowProvider { rect }));
    layout.set_current_window(key);
    layout.solve(rect);
}

/// Byte-tight comparator tolerance — same shape as
/// `uzor-urx-wgpu/tests/parity.rs`'s own per-case tiers. `interior` is
/// carried through for the CALLER's own fixture-specific probe-point
/// assertions (this generic comparator has no notion of which
/// coordinates are "interior" for a given fixture — same division of
/// labour as the crate-level harness, where `ParityCase::interior_probes`
/// is fixture-specific data, not part of the shared comparator).
#[derive(Debug, Clone, Copy)]
pub struct ChannelTolerance {
    /// Whole-image sweep: a pixel counts as "differing" when its max
    /// per-channel diff exceeds this.
    pub edge: i32,
    /// Reference value for the CALLER's own interior-probe assertions
    /// (not read by [`compare_tight`] itself).
    pub interior: i32,
    /// Whole-image sweep budget — the fraction of "differing" pixels
    /// (per `edge` above) allowed before [`DiffReport::within_budget`]
    /// is `false`.
    pub max_differing_fraction: f64,
}

/// Base shape tolerance tier — same numbers as
/// `uzor-urx-wgpu/tests/parity.rs`'s `CHANNEL_TOLERANCE_EDGE` (24) /
/// `CHANNEL_TOLERANCE_INTERIOR` (2) / `MAX_DIFFERING_FRACTION` (0.02),
/// validated per-primitive across Waves 1-4 — the default tier a
/// fixture reaches for unless its own content needs a wider,
/// per-feature-class override (text/gradient/rounded-clip/image/
/// rotated-bbox — see that file's own doc comments for each one's
/// derivation).
impl Default for ChannelTolerance {
    fn default() -> Self {
        Self { edge: 24, interior: 2, max_differing_fraction: 0.02 }
    }
}

/// Result of [`compare_tight`].
#[derive(Debug, Clone, Copy)]
pub struct DiffReport {
    pub differing_fraction: f64,
    pub max_channel_diff: i32,
    pub within_budget: bool,
}

fn max_channel_diff_at(a: &[u8], b: &[u8]) -> i32 {
    a.iter().zip(b.iter()).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0)
}

/// Byte-tight whole-image comparator — same algorithm as
/// `uzor-urx-wgpu/tests/parity.rs`'s `max_channel_diff`/
/// `check_whole_image_budget`, generalized to any two equal-length
/// premultiplied-RGBA8 buffers so app fixtures reuse the SAME proven
/// tolerance tiers rather than reinventing new numbers.
pub fn compare_tight(a: &[u8], b: &[u8], tol: ChannelTolerance) -> DiffReport {
    debug_assert_eq!(a.len(), b.len(), "compare_tight: buffers must be the same length");
    let total_pixels = a.len() / 4;
    if total_pixels == 0 {
        return DiffReport { differing_fraction: 0.0, max_channel_diff: 0, within_budget: true };
    }
    let mut differing = 0usize;
    let mut max_diff = 0i32;
    for i in 0..total_pixels {
        let idx = i * 4;
        let d = max_channel_diff_at(&a[idx..idx + 4], &b[idx..idx + 4]);
        max_diff = max_diff.max(d);
        if d > tol.edge {
            differing += 1;
        }
    }
    let differing_fraction = differing as f64 / total_pixels as f64;
    DiffReport {
        differing_fraction,
        max_channel_diff: max_diff,
        within_budget: differing_fraction <= tol.max_differing_fraction,
    }
}

/// `target/parity-app/` — one level up from `CARGO_MANIFEST_DIR`
/// (`.../uzor-examples`), matching `uzor-urx-wgpu/tests/parity.rs`'s
/// own `target/parity/` convention. Gitignored (the repo's standing
/// "proof renders are gitignored junk" policy) — NEVER commit these
/// PNGs.
fn parity_app_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("target").join("parity-app"))
        .unwrap_or_else(|| std::path::PathBuf::from("target/parity-app"))
}

/// Write `premul_rgba` to a PNG, un-premultiplying first — display-only
/// convenience so a human can eyeball the dump without it looking
/// artificially dark. Same conversion
/// `uzor-urx-wgpu/tests/common/mod.rs::dump_png` already uses; the
/// numeric [`compare_tight`] comparator above never touches this
/// unpremultiplied copy.
fn dump_png(path: &std::path::Path, width: u32, height: u32, premul_rgba: &[u8]) {
    let mut straight = vec![0u8; premul_rgba.len()];
    for (src, dst) in premul_rgba.chunks_exact(4).zip(straight.chunks_exact_mut(4)) {
        let a = src[3];
        if a == 0 {
            dst.copy_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let unmul = |c: u8| -> u8 { ((c as u32 * 255 + (a as u32) / 2) / (a as u32)).min(255) as u8 };
        dst[0] = unmul(src[0]);
        dst[1] = unmul(src[1]);
        dst[2] = unmul(src[2]);
        dst[3] = a;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = image::save_buffer(path, &straight, width, height, image::ColorType::Rgba8) {
        eprintln!("[parity-harness] dump_png: failed to write {}: {e}", path.display());
    }
}

/// Dumps `<dir_name>_{a,b,diff}.png` under `target/parity-app/` —
/// called on EVERY vello-comparison leg unconditionally (not just on
/// failure), since the visual-tolerance gate is human review of these
/// PNGs, not a hard automated threshold (design §4.4). Also usable for
/// the byte-tight leg's own failure artifacts, same pattern
/// `uzor-urx-wgpu/tests/parity.rs::dump_failure_artifacts` establishes.
pub fn dump_comparison_pngs(dir_name: &str, width: u32, height: u32, a: &[u8], b: &[u8]) {
    let dir = parity_app_dir();
    dump_png(&dir.join(format!("{dir_name}_a.png")), width, height, a);
    dump_png(&dir.join(format!("{dir_name}_b.png")), width, height, b);

    let mut heat = vec![0u8; a.len().min(b.len())];
    for (chunk, (achunk, bchunk)) in heat.chunks_exact_mut(4).zip(a.chunks_exact(4).zip(b.chunks_exact(4))) {
        let d = max_channel_diff_at(achunk, bchunk).clamp(0, 255) as u8;
        chunk.copy_from_slice(&[d, d, d, 255]);
    }
    // The heat map is already a plain (non-premultiplied) greyscale
    // image (constant alpha 255) — `dump_png`'s un-premultiply step is
    // a safe no-op here (opaque alpha divides out to itself), so reuse
    // it directly rather than a second PNG-writing code path.
    dump_png(&dir.join(format!("{dir_name}_diff.png")), width, height, &heat);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_urx_core::math::{Affine, Brush, Color};
    use uzor_urx_core::scene::DrawCommand;

    #[test]
    fn compare_tight_identical_buffers_are_within_budget() {
        let a = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let report = compare_tight(&a, &a, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 0);
        assert_eq!(report.differing_fraction, 0.0);
        assert!(report.within_budget);
    }

    #[test]
    fn compare_tight_flags_a_large_difference_outside_the_edge_tolerance() {
        let a = vec![0u8, 0, 0, 255];
        let b = vec![255u8, 255, 255, 255];
        let report = compare_tight(&a, &b, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 255);
        assert_eq!(report.differing_fraction, 1.0);
        assert!(!report.within_budget);
    }

    #[test]
    fn compare_tight_tolerates_a_small_difference_within_the_edge_tolerance() {
        let a = vec![100u8, 100, 100, 255];
        let b = vec![110u8, 100, 100, 255]; // diff = 10, under the default edge of 24
        let report = compare_tight(&a, &b, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 10);
        assert_eq!(report.differing_fraction, 0.0);
        assert!(report.within_budget);
    }

    /// `urx-cpu` and the recording context agree on a trivial solid-fill
    /// scene — a cheap, device-free sanity check that the harness's own
    /// plumbing (recording -> CPU rasterise) produces sane, non-empty
    /// output before any fixture relies on it.
    #[test]
    fn record_via_urx_ctx_and_render_via_urx_cpu_produce_a_solid_fill() {
        let scene = record_via_urx_ctx(4, 4, |ctx| {
            ctx.set_fill_color("#ff0000");
            ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
        });
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(&scene.commands[0], DrawCommand::FillRect { .. }));

        let pixels = render_via_urx_cpu(&scene, 4, 4);
        assert_eq!(pixels.len(), 4 * 4 * 4);
        // Center pixel must be opaque red (premultiplied == straight at
        // full alpha).
        let idx = ((2 * 4 + 2) * 4) as usize;
        assert_eq!(&pixels[idx..idx + 4], &[255, 0, 0, 255]);
    }

    /// Sanity: the two rendering legs agree byte-tight on a trivial
    /// scene with NO GPU-specific behaviour to diverge on (this does
    /// not need a device — it drives `render_via_urx_cpu` twice against
    /// the SAME `Scene`, proving `compare_tight` itself is wired
    /// correctly against a real render, not a synthetic buffer).
    #[test]
    fn compare_tight_against_two_identical_cpu_renders_is_perfect() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: uzor_urx_core::Rect::new(0.0, 0.0, 8.0, 8.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(30, 60, 220, 255)),
            transform: Affine::IDENTITY,
        });
        let a = render_via_urx_cpu(&scene, 8, 8);
        let b = render_via_urx_cpu(&scene, 8, 8);
        let report = compare_tight(&a, &b, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 0);
        assert!(report.within_budget);
    }
}
