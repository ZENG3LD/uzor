//! GPU-gated glyph (kind=4) correctness tests.
//!
//! Verifies that Glyph commands correctly sample the R8Unorm atlas,
//! apply alpha modulation, and composite over-blend correctly.
//!
//! Run:
//!   cargo test --test glyph_render -- --ignored --nocapture

use uzor_urx_wgpu_full::{SceneCmd, TileBuffers, TilePipeline, TILE_SIZE};

const W: u32 = 128;
const H: u32 = 64;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label:                 Some("glyph-render-test-device"),
            required_features:     wgpu::Features::empty(),
            required_limits:       wgpu::Limits::default(),
            memory_hints:          wgpu::MemoryHints::default(),
            trace:                 wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()
}

/// Readback an rgba8unorm texture into a flat Vec<u8> (RGBA, row-major).
fn readback_texture(
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
        label:              Some("glyph-render-test-staging"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("glyph-render-test-readback") },
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

fn pixel_rgba(buf: &[u8], tex_w: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * tex_w + x) * 4) as usize;
    [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
}

/// Upload an R8Unorm atlas from a byte slice (one byte per texel).
fn upload_r8_atlas(
    device:   &wgpu::Device,
    queue:    &wgpu::Queue,
    atlas_w:  u32,
    atlas_h:  u32,
    data:     &[u8],
) -> (wgpu::Texture, wgpu::TextureView) {
    assert_eq!(data.len(), (atlas_w * atlas_h) as usize);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label:               Some("glyph-render-test-atlas"),
        size:                wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
        mip_level_count:     1,
        sample_count:        1,
        dimension:           wgpu::TextureDimension::D2,
        format:              wgpu::TextureFormat::R8Unorm,
        usage:               wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats:        &[],
    });
    // R8Unorm: each row must be aligned to 256 bytes for copy_buffer_to_texture.
    let row_stride = (atlas_w + 255) & !255;
    let buf_size   = (row_stride * atlas_h) as u64;
    let upload_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("glyph-render-test-atlas-upload"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    {
        let mut mapped = upload_buf.slice(..).get_mapped_range_mut();
        for row in 0..atlas_h as usize {
            let src_start = row * atlas_w as usize;
            let dst_start = row * row_stride as usize;
            mapped[dst_start..dst_start + atlas_w as usize]
                .copy_from_slice(&data[src_start..src_start + atlas_w as usize]);
        }
    }
    upload_buf.unmap();

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("glyph-render-test-atlas-copy") },
    );
    enc.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: &upload_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset:         0,
                bytes_per_row:  Some(row_stride),
                rows_per_image: Some(atlas_h),
            },
        },
        wgpu::TexelCopyTextureInfo {
            texture:   &texture,
            mip_level: 0,
            origin:    wgpu::Origin3d::ZERO,
            aspect:    wgpu::TextureAspect::All,
        },
        wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Glyph alpha modulated by colour.
