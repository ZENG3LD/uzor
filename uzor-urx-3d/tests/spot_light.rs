//! Wave 4b spot-light GPU test.
//!
//! A single white spot light points straight DOWN at the center of a
//! large plane. Expectation:
//!   - centre of the plane (under the cone) is BRIGHT
//!   - corner of the plane (outside the cone) stays DARK
//! Proves both cone restriction (corner stays dark even though the
//! light is at finite range) and falloff (corner luma < centre luma).

use std::sync::Arc;
use uzor_urx_3d::{Light, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Vec3};

const W: u32 = 256;
const H: u32 = 256;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-spot-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-spot-target"),
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
        label: Some("spot-readback"),
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

#[inline]
fn luma(p: [u8; 4]) -> i32 { p[0] as i32 + p[1] as i32 + p[2] as i32 }

#[test]
#[ignore]
fn spot_light_cone_is_bright_at_center_dark_at_corner() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0]; // pure direct-light only

    // Big white plane at y=0 facing up.
    let plane = Arc::new(MeshLit::plane_lit(6.0, [1.0, 1.0, 1.0, 1.0]));
    scene.push(Node::new_lit(plane));

    // Spot light positioned 4 units above origin, pointing straight down,
    // inner cone 8°, outer 18°. Range = 10 so the floor is in range.
    // Cone radius at the floor (4 units away) ≈ 4*tan(18°) ≈ 1.3 — well
    // inside the 6-unit plane, so corners get NO light.
    scene.push_light(Light::spot(
        Vec3::new(0.0, 4.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        [1.0, 1.0, 1.0],
        4.0,
        10.0,
        8.0_f32.to_radians(),
        18.0_f32.to_radians(),
    ));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 7.0, 0.001), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // Sample 9×9 grid across the image and dump for debugging.
    eprintln!("9x9 luma grid (centre is the spot):");
    for gy in 0..9 {
        let y = (H as i32 * gy as i32 / 9) as u32;
        let mut line = String::new();
        for gx in 0..9 {
            let x = (W as i32 * gx as i32 / 9) as u32;
            line.push_str(&format!("{:>4} ", luma(at(&px, x, y))));
        }
        eprintln!("{}", line);
    }

    let centre = at(&px, W / 2, H / 2);
    let corner = at(&px, 4, 4);
    eprintln!("centre={:?}  corner={:?}", centre, corner);

    // Centre lit by spot should be bright.
    assert!(
        luma(centre) > 250,
        "centre should be bright under the spot: {:?}",
        centre
    );
    // Corner outside the cone should be near-black (only ambient = 0).
    assert!(
        luma(corner) < 40,
        "corner should stay dark outside the cone: {:?}",
        corner
    );
}

#[test]
#[ignore]
fn spot_light_smoothstep_grades_across_cone_edge() {
    // Same scene but verify there is a GRADIENT between the bright
    // inner cone and the dark outside — not a sharp on/off.

    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0];

    let plane = Arc::new(MeshLit::plane_lit(6.0, [1.0, 1.0, 1.0, 1.0]));
    scene.push(Node::new_lit(plane));

    scene.push_light(Light::spot(
        Vec3::new(0.0, 4.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        [1.0, 1.0, 1.0],
        4.0,
        10.0,
        5.0_f32.to_radians(),   // narrow inner
        25.0_f32.to_radians(),  // wide outer — big falloff band
    ));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 7.0, 0.001), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // Bin pixels by distance-from-centre (in pixels) and average luma.
    // Need bins to be monotonically non-increasing — proves the cone
    // falloff exists.
    let cx = (W / 2) as i32;
    let cy = (H / 2) as i32;
    let mut bins = vec![(0i64, 0i64); 8];
    let bin_w = (W as i32) / 16; // step ~16px
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let d2 = (x - cx).pow(2) + (y - cy).pow(2);
            let d = (d2 as f64).sqrt() as i32;
            let b = (d / bin_w) as usize;
            if b < bins.len() {
                bins[b].0 += luma(at(&px, x as u32, y as u32)) as i64;
                bins[b].1 += 1;
            }
        }
    }
    let means: Vec<i64> = bins.iter().map(|(s, n)| if *n > 0 { s / n } else { 0 }).collect();
    eprintln!("radial luma means: {:?}", means);

    // Mean luma should be non-increasing across the first 5 bins
    // (centre→edge), allowing 10 luma noise tolerance.
    for w in means.windows(2).take(5) {
        assert!(
            w[1] <= w[0] + 10,
            "spot luma should fall off monotonically: bin {} > bin {}",
            w[1], w[0]
        );
    }
    // Outermost bin must be much dimmer than innermost.
    assert!(
        means[0] > means[means.len() - 1] + 80,
        "spot cone should have a strong centre-to-edge contrast: {:?}",
        means
    );
}
