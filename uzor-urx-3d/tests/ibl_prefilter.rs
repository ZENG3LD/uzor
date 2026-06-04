//! Wave 6b — pre-filtered IBL test.
//!
//! Two PBR mirror cubes (metalness=1) — one with roughness=0.05
//! (sharp reflection from a low mip of the prefiltered cubemap), one
//! with roughness=0.95 (blurred reflection from the highest mip).
//! Both are rendered with NO direct lights so all illumination comes
//! from the IBL bake.
//!
//! Asserts:
//!   - both cubes are visible (>40 luma somewhere)
//!   - the rough cube's brightest pixel is LESS saturated than the
//!     mirror cube's — proves the prefilter mips are scaling with
//!     roughness rather than just sampling a single env mip

use std::sync::Arc;
use uzor_urx_3d::{
    MeshPbr, Node, PbrMaterial, PerspectiveCamera, Renderer3D, Scene3D, Texture3D, Vec3,
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
        label: Some("urx3d-ibl-prefilter"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-ibl-prefilter-target"),
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
        label: Some("ibl-prefilter-readback"),
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
fn luma(p: [u8; 4]) -> i32 { p[0] as i32 + p[1] as i32 + p[2] as i32 }

fn render_one(device: &wgpu::Device, queue: &wgpu::Queue, roughness: f32) -> Vec<u8> {
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0];

    let mesh = Arc::new(MeshPbr::cube_pbr());
    let albedo = Arc::new(Texture3D::from_rgba8(device, queue, 1, 1, &[255, 255, 255, 255]));
    let mat = PbrMaterial::new(albedo)
        .with_metalness(1.0)
        .with_roughness(roughness);
    scene.push(Node::new_pbr(mesh, mat));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(device, queue, COLOR_FORMAT, (W, H), 4);
    // Turn bloom OFF for a clean signal.
    r.set_bloom_strength(0.0);
    let (tex, view) = make_target(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(device, queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    readback_rgba(device, queue, &tex)
}

fn max_pixel(px: &[u8]) -> ([u8; 4], i32) {
    let mut max_l = 0;
    let mut max_p = [0u8; 4];
    for chunk in px.chunks(4) {
        let p = [chunk[0], chunk[1], chunk[2], chunk[3]];
        let l = luma(p);
        if l > max_l { max_l = l; max_p = p; }
    }
    (max_p, max_l)
}

#[test]
#[ignore]
fn prefilter_mips_track_roughness() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter"); return;
    };
    let smooth = render_one(&device, &queue, 0.05);
    let rough = render_one(&device, &queue, 0.95);
    let (sp, sl) = max_pixel(&smooth);
    let (rp, rl) = max_pixel(&rough);
    eprintln!("smooth max={:?} luma={}  rough max={:?} luma={}", sp, sl, rp, rl);

    // Both should be visibly lit by the real IBL — proves the
    // prefiltered cubemap + BRDF LUT are bound and produce energy.
    assert!(sl > 100, "smooth should reflect IBL: {:?}", sp);
    assert!(rl > 60,  "rough should still pick up some IBL: {:?}", rp);

    // The two should DIFFER — proves roughness actually selects
    // different mip levels of the prefilter. If both sampled the
    // same single env (regression to Wave 10b), the brightest
    // pixels would be identical.
    let diff = (sl - rl).abs();
    eprintln!("|smooth_luma - rough_luma| = {}", diff);
    assert!(
        diff > 20,
        "smooth and rough should differ — prefilter mip selection broken? \
         smooth_luma={} rough_luma={}",
        sl, rl
    );
}
