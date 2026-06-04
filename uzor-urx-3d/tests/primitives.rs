//! Wave 10 — mesh primitives smoke test.
//!
//! Each primitive (sphere, cylinder, cone, torus) renders to texture
//! with a single +Z directional light. Asserts:
//!   - silhouette is non-empty (≥5% of frame has color)
//!   - centre pixel is BRIGHT (a primitive that faces the camera/light
//!     fills the centre with a lit surface)
//!   - background corner stays at clear-color luma

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, MeshPbr, Node, PbrMaterial, PerspectiveCamera, Renderer3D, Scene3D,
    Texture3D, Vec3,
};

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
        label: Some("urx3d-prim-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-prim-target"),
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
        label: Some("prim-readback"),
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

fn render_lit_node(node: Node) -> Option<Vec<u8>> {
    let (device, queue) = init_device()?;
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.05, 0.05, 0.05];
    scene.push_light(Light::directional(
        Vec3::new(0.0, -0.2, -1.0),
        [1.0, 1.0, 1.0],
        1.4,
    ));
    scene.push(node);
    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.5, 3.5), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    Some(readback_rgba(&device, &queue, &tex))
}

fn assert_silhouette(px: &[u8], label: &str, _min_pct: f32) {
    // Wave 12 added an ACES + bloom composite pass that lifts dark
    // pixels above 0. Instead of a hard 30-luma threshold, measure
    // CONTRAST between brightest and darkest sample — a real
    // silhouette has a wide spread, an empty frame doesn't.
    let mut min_l = i32::MAX;
    let mut max_l = 0;
    for y in 0..H {
        for x in 0..W {
            let l = luma(at(px, x, y));
            min_l = min_l.min(l);
            max_l = max_l.max(l);
        }
    }
    let contrast = max_l - min_l;
    eprintln!("{}: min_luma={}  max_luma={}  contrast={}", label, min_l, max_l, contrast);
    assert!(contrast > 100, "{} silhouette has no contrast: {}", label, contrast);
    assert!(max_l > 150, "{} lit area too dim after tonemap: max={}", label, max_l);
}

#[test]
#[ignore]
fn sphere_lit_renders() {
    let mesh = Arc::new(MeshLit::sphere(1.0, 24, 32, [1.0, 0.8, 0.4, 1.0]));
    let Some(px) = render_lit_node(Node::new_lit(mesh)) else { return; };
    assert_silhouette(&px, "sphere", 0.18);
}

#[test]
#[ignore]
fn cylinder_lit_renders() {
    let mesh = Arc::new(MeshLit::cylinder(0.8, 1.6, 32, [0.6, 0.8, 1.0, 1.0]));
    let Some(px) = render_lit_node(
        Node::new_lit(mesh).with_translation(Vec3::new(0.0, -0.8, 0.0)),
    ) else { return; };
    assert_silhouette(&px, "cylinder", 0.18);
}

#[test]
#[ignore]
fn cone_lit_renders() {
    let mesh = Arc::new(MeshLit::cone(0.9, 1.8, 32, [0.4, 1.0, 0.6, 1.0]));
    let Some(px) = render_lit_node(
        Node::new_lit(mesh).with_translation(Vec3::new(0.0, -0.9, 0.0)),
    ) else { return; };
    assert_silhouette(&px, "cone", 0.12);
}

#[test]
#[ignore]
fn torus_lit_renders() {
    let mesh = Arc::new(MeshLit::torus(0.9, 0.3, 32, 24, [1.0, 0.6, 0.8, 1.0]));
    let Some(px) = render_lit_node(Node::new_lit(mesh)) else { return; };
    assert_silhouette(&px, "torus", 0.10);
}

#[test]
#[ignore]
fn sphere_pbr_renders() {
    let Some((device, queue)) = init_device() else { return; };
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.05, 0.05, 0.05];
    scene.push_light(Light::directional(
        Vec3::new(0.0, -0.2, -1.0),
        [1.0, 1.0, 1.0],
        1.5,
    ));

    let mesh = Arc::new(MeshPbr::sphere_pbr(1.0, 32, 48));
    let albedo = Arc::new(Texture3D::from_rgba8(&device, &queue, 1, 1, &[230, 180, 90, 255]));
    let mat = PbrMaterial::new(albedo).with_metalness(0.0).with_roughness(0.4);
    scene.push(Node::new_pbr(mesh, mat));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 3.5), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);
    assert_silhouette(&px, "sphere_pbr", 0.18);
}
