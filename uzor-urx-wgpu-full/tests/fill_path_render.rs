//! GPU correctness test for `CmdKind::FillPath` (non-zero winding).
//!
//! Validates triangle interior, concave shape interior, and behaviour
//! at the polygon edge.
//!
//! Run:
//!   cargo test -p uzor-urx-wgpu-full --test fill_path_render \
//!     -- --ignored --nocapture

use uzor_urx_wgpu_full::{
    BlitPipeline, DEFAULT_PATH_POINTS_CAP, SceneCmd, TileBuffers, TilePipeline, TILE_SIZE,
};

const W: u32 = 128;
const H: u32 = 128;
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label:                 Some("fill-path-test"),
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
        label:              Some("fill-readback"),
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
    let bufs = TileBuffers::allocate_with(
        device, cmds.len().max(1) as u32, W, H, DEFAULT_PATH_POINTS_CAP,
    );
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let storage_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fill-storage"),
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
        label: Some("fill-surface"),
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

/// Triangle: 3 vertices, interior must be filled; pixels outside must
/// be transparent.
#[test]
#[ignore = "needs gpu adapter"]
fn fill_triangle_interior_filled() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    // Triangle (30, 100) → (90, 100) → (60, 30) — pointing up.
    let points = vec![[30.0, 100.0], [90.0, 100.0], [60.0, 30.0]];
    let cmds = vec![
        SceneCmd::fill_path([20.0, 20.0, 100.0, 110.0], [220, 80, 40, 255], 0, 3),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &points);

    // Centroid of triangle ≈ (60, 76) — must be filled red.
    let centroid = at_bgra(&px, tex_w, 60, 76);
    eprintln!("triangle centroid (60,76) bgra = {:?}", centroid);
    assert_eq!(centroid[2], 220, "centroid pixel must be red");

    // Above apex (y=20) — outside, transparent.
    let above = at_bgra(&px, tex_w, 60, 25);
    eprintln!("above apex (60,25) bgra = {:?}", above);
    assert_eq!(above[3], 0, "above apex must be transparent");

    // Left of base — outside, transparent.
    let left = at_bgra(&px, tex_w, 15, 90);
    eprintln!("far-left (15,90) bgra = {:?}", left);
    assert_eq!(left[3], 0, "far-left must be transparent");
}

/// Concave L-shape — 6 vertices, simple polygon (no self-intersection).
/// Interior filled in both arms; the notch in the corner stays
/// transparent.
#[test]
#[ignore = "needs gpu adapter"]
fn fill_concave_l_shape_filled() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    // L-shape, CCW (screen-space y-down):
    //    (20,20)   ↑     ↑   (50,20)
    //                 │
    //     vertical    │
    //     arm         │
    //                 │
    //    (20,90)   (50,90)
    //                 ↓     ↓ horizontal arm
    //    (20,108)  (108,108)
    //
    // Traversal:
    //   (20, 20) → (50, 20) → (50, 90) → (108, 90) → (108, 108) → (20, 108) → close
    let points = vec![
        [20.0,  20.0],
        [50.0,  20.0],
        [50.0,  90.0],
        [108.0, 90.0],
        [108.0, 108.0],
        [20.0,  108.0],
    ];
    let cmds = vec![
        SceneCmd::fill_path([10.0, 10.0, 120.0, 120.0], [50, 200, 80, 255],
                            0, points.len() as u32),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &points);

    // Vertical arm centre (35, 50) — inside, filled green.
    let vert = at_bgra(&px, tex_w, 35, 50);
    eprintln!("vertical arm (35,50) bgra = {:?}", vert);
    assert!(vert[1] > 150, "vertical arm must be green-filled, got {:?}", vert);

    // Horizontal arm centre (80, 100) — inside, filled green.
    let horiz = at_bgra(&px, tex_w, 80, 100);
    eprintln!("horizontal arm (80,100) bgra = {:?}", horiz);
    assert!(horiz[1] > 150, "horizontal arm must be green-filled, got {:?}", horiz);

    // The L's concave notch (80, 50) — OUTSIDE, transparent.
    let notch = at_bgra(&px, tex_w, 80, 50);
    eprintln!("concave notch (80,50) bgra = {:?}", notch);
    assert_eq!(notch[3], 0, "concave notch must be transparent, got {:?}", notch);

    // Outside bbox — transparent.
    let outside = at_bgra(&px, tex_w, 5, 5);
    eprintln!("outside (5,5) bgra = {:?}", outside);
    assert_eq!(outside[3], 0, "outside must be transparent");
}

/// Star (5-pointed). Centre pixel must be filled; outer corner gaps
/// between arms must be transparent (non-zero winding handles the
/// self-intersecting star).
#[test]
#[ignore = "needs gpu adapter"]
fn fill_star_shape_inside_outside() {
    let (device, queue) = match init_device() {
        Some(d) => d, None => { eprintln!("no gpu adapter"); return; }
    };
    // 5-pointed star traversed (0, 2, 4, 1, 3) — classic self-
    // intersecting pentagram. Non-zero winding lights up the
    // points AND the central pentagon (winding = 2 there); even-odd
    // would leave the centre empty.
    let outer_r = 50.0;
    let cx = 64.0;
    let cy = 64.0;
    let mut outer = Vec::with_capacity(5);
    for i in 0..5 {
        let a = i as f32 * std::f32::consts::TAU / 5.0 - std::f32::consts::FRAC_PI_2;
        outer.push([cx + outer_r * a.cos(), cy + outer_r * a.sin()]);
    }
    // Pentagram traversal order: 0 → 2 → 4 → 1 → 3 → close to 0.
    let points = vec![outer[0], outer[2], outer[4], outer[1], outer[3]];
    let cmds = vec![
        SceneCmd::fill_path([10.0, 10.0, 120.0, 120.0], [255, 215, 60, 255],
                            0, points.len() as u32),
    ];
    let (px, tex_w, _) = run_pipeline(&device, &queue, &cmds, &points);

    // Centre of star — filled (winding count = 2).
    let centre = at_bgra(&px, tex_w, 64, 64);
    eprintln!("star centre (64,64) bgra = {:?}", centre);
    assert!(centre[1] > 150 && centre[2] > 200, "star centre must be yellow-filled, got {:?}", centre);

    // Far corner (5, 5) — outside, transparent.
    let corner = at_bgra(&px, tex_w, 5, 5);
    eprintln!("corner (5,5) bgra = {:?}", corner);
    assert_eq!(corner[3], 0, "corner outside must be transparent");
}
