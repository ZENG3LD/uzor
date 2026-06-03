//! GPU correctness test for `CmdKind::Path`.
//!
//! Validates the full dispatch+blit pipeline produces correct pixels
//! for multi-segment polylines (straight zig-zag) AND flattened curves
//! (sine-wave sampled at 64 points).
//!
//! Run:
//!   cargo test -p uzor-urx-wgpu-full --test path_render \
//!     -- --ignored --nocapture

use uzor_urx_wgpu_full::{
    BlitPipeline, DEFAULT_PATH_POINTS_CAP, SceneCmd, TileBuffers, TilePipeline, TILE_SIZE,
};

const W: u32 = 128;
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
        label:                 Some("path-render-test"),
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
        label:              Some("path-readback"),
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
    device:      &wgpu::Device,
    queue:       &wgpu::Queue,
    cmds:        &[SceneCmd],
    path_points: &[[f32; 2]],
) -> (Vec<u8>, u32, u32) {
    let (bufs, _storage_tex, storage_view) = {
        let bufs = TileBuffers::allocate_with(
            device, cmds.len().max(1) as u32, W, H, DEFAULT_PATH_POINTS_CAP,
        );
        // Re-do with_output_texture's logic by hand since we used allocate_with.
        let tex_w = bufs.tile_count_x * TILE_SIZE;
        let tex_h = bufs.tile_count_y * TILE_SIZE;
        let storage_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("path-storage"),
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
        (bufs, storage_tex, storage_view)
    };
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let surface_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("path-surface"),
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
    let (_dum_img, dummy_img_view) = TilePipeline::dummy_image_atlas(device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    tile.render_to_target(
        device, queue, &mut enc,
        &bufs, cmds, path_points,
        &storage_view, &blit, &surface_view,
        tex_w, tex_h, &dummy_atlas_view, &dummy_img_view,
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    (readback_bgra(device, queue, &surface_tex, tex_w, tex_h), tex_w, tex_h)
}

/// Zig-zag polyline: 4 points, 3 segments. Pixels on each segment must
/// be painted; pixels in the negative space between segments must be
/// transparent.
#[test]
#[ignore = "needs gpu adapter"]
fn path_zigzag_paints_each_segment() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    let points: Vec<[f32; 2]> = vec![
        [10.0, 32.0],   // p0
        [40.0, 10.0],   // p1
        [70.0, 54.0],   // p2
        [100.0, 20.0],  // p3
    ];
    let bbox = {
        let mut xmin = f32::INFINITY; let mut ymin = f32::INFINITY;
        let mut xmax = f32::NEG_INFINITY; let mut ymax = f32::NEG_INFINITY;
        for p in &points {
            if p[0] < xmin { xmin = p[0]; } if p[1] < ymin { ymin = p[1]; }
            if p[0] > xmax { xmax = p[0]; } if p[1] > ymax { ymax = p[1]; }
        }
        [xmin - 2.0, ymin - 2.0, xmax + 2.0, ymax + 2.0]
    };
    let cmds = vec![
        SceneCmd::path(bbox, [255, 100, 0, 255], 3.0, 0, points.len() as u32),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &points);

    // Sample midpoints of each segment.
    for (i, (a, b)) in points.windows(2).enumerate().map(|(i, w)| (i, (w[0], w[1]))) {
        let mx = ((a[0] + b[0]) * 0.5) as u32;
        let my = ((a[1] + b[1]) * 0.5) as u32;
        let s = at_bgra(&px, tex_w, mx, my);
        eprintln!("seg {} mid ({},{}) bgra = {:?}", i, mx, my, s);
        // Premultiplied: r should dominate, blue minimal.
        assert!(s[2] > 100, "seg {i} mid must be orange-ish; got {:?}", s);
    }

    // Hard negative space — between zig-zag arcs.
    let neg = at_bgra(&px, tex_w, 90, 50);
    eprintln!("negative space (90, 50) bgra = {:?}", neg);
    assert_eq!(neg[3], 0, "pixel in zig-zag negative space must be transparent");
}

/// Sine-wave path (64 points, period 64 px) painted across the full
/// 128 px viewport. Two on-curve pixels must be hit; one well-off
/// pixel must stay transparent.
#[test]
#[ignore = "needs gpu adapter"]
fn path_sine_wave_paints_along_curve() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    // Sample sin(x * 2π / 32) — period 32 px, amplitude 20 around y=32.
    // x=2 → sin(0)=0    → y=32 (baseline)
    // x=10 → sin(π/2)=1 → y=52 (trough, since +y = down on screen)
    // x=26 → sin(3π/2)=-1 → y=12 (peak)
    let n_pts = 64usize;
    let mut points = Vec::with_capacity(n_pts);
    for i in 0..n_pts {
        let x = (i as f32 / (n_pts - 1) as f32) * (W as f32 - 4.0) + 2.0;
        let phase = (x - 2.0) / 32.0 * std::f32::consts::TAU;
        let y = 32.0 + 20.0 * phase.sin();
        points.push([x, y]);
    }
    let bbox = [0.0, 8.0, W as f32, 56.0];
    let cmds = vec![
        SceneCmd::path(bbox, [50, 200, 255, 255], 2.5, 0, points.len() as u32),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &points);

    // Probe centres at the actual sine extremes (see comment above).
    let probes: &[(&str, u32, u32, bool)] = &[
        ("x≈10 trough (y≈52)", 10, 52, true),
        ("x≈26 peak   (y≈12)", 26, 12, true),
        ("far below curve",     5, 60, false),  // outside bbox bottom
        ("far above curve",    64, 4,  false),  // outside bbox top
    ];
    for &(label, x, y, expect_painted) in probes {
        // Probe 5×5 block to soften single-pixel mismatches.
        let mut max_alpha = 0u8;
        for dy in 0..5u32 {
            for dx in 0..5u32 {
                if x + dx >= tex_w || y + dy >= 64 { continue; }
                let s = at_bgra(&px, tex_w, x + dx, y + dy);
                if s[3] > max_alpha { max_alpha = s[3]; }
            }
        }
        eprintln!("{label} ({},{}) 5x5 max α = {}", x, y, max_alpha);
        if expect_painted {
            assert!(max_alpha > 100, "{label} block must be painted, max α = {}", max_alpha);
        } else {
            assert!(max_alpha < 20, "{label} block must stay transparent, max α = {}", max_alpha);
        }
    }
}

