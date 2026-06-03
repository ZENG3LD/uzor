//! Headless demo stand for the wgpu-instanced backend.
//!
//! Renders a scene with all 4 primitive types — quad / triangle / line /
//! glyph — to an offscreen texture, reads pixels back, writes a PPM,
//! and prints a small summary. Exercises:
//!
//!   - Painter-order coalesce across `DrawCmd` types
//!   - Packed RGBA u32 wire format (URX 1.5)
//!   - StagingBelt opt-in path (when --features=belt or env URX_STAGING_BELT=1)
//!   - Glyph atlas + cosmic-text shaping
//!
//! Purpose: a single binary to confirm the wgpu-instanced backend still
//! produces correct pixels end-to-end after any refactor — no winit, no
//! examples crate dependency, no live agent. Run after touching
//! `renderer.rs` / `shaders.rs` / `instances.rs` to catch regressions
//! the unit tests can't see.
//!
//! Run:
//!   cargo run -p uzor-render-wgpu-instanced --example primitives_demo --release
//!
//! Toggle staging-belt path:
//!   URX_STAGING_BELT=1 cargo run -p uzor-render-wgpu-instanced --example primitives_demo --release
//!
//! Output:
//!   - `out/wgpu_instanced_primitives.ppm` (800×600, RGB)
//!   - stdout: scene composition + timing

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use uzor::fonts::FontFamily;
use uzor::render::{TextAlign, TextBaseline};

use uzor_render_wgpu_instanced::{
    DrawCmd, GlyphInstance, InstancedRenderer, LineInstance, QuadInstance,
    TextAreaData, TriangleInstance,
};

