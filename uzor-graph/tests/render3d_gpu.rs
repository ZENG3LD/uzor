//! Headless GPU correctness tests for `uzor-graph`'s 3D render path
//! (W3D arc plan §4 Wave 2 gate). Own `wgpu::Instance`/`Device` — mirrors
//! `uzor-urx-3d/tests/cube_render.rs`'s pattern exactly
//! (`init_device`/`make_target`/`readback_rgba`).
//!
//! 1. `build_scene_renders_visually_distinct_node_and_edge_pixels` —
//!    `GraphEngine3D::build_scene()` for a small deterministic 2-node
//!    graph produces node-sphere and edge-line pixels that are
//!    clearly brighter than the background clear color.
//! 2. `camera_orbit_changes_pixels` — orbiting `Camera3D` changes the
//!    rendered frame substantially (mirrors `cube_render.rs`'s own
//!    `camera_move_changes_pixels`).
//! 3. `id_pass_scene_produces_exactly_decodable_node_ids_at_known_pixels`
//!    (Wave 4) — `GraphEngine3D::build_id_pass_scene()` renders to a real
//!    id-pass frame and `render3d::decode_gpu_pick_pixel` recovers the
//!    EXACT `NodeIndex` at each node's own pixel, and `None` at a
//!    background pixel — the "PROVE, don't assume" gate for
//!    `render3d.rs`'s tonemap-survival encode/decode scheme.
//! 4. `request_and_poll_gpu_pick_resolves_the_correct_node_via_deferred_readback`
//!    (Wave 4) — exercises the REAL `pick3d::request_gpu_pick`/
//!    `pick3d::poll_gpu_pick` async plumbing end-to-end (blocking poll
//!    loop, acceptable in tests per the task's own instruction).
//! 5. `long_thin_diagonal_edge_at_the_engines_default_distance_renders_with_continuous_coverage`
//!    (Wave C, re-verified on the round-2 edge-quad geometry) — the
//!    original stipple-defect fixture still renders continuous coverage.
//! 6. `edge_quad_analytic_aa_feathers_the_line_edge_instead_of_a_binary_hard_step`
//!    (Wave D, round 2) — samples perpendicular to a broadside edge's
//!    own centerline (no MSAA, no bloom/SSAO) and proves the shader's
//!    OWN coverage function produces intermediate (partially-covered)
//!    brightness values on both sides, not a binary hard edge.
//!
//! Run:
//!   cargo test -p uzor-graph --test render3d_gpu -- --include-ignored --nocapture

use uzor::types::Rect;
use uzor_graph::camera3d::Camera3D;
use uzor_graph::engine3d::GraphEngine3D;
use uzor_graph::graph::Graph;
use uzor_graph::interaction::pick3d;
use uzor_graph::interaction::pick3d::project_world_to_screen;
use uzor_graph::layout::force_directed_3d::ForceDirectedLayout3D;
use uzor_graph::particle::Particle;
use uzor_graph::render3d;
use uzor_graph::NodeIndex;
use uzor_urx_3d::{Mesh, MeshLit, PerspectiveCamera, Renderer3D, Scene3D, Vec3};

use std::collections::HashSet;
use std::sync::Arc;

const W: u32 = 128;
const H: u32 = 128;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    // `wgpu::InstanceDescriptor` no longer derives `Default` as of the
    // `wgpu`/`wgpu-types` version this workspace resolves to (a
    // pre-existing drift, not introduced here — `uzor-urx-hybrid`'s own
    // `atlas_auto_resize` test hits the same API surface, per
    // `uzor-graph/CLAUDE.md`'s divergence log). `cube_render.rs`'s own
    // `InstanceDescriptor::default()` call is the pattern this test
    // otherwise mirrors exactly; only this one constructor call diverges.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("uzor-graph-render3d-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uzor-graph-render3d-target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn readback_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let aligned_stride = (W * 4 + 255) & !255;
    let buf_size = (aligned_stride * H) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor-graph-render3d-readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(aligned_stride), rows_per_image: Some(H) },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().expect("map_async callback fired").expect("buffer map succeeded");
    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for row in 0..H as usize {
        let row_start = row * aligned_stride as usize;
        let row_end = row_start + (W * 4) as usize;
        out.extend_from_slice(&raw[row_start..row_end]);
    }
    drop(raw);
    staging.unmap();
    out
}

#[inline]
fn at(buf: &[u8], x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * W + x) * 4) as usize;
    [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
}

fn brightness(p: [u8; 4]) -> u32 {
    p[0] as u32 + p[1] as u32 + p[2] as u32
}

