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

use uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES;
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

/// Rounded-clip-specific per-case override (Wave 3 design §2.6) —
/// wider than the base shape tier (24/2%) since stencil+4x-MSAA (4
/// discrete per-sample coverage levels) is structurally coarser than
/// CPU's continuous analytic distance-field coverage mask
/// (`uzor-urx-cpu/src/rounded.rs::build_mask_sdf`) at a rounded-clip
/// boundary specifically; narrower than text's 48/4% since this stacks
/// only ONE extra divergence source (a quantized per-sample stencil
/// test) rather than three stacked ones (bilinear atlas resampling +
/// MSAA supersampling + two different premultiplication arithmetic
/// regimes, see the `_TEXT` doc comment above). Scoped to the clip
/// fixtures below ONLY via `ParityCase::edge_tolerance`/
/// `max_differing_fraction` — the shared shape/text constants above
/// stay untouched.
const CHANNEL_TOLERANCE_EDGE_CLIP: i32 = 32;
const MAX_DIFFERING_FRACTION_CLIP: f64 = 0.03;

/// Radial/Sweep gradient per-case override (Wave 4 design §9
/// "Tolerance tiers") — used ONLY by fixtures that cross a spread-mode
/// wrap boundary (`sweep_gradient_rect`'s full-circle `Extend::Repeat`
/// span). The EXACT concentric Radial case (`radial_gradient_rect`)
/// and the gradient-on-`StrokePath` case stay on the BASE shape tier —
/// both backends sample from byte-identical LUT bytes (the shared
/// `uzor-urx-core::gradient_lut` functions) with no wrap boundary
/// crossed, so they should agree tightly. Wider than the base tier
/// (24/2%) because CPU's `f32` arithmetic and GPU's WGSL `f32`
/// arithmetic can legitimately round a wrap boundary's exact pixel
/// differently by construction; narrower than `_CLIP`'s reasoning class
/// (this is a genuine, expected sub-ULP divergence at a SPECIFIC seam,
/// not a structurally coarser AA mechanism across the whole boundary).
const CHANNEL_TOLERANCE_EDGE_GRADIENT: i32 = 32;
const MAX_DIFFERING_FRACTION_GRADIENT: f64 = 0.03;

/// Image per-case override (Wave 4 design §9 "Tolerance tiers") —
/// bilinear-vs-1:1-fast-path divergence at the image's own edges
/// (design §4.3: GPU always bilinear-samples, even at an exact 1:1
/// mapping, which mathematically degenerates to the same value a
/// direct texel copy would produce EXCEPT at boundary texels where
/// CPU's `unscaled` fast path and GPU's bilinear sampling can round
/// slightly differently) — same class of reasoning as Wave 2's `_TEXT`
/// tier for bilinear-atlas-plus-MSAA stacking. Also absorbs the ROTATED
/// image instance's CPU-bbox-vs-GPU-true-rotated-shape mismatch area
/// (design §0.3 — Image inherits Rect's own similarity-vs-shear
/// routing decision), sized down at the fixture level (small,
/// non-upscaled rotated instance) to fit this SAME budget rather than
/// needing an even wider one.
const CHANNEL_TOLERANCE_EDGE_IMAGE: i32 = 40;
const MAX_DIFFERING_FRACTION_IMAGE: f64 = 0.03;

/// Rotated-rect per-case override (design §9's own tier table calls
/// this "base shape tier," but measurement shows it needs a wider
/// FRACTION budget — same per-pixel edge tolerance as the base tier,
/// since severity per pixel isn't the issue, just the AFFECTED AREA).
/// TWO already-independently-documented divergence sources compound in
/// this ONE fixture: (1) CPU bbox-approximates the rotated rect
/// entirely (design §0.3) — an unavoidable, structural shape mismatch
/// scaling with the rect's own size; (2) the rotated edges are
/// themselves OBLIQUE, and `fwidth(dist)`'s well-known L1-norm
/// over-estimate of the true (L2) gradient magnitude widens the native
/// SDF's AA band by up to `sqrt(2)` at non-axis-aligned angles — the
/// EXACT same finding `fixtures::lines_at_angles`'s own doc comment
/// already documents for diagonal lines, here affecting a rotated
/// rect's edges instead. Shrinking the fixture (a first 34x34 attempt
/// measured 3.31%; 24x24 measured 2.44%) helps but doesn't clear the
/// base 2% budget on its own — the oblique-AA component scales with
/// PERIMETER (roughly linear in side length), not area, so it doesn't
/// shrink away as fast as the quadratic bbox-mismatch term. Widening
/// the fraction budget (rather than shrinking indefinitely into an
/// uncomfortably tiny, hard-to-probe shape) is the same engineering
/// call `_CLIP`/`_GRADIENT`/`_IMAGE` already make for their own
/// legitimate, disclosed divergence sources.
const CHANNEL_TOLERANCE_EDGE_ROTATED_BBOX: i32 = 24;
const MAX_DIFFERING_FRACTION_ROTATED_BBOX: f64 = 0.03;

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