const W: u32 = 800;
const H: u32 = 600;

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[wgpu-demo] init wgpu device");
    let Some((device, queue, info)) = init_device() else {
        eprintln!("[wgpu-demo] no GPU adapter — exit 1");
        return 1;
    };
    println!("[wgpu-demo] adapter: {} ({:?})", info.name, info.backend);

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = InstancedRenderer::new(&device, &queue, format);

    if std::env::var("URX_STAGING_BELT").is_ok() {
        renderer.enable_staging_belt(&device, 256 * 1024);
        println!("[wgpu-demo] staging-belt path: ON (256 KB chunks)");
    } else {
        println!("[wgpu-demo] staging-belt path: OFF (queue.write_buffer)");
    }

    // Scene: panels (quads) + a triangle + lines + glyphs.
    let mut commands: Vec<DrawCmd> = Vec::new();

    // Background panel — full window, dark gray.
    commands.push(DrawCmd::Quad(QuadInstance::from_float_color(
        [0.0, 0.0], [W as f32, H as f32],
        [0.08, 0.08, 0.10, 1.0],
        0.0, 0.0, [0.0; 4],
        [0.0, 0.0, W as f32, H as f32],
    )));

    // Three coloured "cards" with corner radius + border.
    let card_palette: &[[f32; 4]] = &[
        [0.92, 0.32, 0.34, 1.0],
        [0.34, 0.78, 0.46, 1.0],
        [0.30, 0.55, 0.92, 1.0],
    ];
    for (i, &col) in card_palette.iter().enumerate() {
        let x = 80.0 + i as f32 * 220.0;
        commands.push(DrawCmd::Quad(QuadInstance::from_float_color(
            [x, 100.0], [180.0, 120.0],
            col,
            14.0, 2.0, [1.0, 1.0, 1.0, 0.85],
            [0.0, 0.0, W as f32, H as f32],
        )));
    }

    // A triangle.
    commands.push(DrawCmd::Triangle(TriangleInstance::from_float_color(
        [400.0, 320.0],
        [500.0, 470.0],
        [300.0, 470.0],
        [0.95, 0.86, 0.32, 1.0],
        [0.0, 0.0, W as f32, H as f32],
    )));

    // Crosshair lines.
    commands.push(DrawCmd::Line(LineInstance::from_float_color(
        [50.0, 320.0], [W as f32 - 50.0, 320.0],
        [0.6, 0.6, 0.8, 0.85], 2.0, 0.0,
        [0.0, 0.0, W as f32, H as f32],
    )));
    commands.push(DrawCmd::Line(LineInstance::from_float_color(
        [W as f32 / 2.0, 60.0], [W as f32 / 2.0, H as f32 - 60.0],
        [0.6, 0.6, 0.8, 0.85], 2.0, 0.0,
        [0.0, 0.0, W as f32, H as f32],
    )));

    // Text labels.
    let label_clip = [0.0_f32, 0.0, W as f32, H as f32];
    for (i, label) in ["Q", "T", "L"].iter().enumerate() {
        commands.push(DrawCmd::Text(TextAreaData {
            text: format!("Card {} ({})", i + 1, label),
            x: 80.0 + i as f32 * 220.0 + 12.0,
            y: 110.0 + 4.0,
            font_size: 18.0,
            color: [1.0, 1.0, 1.0, 1.0],
            family: FontFamily::Roboto,
            bold: true,
            italic: false,
            align: TextAlign::Left,
            baseline: TextBaseline::Top,
            clip: label_clip,
            estimated_width: 120.0,
            estimated_height: 20.0,
        }));
    }
    commands.push(DrawCmd::Text(TextAreaData {
        text: "URX wgpu-instanced demo".to_string(),
        x: W as f32 / 2.0,
        y: 30.0,
        font_size: 28.0,
        color: [0.92, 0.94, 0.98, 1.0],
        family: FontFamily::Roboto,
        bold: false,
        italic: false,
        align: TextAlign::Center,
        baseline: TextBaseline::Top,
        clip: label_clip,
        estimated_width: 360.0,
        estimated_height: 32.0,
    }));

    println!("[wgpu-demo] command stream: {} primitives", commands.len());
    let by_type = summarise_types(&commands);
    println!("[wgpu-demo]   quads={} triangles={} lines={} text={}",
        by_type.0, by_type.1, by_type.2, by_type.3);

    // Render target.
    let target_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wgpu-demo-target"),
        size:  wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target_tex.create_view(&wgpu::TextureViewDescriptor::default());

    println!("[wgpu-demo] rendering 60 frames");
    let t = std::time::Instant::now();
    for _ in 0..60 {
        renderer.render(
            &device, &queue, &target_view, W, H, &commands,
            Some(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
            None,
        );
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
    let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("[wgpu-demo] 60 frames in {:.1} ms ({:.2} ms/frame)",
        elapsed_ms, elapsed_ms / 60.0);

    println!("[wgpu-demo] reading back");
    let pixels = read_offscreen(&device, &queue, &target_tex, W, H);

    let out_path = Path::new("out/wgpu_instanced_primitives.ppm");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    write_ppm(out_path, &pixels, W, H).expect("ppm write");
    println!("[wgpu-demo] wrote {}", out_path.display());

    // Sanity checks: card 1 centre should be reddish.
    let i = (((100.0 + 60.0) as u32 * W + (80.0 + 90.0) as u32) * 4) as usize;
    let c = &pixels[i..i+4];
    println!("[wgpu-demo] card 1 centre RGBA: {:?}", c);
    if c[0] < 150 {
        eprintln!("[wgpu-demo] card 1 centre red channel too low — expected ~200+, got {}",
            c[0]);
        return 2;
    }

    println!("[wgpu-demo] OK");
    0
}

fn summarise_types(cmds: &[DrawCmd]) -> (usize, usize, usize, usize) {
    let mut q = 0; let mut t = 0; let mut l = 0; let mut x = 0;
    for c in cmds {
        match c {
            DrawCmd::Quad(_) => q += 1,
            DrawCmd::Triangle(_) => t += 1,
            DrawCmd::Line(_) => l += 1,
            DrawCmd::Text(_) => x += 1,
        }
    }
    let _ = std::mem::size_of::<GlyphInstance>();
    (q, t, l, x)
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
            label: Some("wgpu-demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue, info))
}

fn read_offscreen(
    device: &wgpu::Device,
    queue:  &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32, height: u32,
) -> Vec<u8> {
    let unpadded_bpr = width * 4;
    let padded_bpr = (unpadded_bpr + 255) & !255;
    let buf_size = (padded_bpr * height) as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wgpu-demo-readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture, mip_level: 0, origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut out = vec![0u8; (unpadded_bpr * height) as usize];
    for row in 0..height as usize {
        let src = &data[row * padded_bpr as usize ..
                        row * padded_bpr as usize + unpadded_bpr as usize];
        let dst = &mut out[row * unpadded_bpr as usize ..
                           (row + 1) * unpadded_bpr as usize];
        dst.copy_from_slice(src);
    }
    drop(data);
    buffer.unmap();
    out
}

fn write_ppm(path: &Path, rgba: &[u8], width: u32, height: u32)
    -> std::io::Result<()>
{
    let f = File::create(path)?;
    let mut w = BufWriter::new(f);
    write!(w, "P6\n{} {}\n255\n", width, height)?;
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for chunk in rgba.chunks_exact(4) {
        rgb.push(chunk[0]);
        rgb.push(chunk[1]);
        rgb.push(chunk[2]);
    }
    w.write_all(&rgb)?;
    w.flush()?;
    Ok(())
}