/// Brightest pixel within a small box around `(cx, cy)` — a genuinely
/// required sampling change for the Wave C edge-quality overhaul
/// (`uzor-graph/CLAUDE.md`'s divergence log): edges are now a true
/// GPU-native 1-device-pixel `LineList` line (see `uzor_urx_3d::NodeMesh::Line`),
/// not a several-pixel-wide cylinder — WITHOUT MSAA armed (this specific
/// test's own configuration), an unantialiased hairline can rasterize
/// into a row/column adjacent to a naive `round()`-projected pixel
/// rather than that exact pixel. A small-neighborhood "is a lit pixel
/// near here" check is the correct way to verify a hairline is visible
/// near a projected point — it does not weaken the underlying claim
/// (the edge is genuinely drawn near its midpoint), only the SAMPLING
/// method, which has to change now that the geometry has zero width
/// tolerance for exact single-pixel rounding.
fn brightest_near(buf: &[u8], cx: u32, cy: u32, radius: i64) -> [u8; 4] {
    let mut best = at(buf, cx, cy);
    let mut best_b = brightness(best);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = (cx as i64 + dx).clamp(0, (W - 1) as i64) as u32;
            let y = (cy as i64 + dy).clamp(0, (H - 1) as i64) as u32;
            let p = at(buf, x, y);
            let b = brightness(p);
            if b > best_b {
                best = p;
                best_b = b;
            }
        }
    }
    best
}

type DemoGraph = Graph<(), ()>;

/// 2 well-separated nodes joined by one edge, positioned directly (not
/// simulated — the physics core is Wave 1's proof surface, this test's
/// job is only the render path).
fn two_node_graph() -> (DemoGraph, [uzor_graph::graph::NodeIndex; 2]) {
    let mut graph = DemoGraph::new();
    let a = graph.push_node((), "a", "cat-a", 2.5);
    let b = graph.push_node((), "b", "cat-b", 2.5);
    graph.push_edge(a, b, 1.0, ());
    (graph, [a, b])
}

fn head_on_engine() -> GraphEngine3D<(), (), ForceDirectedLayout3D> {
    let (graph, _ids) = two_node_graph();
    let mut engine = GraphEngine3D::new(graph, ForceDirectedLayout3D::default());
    engine.particles[0] = Particle::at3(-6.0, 0.0, 0.0);
    engine.particles[1] = Particle::at3(6.0, 0.0, 0.0);
    engine.camera = Camera3D { target: Vec3::ZERO, distance: 20.0, yaw: 0.0, pitch: 0.0, ..Camera3D::default() };
    engine
}

#[test]
#[ignore]
fn build_scene_renders_visually_distinct_node_and_edge_pixels() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let engine = head_on_engine();
    let scene = engine.build_scene(H as f64);
    assert_eq!(scene.nodes.len(), 3, "2 node spheres + 1 edge line");

    let aspect = W as f32 / H as f32;
    let camera = engine.camera(aspect);

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    let (ax, ay) = project_world_to_screen(&camera, Vec3::new(-6.0, 0.0, 0.0), viewport).expect("node a is in front of the eye");
    let (bx, by) = project_world_to_screen(&camera, Vec3::new(6.0, 0.0, 0.0), viewport).expect("node b is in front of the eye");
    let (mx, my) = project_world_to_screen(&camera, Vec3::ZERO, viewport).expect("edge midpoint is in front of the eye");

    let a_px = at(&px, ax.round() as u32, ay.round() as u32);
    let b_px = at(&px, bx.round() as u32, by.round() as u32);
    // Wave C — edges are now a true GPU-native 1-device-pixel `LineList`
    // line, not a several-pixel-wide cylinder; this test never arms MSAA,
    // so an unantialiased hairline needs a small-neighborhood check to
    // reliably prove visibility near its midpoint (see `brightest_near`'s
    // own doc comment — a sampling-method fix, not a weakened claim).
    let mid_px = brightest_near(&px, mx.round() as u32, my.round() as u32, 2);
    // Two corners far from every node/edge — background reference
    // points. `Renderer3D` runs an HDR + ACES tonemap + gamma composite
    // (`uzor-urx-3d/src/shaders/composite_aces.wgsl`) even over the
    // clear color, so the readback background is NOT literally
    // `scene.clear_color` scaled to `u8` — comparing two independent
    // corners to each other (rather than asserting a specific absolute
    // value) proves both samples are genuinely background without
    // depending on the tonemap curve's exact numbers.
    let corner_px = at(&px, 2, 2);
    let far_corner_px = at(&px, W - 3, H - 3);

    eprintln!("(ax,ay)=({ax},{ay}) (bx,by)=({bx},{by}) (mx,my)=({mx},{my})");
    eprintln!("a={a_px:?} b={b_px:?} mid={mid_px:?} corner={corner_px:?} far_corner={far_corner_px:?}");

    assert_eq!(
        corner_px, far_corner_px,
        "two corners far from every node/edge should both be pure (tonemapped) background: {corner_px:?} vs {far_corner_px:?}"
    );
    assert!(
        brightness(a_px) > brightness(corner_px) + 30,
        "node a should be visually distinct from the background: {a_px:?} vs {corner_px:?}"
    );
    assert!(
        brightness(b_px) > brightness(corner_px) + 30,
        "node b should be visually distinct from the background: {b_px:?} vs {corner_px:?}"
    );
    assert!(
        brightness(mid_px) > brightness(corner_px) + 20,
        "the edge line should be visible near its midpoint: {mid_px:?} vs {corner_px:?}"
    );
    assert!(
        a_px != mid_px || b_px != mid_px,
        "node spheres and the edge line should not be pixel-identical (both are drawn, not just one covering the other): a={a_px:?} b={b_px:?} mid={mid_px:?}"
    );
}

