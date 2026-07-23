//! Pixel-parity harness — render the same `Scene` on `uzor-urx-cpu`
//! (the semantic reference) and `uzor-urx-wgpu`'s native pipeline,
//! compare pixels (design §7).
//!
//! `#[ignore]`-gated — needs a GPU/software adapter:
//! `cargo test -p uzor-urx-wgpu --test parity -- --ignored --nocapture`.
//!
//! Both sides render premultiplied RGBA8 with identical stride (no row
//! padding after `readback_rgba`'s de-stride step) — the comparator
//! diffs the raw bytes directly, no format conversion (design §7
//! "Premultiplication").

#[path = "common/mod.rs"]
mod common;
mod fixtures;

use std::path::{Path, PathBuf};

use uzor_urx_core::scene::Scene;
use uzor_urx_cpu::{CpuBackend, Pixmap};
use uzor_urx_wgpu::{NativeRenderError, NativeUrxRenderer, Viewport};

const CHANNEL_TOLERANCE_EDGE: i32 = 24; // ~9% of 255 — AA-edge slack.
const CHANNEL_TOLERANCE_INTERIOR: i32 = 2; // rounding-only slack — NEVER overridden per-case (design §7).
const MAX_DIFFERING_FRACTION: f64 = 0.02; // 2% of pixels may exceed edge tolerance.

/// Text-specific per-case override (Wave 2 design §7) — 2x both shape
/// budgets, scoped to `parity_glyph_run_two_letters` ONLY via
/// `ParityCase::edge_tolerance`/`max_differing_fraction`. Three real,
/// stacked divergence sources exist for text that don't for shapes
/// (both backends composite byte-identical swash bitmaps, so this is
/// about what happens to that bitmap AFTER rasterisation):
/// 1. Bilinear GPU resampling of an already-antialiased swash mask.
/// 2. 4x MSAA supersampling that same already-smoothed sample again.
/// 3. Different premultiplication arithmetic regimes (CPU integer
///    fixed-point `(a*mask+127)/255` vs native continuous `f32`
///    multiply).
/// `CHANNEL_TOLERANCE_INTERIOR` stays 2 — UNCHANGED, no per-case
/// override exists for it at all — because deep-interior pixels
/// (several texels from any letterform edge) see none of the above:
/// bilinear only perturbs values near a coverage gradient.
const CHANNEL_TOLERANCE_EDGE_TEXT: i32 = 48; // 2x the shape budget (24)
const MAX_DIFFERING_FRACTION_TEXT: f64 = 0.04; // 2x the shape budget (0.02)

/// Native readback target format — non-sRGB, avoids the degamma step
/// a `*Srgb` format would force before comparing against `Pixmap`'s
/// linear-premultiplied bytes (design §7).
const NATIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct ParityCase {
    name: &'static str,
    scene: Scene,
    /// Hand-picked (x, y) pixels known to be interior-solid for this
    /// fixture — checked against `CHANNEL_TOLERANCE_INTERIOR`
    /// unconditionally, regardless of the whole-image budget.
    interior_probes: &'static [(u32, u32)],
    /// Per-case override of the whole-image edge-tolerance/fraction
    /// budget. Defaults (via `Default`, `..Default::default()`) to the
    /// shared `CHANNEL_TOLERANCE_EDGE`/`MAX_DIFFERING_FRACTION` shape
    /// constants, which stay UNTOUCHED — only `parity_glyph_run_two_letters`
    /// overrides these, to the wider `_TEXT` constants (design §7).
    edge_tolerance: i32,
    max_differing_fraction: f64,
}

impl Default for ParityCase {
    fn default() -> Self {
        Self {
            name: "",
            scene: Scene::new(),
            interior_probes: &[],
            edge_tolerance: CHANNEL_TOLERANCE_EDGE,
            max_differing_fraction: MAX_DIFFERING_FRACTION,
        }
    }
}

fn render_cpu(scene: &Scene, width: u32, height: u32) -> Vec<u8> {
    let mut pixmap = Pixmap::new(width, height);
    let backend = CpuBackend::new();
    backend
        .render(scene, &mut pixmap)
        .expect("CpuBackend::render must not error on a well-formed fixture scene");
    pixmap.pixels().to_vec()
}

