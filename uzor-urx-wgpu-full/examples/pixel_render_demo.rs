//! Full-GPU pipeline demo — stages 1+2+4: tile_assign + tile_sort + fine raster.
//!
//! Builds a 1920×1080 scene with a few known-colour rects, runs the full
//! three-stage compute pipeline, reads back the output texture, and writes
//! `pixel_render_output.ppm` to the current directory.
//!
//! The PPM output lets you visually confirm the pipeline produces correct
//! pixels. Sanity-check: rect centres must match their encoded RGBA colour.
//!
//! Run:
//!   cargo run -p uzor-urx-wgpu-full --example pixel_render_demo --release
//!
//! Stdout summary:
//!   adapter / dispatch time / PPM write path / per-rect centre-pixel verify

use uzor_urx_wgpu_full::{
    cmd::SceneCmd,
    tile::{TileBuffers, TilePipeline, TILE_SIZE},
};

const W: u32 = 1920;
const H: u32 = 1080;

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[pixel-render-demo] init wgpu device");
    let Some((device, queue, info)) = init_device() else {
        eprintln!("[pixel-render-demo] no GPU adapter — exit 1");
        return 1;
    };
    println!("[pixel-render-demo] adapter: {} ({:?})", info.name, info.backend);

    // Known-colour rects. Colours are (R,G,B,255) = fully opaque.
    let cmds: Vec<SceneCmd> = vec![
        // Red rect, centre (200, 200).
        SceneCmd::rect(100.0, 100.0, 300.0, 300.0, [255,   0,   0, 255]),
        // Green rect, centre (600, 400).
        SceneCmd::rect(500.0, 300.0, 700.0, 500.0, [  0, 255,   0, 255]),
        // Blue rect, centre (1000, 600).
        SceneCmd::rect(900.0, 500.0, 1100.0, 700.0, [  0,   0, 255, 255]),
        // White rect, centre (1600, 800).
        SceneCmd::rect(1500.0, 700.0, 1700.0, 900.0, [255, 255, 255, 255]),
    ];
    println!("[pixel-render-demo] scene: {} cmds", cmds.len());

    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    println!("[pixel-render-demo] tiles: {}x{} = {}",
        bufs.tile_count_x, bufs.tile_count_y,
        bufs.tile_count_x * bufs.tile_count_y);

    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(&device);

    let t = std::time::Instant::now();
    {
        let mut enc = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("pixel-render-demo") },
        );
        let (_dum_img, dummy_img_view) = TilePipeline::dummy_image_atlas(&device);
        pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &[], &output_view, &dummy_atlas_view, &dummy_img_view);
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    println!("[pixel-render-demo] dispatch+fine: {:.3} ms", t.elapsed().as_secs_f64() * 1000.0);

    // Readback: padded viewport dimensions (tile-aligned).
    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let pixels = readback_texture(&device, &queue, &output_tex, tex_w, tex_h);
    println!("[pixel-render-demo] readback: {} bytes ({} x {})", pixels.len(), tex_w, tex_h);

    // Sanity-check: sample the centre of each known rect.
    let checks: &[(&str, u32, u32, [u8; 3])] = &[
        ("red rect   centre", 200, 200, [255,   0,   0]),
        ("green rect centre", 600, 400, [  0, 255,   0]),
        ("blue rect  centre", 1000, 600, [  0,   0, 255]),
        ("white rect centre", 1600, 800, [255, 255, 255]),
    ];
    let mut all_ok = true;
    for &(label, cx, cy, expected_rgb) in checks {
        let (r, g, b) = sample_rgb(&pixels, tex_w, cx, cy);
        let ok = r == expected_rgb[0] && g == expected_rgb[1] && b == expected_rgb[2];
        println!("[pixel-render-demo] {} ({},{}) = ({},{},{}) expected=({},{},{}) {}",
            label, cx, cy, r, g, b,
            expected_rgb[0], expected_rgb[1], expected_rgb[2],
            if ok { "OK" } else { "MISMATCH" });
        if !ok { all_ok = false; }
    }

    // Write PPM (only first W×H rows, not padded area).
    let ppm_path = "pixel_render_output.ppm";
    write_ppm(ppm_path, &pixels, tex_w, tex_h, W, H);
    println!("[pixel-render-demo] wrote {}", ppm_path);

    if all_ok { 0 } else { 3 }
}

/// Sample the (R,G,B) of one pixel from the readback buffer.
fn sample_rgb(pixels: &[u8], tex_w: u32, x: u32, y: u32) -> (u8, u8, u8) {
    let idx = ((y * tex_w + x) * 4) as usize;
    (pixels[idx], pixels[idx + 1], pixels[idx + 2])
}

/// Write a PPM P6 file, clipped to `out_w` × `out_h` pixels from a
/// `tex_w`-wide RGBA buffer.
fn write_ppm(path: &str, pixels: &[u8], tex_w: u32, _tex_h: u32, out_w: u32, out_h: u32) {
    use std::io::Write;
    let mut f = std::fs::File::create(path).expect("failed to create PPM");
    writeln!(f, "P6\n{} {}\n255", out_w, out_h).unwrap();
    for y in 0..out_h {
        for x in 0..out_w {
            let idx = ((y * tex_w + x) * 4) as usize;
            f.write_all(&pixels[idx..idx + 3]).unwrap();
        }
    }
}

/// Readback rgba8unorm texture into a Vec<u8> (RGBA, row-major).
fn readback_texture(
    device:  &wgpu::Device,
    queue:   &wgpu::Queue,
    texture: &wgpu::Texture,
    tex_w:   u32,
    tex_h:   u32,
) -> Vec<u8> {
    // wgpu requires row stride to be a multiple of 256 bytes.
    let bytes_per_row_unaligned = tex_w * 4;
    let aligned_stride = (bytes_per_row_unaligned + 255) & !255;
    let buf_size = (aligned_stride * tex_h) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel-render-staging"),
        size:  buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("pixel-render-readback") },
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
    // De-stride: copy only the valid `tex_w * 4` bytes per row.
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
            label: Some("pixel-render-demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue, info))
}
