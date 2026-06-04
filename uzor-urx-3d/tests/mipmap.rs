//! Wave 21 — mipmap GPU test.
//!
//! High-frequency 32×32 black/white checkerboard texture on a quad
//! viewed at grazing angle, far enough from camera that only mip
//! levels 3+ get sampled. With proper trilinear mips the result is a
//! smooth GREY (averaged) instead of aliased noise. Without mipmaps
//! (single-level texture) the rendering moires aggressively.

use std::sync::Arc;
use uzor_urx_3d::{
    Light, MeshLit, Node, PerspectiveCamera, PhongMaterial, Renderer3D, Scene3D, Texture3D, Vec3,
};

const W: u32 = 256;
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
        label: Some("urx3d-mipmap-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx3d-mipmap-target"),
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
        label: Some("mipmap-readback"),
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
                offset: 0, bytes_per_row: Some(aligned), rows_per_image: Some(H),
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

fn build_checker_rgba(size: u32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let on = (x + y) & 1 == 0;
            let v = if on { 255 } else { 0 };
            px[i] = v; px[i+1] = v; px[i+2] = v; px[i+3] = 255;
        }
    }
    px
}

#[test]
fn mip_levels_count_is_correct() {
    assert_eq!(uzor_urx_3d::texture::mip_levels(1, 1), 1);
    assert_eq!(uzor_urx_3d::texture::mip_levels(2, 2), 2);
    assert_eq!(uzor_urx_3d::texture::mip_levels(32, 32), 6);
    assert_eq!(uzor_urx_3d::texture::mip_levels(256, 64), 9);
}

#[test]
#[ignore]
fn mipmapped_checker_does_not_alias_at_distance() {
    let Some((device, queue)) = init_device() else { return; };

    // Build two scenes — one with a mipmapped texture, one without.
    // Both render a large textured ground stretching to the horizon.
    // The DISTANT half of the floor (top of the framebuffer) should
    // be GREY (≈127) with mipmaps; without mipmaps it would moire.

    let checker = build_checker_rgba(32);

    // Tiled cube mesh: cube_uv UVs in [0,1]² per face; we render a
    // big tilted plane via cube scaled flat. Cube_uv face UVs cover
    // [0,1] — we want the texture to REPEAT, so scale UV via tint?
    // Simpler: render a wide cube and let it appear small in the
    // distance — the magnification factor across the screen still
    // forces mip 3+ sampling.

    // Use cube_uv but scale UV ×30 so the 32×32 texture TILES many
    // times across the 20-wide floor. That forces shader derivatives
    // to ask for a higher mip level for distant fragments.
    let mut tiled = MeshLit::cube_uv();
    for v in &mut tiled.vertices {
        v.uv[0] *= 30.0;
        v.uv[1] *= 30.0;
    }
    let mesh = Arc::new(tiled);

    fn render_scene(
        device: &wgpu::Device, queue: &wgpu::Queue,
        mesh: Arc<uzor_urx_3d::MeshUv>,
        texture: Arc<Texture3D>,
    ) -> Vec<u8> {
        let mut scene = Scene3D::new();
        scene.clear_color = [0.0, 0.0, 0.0, 1.0];
        scene.ambient = [1.0, 1.0, 1.0]; // pure ambient, no shadow noise

        // A very thin, wide slab tilted away from the camera = floor
        scene.push(
            Node::new_textured(mesh, texture)
                .with_translation(Vec3::new(0.0, -1.0, -8.0))
                .with_scale(Vec3::new(20.0, 0.01, 20.0))
                // Push ambient material strength to 1 so the test
                // measures the TEXTURE SAMPLE directly without phong
                // attenuation muddying the variance.
                .with_material(PhongMaterial {
                    ambient_strength: 1.0,
                    diffuse_strength: 0.0,
                    specular_strength: 0.0,
                    shininess: 1.0,
                }),
        );
        scene.push_light(Light::directional(Vec3::new(0.0, -1.0, 0.0), [1.0; 3], 0.0));

        let camera = PerspectiveCamera::new(
            Vec3::new(0.0, 2.5, 5.0),
            Vec3::new(0.0, -1.0, -10.0),
            W as f32 / H as f32,
        );
        let mut r = Renderer3D::new(device, queue, COLOR_FORMAT, (W, H), 4);
        let (tex, view) = make_target(device);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        r.render(device, queue, &mut enc, &view, &camera, &scene);
        queue.submit(Some(enc.finish()));
        readback_rgba(device, queue, &tex)
    }

    let no_mips = Arc::new(Texture3D::from_rgba8(&device, &queue, 32, 32, &checker));
    let with_mips = Arc::new(Texture3D::from_rgba8_mipped(&device, &queue, 32, 32, &checker));

    let px_no = render_scene(&device, &queue, mesh.clone(), no_mips);
    let px_mip = render_scene(&device, &queue, mesh, with_mips);

    // Look at the DISTANT band — middle row, central 80% of width.
    // Compute variance: aliasing produces wildly alternating black
    // and white pixels (very high variance). Mipmaps average to grey
    // (low variance, mean near 127).
    // Pick a row in the lower-middle of the frame — where the FLOOR
    // is visible (top of frame is sky), and far enough back along the
    // floor that distant texels alias without mipmaps.
    let band_y = H * 6 / 10;
    // Compute variance from each scene's OWN mean — both scenes pass
    // through ACES + gamma, so absolute luma differs but a mipped
    // chain produces a smoother (lower-variance) signal.
    let mut sum_no = 0i64;
    let mut sum_mip = 0i64;
    let mut samples = 0i64;
    let mut row_no = Vec::new();
    let mut row_mip = Vec::new();
    for x in (W / 10)..(W - W / 10) {
        let l_no = luma(at(&px_no, x, band_y)) / 3;
        let l_mip = luma(at(&px_mip, x, band_y)) / 3;
        sum_no += l_no as i64;
        sum_mip += l_mip as i64;
        row_no.push(l_no as i64);
        row_mip.push(l_mip as i64);
        samples += 1;
    }
    let mean_no = sum_no / samples;
    let mean_mip = sum_mip / samples;
    let var_no: i64 = row_no.iter().map(|v| (v - mean_no).pow(2)).sum::<i64>() / samples;
    let var_mip: i64 = row_mip.iter().map(|v| (v - mean_mip).pow(2)).sum::<i64>() / samples;
    eprintln!(
        "distant band: no_mips mean={} var={};  with_mips mean={} var={}",
        mean_no, var_no, mean_mip, var_mip
    );

    // Mipped chain must alias LESS than the non-mipped texture.
    assert!(
        var_mip < var_no,
        "mipmapped texture must alias LESS than non-mipped: \
         no_mips_var={}  mipped_var={}",
        var_no, var_mip
    );
}
