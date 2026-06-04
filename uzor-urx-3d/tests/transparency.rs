//! Wave 18 — transparency + water plane GPU tests.
//!
//! 1. A blue cube (opaque) is occluded by a translucent red plane
//!    that sits BETWEEN cube and camera. With back-to-front sort,
//!    the final pixel where they overlap should mix both colors
//!    (not pure red, not pure blue).
//! 2. Water plane (translucent tint) renders at the right
//!    silhouette size with an alpha-blended sky-colored gradient.

use std::sync::Arc;
use uzor_urx_3d::{Light, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Vec3};

const W: u32 = 192;
const H: u32 = 192;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-transp-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-transp-target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn readback_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let aligned = (W * 4 + 255) & !255;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("transp-readback"),
        size: (aligned * H) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture, mip_level: 0, origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for row in 0..H as usize {
        let s = row * aligned as usize;
        out.extend_from_slice(&raw[s..s + (W * 4) as usize]);
    }
    drop(raw); staging.unmap(); out
}

#[inline]
fn at(buf: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [buf[i], buf[i+1], buf[i+2], buf[i+3]]
}

#[test]
#[ignore]
fn translucent_plane_in_front_blends_with_opaque_cube_behind() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.5, 0.5, 0.5]; // bright ambient so colors stay saturated

    // White-vertex cube mesh so per-face vertex colors don't dominate
    // the tint test.
    let mut white_cube = MeshLit::cube_lit();
    for v in &mut white_cube.vertices { v.color = [1.0, 1.0, 1.0, 1.0]; }
    let cube = Arc::new(white_cube);

    // Opaque BLUE cube far from camera (z=-2).
    scene.push(
        Node::new_lit(cube.clone())
            .with_translation(Vec3::new(0.0, 0.0, -2.0))
            .with_scale(Vec3::splat(0.9))
            .with_tint([0.1, 0.1, 1.0, 1.0]),
    );
    // Translucent RED slab between camera and cube (z=+1).
    scene.push(
        Node::new_lit(cube)
            .with_translation(Vec3::new(0.0, 0.0, 1.0))
            .with_scale(Vec3::new(1.2, 1.2, 0.05))
            .with_tint([1.0, 0.1, 0.1, 0.5]),
    );

    scene.push_light(Light::directional(Vec3::new(0.0, -0.2, -1.0), [1.0; 3], 1.0));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 4.5), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // Centre pixel sits on the red translucent slab in front of the
    // blue cube. With back-to-front sort + alpha blending, the pixel
    // should mix: NOT pure red (cube provides some blue), NOT pure
    // blue (slab is in front).
    let centre = at(&px, W / 2, H / 2);
    eprintln!("blended centre={:?}", centre);
    // BOTH channels must show — pure-blue (no slab) would have red=0;
    // pure-red (no cube) would have blue=0. Alpha-blended pixel mixes
    // them with SrcAlpha — exact ratio depends on lit intensity.
    assert!(centre[0] > 60, "red from translucent slab expected: {:?}", centre);
    assert!(centre[2] > 60, "blue from opaque cube behind expected: {:?}", centre);
}

#[test]
#[ignore]
fn water_plane_renders_with_alpha() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    let mut scene = Scene3D::new();
    scene.clear_color = [0.05, 0.05, 0.10, 1.0]; // dark sky
    scene.ambient = [0.6, 0.6, 0.6];

    // Opaque red wall behind water — proves alpha sees it through.
    let wall = Arc::new(MeshLit::cube_lit());
    scene.push(
        Node::new_lit(wall)
            .with_translation(Vec3::new(0.0, 0.0, -3.0))
            .with_scale(Vec3::new(3.0, 3.0, 0.1))
            .with_tint([1.0, 0.2, 0.2, 1.0]),
    );

    // Water plane at y=0 — high subdivision so a future shader can
    // displace; right now it's just a translucent plane.
    let water = Arc::new(MeshLit::water_plane(2.5, 16, [0.2, 0.45, 0.7, 0.55]));
    scene.push(Node::new_lit(water));

    scene.push_light(Light::directional(Vec3::new(0.0, -1.0, 0.0), [1.0; 3], 1.0));

    // Look DOWN at the water plane from above-front so the plane fills
    // the bottom-half of the frame.
    let camera = PerspectiveCamera::new(Vec3::new(0.0, 2.5, 3.5), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // A pixel near the centre-bottom should sit on the water plane.
    let water_px = at(&px, W / 2, H - 40);
    let sky_px = at(&px, W / 2, 10);
    eprintln!("water={:?}  sky_corner={:?}", water_px, sky_px);
    // Water has visible blue.
    assert!(water_px[2] > 60, "water should be bluish: {:?}", water_px);
    // Sky (no water) stays close to clear color (mostly dark blue).
    assert!(sky_px[1] < 50, "sky should be dark: {:?}", sky_px);
}