// ── Wave 3 Commit 4: rounded-clip + blend-layer parity cases ─────

/// The bbox-vs-real-shape acceptance case (design §6.1) — see
/// `fixtures::rounded_clip_content_crosses_corner`'s doc comment. Uses
/// the `_CLIP` tolerance tier (this module's doc comment) since the
/// whole-image sweep necessarily includes the rounded boundary's own
/// pixel band, where stencil+4x-MSAA's coarser (4-level) graduation
/// legitimately diverges from CPU's continuous analytic mask (design
/// §2.6).
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_rounded_clip_content_crosses_corner() {
    run_case(ParityCase {
        name: "rounded_clip_content_crosses_corner",
        scene: fixtures::rounded_clip_content_crosses_corner(),
        // (52, 52): deep in the top-left CORNER-CUT region — MUST be
        // background; a bbox-approx clip gets this exact pixel wrong
        // (fill color), which is the whole point of the fixture.
        // (125, 125): dead center — fill color under either mechanism,
        // a sanity probe.
        interior_probes: &[(52, 52), (125, 125)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_CLIP,
        max_differing_fraction: MAX_DIFFERING_FRACTION_CLIP,
    });
}

/// Nested rounded+rect clip (design §6.2) — see
/// `fixtures::nested_rounded_and_rect_clip`'s doc comment for the full
/// derivation of probes (a)/(b)/(c). Same `_CLIP` tolerance tier as
/// the single-rounded-clip case above (the nested-rect boundary itself
/// is a hard binary cut on BOTH backends, no extra divergence source;
/// the inner rounded boundary is the only one that needs the wider
/// budget).
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_nested_rounded_and_rect_clip() {
    run_case(ParityCase {
        name: "nested_rounded_and_rect_clip",
        scene: fixtures::nested_rounded_and_rect_clip(),
        // (a) (128, 128): dead center of the inner rounded shape's
        // flat interior — visible fill color.
        // (b) (62, 62): inside the outer rect, ~10.3px outside the
        // inner shape's own corner ARC — must be clipped by the
        // ROUNDED mechanism specifically (proves real stencil, not
        // bbox, for the nested case).
        // (c) (15, 15): outside the outer rect entirely — must be
        // clipped by the plain-rect `clip_rect` mechanism, proving the
        // two clip levels compose rather than one overriding the
        // other.
        interior_probes: &[(128, 128), (62, 62), (15, 15)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_CLIP,
        max_differing_fraction: MAX_DIFFERING_FRACTION_CLIP,
    });
}

/// Blend-layer group (design §6.3) — standard CPU-vs-native parity,
/// proving both backends agree on the CORRECT grouped-composite
/// numeric result (not just that both happen to produce the SAME
/// wrong direct-draw answer — the dedicated isolation-differs proofs
/// that rule that out independently on EACH backend already exist:
/// `uzor-urx-cpu/tests/blend_layer.rs::blend_layer_isolation_differs_from_direct_draw`
/// (CPU) and `uzor-urx-wgpu/src/renderer.rs`'s own
/// `#[cfg(test)] mod tests::blend_layer_group_differs_from_the_same_content_with_no_layer`
/// (native, added Wave 3 Commit 3 — verified NOT duplicated here per
/// the coordinator's explicit instruction).
///
/// This case ALSO does a cheap, same-fixture isolation sanity check
/// (CPU layered vs CPU reference, inline below, not a separate
/// `#[test]`) — the only call site for
/// `fixtures::blend_layer_group_reference_no_layer` in this crate,
/// giving `blend_layer_group`'s EXACT overlap-region numbers (not just
/// the differently-sized rects the CPU crate's own unit test uses) one
/// more free confirmation that the layer path is load-bearing on this
/// specific fixture before the CPU-vs-native comparison runs.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_blend_layer_group() {
    let layered_scene = fixtures::blend_layer_group();
    let reference_scene = fixtures::blend_layer_group_reference_no_layer();
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;

    let cpu_layered = render_cpu(&layered_scene, width, height);
    let cpu_reference = render_cpu(&reference_scene, width, height);
    let probe_idx = ((140u32 * width + 140u32) * 4) as usize;
    let cpu_layered_px = &cpu_layered[probe_idx..probe_idx + 4];
    let cpu_reference_px = &cpu_reference[probe_idx..probe_idx + 4];
    assert!(
        max_channel_diff(cpu_layered_px, cpu_reference_px) > 8,
        "CPU's own layered vs reference must differ meaningfully at the overlap probe on THIS fixture \
         (otherwise PushBlendLayer degraded to identity): layered={cpu_layered_px:?} reference={cpu_reference_px:?}"
    );

    run_case(ParityCase {
        name: "blend_layer_group",
        scene: layered_scene,
        // (140, 140): inside BOTH overlapping rects — the grouped-
        // composite OVERLAP region. (70, 70): inside the first rect
        // `(60,60)-(180,180)` only, outside the second
        // `(110,110)-(230,230)` — a single-shape region within the
        // SAME layer.
        interior_probes: &[(140, 140), (70, 70)],
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

/// Wave 7 tail clip fix (2026-07-24): `GlyphRun` under an active
/// `PushClipRect` that PARTIALLY clips one glyph — see
/// `fixtures::glyph_run_clipped_by_rect`'s own doc comment for the
/// exact probe derivation. Before the fix, `uzor-urx-cpu` painted
/// glyph 37's ink straight through the clip boundary while the native
/// backend correctly scissored it — this fixture's `(102, 110)` probe
/// (just past the clip's `x=100` edge) is exactly the pixel that
/// would have caught that: CPU would have shown glyph ink there,
/// native plain background, a real (not tolerance-noise) divergence.
/// Base `_TEXT` tier — a plain rect clip is a hard binary cut on both
/// backends, no extra AA divergence source (see that fixture's own
/// doc comment).
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_glyph_run_clipped_by_rect() {
    run_case(ParityCase {
        name: "glyph_run_clipped_by_rect",
        scene: fixtures::glyph_run_clipped_by_rect(),
        // (56, 97): glyph 36, fully inside the clip — visible ink on
        // both backends. (97, 105): glyph 37, just inside the clip
        // edge (x=97 < 100) — still visible. (102, 110): glyph 37,
        // just past the clip edge (x=102 >= 100) — must be plain
        // background on BOTH backends; this is the exact probe a
        // regression of the bug this fixture exists for would flip.
        interior_probes: &[(56, 97), (97, 105), (102, 110)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_TEXT,
        max_differing_fraction: MAX_DIFFERING_FRACTION_TEXT,
    });
}

/// Wave 7 tail clip fix: `GlyphRun` under an active
/// `PushClipRoundedRect` — see `fixtures::glyph_run_clipped_by_rounded_rect`'s
/// own doc comment. Proves CPU's `ClipStack` MASK-sampling path (a
/// rounded clip, unlike a plain rect, pushes a `ClipEntry::Mask` — the
/// analytic SDF coverage `uzor-urx-glyph::GlyphClip::mask` now samples
/// per-pixel) agrees with the native stencil path for glyph compositing
/// specifically, not just the plain-rect-bounds case
/// `parity_glyph_run_clipped_by_rect` above already covers. This fixture
/// reuses `rounded_clip_content_crosses_corner`'s own exact, already-
/// proven clip shape (`RoundedRect(50.5, 50.5, 200.5, 200.5, 40.0)`),
/// positioned so its top-left corner cut genuinely overlaps glyph 36's
/// own ink — confirmed empirically (a throwaway per-pixel diff scan
/// against the PRE-FIX code, since removed) to produce a stable,
/// fully-saturated divergence region (`diff=255`, e.g. `cpu=[255;3]`
/// vs `native=[0;3]` at `(47,115)`) before this Wave 7 tail commit;
/// after the fix, both backends agree there too. Probes:
/// - `(56, 97)`: outside the corner's own r×r test box at all (governed
///   by the plain left-edge test only) — comfortably visible, unaffected
///   by the corner cut, a sanity check that fixes to `_TEXT` tier
///   text-only divergence, not clip-only.
/// - `(97, 105)`, `(102, 110)`: `~16-23px` from the corner center,
///   deep inside the circle — visible, another sanity check that the
///   REST of the run still composites normally under an active mask.
/// - `(47, 115)`: inside the corner's own cut region — must be plain
///   BACKGROUND on both backends; this is the exact probe a regression
///   of the fix would flip back to showing raw (unclipped) glyph ink.
/// Uses the `_CLIP` tolerance tier for the whole-image sweep (this
/// fixture DOES cross a real rounded-corner boundary, unlike the
/// plain-rect-clip fixture above) — same reasoning as
/// `rounded_clip_content_crosses_corner`'s own tier choice.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_glyph_run_clipped_by_rounded_rect() {
    run_case(ParityCase {
        name: "glyph_run_clipped_by_rounded_rect",
        scene: fixtures::glyph_run_clipped_by_rounded_rect(),
        interior_probes: &[(56, 97), (97, 105), (102, 110), (47, 115)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_CLIP,
        max_differing_fraction: MAX_DIFFERING_FRACTION_CLIP,
    });
}

/// URX text-gamma design (2026-07-26), §6 Commit 1 gate / §8 testing
/// plan: re-run the SAME Wave 2 glyph fixture with
/// `UrxConfig::text_gamma_enabled: true` on BOTH the CPU and native
/// legs — must STAY 0.000% differing. This is the structural proof
/// (not just "measured small") that Commit 1's placeholder curve
/// (`TEXT_GAMMA_CURVE = [1.0, 1.0]`, both bins the exact-passthrough
/// gamma) is a true no-op end-to-end with the mechanism actually wired
/// and switched ON, not merely inert because the flag defaults off.
/// Deliberately does NOT reuse `run_case`/`render_cpu`/`render_native`
/// (those hardcode `CpuBackend::new()`/`NativeUrxRenderer::new()` —
/// default config, flag off) — every one of the other 26 fixtures must
/// keep exercising the default-off path unmodified.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_glyph_run_two_letters_with_text_gamma_enabled_stays_byte_tight() {
    let scene = fixtures::glyph_run_two_letters();
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;

    let cfg = uzor_urx_core::config::UrxConfig::builder()
        .text_gamma_enabled(true)
        .build()
        .expect("text_gamma_enabled(true) is a valid config");

    let mut pixmap = Pixmap::new(width, height);
    let cpu_backend = CpuBackend::with_config(cfg.clone());
    cpu_backend
        .render(&scene, &mut pixmap)
        .expect("CpuBackend::render must not error on the glyph fixture");
    let cpu = pixmap.pixels().to_vec();

    let Some((device, queue)) = common::init_device() else {
        eprintln!("parity_glyph_run_two_letters_with_text_gamma_enabled_stays_byte_tight: no GPU/software adapter available; skipping");
        return;
    };
    let mut renderer = NativeUrxRenderer::with_config(device.clone(), queue.clone(), NATIVE_FORMAT, 4, &cfg);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-urx-wgpu-parity-text-gamma-target"),
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
        .render_into_encoder(&scene, &mut encoder, &view, Viewport { width, height })
        .expect("render_into_encoder must not error on the glyph fixture");
    queue.submit(Some(encoder.finish()));
    let native = common::readback_rgba(&device, &queue, &target, width, height);

    let case = ParityCase {
        name: "glyph_run_two_letters_text_gamma_enabled",
        scene,
        interior_probes: &[(56, 97), (97, 105), (102, 110)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_TEXT,
        max_differing_fraction: MAX_DIFFERING_FRACTION_TEXT,
    };
    compare_rgba(&cpu, &native, width, height, &case);
}

// ── Wave 4 Commits 5+6: gradients, images, full affine, per-corner ──
// ── radii parity cases ───────────────────────────────────────────────
//
// `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
// §9. 7 new cases (16 -> 23 total).

/// The exact-agreement concentric Radial baseline (design §9) — see
/// `fixtures::radial_gradient_rect`'s doc comment. Base shape tier: no
/// spread-mode wrap boundary crossed, both backends read the SAME LUT
/// bytes.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_radial_gradient_rect() {
    run_case(ParityCase {
        name: "radial_gradient_rect",
        scene: fixtures::radial_gradient_rect(),
        // (128, 128): dead center (t≈0, near the gradient's own
        // start colour). (100, 118): t = dist((100,118),(128.5,128.5))
        // /59 = sqrt(28.5²+10.5²)/59 = 30.38/59 ≈ 0.515 — comfortably
        // mid-LUT-bucket (index ≈131.3, ~0.3 away from either
        // neighbouring integer).
        interior_probes: &[(128, 128), (100, 118)],
        ..Default::default()
    });
}

/// First-ever Sweep fixture in this harness (design §9) — see
/// `fixtures::sweep_gradient_rect`'s doc comment for why this crosses
/// the angle-wrap boundary and needs the `_GRADIENT` tier.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_sweep_gradient_rect() {
    run_case(ParityCase {
        name: "sweep_gradient_rect",
        scene: fixtures::sweep_gradient_rect(),
        // (128, 79.5) i.e. (128, 80): straight up from center — angle
        // = -pi/2, t = (-pi/2 - 0)/(2*pi), wraps via Repeat's `+1.0`
        // to t ≈ 0.75 (index ≈191.25, safely mid-bucket).
        // (177, 128): straight right from center — angle = 0 exactly,
        // t = 0 (index 0, the gradient's own start colour, no wrap
        // involved at all — a stable non-boundary sanity probe).
        interior_probes: &[(128, 80), (177, 128)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_GRADIENT,
        max_differing_fraction: MAX_DIFFERING_FRACTION_GRADIENT,
    });
}

/// "Gradient StrokeRect/FillPath/StrokePath closes for free" (design
/// §2.5/§9) — GPU-ONLY, no CPU comparison. See
/// `fixtures::gradient_on_stroke_path_star`'s doc comment for the THIRD
/// previously-undocumented finding this measurement surfaced: CPU never
/// renders a real gradient on anything but `FillRect` (`Line`/
/// `StrokeRect`/`FillPath`/`StrokePath` all flatten to `brush_to_color`
/// unconditionally), so there is no meaningful CPU baseline to compare
/// against here — the SAME class of "no CPU comparison possible"
/// reasoning as the rotated-rect/sheared-rect cases (§0.3), just from a
/// different root cause (a CPU brush-resolution gap, not a
/// transform-approximation gap).
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn gradient_on_stroke_path_star_gpu_only_correctness() {
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;
    let scene = fixtures::gradient_on_stroke_path_star();
    let Some(native) = render_native(&scene, width, height) else {
        eprintln!("gradient_on_stroke_path_star_gpu_only_correctness: no GPU/software adapter available; skipping");
        return;
    };
    let idx = ((110u32 * width + 190u32) * 4) as usize;
    let got = [native[idx], native[idx + 1], native[idx + 2], native[idx + 3]];
    // Hand-computed: t = dist((190,110), (185.5,140.5)) / 45 ≈ 0.6852,
    // LUT index round(0.6852*255) = 175, straight-lerp between the two
    // fully-opaque stops (straight == premultiplied lerp here, same
    // convention as `linear_gradient_rect`) at t = 175/255 ≈ 0.6863:
    // R = 230 + (60-230)*0.6863 ≈ 113, G = 60 + (90-60)*0.6863 ≈ 81,
    // B = 60 + (230-60)*0.6863 ≈ 177.
    let expected = [113u8, 81, 177, 255];
    for c in 0..4 {
        let diff = (got[c] as i32 - expected[c] as i32).abs();
        assert!(diff <= 4, "gradient_on_stroke_path_star_gpu_only_correctness: channel {c} got {got:?} expected {expected:?} (diff {diff})");
    }
}

/// The Quad-SDF-vs-Triangle-routing acceptance case (design §6.2/§9) —
/// see `fixtures::per_corner_radii_rect`'s doc comment for the
/// discriminating-probe derivation.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_per_corner_radii_rect() {
    run_case(ParityCase {
        name: "per_corner_radii_rect",
        scene: fixtures::per_corner_radii_rect(),
        // (128, 128): dead center, sanity fill probe.
        // (185, 71): 8px diagonally in from the `top_right` (40px
        // radius) corner at (193.5, 63.5) — ~45.25px from that corner's
        // TRUE arc center, outside it (background); a WRONG uniform
        // approximation using `top_left`'s tiny 4px radius would show
        // fill here instead (comfortably past its own tiny rounding
        // zone at only 8px in).
        interior_probes: &[(128, 128), (185, 71)],
        ..Default::default()
    });
}

/// The Quad-SDF rotation-path acceptance case (design §5.3/§9) — see
/// `fixtures::rotated_uniform_radius_rect`'s doc comment. Interior-only
/// probes (design §0.3(ii)): CPU bbox-approximates the rotation
/// entirely, so only points deep inside the TRUE rotated shape (a
/// strict subset of CPU's larger bbox) are analytically forced to
/// agree on both backends. `_ROTATED_BBOX` tolerance tier (this
/// module's own doc comment) — TWO already-documented divergence
/// sources compound here (CPU's bbox approximation + oblique-edge
/// `fwidth` AA widening), needing a wider fraction budget than the base
/// shape tier even after shrinking the fixture down to 24x24.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_rotated_uniform_radius_rect() {
    run_case(ParityCase {
        name: "rotated_uniform_radius_rect",
        scene: fixtures::rotated_uniform_radius_rect(),
        // (128, 128): dead center — invariant under `rotate_about`'s
        // own-center pivot, trivially inside both backends' shapes.
        // (128, 138): center + (0,10) screen offset — inverse-rotated
        // local coordinate (5.0, 8.66) against a 12px half-extent,
        // comfortably (~3.3px margin) inside the TRUE rotated square
        // (radii: None — a sharp-cornered square, no rounding to
        // account for).
        interior_probes: &[(128, 128), (128, 138)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_ROTATED_BBOX,
        max_differing_fraction: MAX_DIFFERING_FRACTION_ROTATED_BBOX,
    });
}

/// The stroke-width-unification regression proof (design §0.2/§9,
/// coordinator's Commit 5 addition) — see
/// `fixtures::scaled_stroke_width`'s doc comment. Base shape tier: both
/// backends now agree exactly on device-constant width, no new
/// divergence source introduced.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_scaled_stroke_width() {
    run_case(ParityCase {
        name: "scaled_stroke_width",
        scene: fixtures::scaled_stroke_width(),
        // StrokeRect (screen rect (20.5,20.5)-(80.5,80.5), border band
        // y in [18.5,22.5] for width 4.0): (50, 20) on the border;
        // (50, 50) deep in the hollow interior — must be BACKGROUND
        // (StrokeRect never fills).
        // StrokePath-as-rect (screen rect (120.5,20.5)-(180.5,80.5),
        // SAME border band shape): (150, 20) on the border; (150, 50)
        // deep hollow interior — background; (150, 17) — OUTSIDE the
        // CORRECT 4px-wide band (edge at y=18.5) but INSIDE where a
        // pre-Commit-4, incorrectly-2x-scaled 8px band (edge at
        // y=16.5) would have painted border colour — the regression
        // probe this fixture exists for.
        interior_probes: &[(50, 20), (50, 50), (150, 20), (150, 50), (150, 17)],
        ..Default::default()
    });
}

/// `ImagePipeline`'s rotation path + the shared `uzor_urx_image`
/// registry (design §9) — see
/// `fixtures::image_axis_aligned_and_rotated`'s doc comment for the
/// probe derivation (axis-aligned instance: any deep-interior
/// checkerboard-cell point; rotated instance: dest's own dead center
/// ONLY, per §0.3(ii)'s interior-only policy for CPU's bbox-approximated
/// rotation). `_IMAGE` tolerance tier.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn parity_image_axis_aligned_and_rotated() {
    run_case(ParityCase {
        name: "image_axis_aligned_and_rotated",
        scene: fixtures::image_axis_aligned_and_rotated(),
        // Axis-aligned instance (dest (20,20)-(84,84), 2x upscale of a
        // 32x32 source with a 3-cell/axis checkerboard, cell=10):
        // (50, 50): uv (0.46875, 0.46875) -> source (15, 15) EXACTLY —
        // dead center of cell(1,1) (source range [10,20) each axis),
        // 5px margin from either boundary, color A.
        // (72, 50): uv (0.8125, 0.46875) -> source (26, 15) — cell(2,1)
        // (source x range [20,32)), 6px margin from the x=20 cell
        // boundary and the x=32 image edge, color B — a
        // different-colour deep-interior point.
        // Rotated instance (dest (150,150)-(182,182), no upscale,
        // rotated 45deg about its own center (166,166)): (166, 166) —
        // the dest's own dead center (uv 0.5,0.5 -> source (16,16),
        // cell(1,1), color A) — the ONE point both backends' mappings
        // agree on regardless of rotation.
        interior_probes: &[(50, 50), (72, 50), (166, 166)],
        edge_tolerance: CHANNEL_TOLERANCE_EDGE_IMAGE,
        max_differing_fraction: MAX_DIFFERING_FRACTION_IMAGE,
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

// ── Wave 4 Commits 5+6: GPU-only correctness + degrade-counter proofs ─
//
// Design §0.3 names TWO GPU-only correctness tests (no CPU comparison
// possible — CPU bbox-approximates any non-axis-aligned transform):
// the rotated-rect corner probe (already added Wave 4 Commit 4, in
// `uzor-urx-wgpu/src/renderer.rs::rotated_uniform_radius_rect_corner_regions_render_correctly`
// — NOT duplicated here) and the sheared-rect probe below (new this
// commit, since nothing covered the shear-routing case at the pixel
// level yet). A THIRD, unplanned GPU-only test
// (`gradient_on_stroke_path_star_gpu_only_correctness`, below) was
// added after measurement surfaced a genuinely new "no CPU baseline
// exists" finding of its own (see that fixture's doc comment) — §0.3's
// "no CPU comparison possible" reasoning turned out to apply to a
// THIRD case design's own defect-list audit didn't anticipate.

/// GPU-ONLY correctness (design §0.3 option (i)/§9) — see
/// `fixtures::sheared_rect_scene`'s doc comment. No CPU comparison: a
/// sheared parallelogram's true shape differs from CPU's bbox
/// approximation by MORE than an AA-edge tolerance, structurally, at
/// the corners.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn sheared_rect_gpu_only_correctness() {
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;
    let scene = fixtures::sheared_rect_scene();
    let Some(native) = render_native(&scene, width, height) else {
        eprintln!("sheared_rect_gpu_only_correctness: no GPU/software adapter available; skipping");
        return;
    };
    let px = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * width + x) * 4) as usize;
        [native[idx], native[idx + 1], native[idx + 2], native[idx + 3]]
    };

    // Local rect [-20,20]x[-20,20] under (x,y) -> (x+0.5y+128, y+128):
    // corners map to (98,108),(138,108),(158,148),(118,148) — a
    // parallelogram, NOT the wider axis-aligned bbox (98,108)-(158,148)
    // CPU would paint for this same transform (design §0.3).
    assert_eq!(px(128, 128), [220, 60, 60, 255], "dead center of the parallelogram must be fill");
    // At screen y=140, the parallelogram's true left boundary sits at
    // x=114 (98 + 20*(32/40), interpolating the left edge from
    // (98,108) to (118,148)) — x=100 is 14px further left, INSIDE the
    // would-be CPU bbox (which starts at x=98) but OUTSIDE the TRUE
    // parallelogram.
    assert_eq!(
        px(100, 140),
        [32, 32, 32, 255],
        "must be background: inside the would-be CPU bbox but outside the TRUE sheared parallelogram"
    );
}

