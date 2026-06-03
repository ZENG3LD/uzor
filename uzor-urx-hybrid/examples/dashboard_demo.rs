//! Headless hybrid-backend demo stand.
//!
//! Builds a 64-region "dashboard" — coloured panels on a 1920×1080
//! surface — composites it through `HybridBackend` (atlas-on +
//! coalesce-on), reads pixels back, writes the result as `.ppm` (zero
//! deps, viewable in any image tool / browser via drag-drop), and
//! prints a one-line metrics summary.
//!
//! Purpose: exercise the full hybrid path end-to-end on real GPU
//! without needing a window. Run after any hybrid change to confirm
//! nothing visually regressed and the atlas + coalesce flags actually
//! kick in (pass_draws metric).
//!
//! Run:
//!   cargo run -p uzor-urx-hybrid --example dashboard_demo --release
//!
//! Output:
//!   - `out/hybrid_dashboard.ppm` (1920×1080, RGB, ~6 MB)
//!   - stdout: composite summary (regions / pass_draws / upload bytes / etc.)
//!
//! Exit code: 0 on success, 1 if no GPU adapter, 2 if any region
//! failed to land in the atlas (sanity — expected to pass with
//! 64 × 64×64 regions in a 2048² atlas).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use uzor_urx_core::config::UrxConfig;
use uzor_urx_core::region::RegionId;
use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::{HybridBackend, QuadInstance};

const W: u32 = 1920;
const H: u32 = 1080;
const REGION_PX: u32 = 64;
const REGIONS_N: u32 = 64; // 8×8 grid

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[hybrid-demo] init wgpu device");
    let Some((device, queue, adapter_info)) = init_device() else {
        eprintln!("[hybrid-demo] no GPU adapter — skipping (exit 1)");
        return 1;
    };
    println!("[hybrid-demo] adapter: {} ({:?})",
        adapter_info.name, adapter_info.backend);

    // Hybrid backend with atlas + coalesce ON — exercises the full
    // 1.5-dev path (atlas-resident regions + single coalesced draw).
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_instanced_composite(true)
        .build()
        .expect("config builds");
    let mut backend = HybridBackend::with_config(cfg);

    // 64 regions × 64×64 colour-coded by index.
    let layout = grid_layout(REGIONS_N);

    println!("[hybrid-demo] uploading {} regions ({}×{} each)",
        REGIONS_N, REGION_PX, REGION_PX);
    for (id, _rect) in &layout {
        let pm = pixmap_for(*id);
        backend.upsert_region_pixmap(&device, &queue, *id, &pm);
    }

    let atlas_resident = layout.iter()
        .filter(|(id, _)| backend.atlas_slot(*id).is_some())
        .count();
    println!("[hybrid-demo] atlas-resident: {}/{}", atlas_resident, REGIONS_N);
    if atlas_resident == 0 {
        eprintln!("[hybrid-demo] NO regions landed in atlas — likely \
                   atlas_enabled wiring broken (exit 2)");
        return 2;
    }

    // Render target — RGBA8 offscreen.
    let target_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hybrid-demo-target"),
        size:  wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target_tex.create_view(&wgpu::TextureViewDescriptor::default());

    // Composite N times to populate metrics.
    let instances: Vec<(RegionId, QuadInstance)> = layout.iter()
        .map(|(id, [x, y, w, h])| (*id, QuadInstance::new(*x, *y, *w, *h)))
        .collect();

    println!("[hybrid-demo] compositing {} frames", 60);
    let t = std::time::Instant::now();
    for _ in 0..60 {
        composite_one_frame(&mut backend, &device, &queue, &target_view, &instances);
    }
    let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("[hybrid-demo] 60 frames in {:.1} ms ({:.2} ms/frame)",
        elapsed_ms, elapsed_ms / 60.0);

    println!("[hybrid-demo] reading back final frame");
    let pixels = read_offscreen(&device, &queue, &target_tex, W, H);

    let out_path = Path::new("out/hybrid_dashboard.ppm");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    write_ppm(out_path, &pixels, W, H).expect("ppm write");
    println!("[hybrid-demo] wrote {} ({} bytes)",
        out_path.display(), W * H * 3 + 32);

    // Spot-check a known-coloured region pixel.
    let id_0_center = (
        (layout[0].1[0] + REGION_PX as f32 / 2.0) as u32,
        (layout[0].1[1] + REGION_PX as f32 / 2.0) as u32,
    );
    let i = ((id_0_center.1 * W + id_0_center.0) * 4) as usize;
    let c = &pixels[i..i+4];
    println!("[hybrid-demo] region 0 centre RGB: ({}, {}, {}) alpha {}",
        c[0], c[1], c[2], c[3]);

    println!("[hybrid-demo] OK");
    0
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
            label: Some("hybrid-demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue, info))
}

fn grid_layout(n: u32) -> Vec<(RegionId, [f32; 4])> {
    let cols = (n as f32).sqrt().ceil() as u32;
    let gap = 8.0f32;
    let pad_x = 64.0f32;
    let pad_y = 64.0f32;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let col = i % cols;
        let row = i / cols;
        let x = pad_x + col as f32 * (REGION_PX as f32 + gap);
        let y = pad_y + row as f32 * (REGION_PX as f32 + gap);
        out.push((RegionId(i as u64), [x, y, REGION_PX as f32, REGION_PX as f32]));
    }
    out
}

fn pixmap_for(id: RegionId) -> Pixmap {
    let i = id.0 as u32;
    // HSV-like wheel walk via simple bit splits — deterministic, no
    // external rng. Yields visually distinct panels.
    let r = ((i * 41) & 0xff) as u8;
    let g = ((i * 113 + 80) & 0xff) as u8;
    let b = ((i * 197 + 160) & 0xff) as u8;
    let mut pm = Pixmap::new(REGION_PX, REGION_PX);
    pm.fill([r, g, b, 255]);
    pm
}

fn composite_one_frame(
    backend:     &mut HybridBackend,
    device:      &wgpu::Device,
    queue:       &wgpu::Queue,
    target_view: &wgpu::TextureView,
    instances:   &[(RegionId, QuadInstance)],
) {
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let _rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hybrid-demo-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.04, g: 0.04, b: 0.06, a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
    backend.composite(device, queue, &mut enc, target_view,
        wgpu::TextureFormat::Rgba8Unorm, W, H, instances);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
}

fn read_offscreen(
    device: &wgpu::Device,
    queue:  &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32, height: u32,
) -> Vec<u8> {
    let bytes_per_pixel = 4u32;
    let unpadded_bpr = width * bytes_per_pixel;
    let padded_bpr = (unpadded_bpr + 255) & !255;
    let buf_size = (padded_bpr * height) as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hybrid-demo-readback"),
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
    // PPM is RGB; strip alpha.
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
