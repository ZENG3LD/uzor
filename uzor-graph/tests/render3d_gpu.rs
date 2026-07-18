//! Headless GPU correctness tests for `uzor-graph`'s 3D render path
//! (W3D arc plan §4 Wave 2 gate). Own `wgpu::Instance`/`Device` — mirrors
//! `uzor-urx-3d/tests/cube_render.rs`'s pattern exactly
//! (`init_device`/`make_target`/`readback_rgba`).
//!
//! 1. `build_scene_renders_visually_distinct_node_and_edge_pixels` —
//!    `GraphEngine3D::build_scene()` for a small deterministic 2-node
//!    graph produces node-sphere and edge-cylinder pixels that are
//!    clearly brighter than the background clear color.
//! 2. `camera_orbit_changes_pixels` — orbiting `Camera3D` changes the
//!    rendered frame substantially (mirrors `cube_render.rs`'s own
//!    `camera_move_changes_pixels`).
//!
//! Run:
//!   cargo test -p uzor-graph --test render3d_gpu -- --include-ignored --nocapture

use uzor::types::Rect;
use uzor_graph::camera3d::Camera3D;
use uzor_graph::engine3d::GraphEngine3D;
use uzor_graph::graph::Graph;
use uzor_graph::interaction::pick3d::project_world_to_screen;
use uzor_graph::layout::force_directed_3d::ForceDirectedLayout3D;
use uzor_graph::particle::Particle;
use uzor_urx_3d::{Renderer3D, Vec3};

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
    engine.camera = Camera3D { target: Vec3::ZERO, distance: 20.0, yaw: 0.0, pitch: 0.0 };
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
    let scene = engine.build_scene();
    assert_eq!(scene.nodes.len(), 3, "2 node spheres + 1 edge cylinder");

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
    let mid_px = at(&px, mx.round() as u32, my.round() as u32);
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
        "the edge cylinder should be visible at its midpoint: {mid_px:?} vs {corner_px:?}"
    );
    assert!(
        a_px != mid_px || b_px != mid_px,
        "node spheres and the edge cylinder should not be pixel-identical (both are drawn, not just one covering the other): a={a_px:?} b={b_px:?} mid={mid_px:?}"
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
    let scene = engine.build_scene();
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