/// Minimal hand-rolled `metrics::Recorder` — same shape as
/// `uzor-urx-wgpu/src/encode.rs`'s own `mod metrics_recorder_proof`
/// (that one is private to the lib crate, unreachable from this
/// integration-test binary, hence a second small copy here rather than
/// a shared export). Used ONLY for the handful of Wave 4 §8 degrade
/// labels whose fixtures are ALSO new this wave — every OTHER label's
/// counter behaviour is already thoroughly proven at the `encode.rs`
/// unit-test level (both device-free and device-gated variants); this
/// is an end-to-end (full `NativeUrxRenderer`) supplement for the
/// specific cases where a fresh, dedicated parity fixture exists to
/// drive it.
mod metrics_probe {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use metrics::{Counter, CounterFn, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit};

    struct RecordedCounter(AtomicU64);
    impl CounterFn for RecordedCounter {
        fn increment(&self, value: u64) {
            self.0.fetch_add(value, Ordering::SeqCst);
        }
        fn absolute(&self, value: u64) {
            self.0.store(value, Ordering::SeqCst);
        }
    }

    #[derive(Default)]
    pub(crate) struct TestRecorder {
        counters: Mutex<HashMap<Key, Arc<RecordedCounter>>>,
    }

    impl TestRecorder {
        pub(crate) fn value_for(&self, metric_name: &str, label: &str) -> u64 {
            let map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
            map.iter()
                .filter(|(key, _)| {
                    key.name() == metric_name && key.labels().any(|l| l.key() == "kind" && l.value() == label)
                })
                .map(|(_, counter)| counter.0.load(Ordering::SeqCst))
                .sum()
        }
    }

