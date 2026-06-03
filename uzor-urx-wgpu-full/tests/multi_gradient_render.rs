//! GPU correctness test for `CmdKind::MultiLinGradient` (N-stop linear).
//!
//! Validates 3-stop and 5-stop gradients with correct color interpolation
//! across the gradient axis. The stops live in the shared `path_points`
//! buffer as packed `(position, bitcast<f32>(packed_rgba))` pairs.
//!
//! Run:
//!   cargo test -p uzor-urx-wgpu-full --test multi_gradient_render \
//!     -- --ignored --nocapture

use uzor_urx_wgpu_full::cmd::lin_dir;
use uzor_urx_wgpu_full::{
    pack_gradient_stop, BlitPipeline, DEFAULT_PATH_POINTS_CAP, SceneCmd,
    TileBuffers, TilePipeline, TILE_SIZE,
};

const W: u32 = 128;
const H: u32 = 32;
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label:                 Some("multi-grad-test"),
        required_features:     wgpu::Features::empty(),
        required_limits:       wgpu::Limits::default(),
        memory_hints:          wgpu::MemoryHints::default(),
        trace:                 wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn readback_bgra(
    device: &wgpu::Device, queue: &wgpu::Queue,
    texture: &wgpu::Texture, tex_w: u32, tex_h: u32,
) -> Vec<u8> {
    let aligned_stride = (tex_w * 4 + 255) & !255;
    let buf_size = (aligned_stride * tex_h) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("mg-readback"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0, bytes_per_row: Some(aligned_stride), rows_per_image: Some(tex_h),
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
    drop(raw); staging.unmap(); out
}

#[inline]
fn at_rgb(buf: &[u8], tex_w: u32, x: u32, y: u32) -> [u8; 3] {
    let idx = ((y * tex_w + x) * 4) as usize;
    [buf[idx+2], buf[idx+1], buf[idx]]
}

fn run_pipeline(
    device:      &wgpu::Device,
    queue:       &wgpu::Queue,
    cmds:        &[SceneCmd],
    path_points: &[[f32; 2]],
) -> (Vec<u8>, u32, u32) {
    let bufs = TileBuffers::allocate_with(
        device, cmds.len().max(1) as u32, W, H, DEFAULT_PATH_POINTS_CAP,
    );
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let storage_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mg-storage"),
        size: wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
             | wgpu::TextureUsages::COPY_SRC
             | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let storage_view = storage_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mg-surface"),
        size: wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format:    SURFACE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let surface_view = surface_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let tile = TilePipeline::new(device);
    let blit = BlitPipeline::new(device, SURFACE_FORMAT);
    let (_dum, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    tile.render_to_target(
        device, queue, &mut enc,
        &bufs, cmds, path_points,
        &storage_view, &blit, &surface_view,
        tex_w, tex_h, &dummy_atlas_view,
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    (readback_bgra(device, queue, &surface_tex, tex_w, tex_h), tex_w, tex_h)
}

/// 3-stop horizontal gradient: red → green → blue across 128 px.
///   t=0   → red    (255, 0, 0)
///   t=0.5 → green  (0, 255, 0)
///   t=1   → blue   (0, 0, 255)
/// Sample at three reference x positions inside the rect (16, 64, 112)
/// and check the channel that should dominate.
#[test]
#[ignore = "needs gpu adapter"]
fn three_stop_horizontal_red_green_blue() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    let stops = vec![
        pack_gradient_stop(0.0, [255,   0,   0, 255]),
        pack_gradient_stop(0.5, [  0, 255,   0, 255]),
        pack_gradient_stop(1.0, [  0,   0, 255, 255]),
    ];
    // Rect spans 0..W, 8..24 vertically.
    let cmds = vec![
        SceneCmd::multi_lin_gradient(
            [0.0, 8.0, W as f32, 24.0],
            lin_dir::HORIZONTAL,
            0,
            stops.len() as u32,
        ),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &stops);

    // Sample at y=16 (middle of rect), x = 4 / 64 / 124 ≈ t=0.03 / 0.5 / 0.97.
    let left   = at_rgb(&px, tex_w,   4, 16);
    let middle = at_rgb(&px, tex_w,  64, 16);
    let right  = at_rgb(&px, tex_w, 124, 16);

    eprintln!("left   (t≈0)   rgb = {:?}", left);
    eprintln!("middle (t≈0.5) rgb = {:?}", middle);
    eprintln!("right  (t≈1)   rgb = {:?}", right);

    // Left should be red-dominant.
    assert!(left[0]  > 200, "left must be red-dominant, got {:?}", left);
    assert!(left[1]  < 60,  "left green channel should be low, got {:?}", left);
    // Middle should be green-dominant.
    assert!(middle[1] > 200, "middle must be green-dominant, got {:?}", middle);
    // Right should be blue-dominant.
    assert!(right[2]  > 200, "right must be blue-dominant, got {:?}", right);
    assert!(right[0]  < 60,  "right red channel should be low, got {:?}", right);
}

/// 5-stop horizontal gradient with non-uniform positions.
/// stops: 0.0 black, 0.1 red, 0.5 yellow, 0.9 cyan, 1.0 white.
/// Probe positions are chosen so each is firmly inside a single segment.
#[test]
#[ignore = "needs gpu adapter"]
fn five_stop_horizontal_non_uniform() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    let stops = vec![
        pack_gradient_stop(0.0, [  0,   0,   0, 255]),
        pack_gradient_stop(0.1, [255,   0,   0, 255]),
        pack_gradient_stop(0.5, [255, 255,   0, 255]),
        pack_gradient_stop(0.9, [  0, 255, 255, 255]),
        pack_gradient_stop(1.0, [255, 255, 255, 255]),
    ];
    let cmds = vec![
        SceneCmd::multi_lin_gradient(
            [0.0, 8.0, W as f32, 24.0],
            lin_dir::HORIZONTAL,
            0,
            stops.len() as u32,
        ),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &stops);

    // x=64 (t≈0.5) → yellow stop exactly.
    let yellow = at_rgb(&px, tex_w, 64, 16);
    eprintln!("t≈0.5 (5-stop yellow) rgb = {:?}", yellow);
    assert!(yellow[0] > 200 && yellow[1] > 200 && yellow[2] < 60,
        "must be yellow at the middle stop, got {:?}", yellow);

    // x=12 (t≈0.094) → just before red stop, still very dark.
    let near_black = at_rgb(&px, tex_w, 12, 16);
    eprintln!("t≈0.094 (near black) rgb = {:?}", near_black);
    // Lerp 0..0.1 is black to red — at t=0.094 we're 94% red.
    assert!(near_black[0] > 200, "near-red position must be red-dominant, got {:?}", near_black);

    // x=125 (t≈0.977) → between cyan and white.
    let near_white = at_rgb(&px, tex_w, 125, 16);
    eprintln!("t≈0.977 (near white) rgb = {:?}", near_white);
    assert!(near_white[0] > 150 && near_white[1] > 200 && near_white[2] > 200,
        "near-white must have all channels high, got {:?}", near_white);
}

/// Edge case: t outside any defined range falls back to first / last
/// stop colour (clamping behaviour).
#[test]
#[ignore = "needs gpu adapter"]
fn t_outside_stops_clamps_to_first_or_last() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    // Stops only cover [0.3, 0.7] — outside that range pixels should
    // clamp to the first / last stop's colour.
    let stops = vec![
        pack_gradient_stop(0.3, [255,   0,   0, 255]),  // red
        pack_gradient_stop(0.7, [  0,   0, 255, 255]),  // blue
    ];
    let cmds = vec![
        SceneCmd::multi_lin_gradient(
            [0.0, 8.0, W as f32, 24.0],
            lin_dir::HORIZONTAL,
            0,
            stops.len() as u32,
        ),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &stops);

    // x=4 → t≈0.03 → before first stop → red.
    let pre  = at_rgb(&px, tex_w, 4,   16);
    // x=124 → t≈0.97 → after last stop → blue.
    let post = at_rgb(&px, tex_w, 124, 16);
    eprintln!("pre-first  rgb = {:?}", pre);
    eprintln!("post-last  rgb = {:?}", post);

    assert!(pre[0]  > 200 && pre[2]  < 60, "pre-first must be red, got {:?}", pre);
    assert!(post[2] > 200 && post[0] < 60, "post-last must be blue, got {:?}", post);
}
