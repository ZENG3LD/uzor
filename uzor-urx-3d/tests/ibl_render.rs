//! Wave 10b — IBL environment map test.
//!
//! Setup: PBR mirror cube (metalness=1, roughness=0.05) with NO direct
//! lights and dark ambient. The cube should pull all its color from
//! the env cubemap via the IBL term. With the default sky cubemap
//! (top blue, sides mid-blue, bottom brown), a mirror cube sampled at
//! eye=+Z reflects -Z direction → sees the side of the cubemap,
//! producing a colored result rather than black.

use std::sync::Arc;
use uzor_urx_3d::{
    MeshPbr, Node, PbrMaterial, PerspectiveCamera, Renderer3D, Scene3D, Texture3D, Vec3,
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
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-ibl-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-ibl-target"),
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
        label: Some("ibl-readback"),
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
fn ibl_metal_cube_picks_up_sky_color_without_direct_lights() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    // No lights, near-zero ambient — only IBL can illuminate.
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0];

    let mesh = Arc::new(MeshPbr::cube_pbr());
    // Solid white albedo — IBL × white = IBL itself.
    let albedo = Arc::new(Texture3D::from_rgba8(&device, &queue, 1, 1, &[255, 255, 255, 255]));
    let mat = PbrMaterial::new(albedo)
        .with_metalness(1.0)
        .with_roughness(0.05); // mirror-ish

    scene.push(Node::new_pbr(mesh, mat));

    let camera = PerspectiveCamera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, W as f32 / H as f32);
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // Find brightest pixel inside the cube silhouette.
    let mut max_l = 0;
    let mut max_p = [0u8; 4];
    for y in 0..H {
        for x in 0..W {
            let p = at(&px, x, y);
            if luma(p) > max_l {
                max_l = luma(p);
                max_p = p;
            }
        }
    }
    eprintln!("brightest IBL pixel: {:?} luma={}", max_p, max_l);

    // Without IBL the metal mirror with no lights = pure black.
    // With IBL the cube should pull sky color → some pixel above 100 luma.
    assert!(
        max_l > 100,
        "IBL should light the metallic cube even without direct lights: max_luma={} px={:?}",
        max_l, max_p
    );
    // And the brightest sample should be blue-ish (sky reflection), so
    // blue channel > 60.
    assert!(max_p[2] > 60, "expected sky-blue reflection in IBL: {:?}", max_p);

    // A corner of the framebuffer (background) must stay near black —
    // proving IBL didn't blanket the entire screen.
    let corner = at(&px, 2, 2);
    assert!(luma(corner) < 30, "background should be black: {:?}", corner);
}