/// Two parallel paths with different offsets: verifies that a Path cmd
/// reads its own slice of `path_points` (offset works) and the second
/// path doesn't paint where the first one is and vice versa.
#[test]
#[ignore = "needs gpu adapter"]
fn path_two_paths_with_different_offsets() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };

    let path_a: Vec<[f32; 2]> = vec![[10.0, 16.0], [60.0, 16.0]];
    let path_b: Vec<[f32; 2]> = vec![[10.0, 48.0], [60.0, 48.0]];
    let mut points = Vec::new();
    points.extend_from_slice(&path_a);
    let offset_b = points.len() as u32;
    points.extend_from_slice(&path_b);

    let cmds = vec![
        SceneCmd::path([8.0, 12.0, 62.0, 20.0], [255, 0, 0, 255], 3.0, 0,
                       path_a.len() as u32),
        SceneCmd::path([8.0, 44.0, 62.0, 52.0], [0, 255, 0, 255], 3.0, offset_b,
                       path_b.len() as u32),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &points);

    let on_a = at_bgra(&px, tex_w, 35, 16);
    let on_b = at_bgra(&px, tex_w, 35, 48);
    let off  = at_bgra(&px, tex_w, 35, 30);

    eprintln!("path A on (35,16) bgra = {:?}", on_a);
    eprintln!("path B on (35,48) bgra = {:?}", on_b);
    eprintln!("between   (35,30) bgra = {:?}", off);

    assert!(on_a[2] > 200, "path A pixel must be red, got {:?}", on_a);
    assert!(on_b[1] > 200, "path B pixel must be green, got {:?}", on_b);
    assert_eq!(off[3], 0, "between paths must be transparent");
}