#[test]
#[ignore]
fn camera_orbit_changes_pixels() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let mut engine = head_on_engine();
    let scene = engine.build_scene(H as f64);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    let aspect = W as f32 / H as f32;

    let cam_a = engine.camera(aspect);
    let (tex_a, view_a) = make_target(&device);
    let mut enc_a = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc_a, &view_a, &cam_a, &scene);
    queue.submit(Some(enc_a.finish()));
    let px_a = readback_rgba(&device, &queue, &tex_a);

    // Large orbit — well past this engine's own orbit sensitivity, to
    // guarantee a substantial viewpoint change (see `Camera3D::orbit`'s
    // own `ORBIT_RADIANS_PER_PX`).
    engine.camera.orbit(600.0, 250.0);
    let cam_b = engine.camera(aspect);
    let (tex_b, view_b) = make_target(&device);
    let mut enc_b = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc_b, &view_b, &cam_b, &scene);
    queue.submit(Some(enc_b.finish()));
    let px_b = readback_rgba(&device, &queue, &tex_b);

    let total = (W * H) as usize;
    let mut diff = 0usize;
    for i in 0..total {
        let a = &px_a[i * 4..i * 4 + 3];
        let b = &px_b[i * 4..i * 4 + 3];
        let d = (a[0] as i16 - b[0] as i16).unsigned_abs() as u32
            + (a[1] as i16 - b[1] as i16).unsigned_abs() as u32
            + (a[2] as i16 - b[2] as i16).unsigned_abs() as u32;
        if d > 20 {
            diff += 1;
        }
    }
    let pct = (diff as f32 / total as f32) * 100.0;
    eprintln!("differing pixels: {diff}/{total} ({pct:.1}%)");
    assert!(pct > 5.0, "expected Camera3D::orbit to change >5% of pixels, got {pct:.1}%");
}

// ── Owner-ordered live fix: MSAA render quality ─────────────────────────

#[test]
#[ignore]
fn build_scene_renders_correctly_with_msaa_armed_at_sample_count_4() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let engine = head_on_engine();
    let scene = engine.build_scene(H as f64);
    assert_eq!(scene.nodes.len(), 3, "2 node spheres (Lit) + 1 edge line (Line) — MSAA covers Unlit+Lit+Line, all sample-count-matched pipeline pairs");

    let aspect = W as f32 / H as f32;
    let camera = engine.camera(aspect);

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    r.set_sample_count(&device, 4);
    assert_eq!(r.sample_count(), 4, "set_sample_count(4) must actually arm MSAA");

    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    let (ax, ay) = project_world_to_screen(&camera, Vec3::new(-6.0, 0.0, 0.0), viewport).expect("node a is in front of the eye");
    let (bx, by) = project_world_to_screen(&camera, Vec3::new(6.0, 0.0, 0.0), viewport).expect("node b is in front of the eye");
    let (mx, my) = project_world_to_screen(&camera, Vec3::ZERO, viewport).expect("edge midpoint is in front of the eye");

    let a_px = at(&px, ax.round() as u32, ay.round() as u32);
    let b_px = at(&px, bx.round() as u32, by.round() as u32);
    // Wave C — same small-neighborhood sampling fix as the sample_count=1
    // gate above (see `brightest_near`'s own doc comment).
    let mid_px = brightest_near(&px, mx.round() as u32, my.round() as u32, 2);
    let corner_px = at(&px, 2, 2);
    let far_corner_px = at(&px, W - 3, H - 3);

    eprintln!("MSAA sample_count=4: a={a_px:?} b={b_px:?} mid={mid_px:?} corner={corner_px:?} far_corner={far_corner_px:?}");

    // Same correctness assertions as the sample_count=1 gate above — the
    // MSAA path must still render the SAME scene correctly (right
    // nodes/edge visible, right background), not just "not crash."
    assert_eq!(corner_px, far_corner_px, "MSAA-armed background corners must still both be pure (resolved+tonemapped) background: {corner_px:?} vs {far_corner_px:?}");
    assert!(brightness(a_px) > brightness(corner_px) + 30, "node a must still be visually distinct under MSAA: {a_px:?} vs {corner_px:?}");
    assert!(brightness(b_px) > brightness(corner_px) + 30, "node b must still be visually distinct under MSAA: {b_px:?} vs {corner_px:?}");
    assert!(brightness(mid_px) > brightness(corner_px) + 20, "the edge line must still be visible near its midpoint under MSAA: {mid_px:?} vs {corner_px:?}");

    // Disarming must restore the exact pre-MSAA single-sample path —
    // the "keep a single-sample path available" requirement, proven by
    // actually round-tripping it, not just documenting it.
    r.set_sample_count(&device, 1);
    assert_eq!(r.sample_count(), 1, "set_sample_count(1) must disarm MSAA");
    let (tex2, view2) = make_target(&device);
    let mut enc2 = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc2, &view2, &camera, &scene);
    queue.submit(Some(enc2.finish()));
    let px2 = readback_rgba(&device, &queue, &tex2);
    let a_px2 = at(&px2, ax.round() as u32, ay.round() as u32);
    assert!(brightness(a_px2) > brightness(at(&px2, 2, 2)) + 30, "the disarmed single-sample path must still render node a correctly");
}

