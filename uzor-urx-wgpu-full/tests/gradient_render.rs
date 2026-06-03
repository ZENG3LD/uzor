//! GPU-gated gradient correctness tests.
//!
//! Tests verify that LinGradient and RadGradient commands produce the
//! expected colour blending at specific pixel positions.
//!
//! Run:
//!   cargo test --test gradient_render -- --ignored --nocapture

use uzor_urx_wgpu_full::{
    SceneCmd, TileBuffers, TilePipeline, TILE_SIZE,
};

const W: u32 = 256;
const H: u32 = 256;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("gradient-render-test-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()
}

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
        label: Some("gradient-render-test-staging"),
        size:  buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("gradient-render-test-readback") },
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

/// LinGradient direction=HORIZONTAL (L→R), red → blue.
///
/// Sample three pixels at y=50 across a 100×100 rect [0,0,100,100]:
///   x=10 (near left):   red channel >> blue channel
///   x=50 (centre):      both channels roughly equal (purple)
///   x=90 (near right):  blue channel >> red channel
#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn linear_gradient_lerps_corners() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None    => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // Red (255,0,0,255) → Blue (0,0,255,255), horizontal, rect [0,0,100,100].
    let cmds = vec![
        SceneCmd::lin_gradient(
            0.0, 0.0, 100.0, 100.0,
            [255, 0, 0, 255],   // red
            [0, 0, 255, 255],   // blue
            uzor_urx_wgpu_full::cmd::lin_dir::HORIZONTAL,
        ),
    ];
    let (pixels, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    let left   = pixel_rgba(&pixels, tex_w, 10, 50);
    let middle = pixel_rgba(&pixels, tex_w, 50, 50);
    let right  = pixel_rgba(&pixels, tex_w, 90, 50);

    eprintln!("left   (10,50) = {:?}", left);
    eprintln!("middle (50,50) = {:?}", middle);
    eprintln!("right  (90,50) = {:?}", right);

    // Left: more red than middle red.
    assert!(left[0] > middle[0],
        "left red ({}) should exceed middle red ({})", left[0], middle[0]);
    // Right: more blue than middle blue.
    assert!(right[2] > middle[2],
        "right blue ({}) should exceed middle blue ({})", right[2], middle[2]);
    // Middle: purple-ish — both channels present.
    assert!(middle[0] > 50 && middle[2] > 50,
        "middle should be purple-ish (r={}, b={})", middle[0], middle[2]);
    // All alpha = 255.
    assert_eq!(left[3],   255, "left alpha must be 255");
    assert_eq!(middle[3], 255, "middle alpha must be 255");
    assert_eq!(right[3],  255, "right alpha must be 255");
}

/// RadGradient white (inner) → black (outer) over 200×200 rect.
///
/// Centre pixel should be near-white (R+G+B all high).
/// A pixel close to the corner should be near-black (R+G+B all low).
#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn radial_gradient_inner_vs_outer() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None    => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    // White inner → Black outer, rect [0,0,200,200].
    let cmds = vec![
        SceneCmd::rad_gradient(
            0.0, 0.0, 200.0, 200.0,
            [255, 255, 255, 255], // inner = white
            [0,   0,   0,   255], // outer = black
        ),
    ];
    let (pixels, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    // Centre of 200×200 bbox = (100, 100) in pixel space.
    let centre = pixel_rgba(&pixels, tex_w, 100, 100);
    // Corner-ish pixel — at (195, 195), far from centre.
    let corner = pixel_rgba(&pixels, tex_w, 195, 195);

    eprintln!("centre (100,100) = {:?}", centre);
    eprintln!("corner (195,195) = {:?}", corner);

    // Centre: near white — all channels high.
    assert!(centre[0] > 200, "centre R ({}) should be > 200 (near white)", centre[0]);
    assert!(centre[1] > 200, "centre G ({}) should be > 200 (near white)", centre[1]);
    assert!(centre[2] > 200, "centre B ({}) should be > 200 (near white)", centre[2]);

    // Corner: near black — all channels low.
    assert!(corner[0] < 55, "corner R ({}) should be < 55 (near black)", corner[0]);
    assert!(corner[1] < 55, "corner G ({}) should be < 55 (near black)", corner[1]);
    assert!(corner[2] < 55, "corner B ({}) should be < 55 (near black)", corner[2]);

    // Alpha = 255 everywhere.
    assert_eq!(centre[3], 255, "centre alpha must be 255");
    assert_eq!(corner[3], 255, "corner alpha must be 255");
}
