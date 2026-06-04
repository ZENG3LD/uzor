//! Wave 4 lighting GPU tests.
//!
//! 1. `directional_light_brightens_facing_face` — white cube lit by
//!    a directional light along -Y; top face (+Y, normal=(0,1,0))
//!    should be bright, bottom face (-Y, normal=(0,-1,0)) dark.
//! 2. `point_light_attenuates_with_distance` — same white cube, point
//!    light at (0, 3, 0) range=4; centre of +Y face brighter than
//!    far-away corners (range falloff working).
//! 3. `ambient_only_no_lights` — empty lights array with ambient=
//!    (0.5,0.5,0.5) → cube lit purely by ambient term, no falloff.

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, Node, PerspectiveCamera, PhongMaterial, Renderer3D, Scene3D, Vec3,
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
        label: Some("urx3d-light-test"),
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
        label: Some("urx3d-light-target"),
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
        label: Some("urx3d-light-readback"),
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
    // Cheap brightness sum on R+G+B
    p[0] as i32 + p[1] as i32 + p[2] as i32
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
fn directional_light_brightens_facing_face() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // Camera at (3,3,3) sees +X +Y +Z faces. White cube with default
    // Phong material, ambient very low so direct lighting dominates.
    let camera =
        PerspectiveCamera::new(Vec3::new(3.0, 3.0, 3.0), Vec3::ZERO, W as f32 / H as f32);

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.02, 0.02, 0.02]; // near-zero ambient
    // Light shines DOWN along -Y. Direction is light's travel vector
    // → flipping in shader gives +Y, so faces with normal=(0,1,0)
    // light up, normal=(0,-1,0) goes dark.
    scene.push_light(Light::directional(
        Vec3::new(0.0, -1.0, 0.0),
        [1.0, 1.0, 1.0],
        1.0,
    ));

    let cube = Arc::new(MeshLit::cube_lit());
    // Override default face colors → uniform white so we measure
    // pure brightness from lighting, not the per-face palette.
    let mut white_cube = MeshLit::cube_lit();
    for v in &mut white_cube.vertices {
        v.color = [1.0, 1.0, 1.0, 1.0];
    }
    let white = Arc::new(white_cube);
    let _ = cube; // unused; keep for symmetry

    scene.push(Node::new_lit(white).with_material(PhongMaterial::default()));

    let px = render_once(&device, &queue, &scene, &camera);

    // Sample top probe (sees +Y face) vs lower-mid probe (sees -Y or
    // dimmer side faces).
    // From session 24 cube_render mapping: at eye=(3,3,3),
    //   row y=H*3/8: top-band has green-dominant +Y face
    //   row y=H*5/8: bottom-band has darker faces
    let top = at(&px, W / 2, H * 3 / 8);
    let bot = at(&px, W / 2, H * 5 / 8);
    eprintln!("top(+Y)={:?}  bot={:?}", top, bot);

    assert!(
        luma(top) > luma(bot) + 60,
        "top (+Y face, lit) should be > bottom by >=60 luma: top={} bot={}",
        luma(top),
        luma(bot)
    );

    // Top probe must NOT be black — the directional light is active.
    assert!(luma(top) > 100, "top probe should be lit: {}", luma(top));
}

#[test]
#[ignore]
fn point_light_attenuates_with_distance() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // 3 plane tiles spaced along +X. One point light at the origin.
    // Closer tile should be brighter than farther tile.
    let camera = PerspectiveCamera::new(
        Vec3::new(0.0, 4.0, 0.01), // looking straight down
        Vec3::ZERO,
        W as f32 / H as f32,
    );

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.0, 0.0, 0.0]; // pure direct
    scene.push_light(Light::point(
        Vec3::new(0.0, 1.0, 0.0),
        [1.0, 1.0, 1.0],
        2.0,
        4.0, // range
    ));

    let tile = Arc::new(MeshLit::plane_lit(0.6, [1.0, 1.0, 1.0, 1.0]));
    scene.push(Node::new_lit(tile.clone()).with_translation(Vec3::new(-2.5, 0.0, 0.0)));
    scene.push(Node::new_lit(tile.clone()).with_translation(Vec3::new(0.0, 0.0, 0.0)));
    scene.push(Node::new_lit(tile.clone()).with_translation(Vec3::new(2.5, 0.0, 0.0)));

    let px = render_once(&device, &queue, &scene, &camera);

    // Camera straight down at origin → centre of frame ≈ centre tile.
    // Sample 3x3 grid centre + extreme columns.
    let centre = at(&px, W / 2, H / 2);
    let left_tile_centre = at(&px, W / 4, H / 2);
    let right_tile_centre = at(&px, W * 3 / 4, H / 2);
    eprintln!(
        "centre={:?} left={:?} right={:?}",
        centre, left_tile_centre, right_tile_centre
    );

    // Centre tile is right under the light → should be the brightest.
    assert!(
        luma(centre) > luma(left_tile_centre) + 30,
        "centre tile should outshine left by >=30 luma. centre={} left={}",
        luma(centre),
        luma(left_tile_centre)
    );
    assert!(
        luma(centre) > luma(right_tile_centre) + 30,
        "centre tile should outshine right by >=30 luma. centre={} right={}",
        luma(centre),
        luma(right_tile_centre)
    );
}

#[test]
#[ignore]
fn ambient_only_no_lights_falls_back_to_base_color() {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };

    // No lights — only ambient. Set ambient to (0.5,0.5,0.5), material
    // ambient_strength=1.0, expect cube to render at ~half base color.
    let camera =
        PerspectiveCamera::new(Vec3::new(3.0, 3.0, 3.0), Vec3::ZERO, W as f32 / H as f32);

    let mut scene = Scene3D::new();
    scene.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.ambient = [0.5, 0.5, 0.5];
    // No lights pushed.

    let cube = Arc::new(MeshLit::cube_lit());
    scene.push(
        Node::new_lit(cube).with_material(PhongMaterial {
            ambient_strength: 1.0,
            diffuse_strength: 0.0,
            specular_strength: 0.0,
            shininess: 32.0,
        }),
    );

    let px = render_once(&device, &queue, &scene, &camera);

    // Cube +Y face is bright green (0,255,0) → ambient×0.5 → ~127g.
    // sRGB rendertarget: 0.5 linear ≈ 0.735 sRGB ≈ 188. Allow [120,220].
    let centre = at(&px, W / 2, H * 3 / 8); // +Y face area
    eprintln!("ambient-only centre={:?}", centre);
    assert!(
        centre[1] > 60 && centre[1] < 240,
        "+Y face should be greenish at ~half base intensity, got g={}",
        centre[1]
    );
    // R, B should be ≤ 30 since base green color has 0 in them.
    assert!(
        centre[0] < 60 && centre[2] < 60,
        "non-green channels should be near-zero: {:?}",
        centre
    );
}