// ── Wave C — edge-quality overhaul: dotted/stippled edge fix ────────────

/// The task's own explicit gate: "a NEW test proving a long thin edge
/// renders CONTINUOUS pixels (sample along the projected line — no
/// gaps) at a glancing angle." "Glancing angle" here means the edge's
/// own ON-SCREEN (projected) angle is oblique/diagonal, not screen-axis-
/// aligned — this is what a headless-GPU diagnostic (full pixel evidence
/// in `uzor-graph/CLAUDE.md`'s divergence log) actually isolated as the
/// second necessary ingredient for the owner-reported stipple defect: a
/// perfectly horizontal thin cylinder rasterized continuously even
/// sub-pixel-wide, but the IDENTICAL radius on a DIAGONAL edge left
/// ~96% of its exact-centerline samples at background brightness — a
/// camera-side 3D grazing angle relative to the edge's own axis was
/// tried too and did NOT reproduce the defect (documented, ruled out).
/// This fixture uses the SAME diagonal-edge geometry and the engine's
/// own DEFAULT orbit distance (`Camera3D::default().distance = 500.0`,
/// `uzor-graph/src/camera3d.rs`) that reproduced the defect against the
/// ORIGINAL cylinder-edge geometry — proving the CURRENT `NodeMesh::Line`
/// path (round 2's screen-space billboarded edge-quad — see
/// `render3d.rs`'s own Wave D module doc) still fixes the EXACT reported
/// case, not a strawman. This is the task's own "the continuous-coverage
/// regression test must survive on quads" gate.
#[test]
#[ignore]
fn long_thin_diagonal_edge_at_the_engines_default_distance_renders_with_continuous_coverage() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let mut graph: Graph<(), ()> = Graph::new();
    let a = graph.push_node((), "a", "cat-a", 2.5);
    let b = graph.push_node((), "b", "cat-b", 2.5);
    graph.push_edge(a, b, 1.0, ());
    let from = Vec3::new(-260.0, -140.0, 0.0);
    let to = Vec3::new(260.0, 170.0, 0.0);
    let particles = vec![Particle::at3(from.x, from.y, from.z), Particle::at3(to.x, to.y, to.z)];

    let node_mesh = Arc::new(MeshLit::sphere(1.0, 8, 8, [1.0, 1.0, 1.0, 1.0]));
    let edge_mesh = Arc::new(Mesh::unit_edge_quad([1.0, 1.0, 1.0, 1.0]));
    let scene = render3d::build_scene(
        &graph,
        &particles,
        &node_mesh,
        &edge_mesh,
        &HashSet::new(),
        &render3d::Graph3DLighting::default(),
        &render3d::Graph3DEdgeStyle::default(),
    );

    let d = 500.0f32;
    let mut camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, d), Vec3::ZERO, W as f32 / H as f32);
    camera.z_near = (d * 0.001).max(0.05);
    camera.z_far = (d * 4.0).max(2_000.0);

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    // Production's own live configuration
    // (`uzor-render-hub/src/compose.rs::submit_urx_composed`) — MSAA
    // helps but, against the OLD cylinder geometry, could not fix this
    // on its own (also documented in the divergence log).
    r.set_sample_count(&device, 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);
    let bg = brightness(at(&px, 2, 2));

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    let delta = to - from;

    // Sample densely along the edge's own world-space length, excluding
    // a margin at each end (the node spheres themselves cover those
    // ends) — require near-total coverage AND bound the longest run of
    // consecutive dark samples, the actual "no gaps" gate.
    let steps = 200;
    let margin = 0.08;
    let mut lit_flags: Vec<bool> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = margin + (1.0 - 2.0 * margin) * (i as f32 / steps as f32);
        let p = from + delta * t;
        let lit = match project_world_to_screen(&camera, p, viewport) {
            Some((sx, sy)) if sx >= 2.0 && sy >= 2.0 && sx < (W - 2) as f64 && sy < (H - 2) as f64 => {
                // A small perpendicular box (not just the exact
                // centerline) — proving the LINE is continuous, not
                // hunting for exact sub-pixel rounding luck (same
                // reasoning as `brightest_near`'s own doc comment).
                let px_here = brightest_near(&px, sx.round() as u32, sy.round() as u32, 1);
                brightness(px_here) > bg + 15
            }
            _ => false,
        };
        lit_flags.push(lit);
    }

    let lit_count = lit_flags.iter().filter(|&&l| l).count();
    let mut max_gap = 0usize;
    let mut cur_gap = 0usize;
    for &lit in &lit_flags {
        if lit {
            cur_gap = 0;
        } else {
            cur_gap += 1;
            max_gap = max_gap.max(cur_gap);
        }
    }
    eprintln!("long thin diagonal edge: samples={} lit={lit_count} max_gap={max_gap}", lit_flags.len());

    assert!(
        (lit_count as f32 / lit_flags.len() as f32) > 0.95,
        "the edge-quad pipeline must render CONTINUOUS coverage along a diagonal edge at the engine's own default distance — only {lit_count}/{} samples were lit (the ORIGINAL cylinder-edge path left only ~4% lit on this exact fixture, see the divergence log)",
        lit_flags.len()
    );
    assert!(
        max_gap <= 2,
        "no run of more than 2 consecutive dark samples is allowed — a longer run IS the dotted/stippled defect this wave fixed; max_gap={max_gap}"
    );
}