///
/// Atlas: 64×64 R8Unorm horizontal gradient — left column = 0, right column = 255.
/// Glyph cmd: full atlas (UV [0,0,1,1]) drawn into a 100×40 bbox at (0,0), red colour.
///
/// Expected:
///   - left edge (x=5, y=20) → low alpha → near-transparent
///   - right edge (x=95, y=20) → full alpha → opaque red (255,0,0,255)
#[test]
#[ignore = "needs gpu adapter"]
fn glyph_alpha_modulated_by_colour() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // Build 64×64 atlas with horizontal gradient: alpha = x / 63 * 255.
    let atlas_w: u32 = 64;
    let atlas_h: u32 = 64;
    let atlas_data: Vec<u8> = (0..atlas_h)
        .flat_map(|_y| (0..atlas_w).map(|x| ((x as f32 / (atlas_w - 1) as f32) * 255.0).round() as u8))
        .collect();
    let (_atlas_tex, atlas_view) = upload_r8_atlas(&device, &queue, atlas_w, atlas_h, &atlas_data);

    // Glyph bbox covers [0..100, 0..40]; UV = full atlas.
    let cmds = vec![
        SceneCmd::glyph(0.0, 0.0, 100.0, 40.0, [255, 0, 0, 255], [0.0, 0.0, 1.0, 1.0]),
    ];

    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &output_view, &atlas_view);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let pixels = readback_texture(&device, &queue, &output_tex, tex_w, tex_h);

    // Left edge: x=5, y=20 → atlas UV ≈ (5/100, 0.5) → alpha ≈ 5/100 * 255 ≈ 12 → low.
    let left = pixel_rgba(&pixels, tex_w, 5, 20);
    // Right edge: x=95, y=20 → atlas UV ≈ (95/100, 0.5) → alpha ≈ 95/100 * 255 ≈ 242 → high.
    let right = pixel_rgba(&pixels, tex_w, 95, 20);

    eprintln!("left  (5,20) = {:?}", left);
    eprintln!("right (95,20) = {:?}", right);

    // Left alpha must be low (well below 128).
    assert!(
        left[3] < 64,
        "left edge alpha ({}) should be < 64 (gradient starts near-transparent)", left[3],
    );
    // Right must be near-opaque red.
    assert!(
        right[3] > 200,
        "right edge alpha ({}) should be > 200 (gradient ends near-opaque)", right[3],
    );
    // Red channel must be present (colour modulation applied).
    assert!(
        right[0] > 200,
        "right edge R ({}) should be > 200 (red colour modulation)", right[0],
    );
    // No blue/green bleed at right.
    assert_eq!(right[1], 0, "right edge G must be 0 (red modulation), got {}", right[1]);
    assert_eq!(right[2], 0, "right edge B must be 0 (red modulation), got {}", right[2]);
}

/// Glyph with partial UV rect samples only the centre quarter of the atlas.
///
/// Atlas: 64×64 R8Unorm filled entirely with 255 (fully opaque).
/// UV rect [0.25, 0.25, 0.75, 0.75] → samples centre quarter; all 255.
/// Colour: red (255, 0, 0, 255).
/// Expected: bbox entirely opaque red.
#[test]
#[ignore = "needs gpu adapter"]
fn glyph_skipped_outside_uv_rect() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // All-opaque atlas.
    let atlas_w: u32 = 64;
    let atlas_h: u32 = 64;
    let atlas_data = vec![255u8; (atlas_w * atlas_h) as usize];
    let (_atlas_tex, atlas_view) = upload_r8_atlas(&device, &queue, atlas_w, atlas_h, &atlas_data);

    // Glyph with centre-quarter UV.
    let cmds = vec![
        SceneCmd::glyph(10.0, 10.0, 60.0, 40.0, [255, 0, 0, 255], [0.25, 0.25, 0.75, 0.75]),
    ];

    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &output_view, &atlas_view);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let pixels = readback_texture(&device, &queue, &output_tex, tex_w, tex_h);

    // Centre of bbox: (35, 25).
    let centre = pixel_rgba(&pixels, tex_w, 35, 25);
    eprintln!("centre (35,25) = {:?}", centre);

    // Atlas is all-255 → full alpha → fully opaque red.
    assert_eq!(centre[0], 255, "centre R must be 255 (fully opaque red), got {}", centre[0]);
    assert_eq!(centre[1],   0, "centre G must be 0, got {}", centre[1]);
    assert_eq!(centre[2],   0, "centre B must be 0, got {}", centre[2]);
    assert_eq!(centre[3], 255, "centre alpha must be 255 (all-opaque atlas), got {}", centre[3]);

    // Pixel outside bbox (0,0) must be transparent.
    let outside = pixel_rgba(&pixels, tex_w, 0, 0);
    eprintln!("outside (0,0)  = {:?}", outside);
    assert_eq!(outside, [0, 0, 0, 0], "pixel outside glyph bbox must be transparent, got {:?}", outside);
}
