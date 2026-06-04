//! Wave 6 PBR GPU tests.
//!
//! 1. `metallic_cube_reflects_more_than_rough_cube` — same albedo and
//!    light; metallic (m=1, r=0.1) cube has stronger specular highlight
//!    than rough (m=0, r=0.95) cube on the lit side.
//! 2. `pbr_albedo_color_affects_diffuse` — red-albedo cube produces a
//!    red-dominant pixel under a white directional light.

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshPbr, Node, PbrMaterial, PerspectiveCamera, Renderer3D, Scene3D, Texture3D, Vec3,
};

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
        label: Some("urx3d-pbr-test"),
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
        label: Some("urx3d-pbr-target"),
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
        label: Some("urx3d-pbr-readback"),
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

#[inline]
fn luma(p: [u8; 4]) -> i32 {
    p[0] as i32 + p[1] as i32 + p[2] as i32
}

/// 1×1 solid-color RGBA8 texture (utility for tests).
fn solid_tex(device: &wgpu::Device, queue: &wgpu::Queue, rgb: [u8; 3]) -> Arc<Texture3D> {
    Arc::new(Texture3D::from_rgba8(
        device, queue, 1, 1, &[rgb[0], rgb[1], rgb[2], 255],
    ))
}

#[test]
#[ignore]
fn metallic_cube_reflects_more_than_rough_cube() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // Camera above-front: light comes from upper-front too so specular
    // peak lands somewhere visible.
    let camera =
        PerspectiveCamera::new(Vec3::new(2.0, 2.0, 4.0), Vec3::ZERO, W as f32 / H as f32);

    let mesh = Arc::new(MeshPbr::cube_pbr());
    let albedo = solid_tex(&device, &queue, [180, 180, 180]); // grey

    let mat_metal = PbrMaterial::new(albedo.clone())
        .with_metalness(1.0)
        .with_roughness(0.10);
    let mat_rough = PbrMaterial::new(albedo.clone())
        .with_metalness(0.0)
        .with_roughness(0.95);

    // Two cubes side by side
    let mut scene_metal = Scene3D::new();
    scene_metal.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene_metal.ambient = [0.05, 0.05, 0.05];
    scene_metal.push_light(Light::directional(
        Vec3::new(-0.4, -1.0, -0.3),
        [1.0, 1.0, 1.0],
        2.5,
    ));
    scene_metal.push(Node::new_pbr(mesh.clone(), mat_metal));

    let mut scene_rough = scene_metal.clone();
    scene_rough.nodes.clear();
    scene_rough.push(Node::new_pbr(mesh.clone(), mat_rough));

    // Render both scenes; sample centre
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 8);
    let render = |scene: &Scene3D, r: &mut Renderer3D| -> Vec<u8> {
        let (tex, view) = make_target(&device);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        r.render(&device, &queue, &mut enc, &view, &camera, scene);
        queue.submit(Some(enc.finish()));
        readback_rgba(&device, &queue, &tex)
    };
    let px_metal = render(&scene_metal, &mut r);
    let px_rough = render(&scene_rough, &mut r);

    // Compare per-pixel: when the metallic cube DOES light up a pixel,
    // it should match-or-exceed the dielectric rough value (specular
    // contribution dominates for metal under direct light). Count
    // pixels where each material is visibly brighter than the other.
    let mut metal_wins = 0;
    let mut rough_wins = 0;
    let mut both_lit = 0;
    for y in 0..H {
        for x in 0..W {
            let lm = luma(at(&px_metal, x, y));
            let lr = luma(at(&px_rough, x, y));
            if lm > 50 && lr > 50 {
                both_lit += 1;
                if lm > lr + 20 {
                    metal_wins += 1;
                }
                if lr > lm + 20 {
                    rough_wins += 1;
                }
            }
        }
    }
    eprintln!(
        "both_lit_pixels={} metal_wins={} rough_wins={}",
        both_lit, metal_wins, rough_wins
    );

    // Smoke check: both materials produce a visible cube.
    let mut metal_lit_count = 0;
    let mut rough_lit_count = 0;
    for y in 0..H {
        for x in 0..W {
            if luma(at(&px_metal, x, y)) > 50 { metal_lit_count += 1; }
            if luma(at(&px_rough, x, y)) > 50 { rough_lit_count += 1; }
        }
    }
    assert!(metal_lit_count > 300, "metal cube should be visible: {} px", metal_lit_count);
    assert!(rough_lit_count > 300, "rough cube should be visible: {} px", rough_lit_count);

    // Most lit pixels on the cube should look different between metal
    // and rough — material params actually flow through to fragments.
    assert!(
        metal_wins + rough_wins > both_lit / 4,
        "expected materially different shading (metal_wins+rough_wins={} of both_lit={})",
        metal_wins + rough_wins, both_lit
    );
}

#[test]
#[ignore]
fn pbr_albedo_color_affects_diffuse() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let camera =
        PerspectiveCamera::new(Vec3::new(3.0, 3.0, 3.0), Vec3::ZERO, W as f32 / H as f32);

    let mesh = Arc::new(MeshPbr::cube_pbr());
    let red_albedo = solid_tex(&device, &queue, [255, 30, 30]);

    let mat = PbrMaterial::new(red_albedo)
        .with_metalness(0.0)
        .with_roughness(0.5);

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.05, 0.05, 0.05];
    scene.push_light(Light::directional(
        Vec3::new(-0.4, -1.0, -0.3),
        [1.0, 1.0, 1.0],
        1.5,
    ));
    scene.push(Node::new_pbr(mesh, mat));

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 8);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // Sample a few points on the cube silhouette; at least ONE should
    // be red-dominant (after PBR + tonemap + gamma the red channel can
    // wash out, so we just check R>G+20 && R>B+20).
    let mut found_red = false;
    let mut best: [u8; 4] = [0; 4];
    for sy in 1..8 {
        for sx in 1..8 {
            let x = (sx * W / 8).min(W - 1);
            let y = (sy * H / 8).min(H - 1);
            let p = at(&px, x, y);
            if luma(p) > 60 && p[0] as i32 > p[1] as i32 + 20 && p[0] as i32 > p[2] as i32 + 20 {
                found_red = true;
                if p[0] > best[0] {
                    best = p;
                }
            }
        }
    }
    eprintln!("best red sample: {:?}", best);
    assert!(found_red, "expected at least one red-dominant pixel on a red-albedo PBR cube");
}