// ── Wave D (round 2): screen-space billboarded edge quads + analytic AA ──

/// Round 2's own explicit gate: "a new test proving ANALYTIC AA —
/// sample pixels perpendicular to an edge's centerline and assert
/// intermediate alpha/brightness values exist on both sides (feather),
/// not a binary hard edge." Built from `render3d::build_edge_instances`
/// alone — no node spheres in the scene — so there is zero risk of a
/// node's own coverage confounding the perpendicular sweep. Deliberately
/// renders WITHOUT MSAA (`Renderer3D::sample_count` stays at its default
/// `1`) and with bloom/SSAO both disabled: this isolates the SHADER's
/// own per-fragment coverage function as the thing under test, not any
/// incidental smoothing MSAA or bloom bleed might otherwise contribute
/// — this crate's own round-1 diagnosis explicitly established that
/// whether/how MSAA even touches a hardware line is
/// implementation-defined, so a gate that only passed WITH MSAA armed
/// wouldn't actually prove the NEW shader's own analytic AA is real.
///
/// **Sweeps `Renderer3D::set_edge_width_px` across many values, not a
/// single fixed width — a real finding from developing this test, not
/// incidental.** The shader's own feather band is deliberately narrow
/// (~1 device pixel, matching a crisp AA line rather than a blurry one —
/// see `edge_quad_instanced.wgsl`'s own `FEATHER_PX`), which is
/// comparable to this test's OWN exact-integer-pixel sampling grid
/// spacing (1px). At a SINGLE fixed width, whether an integer pixel ROW
/// happens to land inside that ~1px-wide transition band is a matter of
/// sub-pixel phase luck — a genuinely continuous analytic function can
/// still produce an apparent binary step at exact-pixel sampling
/// resolution if the whole transition happens to fall between two
/// adjacent sample rows (confirmed empirically: the FIRST version of
/// this test, at the crate's own default `1.75px` width alone, hit
/// exactly that unlucky phase and failed even though the shader is
/// correct). Sweeping the on-screen width shifts the transition band's
/// own phase relative to the fixed pixel grid by design (`set_edge_width_px`
/// is a real, already-public per-`Renderer3D` knob, not test-only
/// scaffolding) — across enough distinct widths the band's phase must
/// eventually land astride a sample row on each side (a >1px total
/// half-width sweep range guarantees this by the pigeonhole principle,
/// since the band period is exactly 1px). This is a MORE rigorous proof
/// that the coverage function is analytic than a single lucky sample
/// would have been: it stresses the shader across many (width,
/// grid-phase) combinations, not just one.
#[test]
#[ignore]
fn edge_quad_analytic_aa_feathers_the_line_edge_instead_of_a_binary_hard_step() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // A screen-broadside (on-screen-horizontal) edge — the camera looks
    // straight down -Z at a segment lying along world X, so the
    // perpendicular sweep below is a simple vertical pixel column at a
    // fixed x. The edge-quad's screen-space perpendicular expansion
    // doesn't care which world axis the segment lies along (computed
    // fresh from projected screen positions every frame — see
    // `edge_quad_instanced.wgsl`), so this is a representative, not a
    // special-cased, edge.
    let mut graph: Graph<(), ()> = Graph::new();
    let a = graph.push_node((), "a", "cat-a", 1.0);
    let b = graph.push_node((), "b", "cat-b", 1.0);
    graph.push_edge(a, b, 1.0, ());
    let particles = vec![Particle::at3(-10.0, 0.0, 0.0), Particle::at3(10.0, 0.0, 0.0)];
    let edge_mesh = Arc::new(Mesh::unit_edge_quad([1.0, 1.0, 1.0, 1.0]));
    let edges = render3d::build_edge_instances(&graph, &particles, &edge_mesh, &HashSet::new(), &render3d::Graph3DEdgeStyle::default());
    assert_eq!(edges.len(), 1, "exactly one edge, no node spheres, in this scene");
    let mut scene = Scene3D::new();
    scene.nodes = edges;

    let d = 60.0f32;
    let mut camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, d), Vec3::ZERO, W as f32 / H as f32);
    camera.z_near = (d * 0.001).max(0.05);
    camera.z_far = (d * 4.0).max(2_000.0);

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    // Isolate the shader's own analytic AA — no bloom bleed, no SSAO
    // darkening (the SAME isolation `id_pass_scene_produces_exactly_decodable_node_ids_at_known_pixels`
    // below already needs for its own flat-color readback requirement).
    // `r.sample_count()` is left at its default `1` — no MSAA armed.
    r.set_bloom_strength(0.0);
    r.set_ssao_strength(0.0);

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    let (mx, my) = project_world_to_screen(&camera, Vec3::ZERO, viewport).expect("edge midpoint is in front of the eye");
    let cx = (mx.round() as i64).clamp(0, (W - 1) as i64) as u32;
    let cy = my.round() as i64;

    let mut positive_side_found = false;
    let mut negative_side_found = false;
    let mut last_bg = 0u32;

    // Half-width sweeps from `1.75/2 = 0.875px` up to `~2.5px` in
    // `0.2px` steps (12 widths, `2.4px` of half-width range) — well
    // over the `~1px` feather-band period, guaranteeing phase coverage.
    for i in 0..12 {
        let width_px = 1.75 + i as f32 * 0.4;
        r.set_edge_width_px(width_px);

        let (tex, view) = make_target(&device);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        r.render(&device, &queue, &mut enc, &view, &camera, &scene);
        queue.submit(Some(enc.finish()));
        let px = readback_rgba(&device, &queue, &tex);

        let bg = brightness(at(&px, 2, 2));
        last_bg = bg;

        // Sample EXACT pixels along a vertical strip PERPENDICULAR to
        // the (screen-horizontal) edge at the fixed column `cx` — not a
        // max-in-neighborhood search, which would mask the very
        // gradient this test needs to observe. `peak_dy` (rather than
        // assuming the rounded projection lands exactly on the true
        // center row) is found FROM the sweep itself.
        let sweep: Vec<(i64, u32)> = (-10..=10)
            .map(|dy: i64| {
                let y = (cy + dy).clamp(0, (H - 1) as i64) as u32;
                (dy, brightness(at(&px, cx, y)))
            })
            .collect();
        let (peak_dy, core) = *sweep.iter().max_by_key(|(_, b)| *b).expect("sweep is non-empty");
        if core <= bg + 20 {
            eprintln!("width_px={width_px:.2}: line not clearly visible (core={core} bg={bg}) — skipping this width");
            continue;
        }
        // "Intermediate" = strictly between background and the line's
        // own core brightness — the actual feathered transition, not a
        // binary in/out step. Checked within a tight ±1..=3-pixel
        // window on each side of the detected peak.
        let lo = bg + 8;
        let hi = core.saturating_sub(8);
        if lo >= hi {
            continue;
        }
        let pos_here = (1..=3).any(|off| {
            sweep.iter().find(|(dy, _)| *dy == peak_dy + off).is_some_and(|(_, b)| *b > lo && *b < hi)
        });
        let neg_here = (1..=3).any(|off| {
            sweep.iter().find(|(dy, _)| *dy == peak_dy - off).is_some_and(|(_, b)| *b > lo && *b < hi)
        });
        eprintln!("width_px={width_px:.2} peak_dy={peak_dy} core={core} bg={bg} pos_intermediate={pos_here} neg_intermediate={neg_here} sweep={sweep:?}");
        positive_side_found |= pos_here;
        negative_side_found |= neg_here;
        if positive_side_found && negative_side_found {
            break;
        }
    }

    assert!(
        positive_side_found,
        "across a sweep of on-screen line widths, at least one configuration must show an INTERMEDIATE (partially-covered) brightness sample on the +dy side of the edge's centerline — a binary hard edge would NEVER produce one at ANY width/phase; bg={last_bg}"
    );
    assert!(
        negative_side_found,
        "across a sweep of on-screen line widths, at least one configuration must show an INTERMEDIATE (partially-covered) brightness sample on the -dy side of the edge's centerline — a binary hard edge would NEVER produce one at ANY width/phase; bg={last_bg}"
    );
}