    impl Recorder for TestRecorder {
        fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}
        fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}
        fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

        fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
            let mut map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
            let counter = map.entry(key.clone()).or_insert_with(|| Arc::new(RecordedCounter(AtomicU64::new(0))));
            Counter::from_arc(counter.clone())
        }
        fn register_gauge(&self, _key: &Key, _metadata: &Metadata<'_>) -> Gauge {
            Gauge::noop()
        }
        fn register_histogram(&self, _key: &Key, _metadata: &Metadata<'_>) -> Histogram {
            Histogram::noop()
        }
    }
}

/// `native_rect_shear_to_triangle_pipeline` (design §8, new, telemetry-
/// only) fires exactly once for `sheared_rect_scene`'s one sheared
/// `FillRect` — end-to-end through the full `NativeUrxRenderer`, not
/// just at the `encode.rs` unit-test level.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn sheared_rect_counts_shear_telemetry_end_to_end() {
    let recorder = metrics_probe::TestRecorder::default();
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;
    let rendered = metrics::with_local_recorder(&recorder, || {
        let scene = fixtures::sheared_rect_scene();
        render_native(&scene, width, height)
    });
    if rendered.is_none() {
        eprintln!("sheared_rect_counts_shear_telemetry_end_to_end: no GPU/software adapter available; skipping");
        return;
    }
    assert_eq!(
        recorder.value_for(KEY_RENDER_PRIMITIVES, "native_rect_shear_to_triangle_pipeline"),
        1
    );
}

