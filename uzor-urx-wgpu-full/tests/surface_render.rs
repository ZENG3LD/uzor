//! Blit-path correctness test.
//!
//! Renders a scene to the internal rgba8unorm storage texture via
//! `dispatch_full`, then blits it to a bgra8unorm render-attachment texture
//! via `BlitPipeline`, reads back the bgra8unorm result, and asserts that
//! known-colour rects are correctly reproduced.
//!
//! The entire pipeline (dispatch + blit) is encoded into ONE encoder and
//! submitted in a single `queue.submit` call.
//!
//! Run:
//!   cargo test --test surface_render -- --ignored --nocapture

use uzor_urx_wgpu_full::{BlitPipeline, SceneCmd, TileBuffers, TilePipeline, TILE_SIZE};

const W: u32 = 64;
const H: u32 = 64;

const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label:                 Some("surface-render-test-device"),
            required_features:     wgpu::Features::empty(),
            required_limits:       wgpu::Limits::default(),
            memory_hints:          wgpu::MemoryHints::default(),
            trace:                 wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()
}

/// Readback a bgra8unorm texture into flat Vec<u8> [B,G,R,A, ...].
fn readback_bgra(
    device:  &wgpu::Device,
    queue:   &wgpu::Queue,
    texture: &wgpu::Texture,
    tex_w:   u32,
    tex_h:   u32,
) -> Vec<u8> {
    let bytes_per_row_unaligned = tex_w * 4;
    let aligned_stride = (bytes_per_row_unaligned + 255) & !255;
    let buf_size = (aligned_stride * tex_h) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("surface-render-test-staging"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("surface-render-test-readback") },
    );
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin:    wgpu::Origin3d::ZERO,
            aspect:    wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset:         0,
                bytes_per_row:  Some(aligned_stride),
                rows_per_image: Some(tex_h),
            },
        },
        wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();

    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((tex_w * tex_h * 4) as usize);
    for row in 0..tex_h as usize {
        let row_start = row * aligned_stride as usize;
        let row_end   = row_start + (tex_w * 4) as usize;
        out.extend_from_slice(&raw[row_start..row_end]);
    }
    drop(raw);
    staging.unmap();
    out
}

/// Extract RGB from a bgra8unorm readback buffer (bytes = [B, G, R, A]).
fn pixel_rgb_from_bgra(buf: &[u8], tex_w: u32, x: u32, y: u32) -> [u8; 3] {
    let idx = ((y * tex_w + x) * 4) as usize;
    let b = buf[idx];
    let g = buf[idx + 1];
    let r = buf[idx + 2];
    [r, g, b]
}

/// Run the full dispatch+blit pipeline and return readback pixels from the
/// bgra8unorm surface texture.
fn run_blit_pipeline(
    device: &wgpu::Device,
    queue:  &wgpu::Queue,
    cmds:   &[SceneCmd],
) -> (Vec<u8>, u32, u32, wgpu::Texture) {
    let (bufs, _storage_tex, storage_view) =
        TileBuffers::with_output_texture(device, cmds.len() as u32, W, H);
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;

    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:               Some("surface-render-test-surface"),
        size:                wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
        mip_level_count:     1,
        sample_count:        1,
        dimension:           wgpu::TextureDimension::D2,
        format:              SURFACE_FORMAT,
        usage:               wgpu::TextureUsages::RENDER_ATTACHMENT
                           | wgpu::TextureUsages::COPY_SRC,
        view_formats:        &[],
    });
    let surface_view = surface_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let tile_pipeline = TilePipeline::new(device);
    let blit_pipeline = BlitPipeline::new(device, SURFACE_FORMAT);

    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(device);
    let (_dum_img, dummy_img_view)     = TilePipeline::dummy_image_atlas(device);

    // ONE encoder, ONE submit — same as real consumer path.
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    tile_pipeline.render_to_target(
        device, queue, &mut enc,
        &bufs, cmds, &[],
        &storage_view, &blit_pipeline, &surface_view,
        tex_w, tex_h,
        &dummy_atlas_view, &dummy_img_view,
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let pixels = readback_bgra(device, queue, &surface_tex, tex_w, tex_h);
    (pixels, tex_w, tex_h, surface_tex)
}

/// Blit correctly reproduces a red rect at the centre pixel in bgra8unorm output.
#[test]
#[ignore = "needs gpu adapter"]
fn surface_render_blits_storage_to_bgra_format() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // Four known-colour rects (same as pixel_render_demo scene).
    let cmds = vec![
        SceneCmd::rect( 2.0,  2.0, 14.0, 14.0, [255,   0,   0, 255]), // red    — centre (8,  8)
        SceneCmd::rect(16.0,  2.0, 28.0, 14.0, [  0, 255,   0, 255]), // green  — centre (22, 8)
        SceneCmd::rect( 2.0, 16.0, 14.0, 28.0, [  0,   0, 255, 255]), // blue   — centre (8, 22)
        SceneCmd::rect(32.0, 32.0, 48.0, 48.0, [255, 255, 255, 255]), // white  — centre (40,40)
    ];

    let (pixels, tex_w, _tex_h, _surf) = run_blit_pipeline(&device, &queue, &cmds);

    let checks: &[(&str, u32, u32, [u8; 3])] = &[
        ("red   rect centre",  8,  8, [255,   0,   0]),
        ("green rect centre", 22,  8, [  0, 255,   0]),
        ("blue  rect centre",  8, 22, [  0,   0, 255]),
        ("white rect centre", 40, 40, [255, 255, 255]),
    ];

    let mut all_ok = true;
    for &(label, cx, cy, expected) in checks {
        let got = pixel_rgb_from_bgra(&pixels, tex_w, cx, cy);
        eprintln!("{label} ({cx},{cy}) = {:?}  expected={:?}", got, expected);
        if got != expected {
            eprintln!("  MISMATCH");
            all_ok = false;
        }
    }
    assert!(all_ok, "one or more blit pixels had wrong colour — see eprintln output above");
}

/// Pixel outside every rect stays transparent (0,0,0,0) after blit.
#[test]
#[ignore = "needs gpu adapter"]
fn surface_render_blit_transparent_pixel_outside_rects() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    let cmds = vec![
        SceneCmd::rect(0.0, 0.0, 8.0, 8.0, [255, 0, 0, 255]),
    ];

    let (pixels, tex_w, _tex_h, _surf) = run_blit_pipeline(&device, &queue, &cmds);

    // Pixel (50, 50) is outside the rect — must be transparent after blit.
    let idx = ((50 * tex_w + 50) * 4) as usize;
    let bgra = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];
    eprintln!("transparent pixel (50,50) bgra = {:?}", bgra);
    assert_eq!(bgra, [0, 0, 0, 0],
        "pixel outside all rects must stay transparent after blit, got {:?}", bgra);
}