/// Render `scene` through the native pipeline into an offscreen
/// `RENDER_ATTACHMENT | COPY_SRC` texture and read back premultiplied
/// RGBA8 bytes. Returns `None` (never panics) when no GPU/software
/// adapter is available.
fn render_native(scene: &Scene, width: u32, height: u32) -> Option<Vec<u8>> {
    let (device, queue) = common::init_device()?;

    // Default 4x MSAA (`NativeUrxRenderer::new`) — exercises the real
    // production path, not the `sample_count = 1` escape hatch.
    let mut renderer = NativeUrxRenderer::new(device.clone(), queue.clone(), NATIVE_FORMAT);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-urx-wgpu-parity-target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: NATIVE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer
        .render_into_encoder(scene, &mut encoder, &view, Viewport { width, height })
        .expect("render_into_encoder must not error on a well-formed fixture scene + matching format");
    queue.submit(Some(encoder.finish()));

    Some(common::readback_rgba(&device, &queue, &target, width, height))
}

fn max_channel_diff(a: &[u8], b: &[u8]) -> i32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as i32 - *y as i32).abs())
        .max()
        .unwrap_or(0)
}

fn parity_dump_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = .../uzor/uzor-urx-wgpu ; this is a single
    // Cargo workspace so the shared target dir is one level up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("target").join("parity"))
        .unwrap_or_else(|| PathBuf::from("target/parity"))
}

