//! Engine-level GPU test for `Backend::FullGpu`.
//!
//! Validates that `UrxEngine::render(RenderTarget::FullGpu { ... })` end-
//! to-end produces correct pixels for a region whose scene contains a
//! solid-colour rect. Region bounds are in screen-space; engine MUST shift
//! the scene to bounds.origin before encoding so the cmd appears at the
//! right screen coordinate.
//!
//! Run with:
//!   cargo test -p uzor-urx-engine --features full-gpu-backend \
//!     --test full_gpu_engine -- --ignored --nocapture
//!
//! Requires a real wgpu adapter; skipped on hosts without one.

#![cfg(feature = "full-gpu-backend")]

use uzor_urx_core::math::{Color, Rect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_engine::cadence::RenderCadence;
use uzor_urx_engine::engine::{Backend, RenderTarget, RenderError, UrxEngine, WorkloadHint};
use uzor_urx_wgpu_full::{BlitPipeline, TileBuffers, TilePipeline, TILE_SIZE};

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
        label:                 Some("engine-full-gpu-test"),
        required_features:     wgpu::Features::empty(),
        required_limits:       wgpu::Limits::default(),
        memory_hints:          wgpu::MemoryHints::default(),
        trace:                 wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn readback_bgra(
    device:  &wgpu::Device,
    queue:   &wgpu::Queue,
    texture: &wgpu::Texture,
    tex_w:   u32,
    tex_h:   u32,
) -> Vec<u8> {
    let aligned_stride = (tex_w * 4 + 255) & !255;
    let buf_size = (aligned_stride * tex_h) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("engine-full-gpu-readback"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
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
                offset: 0,
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

#[inline]
fn pixel_rgb_from_bgra(buf: &[u8], tex_w: u32, x: u32, y: u32) -> [u8; 3] {
    let idx = ((y * tex_w + x) * 4) as usize;
    [buf[idx + 2], buf[idx + 1], buf[idx]]
}

/// FullGpu engine renders a single red rect region into the correct
/// screen-space pixel. Region bounds (10..50, 10..50); scene fills
/// the full region; expected red pixel at the centre (30, 30).
#[test]
#[ignore = "needs gpu adapter"]
fn full_gpu_engine_renders_single_red_region() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    let mut engine = UrxEngine::new_full_gpu(W, H);
    let mut scene = Scene::new();
    // Scene-local coordinates: fills the FULL region (40×40 area).
    scene.fill_rect_solid(Rect::new(0.0, 0.0, 40.0, 40.0), Color::rgba8(255, 0, 0, 255));
    engine.upsert_region(
        RegionId(0),
        scene,
        Rect::new(10.0, 10.0, 50.0, 50.0),
        RenderCadence::Static,
    );

    let pipeline = TilePipeline::new(&device);
    let blit     = BlitPipeline::new(&device, SURFACE_FORMAT);
    let (bufs, _storage_tex, storage_view) =
        TileBuffers::with_output_texture(&device, 16, W, H);
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;

    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:               Some("engine-full-gpu-surface"),
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
    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(&device);
    let (_dum_img, dummy_img_view)     = TilePipeline::dummy_image_atlas(&device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let stats = engine.render(RenderTarget::FullGpu {
        pipeline:         &pipeline,
        blit:             &blit,
        bufs:             &bufs,
        device:           &device,
        queue:            &queue,
        encoder:          &mut enc,
        storage_view:     &storage_view,
        target_view:      &surface_view,
        glyph_atlas_view: &dummy_atlas_view,
        image_atlas_view: &dummy_img_view,
        src_w:            tex_w,
        src_h:            tex_h,
    }).expect("engine render OK");
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    eprintln!("engine stats = {:?}", stats);
    let pixels = readback_bgra(&device, &queue, &surface_tex, tex_w, tex_h);

    // Centre of the rect in SCREEN space = (30, 30).
    let centre = pixel_rgb_from_bgra(&pixels, tex_w, 30, 30);
    eprintln!("centre pixel (30,30) rgb = {:?}", centre);
    assert_eq!(centre, [255, 0, 0],
        "engine FullGpu render must produce red rect at screen centre (30,30); got {:?}", centre);

    // Pixel well outside the region must stay transparent.
    let idx = ((58 * tex_w + 58) * 4) as usize;
    let outside_alpha = pixels[idx + 3];
    eprintln!("pixel (58,58) alpha = {}", outside_alpha);
    assert_eq!(outside_alpha, 0,
        "pixel outside region must be transparent");

    assert_eq!(stats.regions_total, 1, "1 region in engine");
    assert_eq!(stats.cache_misses, 1, "FullGpu treats every region as miss");
}

/// FullGpu returns CmdBufferTooSmall when supplied bufs is sized below
/// the cmd count produced by encoding.
#[test]
#[ignore = "needs gpu adapter"]
fn full_gpu_engine_returns_too_small_buf_error() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => { eprintln!("no wgpu adapter — skipping"); return; }
    };

    let mut engine = UrxEngine::new_full_gpu(W, H);
    // 16 regions → 16 cmds. bufs allocated with cmds_n=4 → overflow.
    for i in 0..16 {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::rgba8(((i * 16) & 0xff) as u8, 0, 0, 255));
        engine.upsert_region(
            RegionId(i as u64),
            scene,
            Rect::new((i % 4) as f64 * 12.0, (i / 4) as f64 * 12.0,
                       (i % 4) as f64 * 12.0 + 10.0, (i / 4) as f64 * 12.0 + 10.0),
            RenderCadence::Static,
        );
    }

    let pipeline = TilePipeline::new(&device);
    let blit     = BlitPipeline::new(&device, SURFACE_FORMAT);
    let (bufs, _storage_tex, storage_view) =
        TileBuffers::with_output_texture(&device, 4, W, H);
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;

    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:               Some("engine-too-small-surface"),
        size:                wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
        mip_level_count:     1,
        sample_count:        1,
        dimension:           wgpu::TextureDimension::D2,
        format:              SURFACE_FORMAT,
        usage:               wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats:        &[],
    });
    let surface_view = surface_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(&device);
    let (_dum_img, dummy_img_view)     = TilePipeline::dummy_image_atlas(&device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let res = engine.render(RenderTarget::FullGpu {
        pipeline:         &pipeline,
        blit:             &blit,
        bufs:             &bufs,
        device:           &device,
        queue:            &queue,
        encoder:          &mut enc,
        storage_view:     &storage_view,
        target_view:      &surface_view,
        glyph_atlas_view: &dummy_atlas_view,
        image_atlas_view: &dummy_img_view,
        src_w:            tex_w,
        src_h:            tex_h,
    });
    let _ = queue; // unused on error path

    match res {
        Err(RenderError::CmdBufferTooSmall { needed, available }) => {
            eprintln!("got expected error: needed={needed} available={available}");
            assert_eq!(needed, 16);
            assert_eq!(available, 4);
        }
        other => panic!("expected CmdBufferTooSmall error, got {:?}", other),
    }
}

/// Backend::auto picks FullGpu when heavy_compute hint is set + GPU available.
#[test]
fn backend_auto_picks_full_gpu_on_heavy_compute_hint() {
    let hint = WorkloadHint {
        gpu_available: true,
        heavy_compute: true,
        region_count:  10,
        total_pixels:  1_000_000,
        ..Default::default()
    };
    let picked = Backend::auto(hint);
    assert_eq!(picked, Backend::FullGpu,
        "heavy_compute + gpu_available must pick FullGpu, got {:?}", picked);
}

/// Backend::auto does NOT pick FullGpu when heavy_compute is false.
#[test]
fn backend_auto_does_not_pick_full_gpu_without_hint() {
    let hint = WorkloadHint {
        gpu_available: true,
        heavy_compute: false,
        region_count:  10,
        total_pixels:  1_000_000,
        retained:      true,
        ..Default::default()
    };
    let picked = Backend::auto(hint);
    assert_ne!(picked, Backend::FullGpu,
        "no heavy_compute hint → must NOT pick FullGpu, got {:?}", picked);
}
