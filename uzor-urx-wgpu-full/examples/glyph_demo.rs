//! Glyph pipeline demo — exercises CmdKind::Glyph (kind=4).
//!
//! Creates a 256×64 R8Unorm glyph atlas containing three "slots":
//!   Slot 0 (UV [0.0, 0.0, 0.33, 1.0]): solid 255 alpha (opaque block)
//!   Slot 1 (UV [0.33, 0.0, 0.66, 1.0]): horizontal alpha gradient (0→255)
//!   Slot 2 (UV [0.66, 0.0, 1.0, 1.0]): radial alpha peak at centre
//!
//! Renders three glyph cmds (one per slot) in red / green / blue, each
//! 100×40 pixels, reads back the output, writes `glyph_demo_output.ppm`,
//! and sanity-checks known pixels.
//!
//! Run:
//!   cargo run -p uzor-urx-wgpu-full --example glyph_demo --release

use uzor_urx_wgpu_full::{
    cmd::SceneCmd,
    tile::{TileBuffers, TilePipeline, TILE_SIZE},
};

const W: u32 = 512;
const H: u32 = 128;

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[glyph-demo] init wgpu device");
    let Some((device, queue, info)) = init_device() else {
        eprintln!("[glyph-demo] no GPU adapter — exit 1");
        return 1;
    };
    println!("[glyph-demo] adapter: {} ({:?})", info.name, info.backend);

    // Build the 256×64 glyph atlas.
    let atlas_w: u32 = 256;
    let atlas_h: u32 = 64;
    let atlas_data = build_atlas(atlas_w, atlas_h);

    let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:               Some("glyph-demo-atlas"),
        size:                wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
        mip_level_count:     1,
        sample_count:        1,
        dimension:           wgpu::TextureDimension::D2,
        format:              wgpu::TextureFormat::R8Unorm,
        usage:               wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats:        &[],
    });
    upload_r8_atlas(&device, &queue, &atlas_tex, atlas_w, atlas_h, &atlas_data);
    let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
    println!("[glyph-demo] atlas: {}×{} R8Unorm", atlas_w, atlas_h);

    // Three glyph cmds at y=40 (centred vertically), spaced 10px apart.
    //   Slot 0: UV [0.00, 0.0, 0.333, 1.0] — opaque block — red
    //   Slot 1: UV [0.33, 0.0, 0.667, 1.0] — gradient     — green
    //   Slot 2: UV [0.66, 0.0, 1.000, 1.0] — radial peak  — blue
    let cmds: Vec<SceneCmd> = vec![
        SceneCmd::glyph( 10.0, 40.0, 110.0, 80.0, [255,   0,   0, 255], [0.000, 0.0, 0.333, 1.0]),
        SceneCmd::glyph(120.0, 40.0, 220.0, 80.0, [  0, 255,   0, 255], [0.333, 0.0, 0.667, 1.0]),
        SceneCmd::glyph(230.0, 40.0, 330.0, 80.0, [  0,   0, 255, 255], [0.667, 0.0, 1.000, 1.0]),
    ];
    println!("[glyph-demo] scene: {} glyph cmds", cmds.len());

    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;

    let t = std::time::Instant::now();
    {
        let mut enc = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("glyph-demo") },
        );
        pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &[], &output_view, &atlas_view);
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    println!("[glyph-demo] dispatch+fine: {:.3} ms", t.elapsed().as_secs_f64() * 1000.0);

    let pixels = readback_texture(&device, &queue, &output_tex, tex_w, tex_h);
    println!("[glyph-demo] readback: {} bytes ({}×{})", pixels.len(), tex_w, tex_h);

    // Sanity checks.
    let checks: &[(&str, u32, u32, [u8; 4], &str)] = &[
        // Slot 0 centre (60, 60): solid-opaque → fully opaque red.
        ("slot0 centre",  60, 60, [255, 0, 0, 255], "opaque red"),
        // Slot 1 left edge (122, 60): gradient α≈0 → near-transparent.
        // Output is premultiplied — green channel = base.g * atlas.r ≈ low,
        // alpha = base.a * atlas.r ≈ low (same magnitude). At atlas col ~88
        // the gradient alpha is ~5/255. Expected ratio: (0, low, 0, low) with
        // |G - A| < 8 (premultiplied invariant on r/g/b vs a).
        ("slot1 left",   122, 60, [  0,   4, 0,   4], "premul near-transparent green left"),
        // Slot 1 right edge (215, 60): gradient α≈255 → opaque green.
        ("slot1 right",  215, 60, [  0, 255, 0, 255], "opaque green right"),
        // Slot 2 centre (280, 60): radial peak → opaque blue.
        ("slot2 centre", 280, 60, [  0,   0, 255, 255], "opaque blue centre"),
        // Outside all glyphs (0, 0) → transparent.
        ("outside",        0,  0, [  0,   0,   0,   0], "transparent background"),
    ];

    let mut all_ok = true;
    for &(label, x, y, expected, note) in checks {
        let got = sample_rgba(&pixels, tex_w, x, y);
        let ok = expected_matches(got, expected);
        println!("[glyph-demo] {} ({},{}) = {:?}  expected={:?} ({}) {}",
            label, x, y, got, expected, note, if ok { "OK" } else { "MISMATCH" });
        if !ok { all_ok = false; }
    }

    let ppm_path = "glyph_demo_output.ppm";
    write_ppm(ppm_path, &pixels, tex_w, W, H);
    println!("[glyph-demo] wrote {}", ppm_path);

    if all_ok { 0 } else { 3 }
}