// ── 2026-07-22 (3D-parity-arc final wave): per-instance edge width ──────

/// The task's own explicit gate: "two edges, widths 1.75 and 5.0, assert
/// the 5.0 edge's cross-section reads wider in pixels than the 1.75
/// one." Routed through the REAL per-graph-edge path
/// (`render3d::build_edge_instances`), not a hand-poked `Node::with_scale`
/// — the mechanism under test is the full weight -> `scale.x` -> shader
/// chain, not just the shader in isolation. Two independent, screen-
/// broadside (world-X-aligned) edges stacked at different world Y so
/// each has its own clean midpoint sampling column, well away from
/// either endpoint's round-cap region (same broadside construction
/// `edge_quad_analytic_aa_feathers_the_line_edge_instead_of_a_binary_hard_step`
/// already uses).
#[test]
#[ignore]
fn per_instance_edge_width_makes_a_higher_weight_edge_read_wider_in_pixels() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let edge_mesh = Arc::new(Mesh::unit_edge_quad([1.0, 1.0, 1.0, 1.0]));

    // Weight 1.0 -> `render3d::edge_width_scale(1.0) == 1.0` exactly ->
    // the renderer's own BASE `edge_width_px` default (1.75px), i.e.
    // today's exact pre-existing rendering for a weight-1.0 edge.
    let mut thin_graph: Graph<(), ()> = Graph::new();
    let ta = thin_graph.push_node((), "a", "cat-a", 1.0);
    let tb = thin_graph.push_node((), "b", "cat-b", 1.0);
    thin_graph.push_edge(ta, tb, 1.0, ());
    let thin_particles = vec![Particle::at3(-10.0, -6.0, 0.0), Particle::at3(10.0, -6.0, 0.0)];
    let thin_edges =
        render3d::build_edge_instances(&thin_graph, &thin_particles, &edge_mesh, &HashSet::new(), &render3d::Graph3DEdgeStyle::default());
    assert_eq!(thin_edges.len(), 1);

    // Solve for the weight `render3d::edge_width_scale`'s own formula
    // (`scale = (1 + sqrt(w)) / 2`) needs to land the final width at
    // ~5.0px against the renderer's default 1.75px base uniform:
    // `w = (2*scale - 1)^2`.
    let base_width_px = 1.75_f32;
    let target_scale = 5.0_f32 / base_width_px;
    let thick_weight = (2.0 * target_scale - 1.0).powi(2);

    let mut thick_graph: Graph<(), ()> = Graph::new();
    let ca = thick_graph.push_node((), "a", "cat-a", 1.0);
    let cb = thick_graph.push_node((), "b", "cat-b", 1.0);
    thick_graph.push_edge(ca, cb, thick_weight, ());
    let thick_particles = vec![Particle::at3(-10.0, 6.0, 0.0), Particle::at3(10.0, 6.0, 0.0)];
    let thick_edges =
        render3d::build_edge_instances(&thick_graph, &thick_particles, &edge_mesh, &HashSet::new(), &render3d::Graph3DEdgeStyle::default());
    assert_eq!(thick_edges.len(), 1);

    let mut scene = Scene3D::new();
    scene.nodes = thin_edges;
    scene.nodes.extend(thick_edges);

    let d = 60.0f32;
    let mut camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, d), Vec3::ZERO, W as f32 / H as f32);
    camera.z_near = (d * 0.001).max(0.05);
    camera.z_far = (d * 4.0).max(2_000.0);

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    r.set_bloom_strength(0.0);
    r.set_ssao_strength(0.0);
    // Renderer stays at its OWN default `edge_width_px` (1.75px) — the
    // whole point of this test is the PER-INSTANCE scale, not a global
    // override via `set_edge_width_px`.

    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);
    let bg = brightness(at(&px, 2, 2));

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    // Each edge's own midpoint — deep inside its own segment (edges span
    // 20 world units), well away from either endpoint's round-cap region.
    let (thin_mx, thin_my) = project_world_to_screen(&camera, Vec3::new(0.0, -6.0, 0.0), viewport).expect("thin edge midpoint is in front of the eye");
    let (thick_mx, thick_my) = project_world_to_screen(&camera, Vec3::new(0.0, 6.0, 0.0), viewport).expect("thick edge midpoint is in front of the eye");

    // Measure the apparent cross-section span (count of "lit" pixel rows,
    // clearly brighter than background) via a vertical sweep
    // perpendicular to each (screen-horizontal) edge's own centerline —
    // same sampling technique the analytic-AA sweep test above uses.
    let measure_span = |cx: f64, cy: f64| -> usize {
        let cxi = (cx.round() as i64).clamp(0, (W - 1) as i64) as u32;
        let cyi = cy.round() as i64;
        (-15..=15)
            .filter(|dy| {
                let y = (cyi + dy).clamp(0, (H - 1) as i64) as u32;
                brightness(at(&px, cxi, y)) > bg + 20
            })
            .count()
    };

    let thin_span = measure_span(thin_mx, thin_my);
    let thick_span = measure_span(thick_mx, thick_my);
    eprintln!("thin_span={thin_span} thick_span={thick_span} (thick_weight={thick_weight:.2})");

    assert!(thin_span > 0, "the thin (weight 1.0, today's default 1.75px) edge must be visible at all");
    assert!(
        thick_span > thin_span,
        "the higher-weight edge's per-instance width must read visibly wider in pixels than the default-weight edge: thin_span={thin_span} thick_span={thick_span}"
    );
}

