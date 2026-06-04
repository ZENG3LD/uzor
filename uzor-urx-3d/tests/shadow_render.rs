//! Wave 7 shadow mapping GPU tests.
//!
//! 1. `cube_casts_shadow_on_plane` — a small lit cube above a lit plane,
//!    directional light from above-front. The plane should have a
//!    visibly DARKER region directly under the cube (shadow) vs an
//!    unobstructed region of the plane.
//! 2. `no_shadow_caster_means_plane_is_uniformly_lit` — same scene with
//!    NO cube; the plane should be uniformly lit (sanity baseline for
//!    the shadow logic — confirms the dark patch in test 1 is the
//!    shadow, not some artifact of the camera/light arrangement).

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Vec3,
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
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-shadow-test"),
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
        label: Some("urx3d-shadow-target"),
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
        label: Some("urx3d-shadow-readback"),
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

fn build_scene(with_cube: bool) -> Scene3D {
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.04, 0.04, 0.05]; // mostly direct light

    // Light comes from above + slightly forward — casts a shadow toward
    // -Z on the plane below.
    scene.push_light(Light::directional(
        Vec3::new(0.3, -1.0, 0.4),
        [1.0, 1.0, 1.0],
        1.5,
    ));

    // Plane lying on Y=0, facing up, white.
    let plane = Arc::new(MeshLit::plane_lit(5.0, [1.0, 1.0, 1.0, 1.0]));
    scene.push(Node::new_lit(plane));

    if with_cube {
        // Small cube hovering above the plane center.
        let cube = Arc::new(MeshLit::cube_lit());
        // Tint white so colors don't muddle the brightness comparison.
        let mut white_cube = MeshLit::cube_lit();
        for v in &mut white_cube.vertices {
            v.color = [1.0, 1.0, 1.0, 1.0];
        }
        let _ = cube;
        scene.push(
            Node::new_lit(Arc::new(white_cube))
                .with_translation(Vec3::new(0.0, 1.5, 0.0))
                .with_scale(Vec3::splat(0.7)),
        );
    }
    scene
}

fn render_once(scene: &Scene3D) -> Option<Vec<u8>> {
    let (device, queue) = init_device()?;
    let camera = PerspectiveCamera::new(
        Vec3::new(0.0, 6.0, 5.5),
        Vec3::ZERO,
        W as f32 / H as f32,
    );
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 8);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, scene);
    queue.submit(Some(enc.finish()));
    Some(readback_rgba(&device, &queue, &tex))
}

#[test]
#[ignore]
fn cube_casts_shadow_on_plane() {
    let Some(px) = render_once(&build_scene(true)) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // Camera at (0,6,5.5) looking at origin. Plane fills bottom half
    // of the screen. The cube hovers at (0, 1.5, 0); its shadow with
    // light dir (0.3, -1, 0.4) lands SLIGHTLY toward the back-left
    // of the cube on the plane.
    //
    // Strategy: take horizontal scanlines on the plane and find the
    // darkest cell vs the brightest cell. If shadow is working, the
    // darkest plane cell should be substantially dimmer than the
    // brightest plane cell.

    // Bottom half of the screen is the plane (camera is high).
    let plane_y_start = H / 2 + 16;
    let plane_y_end = H - 16;

    let mut min_luma: i32 = i32::MAX;
    let mut max_luma: i32 = 0;
    let mut min_at = (0u32, 0u32);
    let mut max_at = (0u32, 0u32);
    for y in plane_y_start..plane_y_end {
        for x in 16..(W - 16) {
            let l = luma(at(&px, x, y));
            if l > 30 {
                if l < min_luma { min_luma = l; min_at = (x, y); }
                if l > max_luma { max_luma = l; max_at = (x, y); }
            }
        }
    }
    eprintln!(
        "plane min_luma={} at {:?},  max_luma={} at {:?}",
        min_luma, min_at, max_luma, max_at
    );

    assert!(
        max_luma > 200,
        "plane should have a brightly-lit region: max={}",
        max_luma
    );
    assert!(
        min_luma < max_luma - 80,
        "plane should have a SHADOW (a much dimmer region): min={} max={}",
        min_luma, max_luma
    );
}

#[test]
#[ignore]
fn no_shadow_caster_means_plane_is_uniformly_lit() {
    let Some(px) = render_once(&build_scene(false)) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let plane_y_start = H / 2 + 16;
    let plane_y_end = H - 16;
    let mut min_luma: i32 = i32::MAX;
    let mut max_luma: i32 = 0;
    for y in plane_y_start..plane_y_end {
        for x in 16..(W - 16) {
            let l = luma(at(&px, x, y));
            if l > 30 {
                min_luma = min_luma.min(l);
                max_luma = max_luma.max(l);
            }
        }
    }
    eprintln!("(no-cube) plane min_luma={} max_luma={}", min_luma, max_luma);
    assert!(
        max_luma - min_luma < 50,
        "plane should be uniformly lit when nothing casts a shadow: \
         min={} max={} delta={}",
        min_luma, max_luma, max_luma - min_luma
    );
}
