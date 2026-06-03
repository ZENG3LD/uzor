//! A/B benchmark: atlas-on vs atlas-off vs atlas+coalesce-on
//! across varying region counts.
//!
//! Measures the WARM-STATIC frame cost (all regions already uploaded,
//! every iteration is just composite). This is the steady-state path
//! a real dashboard hits at 60 Hz; per-bind-group switch overhead
//! shows up most cleanly here.
//!
//! Three modes:
//!   A0: atlas off               — N standalone textures, N draws
//!   B1: atlas on, coalesce off  — N atlas slots, N draws  (per-bg switch)
//!   B2: atlas on, coalesce on   — N atlas slots, 1 draw   (single instanced)
//!
//! N varies across 8 / 32 / 64 / 128 / 256 regions.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::config::UrxConfig;
use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::{HybridBackend, QuadInstance};

const W: u32 = 1920;
const H: u32 = 1080;

/// Small-region size — fits the default 2048×2048 atlas (half = 1024).
const REGION_PX: u32 = 64;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx-hybrid-atlas-ab-bench"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn make_pixmap(seed: u32) -> Pixmap {
    let mut pm = Pixmap::new(REGION_PX, REGION_PX);
    let r = ((seed.wrapping_mul(0xa)) & 0xff) as u8;
    let g = ((seed.wrapping_mul(0xb)) & 0xff) as u8;
    let b = ((seed.wrapping_mul(0xc)) & 0xff) as u8;
    pm.fill([r, g, b, 255]);
    pm
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx-bench-target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn region_layout(n: usize) -> Vec<(RegionId, [f32; 4])> {
    // Grid of REGION_PX × REGION_PX cells across the screen.
    let cols = (n as f32).sqrt().ceil() as u32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let x = (col * REGION_PX) as f32;
        let y = (row * REGION_PX) as f32;
        out.push((RegionId(i as u64), [x, y, REGION_PX as f32, REGION_PX as f32]));
    }
    out
}

fn run_composite_with_target(
    backend:   &mut HybridBackend,
    device:    &wgpu::Device,
    queue:     &wgpu::Queue,
    target_view: &wgpu::TextureView,
    instances: &[(RegionId, QuadInstance)],
) {
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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

fn build_seeded_backend(
    device:   &wgpu::Device,
    queue:    &wgpu::Queue,
    cfg:      UrxConfig,
    layout:   &[(RegionId, [f32; 4])],
) -> HybridBackend {
    let mut backend = HybridBackend::with_config(cfg);
    for (id, _) in layout {
        let pm = make_pixmap(id.0 as u32);
        backend.upsert_region_pixmap(device, queue, *id, &pm);
    }
    backend
}

fn build_instances(layout: &[(RegionId, [f32; 4])]) -> Vec<(RegionId, QuadInstance)> {
    layout.iter().map(|(id, rect)| {
        (*id, QuadInstance::new(rect[0], rect[1], rect[2], rect[3]))
    }).collect()
}

fn bench_warm_static_ab(c: &mut Criterion) {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter — skipping hybrid atlas_ab benches");
        return;
    };
    let (_target, target_view) = make_target(&device);

    let mut g = c.benchmark_group("hybrid_atlas_ab");
    for &n in &[8usize, 32, 64, 128, 256] {
        let layout = region_layout(n);
        let instances = build_instances(&layout);

        // A0: atlas off (default 1.4.2 path).
        g.bench_with_input(
            BenchmarkId::new("atlas_off", n), &n,
            |b, _| {
                let mut backend = build_seeded_backend(
                    &device, &queue, UrxConfig::default(), &layout,
                );
                b.iter(|| {
                    run_composite_with_target(&mut backend, &device, &queue, &target_view, &instances);
                    black_box(&backend);
                });
            }
        );

        // B1: atlas on, coalesce off.
        g.bench_with_input(
            BenchmarkId::new("atlas_on_no_coalesce", n), &n,
            |b, _| {
                let cfg = UrxConfig::builder()
                    .hybrid_atlas_enabled(true)
                    .hybrid_instanced_composite(false)
                    .build().unwrap();
                let mut backend = build_seeded_backend(&device, &queue, cfg, &layout);
                b.iter(|| {
                    run_composite_with_target(&mut backend, &device, &queue, &target_view, &instances);
                    black_box(&backend);
                });
            }
        );

        // B2: atlas on, coalesce on (the full B6+B7 win).
        g.bench_with_input(
            BenchmarkId::new("atlas_on_coalesce", n), &n,
            |b, _| {
                let cfg = UrxConfig::builder()
                    .hybrid_atlas_enabled(true)
                    .hybrid_instanced_composite(true)
                    .build().unwrap();
                let mut backend = build_seeded_backend(&device, &queue, cfg, &layout);
                b.iter(|| {
                    run_composite_with_target(&mut backend, &device, &queue, &target_view, &instances);
                    black_box(&backend);
                });
            }
        );
    }
    g.finish();
}

criterion_group!(benches, bench_warm_static_ab);
criterion_main!(benches);