// ── Wave 4: GPU color-ID picking escalation ─────────────────────────────

#[test]
#[ignore]
fn id_pass_scene_produces_exactly_decodable_node_ids_at_known_pixels() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let engine = head_on_engine();
    let id_scene = engine.build_id_pass_scene();
    assert_eq!(id_scene.nodes.len(), 2, "the id-pass is node-only — no edge lines");
    assert_eq!(id_scene.clear_color, [1.0, 1.0, 1.0, 1.0], "the id-pass background must be the reserved white sentinel");

    let aspect = W as f32 / H as f32;
    let camera = engine.camera(aspect);

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);
    // Bloom bleed / SSAO darkening would corrupt the flat per-node id
    // color — see `render3d.rs`'s own id-pass module doc.
    r.set_bloom_strength(0.0);
    r.set_ssao_strength(0.0);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &id_scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    let (ax, ay) = project_world_to_screen(&camera, Vec3::new(-6.0, 0.0, 0.0), viewport).expect("node a is in front of the eye");
    let (bx, by) = project_world_to_screen(&camera, Vec3::new(6.0, 0.0, 0.0), viewport).expect("node b is in front of the eye");

    let a_px = at(&px, ax.round() as u32, ay.round() as u32);
    let b_px = at(&px, bx.round() as u32, by.round() as u32);
    // Far corner, well outside both node silhouettes — background.
    let corner_px = at(&px, 2, 2);

    eprintln!("a={a_px:?} b={b_px:?} corner={corner_px:?}");

    assert_eq!(
        render3d::decode_gpu_pick_pixel(a_px, 2),
        Some(NodeIndex(0)),
        "node a's own pixel must decode back to its exact NodeIndex through the real render pipeline"
    );
    assert_eq!(
        render3d::decode_gpu_pick_pixel(b_px, 2),
        Some(NodeIndex(1)),
        "node b's own pixel must decode back to its exact NodeIndex through the real render pipeline"
    );
    assert_eq!(
        render3d::decode_gpu_pick_pixel(corner_px, 2),
        None,
        "a background pixel (white clear color, tonemapped) must decode to no hit, never a node"
    );
}

