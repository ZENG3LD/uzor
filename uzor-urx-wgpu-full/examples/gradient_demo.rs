//! Gradient pipeline demo — exercises all three cmd types: solid Rect,
//! LinGradient (4 directions), and RadGradient.
//!
//! Renders a 512×512 scene, reads back the output texture, and writes
//! `gradient_demo_output.ppm` to the current directory.
//!
//! Run:
//!   cargo run -p uzor-urx-wgpu-full --example gradient_demo --release

use uzor_urx_wgpu_full::{
    cmd::{lin_dir, SceneCmd},
    tile::{TileBuffers, TilePipeline, TILE_SIZE},
};

const W: u32 = 512;
const H: u32 = 512;

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[gradient-demo] init wgpu device");
    let Some((device, queue, info)) = init_device() else {
        eprintln!("[gradient-demo] no GPU adapter — exit 1");
        return 1;
    };
    println!("[gradient-demo] adapter: {} ({:?})", info.name, info.backend);

    // Scene:
    //   1 solid rect (grey background strip)
    //   4 linear gradients (one per direction)
    //   1 radial gradient
    let cmds: Vec<SceneCmd> = vec![
        // --- Solid grey rect as background landmark [200, 200, 312, 312] ---
        SceneCmd::rect(200.0, 200.0, 312.0, 312.0, [128, 128, 128, 255]),

        // --- LinGradient: HORIZONTAL (L→R) red→blue [0,0,120,100] ---
        SceneCmd::lin_gradient(
            0.0, 0.0, 120.0, 100.0,
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            lin_dir::HORIZONTAL,
        ),

        // --- LinGradient: VERTICAL (T→B) green→yellow [130,0,250,100] ---
        SceneCmd::lin_gradient(
            130.0, 0.0, 250.0, 100.0,
            [0, 200, 0, 255],
            [255, 220, 0, 255],
            lin_dir::VERTICAL,
        ),

        // --- LinGradient: DIAGONAL_TLBR white→black [260,0,380,100] ---
        SceneCmd::lin_gradient(
            260.0, 0.0, 380.0, 100.0,
            [255, 255, 255, 255],
            [0, 0, 0, 255],
            lin_dir::DIAGONAL_TLBR,
        ),

        // --- LinGradient: DIAGONAL_BLTR cyan→magenta [390,0,510,100] ---
        SceneCmd::lin_gradient(
            390.0, 0.0, 510.0, 100.0,
            [0, 220, 220, 255],
            [220, 0, 220, 255],
            lin_dir::DIAGONAL_BLTR,
        ),

        // --- RadGradient: white inner → dark-blue outer [160,130,352,322] ---
        SceneCmd::rad_gradient(
            160.0, 130.0, 352.0, 322.0,
            [255, 255, 255, 255],
            [0, 0, 80, 255],
        ),
    ];
    println!("[gradient-demo] scene: {} cmds", cmds.len());

    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(&device);

    let t = std::time::Instant::now();
    {
        let mut enc = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("gradient-demo") },
        );
        pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &[], &output_view, &dummy_atlas_view);
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    println!("[gradient-demo] dispatch+fine: {:.3} ms", t.elapsed().as_secs_f64() * 1000.0);

    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;
    let pixels = readback_texture(&device, &queue, &output_tex, tex_w, tex_h);
    println!("[gradient-demo] readback: {} bytes ({}×{})", pixels.len(), tex_w, tex_h);

    // Spot-checks: sample a few pixels to confirm gradients fired.
    let checks: &[(&str, u32, u32)] = &[
        ("lin-H  left-edge  (5,50)",   5,  50),
        ("lin-H  right-edge (115,50)", 115, 50),
        ("lin-V  top-edge  (190,5)",   190,  5),
        ("lin-V  bot-edge  (190,95)",  190, 95),
        ("rad-gradient centre (256,226)", 256, 226),
        ("solid-rect centre   (256,256)", 256, 256),
    ];
    for &(label, x, y) in checks {
        let idx = ((y * tex_w + x) * 4) as usize;
        let (r, g, b, a) = (pixels[idx], pixels[idx+1], pixels[idx+2], pixels[idx+3]);
        println!("[gradient-demo] {} = ({},{},{},{})", label, r, g, b, a);
    }

    let ppm_path = "gradient_demo_output.ppm";
    write_ppm(ppm_path, &pixels, tex_w, W, H);
    println!("[gradient-demo] wrote {}", ppm_path);

    0
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
        label: Some("gradient-demo-staging"),
        size:  buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("gradient-demo-readback") },
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
            label: Some("gradient-demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue, info))
}
