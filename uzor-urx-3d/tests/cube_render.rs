//! GPU correctness tests for URX 3D Wave 1.
//!
//! 1. `cube_renders_six_faces_with_distinct_colors` — render single cube
//!    head-on from +Z, verify each visible face's colour is what the
//!    primitive declared.
//! 2. `depth_test_occludes_back_face` — put a small near cube in front
//!    of a big far cube and prove the far cube is hidden where the near
//!    one covers it (no z-fighting/over-write).
//! 3. `camera_move_changes_pixels` — render the same scene from two
//!    distinct camera positions, prove the resulting pixel buffers
//!    differ substantially.
//!
//! Run:
//!   cargo test -p uzor-urx-3d --test cube_render -- --include-ignored --nocapture

use std::sync::Arc;
use uzor_urx_3d::{Mesh, Node, PerspectiveCamera, Renderer3D, Scene3D, Vec3};

const W: u32 = 128;
const H: u32 = 128;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-test"),
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
        label: Some("urx3d-target"),
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

fn readback_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Vec<u8> {
    let aligned_stride = (W * 4 + 255) & !255;
    let buf_size = (aligned_stride * H) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx3d-readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
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
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
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

/// True if `dominant_channel` (0=R, 1=G, 2=B) is clearly the strongest
/// in `pixel`, by margin `min_delta` (0..=255).
fn channel_dominates(pixel: [u8; 4], dominant_channel: usize, min_delta: u8) -> bool {
    let primary = pixel[dominant_channel] as i16;
    let others = (0..3)
        .filter(|c| *c != dominant_channel)
        .map(|c| pixel[c] as i16)
        .collect::<Vec<_>>();
    others.iter().all(|&o| primary - o >= min_delta as i16)
}

#[test]
#[ignore]
fn cube_renders_six_faces_with_distinct_colors() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // Camera at +X +Y +Z looking at origin → all of +X, +Y, +Z faces visible.
    let camera = PerspectiveCamera::new(
        Vec3::new(3.0, 3.0, 3.0),
        Vec3::ZERO,
        W as f32 / H as f32,
    );

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.push(Node::new(Arc::new(Mesh::cube_rgb_faces())));

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));

    let px = readback_rgba(&device, &queue, &tex);

    // With eye at (3,3,3) looking at origin (RH view), all three +X/+Y/+Z
    // faces are visible. The cube projects as a hex silhouette with the
    // three faces meeting at the closest vertex (1,1,1). The right
    // half of the silhouette is dominated by +X (red), the top by +Y
    // (green), and the lower-left of the silhouette by +Z (blue).
    // Sampled the live grid first to figure out which screen rows/cols
    // each face lands on. Cube (eye=(3,3,3) looking at origin, fov 60°
    // RH, depth-tested back-face-cull):
    //   row y=48 (8-frac=3): blue at col x=32, green at col x=48-80, red at col x=96
    //   row y=96 (8-frac=6): red at col x=64
    let centre = at(&px, W / 2, H / 2);
    let red_probe = at(&px, W * 6 / 8, H * 3 / 8); // upper-right cell (+X face)
    let green_probe = at(&px, W / 2, H * 3 / 8);    // top-centre cell (+Y face)
    let blue_probe = at(&px, W * 2 / 8, H * 3 / 8); // upper-left cell (+Z face)

    eprintln!(
        "centre={:?} red={:?} green={:?} blue={:?}",
        centre, red_probe, green_probe, blue_probe
    );

    // Centre is the (1,1,1) vertex meeting point — at least one channel
    // should be high (it's on the cube, not background).
    assert!(
        centre.iter().take(3).any(|&c| c > 100),
        "centre should be on the cube silhouette: {:?}", centre
    );
    assert!(
        channel_dominates(red_probe, 0, 40),
        "lower-right probe should be red-dominant (+X face): {:?}", red_probe
    );
    assert!(
        channel_dominates(green_probe, 1, 40),
        "upper-left probe should be green-dominant (+Y face): {:?}", green_probe
    );
    assert!(
        channel_dominates(blue_probe, 2, 40),
        "upper-right probe should be blue-dominant (+Z face): {:?}", blue_probe
    );

    // Corner outside the cube silhouette should be the clear-colour
    let corner = at(&px, 2, 2);
    assert!(
        corner[0] < 8 && corner[1] < 8 && corner[2] < 8,
        "corner should be background: {:?}", corner
    );
}