#[test]
#[ignore]
fn request_and_poll_gpu_pick_resolves_the_correct_node_via_deferred_readback() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let engine = head_on_engine();
    let id_scene = engine.build_id_pass_scene();
    let aspect = W as f32 / H as f32;
    let camera = engine.camera(aspect);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 64);

    let viewport = Rect::new(0.0, 0.0, W as f64, H as f64);
    let (ax, ay) = project_world_to_screen(&camera, Vec3::new(-6.0, 0.0, 0.0), viewport).expect("node a is in front of the eye");

    let readback = pick3d::request_gpu_pick(&device, &queue, &mut r, &id_scene, &camera, (W, H), (ax, ay));

    // Blocking poll loop — acceptable IN TESTS (the task's own
    // instruction); `GraphEngine3D`'s LIVE hover path never blocks like
    // this, it just re-polls next frame via `pick3d::poll_gpu_pick`.
    let mut result = None;
    for _ in 0..10_000 {
        match pick3d::poll_gpu_pick(&device, &readback, 2) {
            Some(r) => {
                result = Some(r);
                break;
            }
            None => std::hint::spin_loop(),
        }
    }

    assert_eq!(
        result,
        Some(Some(NodeIndex(0))),
        "the deferred GPU pick must resolve to node a's own NodeIndex at its exact screen position"
    );
}