/// `native_per_corner_radii_uniform_approx` (design §8, CLOSED) never
/// fires again — end-to-end proof on the exact fixture designed to
/// exercise non-uniform per-corner radii.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn per_corner_radii_rect_never_counts_the_closed_approximation_label() {
    let recorder = metrics_probe::TestRecorder::default();
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;
    let rendered = metrics::with_local_recorder(&recorder, || {
        let scene = fixtures::per_corner_radii_rect();
        render_native(&scene, width, height)
    });
    if rendered.is_none() {
        eprintln!("per_corner_radii_rect_never_counts_the_closed_approximation_label: no GPU/software adapter available; skipping");
        return;
    }
    assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "native_per_corner_radii_uniform_approx"), 0);
}

/// `gradient_radial_focal_degraded` (unprefixed, shared with CPU, design
/// §2.6) never fires for the deliberately-concentric
/// `radial_gradient_rect` fixture — end-to-end proof that this
/// fixture really is the "exact, non-approximated" baseline case its
/// own doc comment claims to be.
#[test]
#[ignore = "needs a GPU/software adapter; run with --ignored"]
fn radial_gradient_rect_never_counts_the_focal_degrade() {
    let recorder = metrics_probe::TestRecorder::default();
    let width = fixtures::CANVAS;
    let height = fixtures::CANVAS;
    let rendered = metrics::with_local_recorder(&recorder, || {
        let scene = fixtures::radial_gradient_rect();
        render_native(&scene, width, height)
    });
    if rendered.is_none() {
        eprintln!("radial_gradient_rect_never_counts_the_focal_degrade: no GPU/software adapter available; skipping");
        return;
    }
    assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "gradient_radial_focal_degraded"), 0);
}