fn dump_diff_png(path: &Path, width: u32, height: u32, cpu: &[u8], native: &[u8], edge_tolerance: i32) {
    let mut heat = vec![0u8; cpu.len()];
    for (i, chunk) in heat.chunks_exact_mut(4).enumerate() {
        let idx = i * 4;
        let d = max_channel_diff(&cpu[idx..idx + 4], &native[idx..idx + 4]);
        if d > edge_tolerance {
            // Tint pixels that exceed the edge budget red for quick
            // eyeballing which region regressed.
            chunk.copy_from_slice(&[255, 0, 0, 255]);
        } else {
            let v = d.clamp(0, 255) as u8;
            chunk.copy_from_slice(&[v, v, v, 255]);
        }
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = image::save_buffer(path, &heat, width, height, image::ColorType::Rgba8) {
        eprintln!("dump_diff_png: failed to write {}: {e}", path.display());
    }
}

/// Dump `cpu`/`native`/`diff` PNGs to `target/parity/<case_name>_*.png`
/// BEFORE the caller panics — never lose the artifacts on a failure.
fn dump_failure_artifacts(cpu: &[u8], native: &[u8], width: u32, height: u32, case: &ParityCase) {
    let dir = parity_dump_dir();
    common::dump_png(&dir.join(format!("{}_cpu.png", case.name)), width, height, cpu);
    common::dump_png(&dir.join(format!("{}_native.png", case.name)), width, height, native);
    dump_diff_png(&dir.join(format!("{}_diff.png", case.name)), width, height, cpu, native, case.edge_tolerance);
}

/// Interior probes — exact-ish match required, fail immediately (with
/// dumped artifacts) on the first offender.
fn check_interior_probes(cpu: &[u8], native: &[u8], width: u32, height: u32, case: &ParityCase) {
    for &(x, y) in case.interior_probes {
        let idx = ((y * width + x) * 4) as usize;
        let c = &cpu[idx..idx + 4];
        let n = &native[idx..idx + 4];
        let diff = max_channel_diff(c, n);
        if diff > CHANNEL_TOLERANCE_INTERIOR {
            dump_failure_artifacts(cpu, native, width, height, case);
            panic!(
                "{}: interior probe ({x},{y}) diverged beyond tolerance {CHANNEL_TOLERANCE_INTERIOR}: cpu={c:?} native={n:?} (max_diff={diff})",
                case.name
            );
        }
    }
}

/// Whole-image sweep — count pixels exceeding the edge tolerance, fail
/// if the fraction exceeds the (per-case) budget. Always PRINTS the
/// measured fraction (pass or fail) — under `--nocapture`, this is
/// how every case's actual number vs its budget gets reported, not
/// just a binary pass/fail.
fn check_whole_image_budget(cpu: &[u8], native: &[u8], width: u32, height: u32, case: &ParityCase) {
    let total_pixels = (width as usize) * (height as usize);
    let mut differing = 0usize;
    for i in 0..total_pixels {
        let idx = i * 4;
        if max_channel_diff(&cpu[idx..idx + 4], &native[idx..idx + 4]) > case.edge_tolerance {
            differing += 1;
        }
    }
    let fraction = differing as f64 / total_pixels as f64;
    println!(
        "{}: {differing}/{total_pixels} pixels ({:.3}%) exceeded edge tolerance {} — budget is {:.2}%",
        case.name,
        fraction * 100.0,
        case.edge_tolerance,
        case.max_differing_fraction * 100.0
    );
    if fraction > case.max_differing_fraction {
        dump_failure_artifacts(cpu, native, width, height, case);
        panic!(
            "{}: {differing}/{total_pixels} pixels ({:.2}%) exceeded edge tolerance {} — budget is {:.2}%",
            case.name,
            fraction * 100.0,
            case.edge_tolerance,
            case.max_differing_fraction * 100.0
        );
    }
}

fn compare_rgba(cpu: &[u8], native: &[u8], width: u32, height: u32, case: &ParityCase) {
    debug_assert_eq!(cpu.len(), native.len());
    check_interior_probes(cpu, native, width, height, case);
    check_whole_image_budget(cpu, native, width, height, case);
}

fn run_case(case: ParityCase) {
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;
    let cpu = render_cpu(&case.scene, width, height);
    let Some(native) = render_native(&case.scene, width, height) else {
        eprintln!("{}: no GPU/software adapter available; skipping", case.name);
        return;
    };
    compare_rgba(&cpu, &native, width, height, &case);
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_solid_rects_axis_aligned() {
    run_case(ParityCase {
        name: "solid_rects_axis_aligned",
        scene: fixtures::solid_rects_axis_aligned(),
        interior_probes: &[(60, 60), (190, 190), (10, 245)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_solid_rects_with_radii() {
    run_case(ParityCase {
        name: "solid_rects_with_radii",
        scene: fixtures::solid_rects_with_radii(),
        interior_probes: &[(128, 128), (5, 5)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_stroked_rects() {
    run_case(ParityCase {
        name: "stroked_rects",
        scene: fixtures::stroked_rects(),
        // (75, 30): dead center of the first rect's top border band.
        // (75, 75): deep interior of the first (unfilled) stroke rect —
        // must show the background, not the border color.
        interior_probes: &[(75, 30), (75, 75)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_overlapping_translucent_rects() {
    run_case(ParityCase {
        name: "overlapping_translucent_rects",
        scene: fixtures::overlapping_translucent_rects(),
        interior_probes: &[(60, 60), (200, 200), (130, 130)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_lines_at_angles() {
    run_case(ParityCase {
        name: "lines_at_angles",
        scene: fixtures::lines_at_angles(),
        // Mid-body points only — never on/near an endpoint, since the
        // CPU reference always renders round caps regardless of
        // `Stroke.cap` (see the cap-semantics finding in
        // `fixtures::lines_at_angles`'s doc comment) and would
        // genuinely mismatch the native pipeline's true butt caps
        // right at the tips of lines B and D.
        interior_probes: &[(65, 50), (140, 65), (30, 160), (159, 155)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_quad_line_interleave() {
    run_case(ParityCase {
        name: "quad_line_interleave",
        scene: fixtures::quad_line_interleave(),
        // (160, 100): inside the top quad's bounds AND on the line's
        // path — the top quad must win (painter's order).
        // (80, 100): inside the bottom quad's bounds AND on the
        // line's path, but outside the top quad — the line must win
        // over the bottom quad.
        interior_probes: &[(160, 100), (80, 100)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_linear_gradient_rect() {
    run_case(ParityCase {
        name: "linear_gradient_rect",
        scene: fixtures::linear_gradient_rect(),
        // Near-endpoint / midpoint probes (design risk 5), all
        // comfortably inside the rounded shape's flat interior — see
        // `fixtures::linear_gradient_rect`'s doc comment for why the
        // WHOLE interior (not just these 3 points) is expected to
        // match near-exactly, and why only the rounded-corner region
        // (excluded here) is expected to diverge.
        interior_probes: &[(93, 128), (128, 128), (163, 128)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_tessellated_path_star() {
    run_case(ParityCase {
        name: "tessellated_path_star",
        scene: fixtures::tessellated_path_star(),
        // Fill star (nonzero winding): center + a point deep inside
        // the top spike, both comfortably away from the outline edge
        // (design §8 commit 3: "probes at star center + a point deep
        // inside one arm"). Stroke star: an edge midpoint (the
        // straight segment between vertex 0 and vertex 1), deep in
        // the stroke band, away from both its endpoints' round joins.
        interior_probes: &[(70, 140), (70, 109), (190, 110)],
        ..Default::default()
    });
}

#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_clip_rect_stack() {
    run_case(ParityCase {
        name: "clip_rect_stack",
        scene: fixtures::clip_rect_stack(),
        // (128, 55): between outer and inner clip levels — only the
        // outer-level teal band is visible here.
        // (80, 80): inside the outer clip but OUTSIDE the inner one,
        // in a spot the (clipped-away) orange rect would otherwise
        // reach — must show plain BACKGROUND if inner-clip wiring is
        // correct on the Quad instance type.
        // (128, 100): inside the inner clip, on the yellow FillPath
        // triangle (topmost layer there) — proves clip_rect doesn't
        // accidentally cut CONTENT that's legitimately inside the
        // active clip.
        interior_probes: &[(128, 55), (80, 80), (128, 100)],
        ..Default::default()
    });
}

/// `GlyphRun` via the native glyph atlas (Wave 2 design §7) — the ONE
/// fixture in this file using the widened `_TEXT` tolerance budget
/// (`CHANNEL_TOLERANCE_EDGE_TEXT`/`MAX_DIFFERING_FRACTION_TEXT`, this
/// module's doc comment). `CHANNEL_TOLERANCE_INTERIOR` is UNCHANGED
/// (2) even here — `check_interior_probes` has no per-case override.
///
/// **Probe derivation**: rendered `fixtures::glyph_run_two_letters()`
/// through `render_cpu` once (a throwaway scratch test, since removed)
/// and scanned every pixel for "pure white (255,255,255,255) AND every
/// pixel in its 8-neighborhood also pure white" — i.e. a point several
/// texels inside a letterform stroke, nowhere near an AA edge. Three
/// were picked from the resulting candidate set, spread across both
/// glyphs and both letterforms' distinct stroke shapes:
/// - `(56, 97)`: inside glyph 36's upper curve/stroke.
/// - `(97, 105)`: inside glyph 37's vertical stem — this exact column
///   (x=96-97) is pure-white-with-white-neighbors across ~25 CONSECUTIVE
///   rows in the scan (y≈97 through y≈123), i.e. a deep, wide, very
///   stable stroke — the least edge-adjacent point found.
/// - `(102, 110)`: inside glyph 37's horizontal bar (a ~15px-wide
///   pure-white band at this row), comfortably centered away from
///   either end.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_glyph_run_two_letters() {
    run_case(ParityCase {
        name: "glyph_run_two_letters",
        scene: fixtures::glyph_run_two_letters(),
        interior_probes: &[(56, 97), (97, 105), (102, 110)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_TEXT,
        max_differing_fraction: MAX_DIFFERING_FRACTION_TEXT,
    });
}

/// Design §8 commit 4 gate (item 5): `NativeRenderError::ZeroViewport`
/// needs no device at all to trigger (the check is the very first
/// thing `render_into_encoder` does), but constructing a
/// `NativeUrxRenderer` in the first place still needs one — so this
/// lives here, in the GPU-gated file, rather than as a true no-device
/// lib unit test.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn native_render_error_zero_viewport() {
    let Some((device, queue)) = common::init_device() else {
        eprintln!("native_render_error_zero_viewport: no GPU/software adapter available; skipping");
        return;
    };
    let mut renderer = NativeUrxRenderer::new(device.clone(), queue.clone(), NATIVE_FORMAT);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-urx-wgpu-zero-viewport-target"),
        size: wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: NATIVE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let scene = fixtures::resize_sanity_scene(4);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let err = renderer
        .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: 0, height: 0 })
        .expect_err("a zero-area viewport must be rejected");
    assert!(
        matches!(err, NativeRenderError::ZeroViewport { width: 0, height: 0 }),
        "expected ZeroViewport{{0,0}}, got {err:?}"
    );
}

/// `NativeRenderError::FormatMismatch` — build the renderer for one
/// format, hand it a view created from a texture of a DIFFERENT
/// format.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn native_render_error_format_mismatch() {
    let Some((device, queue)) = common::init_device() else {
        eprintln!("native_render_error_format_mismatch: no GPU/software adapter available; skipping");
        return;
    };
    let mut renderer = NativeUrxRenderer::new(device.clone(), queue.clone(), NATIVE_FORMAT);

    // A format the renderer was NOT built for.
    const MISMATCHED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
    assert_ne!(NATIVE_FORMAT, MISMATCHED_FORMAT, "test setup: formats must actually differ");

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-urx-wgpu-format-mismatch-target"),
        size: wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MISMATCHED_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let scene = fixtures::resize_sanity_scene(4);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let err = renderer
        .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: 4, height: 4 })
        .expect_err("a mismatched-format view must be rejected");
    assert!(
        matches!(err, NativeRenderError::FormatMismatch { expected } if expected == NATIVE_FORMAT),
        "expected FormatMismatch{{expected: {NATIVE_FORMAT:?}}}, got {err:?}"
    );
}

/// Design §8 commit 3 gate: render the SAME renderer instance at
/// 64x64, then 512x512, then back to 64x64 — must not panic (this is
/// exactly the scenario that caught the `MsaaTarget` resolve-target
/// size-mismatch bug fixed in `msaa.rs` this commit), and the FINAL
/// (64x64) output must still be parity-green against CPU rendered at
/// the same final size.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn resize_sanity_64_512_64() {
    let Some((device, queue)) = common::init_device() else {
        eprintln!("resize_sanity_64_512_64: no GPU/software adapter available; skipping");
        return;
    };

    let mut renderer = NativeUrxRenderer::new(device.clone(), queue.clone(), NATIVE_FORMAT);

    const FINAL_SIZE: u32 = 64;
    let sizes = [64u32, 512u32, FINAL_SIZE];
    let mut last_native: Option<Vec<u8>> = None;
    for &size in &sizes {
        let scene = fixtures::resize_sanity_scene(size);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor-urx-wgpu-resize-sanity-target"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: NATIVE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer
            .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: size, height: size })
            .unwrap_or_else(|e| panic!("render_into_encoder must not error at size {size}: {e}"));
        queue.submit(Some(encoder.finish()));
        last_native = Some(common::readback_rgba(&device, &queue, &target, size, size));
    }

    let native = last_native.expect("loop runs at least once");
    let cpu = render_cpu(&fixtures::resize_sanity_scene(FINAL_SIZE), FINAL_SIZE, FINAL_SIZE);

    let case = ParityCase {
        name: "resize_sanity_64_512_64",
        scene: fixtures::resize_sanity_scene(FINAL_SIZE),
        interior_probes: &[(FINAL_SIZE / 2, FINAL_SIZE / 2)],
        ..Default::default()
    };
    // Interior-probe-only, deliberately NOT `compare_rgba`'s full
    // whole-image sweep: at a 64x64 canvas, ANY simple rect's AA
    // fringe (a near-constant ~1.5-2px band regardless of canvas
    // size) covers a MUCH larger fraction of total pixels than at the
    // 256x256 canvas every dedicated fixture in this file uses —
    // perimeter scales linearly with size, area quadratically, so the
    // AA-affected fraction scales as roughly `6 / canvas_size`
    // (empirically ~6% at 64x64 vs a comfortable <1% at 256x256 for
    // the same shape). That is the well-documented "two different AA
    // algorithms" divergence already covered by `solid_rects_axis_aligned`
    // et al. at their designed scale — this test's actual job is
    // proving the resize machinery itself doesn't corrupt output
    // (no panic across 64->512->64, content lands in the right place,
    // right color), which the interior probe confirms.
    check_interior_probes(&cpu, &native, FINAL_SIZE, FINAL_SIZE, &case);
}
