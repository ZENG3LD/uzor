//! Pixel-level correctness tests for the fine raster pass.
//!
//! Checks:
//!   1. A pixel inside a rect gets the rect's colour.
//!   2. A pixel outside every rect stays transparent (0,0,0,0).
//!   3. Painter order: front rect (higher cmd index) wins at overlap pixel.
//!
//! Run:
//!   cargo test --test pixel_render -- --ignored --nocapture

use uzor_urx_wgpu_full::{
    SceneCmd, TileBuffers, TilePipeline, TILE_SIZE,
};

const W: u32 = 64;
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
            label: Some("pixel-render-test-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()
}

/// Readback rgba8unorm texture into a flat Vec<u8> (RGBA, row-major).
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
        label: Some("pixel-render-test-staging"),
        size:  buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("pixel-render-test-readback") },
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

fn run_pipeline(
    device: &wgpu::Device,
    queue:  &wgpu::Queue,
    cmds:   &[SceneCmd],
) -> (Vec<u8>, u32, u32) {
    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(device);

    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    pipeline.dispatch_full(device, queue, &mut enc, &bufs, cmds, &output_view, &dummy_atlas_view);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let pixels = readback_texture(device, queue, &output_tex, tex_w, tex_h);
    (pixels, tex_w, tex_h)
}

/// Pixel inside a red rect must be (255, 0, 0, 255).
#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn pixel_inside_rect_matches_rect_colour() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // Red rect fills pixels [10..20, 10..20].
    let cmds = vec![
        SceneCmd::rect(10.0, 10.0, 20.0, 20.0, [255, 0, 0, 255]),
    ];
    let (pixels, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    // Centre pixel (15, 15) must be fully red.
    let px = pixel_rgba(&pixels, tex_w, 15, 15);
    eprintln!("pixel (15,15) = {:?}", px);
    assert_eq!(px, [255, 0, 0, 255], "pixel inside red rect must be red, got {:?}", px);
}

/// Pixel outside every rect must stay transparent (0, 0, 0, 0).
///
/// The fine pass initialises the accumulator to (0,0,0,0) and only
/// writes when a cmd covers the pixel — so uncovered pixels stay zero.
#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn pixel_outside_all_rects_is_transparent() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // Red rect only at top-left corner [0..10, 0..10].
    let cmds = vec![
        SceneCmd::rect(0.0, 0.0, 10.0, 10.0, [255, 0, 0, 255]),
    ];
    let (pixels, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    // Pixel at (50, 50) is outside the rect — expect (0, 0, 0, 0).
    let px = pixel_rgba(&pixels, tex_w, 50, 50);
    eprintln!("pixel (50,50) = {:?}", px);
    assert_eq!(px, [0, 0, 0, 0], "pixel outside all rects must be transparent, got {:?}", px);
}

/// Painter order: front rect (higher cmd index) wins at overlap pixel.
///
/// cmd 0 = blue rect covering [10..50, 10..50].
/// cmd 1 = red  rect covering [20..40, 20..40] (fully inside blue).
/// Pixel (30, 30) is inside both — must be red (opaque front wins).
#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn painter_order_front_rect_wins_at_overlap() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // Blue painted first (cmd 0), red on top (cmd 1).
    let cmds = vec![
        SceneCmd::rect(10.0, 10.0, 50.0, 50.0, [  0,   0, 255, 255]), // blue
        SceneCmd::rect(20.0, 20.0, 40.0, 40.0, [255,   0,   0, 255]), // red (on top)
    ];
    let (pixels, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    // Overlap pixel (30, 30): red is on top of blue → expect pure red.
    let overlap_px = pixel_rgba(&pixels, tex_w, 30, 30);
    eprintln!("overlap pixel (30,30) = {:?}", overlap_px);
    assert_eq!(overlap_px, [255, 0, 0, 255],
        "overlap pixel must be red (front rect wins), got {:?}", overlap_px);

    // Non-overlap pixel (15, 15): only blue covers it.
    let blue_px = pixel_rgba(&pixels, tex_w, 15, 15);
    eprintln!("blue-only pixel (15,15) = {:?}", blue_px);
    assert_eq!(blue_px, [0, 0, 255, 255],
        "pixel only under blue rect must be blue, got {:?}", blue_px);
}