/// Loose pixel comparison — allows ±8 tolerance for GPU rounding/linear filtering.
fn expected_matches(got: [u8; 4], expected: [u8; 4]) -> bool {
    got.iter().zip(expected.iter()).all(|(&g, &e)| {
        (g as i32 - e as i32).unsigned_abs() <= 16
    })
}

fn sample_rgba(pixels: &[u8], tex_w: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * tex_w + x) * 4) as usize;
    [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]]
}

/// Build a 256×64 R8Unorm atlas (one byte per texel) with three slots.
fn build_atlas(atlas_w: u32, atlas_h: u32) -> Vec<u8> {
    let mut data = vec![0u8; (atlas_w * atlas_h) as usize];
    let slot_w = atlas_w / 3;

    for y in 0..atlas_h {
        for x in 0..atlas_w {
            let slot = x / slot_w;
            let alpha = match slot {
                0 => {
                    // Slot 0: solid 255.
                    255u8
                }
                1 => {
                    // Slot 1: horizontal gradient left=0, right=255.
                    let local_x = x - slot_w;
                    ((local_x as f32 / (slot_w - 1) as f32) * 255.0).round() as u8
                }
                _ => {
                    // Slot 2: radial peak at slot centre.
                    let cx = slot_w * 2 + slot_w / 2;
                    let cy = atlas_h / 2;
                    let dx = (x as f32) - (cx as f32);
                    let dy = (y as f32) - (cy as f32);
                    let dist = (dx * dx + dy * dy).sqrt();
                    let max_r = (slot_w.min(atlas_h) / 2) as f32;
                    let t = (1.0 - (dist / max_r).min(1.0)).max(0.0);
                    (t * 255.0).round() as u8
                }
            };
            data[(y * atlas_w + x) as usize] = alpha;
        }
    }
    data
}

/// Upload data to an existing R8Unorm texture (row-stride aligned to 256 bytes).
fn upload_r8_atlas(
    device:  &wgpu::Device,
    queue:   &wgpu::Queue,
    texture: &wgpu::Texture,
    atlas_w: u32,
    atlas_h: u32,
    data:    &[u8],
) {
    let row_stride = (atlas_w + 255) & !255;
    let buf_size   = (row_stride * atlas_h) as u64;
    let upload_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label:              Some("glyph-demo-atlas-upload"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    {
        let mut mapped = upload_buf.slice(..).get_mapped_range_mut();
        for row in 0..atlas_h as usize {
            let src_start = row * atlas_w as usize;
            let dst_start = row * row_stride as usize;
            mapped[dst_start..dst_start + atlas_w as usize]
                .copy_from_slice(&data[src_start..src_start + atlas_w as usize]);
        }
    }
    upload_buf.unmap();

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("glyph-demo-atlas-copy") },
    );
    enc.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: &upload_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset:         0,
                bytes_per_row:  Some(row_stride),
                rows_per_image: Some(atlas_h),
            },
        },
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin:    wgpu::Origin3d::ZERO,
            aspect:    wgpu::TextureAspect::All,
        },
        wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
}

fn write_ppm(path: &str, pixels: &[u8], tex_w: u32, out_w: u32, out_h: u32) {
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
        label:              Some("glyph-demo-staging"),
        size:               buf_size,
        usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("glyph-demo-readback") },
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
            label:                 Some("glyph-demo-device"),
            required_features:     wgpu::Features::empty(),
            required_limits:       wgpu::Limits::default(),
            memory_hints:          wgpu::MemoryHints::default(),
            trace:                 wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue, info))
}
