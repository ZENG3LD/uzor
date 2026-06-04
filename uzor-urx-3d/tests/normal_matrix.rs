//! Wave 11 normal-matrix GPU test.
//!
//! Compare two identical lit cubes lit by a single +Y directional
//! light, but cube A has uniform scale and cube B has a STRONG
//! non-uniform scale that would skew normals without an inverse-
//! transpose. We sample a face that the uniform cube lights cleanly
//! and verify the non-uniform cube's matching face is ALSO well lit
//! (which only happens if the normal matrix corrects the squash).

use std::sync::Arc;
use uzor_urx_3d::{Light, MeshLit, Node, PerspectiveCamera, Renderer3D, Scene3D, Vec3};

const W: u32 = 320;
const H: u32 = 160;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-nmat-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-nmat-target"),
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
        label: Some("nmat-readback"),
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
fn nonuniform_scale_preserves_top_face_lighting() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping"); return;
    };

    // Two cubes side-by-side, both with white tint, single +Y directional
    // light from straight above. Top face should be MAX bright on both.
    //
    // Without the normal-matrix fix, the squashed cube's top face would
    // be DIM because the upper-3×3 model of `scale(2,0.3,1)` rotates
    // the surface normal into XZ — dot(N, +Y) drops, top goes dark.
    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0]; // direct light only

    // Override all face colors to white so we test lighting only.
    let mut white = MeshLit::cube_lit();
    for v in &mut white.vertices { v.color = [1.0, 1.0, 1.0, 1.0]; }
    let mesh = Arc::new(white);

    // Cube A — uniform scale, on the LEFT.
    scene.push(
        Node::new_lit(mesh.clone())
            .with_translation(Vec3::new(-2.5, 0.0, 0.0))
            .with_scale(Vec3::new(1.0, 1.0, 1.0)),
    );
    // Cube B — STRONG non-uniform scale (squashed thin on Y, wider on X),
    // on the RIGHT.
    scene.push(
        Node::new_lit(mesh.clone())
            .with_translation(Vec3::new(2.5, 0.0, 0.0))
            .with_scale(Vec3::new(2.0, 0.3, 1.0)),
    );

    scene.push_light(Light::directional(
        Vec3::new(0.0, -1.0, 0.0), // pointing down, lights top face
        [1.0, 1.0, 1.0],
        1.4,
    ));

    // Camera above, looking down at both cubes.
    let camera = PerspectiveCamera::new(
        Vec3::new(0.0, 5.5, 4.5),
        Vec3::ZERO,
        W as f32 / H as f32,
    );
    let mut r = Renderer3D::new(&device, &queue, COLOR_FORMAT, (W, H), 4);
    let (tex, view) = make_target(&device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(&device, &queue, &mut enc, &view, &camera, &scene);
    queue.submit(Some(enc.finish()));
    let px = readback_rgba(&device, &queue, &tex);

    // Find brightest pixel in the left half vs right half (cubes split
    // exactly on x=W/2).
    let mut max_left: i32 = 0;
    let mut max_right: i32 = 0;
    for y in 0..H {
        for x in 0..W {
            let l = luma(at(&px, x, y));
            if x < W / 2 { if l > max_left { max_left = l; } }
            else if l > max_right { max_right = l; }
        }
    }
    eprintln!("max_left (uniform cube)={}  max_right (squashed cube)={}", max_left, max_right);

    // Both should be lit; the squashed cube's top should not be more
    // than 30 luma dimmer than the uniform one (small camera-grazing
    // differences are OK, large = normal-matrix is broken).
    assert!(
        max_left > 200,
        "uniform cube top should be bright: {}",
        max_left
    );
    assert!(
        max_right > max_left - 30,
        "squashed cube top should be ~equally bright (normal matrix corrects scale): \
         left={} right={}",
        max_left, max_right
    );
}
