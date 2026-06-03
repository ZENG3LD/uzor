//! GPU correctness test for `CmdKind::Stroke`.
//!
//! Validates the full dispatch+blit pipeline produces correct pixels for
//! axis-aligned and diagonal stroke segments, with butt / round / square
//! cap kinds.
//!
//! Run:
//!   cargo test -p uzor-urx-wgpu-full --test stroke_render \
//!     -- --ignored --nocapture

use uzor_urx_wgpu_full::cmd::cap_kind;
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
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label:                 Some("stroke-render-test"),
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
        label:              Some("stroke-readback"),
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
fn at_bgra(buf: &[u8], tex_w: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * tex_w + x) * 4) as usize;
    [buf[idx], buf[idx+1], buf[idx+2], buf[idx+3]]
}

fn run_pipeline(
    device: &wgpu::Device, queue: &wgpu::Queue, cmds: &[SceneCmd],
) -> (Vec<u8>, u32, u32) {
    let (bufs, _storage_tex, storage_view) =
        TileBuffers::with_output_texture(device, cmds.len().max(1) as u32, W, H);
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("stroke-surface"),
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
        &bufs, cmds,
        &storage_view, &blit, &surface_view,
        tex_w, tex_h, &dummy_atlas_view,
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    (readback_bgra(device, queue, &surface_tex, tex_w, tex_h), tex_w, tex_h)
}

/// Horizontal red stroke 2px wide on row y=32 should paint red pixels
/// on the line AND leave row y=10 transparent.
#[test]
#[ignore = "needs gpu adapter"]
fn stroke_horizontal_paints_along_segment() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    let cmds = vec![
        SceneCmd::stroke(8.0, 32.0, 56.0, 32.0, 4.0, [255, 0, 0, 255], cap_kind::BUTT),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    // On the stroke centre line (y=32): pixel (30, 32) must be red.
    let on  = at_bgra(&px, tex_w, 30, 32);
    eprintln!("on-line   (30,32) bgra = {:?}", on);
    assert_eq!(on[2], 255, "red channel on stroke centre line, got {:?}", on);
    assert!(on[3] > 200, "alpha should be near 1 on stroke centre, got {:?}", on);

    // Well above stroke: pixel (30, 10) must be transparent.
    let off = at_bgra(&px, tex_w, 30, 10);
    eprintln!("off-line  (30,10) bgra = {:?}", off);
    assert_eq!(off[3], 0, "alpha should be 0 well above stroke, got {:?}", off);
}

/// Diagonal stroke ~45° from (8,8) to (56,56) width 4.
/// Centre pixel (32, 32) should be on the stroke; (8, 56) should be off.
#[test]
#[ignore = "needs gpu adapter"]
fn stroke_diagonal_paints_along_segment() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    let cmds = vec![
        SceneCmd::stroke(8.0, 8.0, 56.0, 56.0, 4.0, [0, 200, 0, 255], cap_kind::ROUND),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds);

    let on = at_bgra(&px, tex_w, 32, 32);
    eprintln!("diagonal on  (32,32) bgra = {:?}", on);
    // Green channel premultiplied by alpha — non-zero.
    assert!(on[1] > 150, "green channel on diagonal centre, got {:?}", on);

    let off = at_bgra(&px, tex_w, 8, 56);
    eprintln!("diagonal off (8,56)  bgra = {:?}", off);
    assert_eq!(off[3], 0, "pixel far off the diagonal must be transparent");
}

/// Butt-capped stroke must NOT paint past its endpoints. Round cap MUST.
///
/// Probe just past the stroke's end at p1=(40, 32) width=8 → half_w=4.
/// Pixel (42, 32) is 2px past the end ON the centre line:
///   - butt → must be transparent (alpha=0)
///   - round → must be visible (alpha>0)
#[test]
#[ignore = "needs gpu adapter"]
fn stroke_caps_differ_past_endpoint() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    // BUTT
    let cmds_butt = vec![
        SceneCmd::stroke(8.0, 32.0, 40.0, 32.0, 8.0, [0, 0, 255, 255], cap_kind::BUTT),
    ];
    let (px_b, tex_w, _) = run_pipeline(&device, &queue, &cmds_butt);
    let past_b = at_bgra(&px_b, tex_w, 42, 32);
    eprintln!("BUTT past-end  (42,32) bgra = {:?}", past_b);
    assert_eq!(past_b[3], 0, "BUTT cap must not paint past endpoint, got {:?}", past_b);

    // ROUND
    let cmds_round = vec![
        SceneCmd::stroke(8.0, 32.0, 40.0, 32.0, 8.0, [0, 0, 255, 255], cap_kind::ROUND),
    ];
    let (px_r, _, _) = run_pipeline(&device, &queue, &cmds_round);
    let past_r = at_bgra(&px_r, tex_w, 42, 32);
    eprintln!("ROUND past-end (42,32) bgra = {:?}", past_r);
    assert!(past_r[3] > 100,
        "ROUND cap must paint past endpoint within half-width radius, got {:?}", past_r);
}