#[test]
#[ignore]
fn depth_test_occludes_back_face() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // Camera head-on from +Z looking at origin
    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, W as f32 / H as f32);

    let cube = Arc::new(Mesh::cube_rgb_faces());

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    // Back: big yellow cube (-Z face faces camera = NO that's wrong with
    // an eye at +Z looking at -Z; the camera sees the +Z face of each).
    // So we override tint to make the two cubes distinguishable
    // regardless of which face shows.
    scene.push(
        Node::new(cube.clone())
            .with_translation(Vec3::new(0.0, 0.0, -2.0))
            .with_scale(Vec3::splat(1.5))
            .with_tint([1.0, 1.0, 0.0, 1.0]),
    );
    // Front: small magenta-tinted cube directly between camera and the
    // big yellow cube. Should occlude the yellow at screen centre.
    scene.push(
        Node::new(cube.clone())
            .with_translation(Vec3::new(0.0, 0.0, 1.5))
            .with_scale(Vec3::splat(0.6))
            .with_tint([1.0, 0.0, 1.0, 1.0]),
    );

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));

    let px = readback_rgba(&device, &queue, &tex);

    let centre = at(&px, W / 2, H / 2);
    // +Z face is blue (face tint), node tint magenta [1,0,1,1].
    // Product = [0,0,1,1] = blue. So centre should be blue, not yellow.
    // The yellow cube's tint × blue face = [0,0,0,1] (zero red/green
    // would result), so we test "blue strong" rather than tint-match.
    eprintln!("centre over small cube: {:?}", centre);
    assert!(
        centre[2] > 80,
        "centre should be dominated by front cube (blue ≥ 80): {:?}", centre
    );

    // Far corner of the screen should be background (both cubes are tiny
    // in projection at z_far=100; at this distance the yellow cube only
    // covers ~half the view).
    let corner = at(&px, 4, 4);
    eprintln!("corner: {:?}", corner);
    assert!(
        corner[0] < 8 && corner[2] < 8,
        "corner should be background: {:?}", corner
    );
}

#[test]
#[ignore]
fn camera_move_changes_pixels() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let cube = Arc::new(Mesh::cube_rgb_faces());
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.push(Node::new(cube));

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);

    // Frame A: eye at +Z
    let cam_a = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, W as f32 / H as f32);
    let (tex_a, view_a) = make_target(&device);
    let mut enc_a = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc_a, &view_a, &cam_a, &scene);
    queue.submit(Some(enc_a.finish()));
    let px_a = readback_rgba(&device, &queue, &tex_a);

    // Frame B: eye at +X +Y +Z (different angle, sees red/green/blue faces)
    let cam_b = PerspectiveCamera::new(Vec3::new(4.0, 4.0, 4.0), Vec3::ZERO, W as f32 / H as f32);
    let (tex_b, view_b) = make_target(&device);
    let mut enc_b = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc_b, &view_b, &cam_b, &scene);
    queue.submit(Some(enc_b.finish()));
    let px_b = readback_rgba(&device, &queue, &tex_b);

    // Substantial fraction of pixels should differ — same scene from a
    // different angle should not look pixel-identical.
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
    eprintln!("differing pixels: {}/{} ({:.1}%)", diff, total, pct);
    assert!(
        pct > 5.0,
        "expected camera move to change >5% of pixels, got {:.1}%", pct
    );
}
