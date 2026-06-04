//! Wave 12 — HDR + bloom test.
//!
//! Verify that:
//! - A bright light's halo bleeds beyond the geometry boundary
//!   (bloom_strength > 0 → pixels OFF the geometry are brighter).
//! - bloom_strength = 0 → identical result to a tight silhouette
//!   (i.e. those off-geometry pixels are darker).
//!
//! The cube is a small white cube emitting via high light intensity
//! (5×) so its tonemapped pixel saturates and feeds the bright-pass.

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
        label: Some("urx3d-hdr-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-hdr-target"),
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
        label: Some("hdr-readback"),
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
                offset: 0, bytes_per_row: Some(aligned), rows_per_image: Some(H),
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
#[inline]
fn luma(p: [u8; 4]) -> i32 { p[0] as i32 + p[1] as i32 + p[2] as i32 }

fn render_with(strength: f32) -> Option<Vec<u8>> {
    let (device, queue) = init_device()?;
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0];

    // White cube, blasted by a 5× intensity light from camera direction
    // so the front face goes well past 1.0 in HDR.
    let mut white = MeshLit::cube_lit();
    for v in &mut white.vertices { v.color = [1.0, 1.0, 1.0, 1.0]; }
    let cube = Arc::new(white);
    scene.push(Node::new_lit(cube).with_scale(Vec3::splat(0.6)));

    scene.push_light(Light::directional(Vec3::new(0.0, 0.0, -1.0), [1.0; 3], 8.0));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    r.set_bloom_strength(strength);
    r.set_bloom_threshold(0.8);

    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    Some(readback_rgba(&device, &queue, &tex))
}

#[test]
#[ignore]
fn bloom_lifts_offgeo_pixels_brighter_than_no_bloom() {
    let Some(no_bloom) = render_with(0.0) else { return; };
    let Some(with_bloom) = render_with(2.0) else { return; };

    // Sample pixels in a horizontal strip just OUTSIDE the cube.
    // Camera is square-on, cube fills roughly the central 1/3 of the
    // frame. We measure x = 0.85 * W on the centre row.
    let y = H / 2;
    let off_x = (W as f32 * 0.85) as u32;
    let p_no = at(&no_bloom, off_x, y);
    let p_yes = at(&with_bloom, off_x, y);
    eprintln!("off-geo at x={}: no_bloom={:?}  with_bloom={:?}", off_x, p_no, p_yes);
    assert!(
        luma(p_yes) > luma(p_no) + 5,
        "bloom should brighten off-geometry pixels: no={:?} yes={:?}",
        p_no, p_yes
    );

    // Centre of cube must still saturate (white) in both versions —
    // tonemap + gamma should pin near 255.
    let c_no = at(&no_bloom, W / 2, H / 2);
    let c_yes = at(&with_bloom, W / 2, H / 2);
    assert!(luma(c_no) > 600, "cube centre too dim without bloom: {:?}", c_no);
    assert!(luma(c_yes) > 600, "cube centre too dim with bloom: {:?}", c_yes);
}
