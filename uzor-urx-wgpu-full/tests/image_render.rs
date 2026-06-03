//! GPU correctness test for `CmdKind::Image`.
//!
//! Validates that the bound RGBA8 atlas is sampled with correct UV
//! mapping (full-rect + sub-rect crop) and the tint modulation
//! multiplies the texel colour.
//!
//! Run:
//!   cargo test -p uzor-urx-wgpu-full --test image_render \
//!     -- --ignored --nocapture

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
        label:                 Some("image-render-test"),
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
        label:              Some("img-readback"),
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

/// Build an RGBA8 atlas: 4-quadrant solid colours
///   top-left  red,    top-right    green,
///   bot-left  blue,   bot-right    yellow.
/// 32×32 px so each quadrant is 16×16.
fn build_quadrant_atlas(
    device: &wgpu::Device, queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    const ATLAS_PX: u32 = 32;
    let mut pixels = Vec::with_capacity((ATLAS_PX * ATLAS_PX * 4) as usize);
    for y in 0..ATLAS_PX {
        for x in 0..ATLAS_PX {
            let is_right  = x >= ATLAS_PX / 2;
            let is_bottom = y >= ATLAS_PX / 2;
            let rgba = match (is_right, is_bottom) {
                (false, false) => [255,   0,   0, 255], // TL = red
                (true,  false) => [  0, 255,   0, 255], // TR = green
                (false, true)  => [  0,   0, 255, 255], // BL = blue
                (true,  true)  => [255, 255,   0, 255], // BR = yellow
            };
            pixels.extend_from_slice(&rgba);
        }
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("quad-atlas"),
        size: wgpu::Extent3d { width: ATLAS_PX, height: ATLAS_PX, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row:  Some(ATLAS_PX * 4),
            rows_per_image: Some(ATLAS_PX),
        },
        wgpu::Extent3d { width: ATLAS_PX, height: ATLAS_PX, depth_or_array_layers: 1 },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn run_pipeline(
    device:           &wgpu::Device,
    queue:            &wgpu::Queue,
    cmds:             &[SceneCmd],
    image_atlas_view: &wgpu::TextureView,
) -> (Vec<u8>, u32, u32) {
    let (bufs, _storage_tex, storage_view) =
        TileBuffers::with_output_texture(device, cmds.len().max(1) as u32, W, H);
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("img-surface"),
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
        &bufs, cmds, &[],
        &storage_view, &blit, &surface_view,
        tex_w, tex_h, &dummy_atlas_view, image_atlas_view,
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    (readback_bgra(device, queue, &surface_tex, tex_w, tex_h), tex_w, tex_h)
}

/// Full-atlas Image cmd into a 32×32 rect — each of the 4 quadrants
/// of the rect must match the corresponding atlas quadrant colour.
#[test]
#[ignore = "needs gpu adapter"]
fn image_full_uv_maps_quadrant_colors() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    let (_atlas_tex, atlas_view) = build_quadrant_atlas(&device, &queue);

    // 32×32 rect from (16, 16) to (48, 48), full atlas UV.
    let cmds = vec![
        SceneCmd::image(16.0, 16.0, 48.0, 48.0, [255, 255, 255, 255], [0.0, 0.0, 1.0, 1.0]),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &atlas_view);

    // Probe the centre of each quadrant: (24, 24), (40, 24), (24, 40), (40, 40)
    let tl = at_rgb(&px, tex_w, 24, 24);
    let tr = at_rgb(&px, tex_w, 40, 24);
    let bl = at_rgb(&px, tex_w, 24, 40);
    let br = at_rgb(&px, tex_w, 40, 40);
    eprintln!("tl(24,24)={:?}  tr(40,24)={:?}  bl(24,40)={:?}  br(40,40)={:?}",
              tl, tr, bl, br);
    assert!(tl[0] > 200 && tl[1] < 80,  "TL must be red, got {:?}", tl);
    assert!(tr[1] > 200 && tr[0] < 80,  "TR must be green, got {:?}", tr);
    assert!(bl[2] > 200 && bl[0] < 80,  "BL must be blue, got {:?}", bl);
    assert!(br[0] > 200 && br[1] > 200 && br[2] < 80, "BR must be yellow, got {:?}", br);
}

/// Sub-UV: sample only top-right quadrant (UV [0.5..1.0, 0..0.5]) →
/// entire destination rect should be green.
#[test]
#[ignore = "needs gpu adapter"]
fn image_sub_uv_rect_crops_correctly() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    let (_atlas_tex, atlas_view) = build_quadrant_atlas(&device, &queue);

    // 16×16 dest rect, sampled from TR quadrant of the atlas only.
    let cmds = vec![
        SceneCmd::image(16.0, 16.0, 32.0, 32.0, [255, 255, 255, 255],
                        [0.5, 0.0, 1.0, 0.5]),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &atlas_view);

    // Whole dest rect must be green.
    let p = at_rgb(&px, tex_w, 24, 24);
    eprintln!("centre (24,24) = {:?}", p);
    assert!(p[1] > 200 && p[0] < 80, "must be green (cropped TR), got {:?}", p);
}

/// Tint modulation: half-alpha red tint over a green pixel = green
/// dimmed proportional to tint.a, premultiplied output keeps green high
/// in normalised g channel.
///
/// Tint multiplies the texel rgba component-wise. With tint=[255,0,0,128]:
///   texel = (0, 1, 0, 1)  // green from BL quadrant via [0,0.5,0.5,1] uv? — use full TR (green)
///   src   = (0*1, 1*0, 0*0, 1*0.5) = (0, 0, 0, 0.5) — premultiplied
///   on transparent bg: blends to (0, 0, 0, 0.5)
///
/// Easier assertion: tint=[255,255,255,255] (white) → unchanged texel;
/// tint=[0,0,0,255] (black) → fully black with alpha kept.
#[test]
#[ignore = "needs gpu adapter"]
fn image_tint_black_kills_color_keeps_alpha() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    let (_atlas_tex, atlas_view) = build_quadrant_atlas(&device, &queue);

    // Full atlas, black tint.
    let cmds = vec![
        SceneCmd::image(16.0, 16.0, 48.0, 48.0, [0, 0, 0, 255], [0.0, 0.0, 1.0, 1.0]),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &atlas_view);

    let p = at_rgb(&px, tex_w, 24, 24);
    let alpha_idx = ((24u32 * tex_w + 24u32) * 4) as usize;
    let alpha = px[alpha_idx + 3];
    eprintln!("black-tinted TL (24,24) rgb = {:?}  α = {}", p, alpha);
    // Black tint × red texel = black; alpha 1 × 1 = full alpha.
    assert!(p[0] < 20 && p[1] < 20 && p[2] < 20, "black tint must zero rgb, got {:?}", p);
    assert!(alpha > 200, "alpha must stay near 1 (tint.a=1), got {}", alpha);
}
