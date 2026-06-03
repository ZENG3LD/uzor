//! Surface-render demo — compute dispatch + blit pass in one encoder.
//!
//! Demonstrates the full pipeline for rendering to a swapchain-like surface:
//!
//!   1. Scene cmds → internal rgba8unorm storage texture via `dispatch_full`
//!   2. Blit that texture → bgra8unorm render-attachment texture via `BlitPipeline`
//!   3. Read back the bgra8unorm result and verify known-colour rects.
//!
//! Both passes are recorded into ONE `CommandEncoder` and submitted in a
//! single `queue.submit` call — no intermediate readback on the GPU path.
//!
//! Timing printed separately: dispatch+fine vs blit pass.
//!
//! Run:
//!   cargo run -p uzor-urx-wgpu-full --example surface_render_demo --release

use uzor_urx_wgpu_full::{
    cmd::SceneCmd,
    tile::{BlitPipeline, TileBuffers, TilePipeline, TILE_SIZE},
};

const W: u32 = 1920;
const H: u32 = 1080;

// bgra8unorm — typical swapchain format on Windows/macOS.
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[surface-render-demo] init wgpu device");
    let Some((device, queue, info)) = init_device() else {
        eprintln!("[surface-render-demo] no GPU adapter — exit 1");
        return 1;
    };
    println!("[surface-render-demo] adapter: {} ({:?})", info.name, info.backend);

    // Same known-colour scene as pixel_render_demo.
    let cmds: Vec<SceneCmd> = vec![
        SceneCmd::rect(100.0, 100.0, 300.0, 300.0, [255,   0,   0, 255]), // red
        SceneCmd::rect(500.0, 300.0, 700.0, 500.0, [  0, 255,   0, 255]), // green
        SceneCmd::rect(900.0, 500.0, 1100.0, 700.0, [  0,   0, 255, 255]), // blue
        SceneCmd::rect(1500.0, 700.0, 1700.0, 900.0, [255, 255, 255, 255]), // white
    ];
    println!("[surface-render-demo] scene: {} cmds", cmds.len());

    // Internal storage texture (rgba8unorm) — compute writes here.
    let (bufs, _storage_tex, storage_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;

    println!("[surface-render-demo] storage tex: {}x{} (tile-aligned)", tex_w, tex_h);

    // Surface target texture (bgra8unorm) — blit writes here.
    // In a real consumer this would be the swapchain surface texture.
    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:               Some("surface-render-demo-surface"),
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

    let tile_pipeline = TilePipeline::new(&device);
    let blit_pipeline = BlitPipeline::new(&device, SURFACE_FORMAT);

    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(&device);

    // — Timing: dispatch+fine pass only —
    let t_dispatch = std::time::Instant::now();
    {
        let mut enc = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("surface-render-demo-dispatch-timing") },
        );
        tile_pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &storage_view, &dummy_atlas_view);
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    let dispatch_ms = t_dispatch.elapsed().as_secs_f64() * 1000.0;

    // — Timing: blit pass only —
    let t_blit = std::time::Instant::now();
    {
        let mut enc = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("surface-render-demo-blit-timing") },
        );
        blit_pipeline.blit(&device, &mut enc, &storage_view, &surface_view, tex_w, tex_h, &queue);
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    let blit_ms = t_blit.elapsed().as_secs_f64() * 1000.0;

    println!("[surface-render-demo] dispatch+fine: {dispatch_ms:.3} ms");
    println!("[surface-render-demo] blit pass:     {blit_ms:.3} ms");

    // — Combined pass: ONE encoder, ONE submit (the real consumer path) —
    println!("[surface-render-demo] combined pass (one submit) ...");
    let t_combined = std::time::Instant::now();
    {
        let mut enc = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("surface-render-demo-combined") },
        );
        tile_pipeline.render_to_target(
            &device, &queue, &mut enc,
            &bufs, &cmds,
            &storage_view, &blit_pipeline, &surface_view,
            tex_w, tex_h,
            &dummy_atlas_view,
        );
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    println!("[surface-render-demo] combined (one submit): {:.3} ms",
        t_combined.elapsed().as_secs_f64() * 1000.0);

    // Readback from the bgra8unorm surface texture.
    let pixels = readback_texture_bgra(&device, &queue, &surface_tex, tex_w, tex_h);
    println!("[surface-render-demo] readback: {} bytes ({} x {})", pixels.len(), tex_w, tex_h);

    // Verify known-colour rects (bgra byte order: B,G,R,A).
    // bgra8unorm stores bytes as [B, G, R, A].
    let checks: &[(&str, u32, u32, [u8; 3])] = &[
        ("red rect   centre", 200, 200, [255,   0,   0]),
        ("green rect centre", 600, 400, [  0, 255,   0]),
        ("blue rect  centre", 1000, 600, [  0,   0, 255]),
        ("white rect centre", 1600, 800, [255, 255, 255]),
    ];
    let mut all_ok = true;
    for &(label, cx, cy, expected_rgb) in checks {
        let (r, g, b) = sample_rgb_bgra(&pixels, tex_w, cx, cy);
        let ok = r == expected_rgb[0] && g == expected_rgb[1] && b == expected_rgb[2];
        println!("[surface-render-demo] {} ({},{}) = RGB({},{},{}) expected=({},{},{}) {}",
            label, cx, cy, r, g, b,
            expected_rgb[0], expected_rgb[1], expected_rgb[2],
            if ok { "OK" } else { "MISMATCH" });
        if !ok { all_ok = false; }
    }

    if all_ok { 0 } else { 3 }
}

/// Sample RGB from a bgra8unorm readback buffer (bytes stored as [B, G, R, A]).
fn sample_rgb_bgra(pixels: &[u8], tex_w: u32, x: u32, y: u32) -> (u8, u8, u8) {
    let idx = ((y * tex_w + x) * 4) as usize;
    let b = pixels[idx];
    let g = pixels[idx + 1];
    let r = pixels[idx + 2];
    (r, g, b)
}

/// Readback a bgra8unorm texture into a flat Vec<u8> (row-major, [B,G,R,A] per pixel).
fn readback_texture_bgra(
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
        label:              Some("surface-render-demo-staging"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("surface-render-demo-readback") },
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

fn init_device() -> Option<(wgpu::Device, wgpu::Queue, wgpu::AdapterInfo)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label:                 Some("surface-render-demo-device"),
            required_features:     wgpu::Features::empty(),
            required_limits:       wgpu::Limits::default(),
            memory_hints:          wgpu::MemoryHints::default(),
            trace:                 wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue, info))
}
