//! Wave 20 — SSAO crease-darkening test.
//!
//! Two adjacent cubes touching at x=0. With SSAO enabled the pixels
//! in the seam between them should be DIMMER than the same pixels
//! with SSAO disabled.

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Vec3,
};

const W: u32 = 256;
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
        label: Some("urx3d-ssao-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-ssao-target"),
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
        label: Some("ssao-readback"),
        size: (aligned * H) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(aligned), rows_per_image: Some(H) },
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
#[inline]
fn luma(p: [u8; 4]) -> i32 { p[0] as i32 + p[1] as i32 + p[2] as i32 }

fn render_scene(device: &wgpu::Device, queue: &wgpu::Queue, ssao: f32) -> Vec<u8> {
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.6, 0.6, 0.6];

    // Two white cubes touching at x=0 to form a deep seam in the middle.
    let mut white = MeshLit::cube_lit();
    for v in &mut white.vertices { v.color = [1.0, 1.0, 1.0, 1.0]; }
    let cube = Arc::new(white);
    scene.push(
        Node::new_lit(cube.clone())
            .with_translation(Vec3::new(-1.0, 0.0, 0.0))
            .with_scale(Vec3::splat(0.95)),
    );
    scene.push(
        Node::new_lit(cube)
            .with_translation(Vec3::new(1.0, 0.0, 0.0))
            .with_scale(Vec3::splat(0.95)),
    );
    scene.push_light(Light::directional(Vec3::new(0.0, -0.3, -1.0), [1.0; 3], 0.5));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(device, queue, COLOR_FORMAT, (W, H), 4);
    r.set_bloom_strength(0.0);
    r.set_ssao_strength(ssao);
    r.set_ssao_radius_px(20.0);
    r.set_ssao_max_delta(0.6);
    let (tex, view) = make_target(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(device, queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    readback_rgba(device, queue, &tex)
}

#[test]
#[ignore]
fn ssao_darkens_crease_between_cubes() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter"); return;
    };
    let off = render_scene(&device, &queue, 0.0);
    let on  = render_scene(&device, &queue, 1.0);

    // Sample a vertical strip at x = W/2 (the seam between the two
    // cubes). Average luma over the strip — SSAO should drop it.
    let cx = W / 2;
    let y0 = H / 3;
    let y1 = (H * 2) / 3;
    let mut sum_off = 0i64;
    let mut sum_on  = 0i64;
    let mut n = 0i64;
    for y in y0..y1 {
        for dx in -2i32..=2i32 {
            let xi = (cx as i32 + dx).clamp(0, (W - 1) as i32) as u32;
            sum_off += luma(at(&off, xi, y)) as i64;
            sum_on  += luma(at(&on,  xi, y)) as i64;
            n += 1;
        }
    }
    let mean_off = sum_off / n;
    let mean_on  = sum_on  / n;
    eprintln!("seam luma off={} on={}  delta={}", mean_off, mean_on, mean_off - mean_on);

    // SSAO should darken the seam by at least 8 luma — proves the
    // depth-derivative crease detection is firing AND the composite
    // multiplier is wired through.
    assert!(
        mean_on < mean_off - 8,
        "SSAO should darken the seam: off={} on={}",
        mean_off, mean_on
    );
}
