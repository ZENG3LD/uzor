//! Wave 5 textured-pipeline GPU tests.
//!
//! 1. `textured_cube_samples_atlas` — 2×2 RGBY atlas mapped onto a cube;
//!    four faces show four distinct colors picked up from texture
//!    sampling. (UV layout in cube_uv is identical across faces — so
//!    all faces sample the same texture but rotated/oriented; this
//!    test only checks that ONE color from the atlas reaches the
//!    screen and that the cube actually lights up).
//! 2. `texture_tint_multiplies_texel` — same textured cube but with
//!    tint=[1,0,0,1]; result should be reddish (G,B channels killed).

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, PerspectiveCamera, PhongMaterial, Renderer3D, Scene3D, Texture3D, Vec3,
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
        label: Some("urx3d-tex-test"),
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
        label: Some("urx3d-tex-target"),
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
        label: Some("urx3d-tex-readback"),
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

/// 2×2 atlas: TL red, TR green, BL yellow, BR blue. 64×64 result for
/// safe linear filtering (sampler is bilinear).
fn rgby_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> Arc<Texture3D> {
    let w = 64u32;
    let h = 64u32;
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let left = x < w / 2;
            let top = y < h / 2;
            let (r, g, b) = match (left, top) {
                (true, true) => (255, 0, 0),
                (false, true) => (0, 255, 0),
                (true, false) => (255, 255, 0),
                (false, false) => (0, 0, 255),
            };
            px[i] = r;
            px[i + 1] = g;
            px[i + 2] = b;
            px[i + 3] = 255;
        }
    }
    Arc::new(Texture3D::from_rgba8(device, queue, w, h, &px))
}

fn render_once(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &Scene3D,
    camera: &PerspectiveCamera,
) -> Vec<u8> {
    let mut r = Renderer3D::new(device, queue, COLOR_FORMAT, (W, H), 8);
    let (tex, view) = make_target(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    r.render(device, queue, &mut enc, &view, camera, scene);
    queue.submit(Some(enc.finish()));
    readback_rgba(device, queue, &tex)
}

#[test]
#[ignore]
fn textured_cube_samples_atlas() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    let camera =
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO, W as f32 / H as f32);

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [1.0, 1.0, 1.0]; // pure ambient -> sampling alone
    // no lights — ambient + texture sampling only, makes the test deterministic

    let atlas = rgby_atlas(&device, &queue);
    let mesh = Arc::new(MeshLit::cube_uv());
    // Material: ambient=1, no diffuse/spec; result = texel × tint × ambient
    let mat = PhongMaterial {
        ambient_strength: 1.0,
        diffuse_strength: 0.0,
        specular_strength: 0.0,
        shininess: 32.0,
    };

    scene.push(
        uzor_urx_3d::Node::new_textured(mesh, atlas).with_material(mat),
    );

    let px = render_once(&device, &queue, &scene, &camera);

    // The cube_uv +Z face UV winding maps:
    //   corner(-1,-1,1) → UV(0,1) atlas BL = YELLOW
    //   corner( 1,-1,1) → UV(0,0) atlas TL = RED
    //   corner( 1, 1,1) → UV(1,0) atlas TR = GREEN
    //   corner(-1, 1,1) → UV(1,1) atlas BR = BLUE
    //
    // Camera eye=(0,0,4) looking at origin (RH) → screen X→world X,
    // screen Y(down)→world -Y. So:
    //   screen TL → world (x<0, y>0, z>0) ≈ corner(-1, 1, 1) → BLUE
    //   screen TR → world (x>0, y>0, z>0) ≈ corner( 1, 1, 1) → GREEN
    //   screen BL → world (x<0, y<0, z>0) ≈ corner(-1,-1, 1) → YELLOW
    //   screen BR → world (x>0, y<0, z>0) ≈ corner( 1,-1, 1) → RED
    let tl = at(&px, W / 4, H / 4);
    let tr = at(&px, W * 3 / 4, H / 4);
    let bl = at(&px, W / 4, H * 3 / 4);
    let br = at(&px, W * 3 / 4, H * 3 / 4);

    eprintln!("tl={:?} tr={:?} bl={:?} br={:?}", tl, tr, bl, br);

    assert!(tl[2] > 150 && tl[0] < 80 && tl[1] < 80, "TL should be BLUE: {:?}", tl);
    assert!(tr[1] > 150 && tr[0] < 80 && tr[2] < 80, "TR should be GREEN: {:?}", tr);
    assert!(bl[0] > 150 && bl[1] > 150 && bl[2] < 80, "BL should be YELLOW: {:?}", bl);
    assert!(br[0] > 150 && br[1] < 80 && br[2] < 80, "BR should be RED: {:?}", br);
}

#[test]
#[ignore]
fn texture_tint_multiplies_texel() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // Same setup as textured_cube_samples_atlas, but tint=[1,0,0,1] →
    // only the RED channel of the texel survives.
    let camera =
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO, W as f32 / H as f32);

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [1.0, 1.0, 1.0];

    let atlas = rgby_atlas(&device, &queue);
    let mesh = Arc::new(MeshLit::cube_uv());
    let mat = PhongMaterial {
        ambient_strength: 1.0,
        diffuse_strength: 0.0,
        specular_strength: 0.0,
        shininess: 32.0,
    };

    scene.push(
        uzor_urx_3d::Node::new_textured(mesh, atlas)
            .with_tint([1.0, 0.0, 0.0, 1.0])
            .with_material(mat),
    );

    // Silence unused warning on Light import
    let _ = Light::directional(Vec3::Y, [1.0; 3], 1.0);

    let px = render_once(&device, &queue, &scene, &camera);

    // Mapping (see textured_cube_samples_atlas comment block):
    //   screen TL = BLUE  texel ×[1,0,0] → black
    //   screen TR = GREEN texel ×[1,0,0] → black
    //   screen BR = RED   texel ×[1,0,0] → red (survives)
    let tl = at(&px, W / 4, H / 4);
    let tr = at(&px, W * 3 / 4, H / 4);
    let br = at(&px, W * 3 / 4, H * 3 / 4);

    eprintln!("tl={:?} tr={:?} br={:?}", tl, tr, br);

    assert!(luma(tl) < 60, "TL blue should be killed by red tint: {:?}", tl);
    assert!(luma(tr) < 60, "TR green should be killed by red tint: {:?}", tr);
    assert!(br[0] > 150, "BR red should survive: {:?}", br);
}
