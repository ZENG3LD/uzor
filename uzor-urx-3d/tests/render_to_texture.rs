//! Wave 8 GPU tests — render-to-texture bridge.
//!
//! 1. `scene3d_renders_into_texture` — render Scene A into an
//!    OffscreenTarget; sample readback shows the expected face colors.
//! 2. `scene3d_inside_scene3d_via_render_to_texture` — render Scene A
//!    into texture T; render Scene B (with one node textured by T) into
//!    the swapchain; verify Scene A's content appears on the textured
//!    cube in Scene B.

use std::sync::Arc;
use uzor_urx_3d::{
    Mesh, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Texture3D, Vec3,
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
        label: Some("urx3d-rt-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()
}

fn make_swapchain(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-rt-swap"),
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

fn readback_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let aligned_stride = (width * 4 + 255) & !255;
    let buf_size = (aligned_stride * height) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx3d-rt-readback"),
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
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height as usize {
        let row_start = row * aligned_stride as usize;
        let row_end = row_start + (width * 4) as usize;
        out.extend_from_slice(&raw[row_start..row_end]);
    }
    drop(raw);
    staging.unmap();
    out
}

#[inline]
fn at(buf: &[u8], stride_w: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * stride_w + x) * 4) as usize;
    [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
}

#[test]
#[ignore]
fn scene3d_renders_into_texture() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let target = Texture3D::render_target(&device, W, H);

    // Cube head-on from +Z, +Z face visible (blue)
    let camera =
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, W as f32 / H as f32);

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.push(Node::new(Arc::new(Mesh::cube_rgb_faces())));

    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render_to_texture(&device, &queue, &mut enc, &target, &camera, &scene);
    queue.submit(Some(enc.finish()));

    let px = readback_rgba(&device, &queue, &target.texture, W, H);

    // Cube +Z face is blue in cube_rgb_faces — centre of frame should
    // be blue-dominant.
    let centre = at(&px, W, W / 2, H / 2);
    eprintln!("RT centre={:?}", centre);
    assert!(centre[2] > 80 && centre[0] < 80 && centre[1] < 80,
        "RT centre should be blue (+Z face): {:?}", centre);

    // Far corner should be background (cleared to black).
    let corner = at(&px, W, 2, 2);
    assert!(corner[0] < 16 && corner[1] < 16 && corner[2] < 16,
        "RT corner should be black background: {:?}", corner);
}

#[test]
#[ignore]
fn scene3d_inside_scene3d_via_render_to_texture() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // 1) Inner scene — solid green cube against red background; rendered
    //    into a 64×64 texture.
    let inner_target = Texture3D::render_target(&device, 64, 64);

    let inner_camera =
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO, 1.0);

    let mut inner_scene = Scene3D::new();
    inner_scene.clear_color = [0.7, 0.0, 0.0, 1.0]; // red background
    // White cube but tinted GREEN — covers most of the inner texture.
    let mut white_cube = Mesh::cube_rgb_faces();
    for v in &mut white_cube.vertices {
        v.color = [0.0, 1.0, 0.0, 1.0]; // green vertex color
    }
    inner_scene.push(Node::new(Arc::new(white_cube)));

    let mut r_inner = Renderer3D::new(&device, &queue, COLOR_FORMAT, (64, 64), 4);
    let mut enc_inner = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r_inner.render_to_texture(&device, &queue, &mut enc_inner, &inner_target, &inner_camera, &inner_scene);
    queue.submit(Some(enc_inner.finish()));

    // 2) Outer scene — a cube TEXTURED with inner_target. Camera +Z
    //    looking at origin so +Z face fills the centre.
    let outer_camera =
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, W as f32 / H as f32);

    let mut outer_scene = Scene3D::new();
    outer_scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    outer_scene.ambient = [1.0, 1.0, 1.0]; // textured pipeline uses ambient if no lights

    let outer_mesh = Arc::new(MeshLit::cube_uv());
    outer_scene.push(
        Node::new_textured(outer_mesh, Arc::new(Texture3D {
            texture: inner_target.texture,
            view: inner_target.view,
            sampler: inner_target.sampler,
            width: inner_target.width,
            height: inner_target.height,
        }))
        .with_material(uzor_urx_3d::PhongMaterial {
            ambient_strength: 1.0,
            diffuse_strength: 0.0,
            specular_strength: 0.0,
            shininess: 32.0,
        }),
    );

    let mut r_outer = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (swap_tex, swap_view) = make_swapchain(&device);
    let mut enc_outer = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r_outer.render(&device, &queue, &mut enc_outer, &swap_view, &outer_camera, &outer_scene);
    queue.submit(Some(enc_outer.finish()));

    let px = readback_rgba(&device, &queue, &swap_tex, W, H);

    // The inner cube was green on red. After the outer cube samples
    // that texture onto its +Z face, the centre of the outer frame
    // should be green-dominant (inner cube projects onto most of the
    // texture).
    let centre = at(&px, W, W / 2, H / 2);
    eprintln!("nested-3D centre={:?}", centre);
    assert!(
        centre[1] > 80 && centre[1] as i32 > centre[0] as i32 + 30 && centre[1] as i32 > centre[2] as i32 + 30,
        "centre of nested 3D should be green-dominant (sampled from inner texture): {:?}",
        centre
    );
}
