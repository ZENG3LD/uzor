//! Wave 14 — textured node casts a shadow on a lit plane.
//!
//! A textured cube hovers above a lit white plane. With the textured
//! shader receiving shadow + the textured-shadow caster pipeline, the
//! plane should have a dimmer patch beneath the cube. Removing the
//! cube (control scene) yields a uniformly-lit plane.

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Texture3D, Vec3,
};

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
        label: Some("urx3d-tex-shadow-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-tex-shadow-target"),
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
        label: Some("tex-shadow-readback"),
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

fn build_scene(with_cube: bool, device: &wgpu::Device, queue: &wgpu::Queue) -> Scene3D {
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.04, 0.04, 0.05];

    scene.push_light(Light::directional(
        Vec3::new(0.3, -1.0, 0.4),
        [1.0, 1.0, 1.0],
        1.5,
    ));

    let plane = Arc::new(MeshLit::plane_lit(5.0, [1.0, 1.0, 1.0, 1.0]));
    scene.push(Node::new_lit(plane));

    if with_cube {
        // A textured cube (solid yellow texture). The cube is a Wave 5
        // textured node, so the cube is rendered via the textured
        // pipeline AND must cast a shadow via pipeline_shadow_tex.
        let mesh_uv = Arc::new(MeshLit::cube_uv());
        let yellow = Arc::new(Texture3D::from_rgba8(
            device, queue, 1, 1, &[255, 220, 80, 255],
        ));
        scene.push(
            Node::new_textured(mesh_uv, yellow)
                .with_translation(Vec3::new(0.0, 1.5, 0.0))
                .with_scale(Vec3::splat(0.7)),
        );
    }
    scene
}

fn render_once(scene: &Scene3D, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
    let camera = PerspectiveCamera::new(
        Vec3::new(0.0, 6.0, 5.5),
        Vec3::ZERO,
        W as f32 / H as f32,
    );
    let mut r = Renderer3D::new(device, queue, COLOR_FORMAT, (W, H), 8);
    let (tex, view) = make_target(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(device, queue, &mut enc, &view, &camera, scene);
    queue.submit(Some(enc.finish()));
    readback_rgba(device, queue, &tex)
}

#[test]
#[ignore]
fn textured_cube_casts_shadow_on_plane() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    let scene = build_scene(true, &device, &queue);
    let px = render_once(&scene, &device, &queue);

    let plane_y_start = H / 2 + 16;
    let plane_y_end = H - 16;
    let mut min_luma = i32::MAX;
    let mut max_luma = 0;
    for y in plane_y_start..plane_y_end {
        for x in 16..(W - 16) {
            let p = at(&px, x, y);
            // Skip pixels that are clearly the yellow cube (R≫B).
            if p[0] as i32 > 200 && (p[2] as i32) < 120 { continue; }
            let l = luma(p);
            if l > 30 {
                if l < min_luma { min_luma = l; }
                if l > max_luma { max_luma = l; }
            }
        }
    }
    eprintln!("textured-shadow plane min={} max={}", min_luma, max_luma);
    assert!(
        max_luma > 200,
        "plane should have a brightly-lit region: max={}", max_luma
    );
    assert!(
        min_luma < max_luma - 80,
        "textured cube should cast a SHADOW: min={} max={}", min_luma, max_luma
    );
}

#[test]
#[ignore]
fn textured_no_caster_leaves_plane_uniform() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };
    let scene = build_scene(false, &device, &queue);
    let px = render_once(&scene, &device, &queue);

    let plane_y_start = H / 2 + 16;
    let plane_y_end = H - 16;
    let mut min_luma = i32::MAX;
    let mut max_luma = 0;
    for y in plane_y_start..plane_y_end {
        for x in 16..(W - 16) {
            let l = luma(at(&px, x, y));
            if l > 30 {
                min_luma = min_luma.min(l);
                max_luma = max_luma.max(l);
            }
        }
    }
    eprintln!("textured no-caster plane min={} max={}", min_luma, max_luma);
    assert!(
        max_luma - min_luma < 50,
        "plane should be uniformly lit when nothing casts a shadow: \
         min={} max={} delta={}",
        min_luma, max_luma, max_luma - min_luma
    );
}
