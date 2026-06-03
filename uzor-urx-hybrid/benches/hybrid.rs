//! Hybrid pipeline benches: cold-frame / warm-static / warm-delta.
//! Measures CPU rasterise → upload → composite → finish.
//!
//! Cold-frame: per iteration, fresh scene + fresh upload + composite.
//!             Tests one-shot rendering cost (worst case).
//! Warm-static: regions uploaded once, every subsequent composite
//!              reuses textures — tests the dirty-skip win.
//! Warm-delta: 10% of regions change/iter — tests partial re-upload.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};
use uzor_urx_hybrid::{HybridBackend, QuadInstance};

const W: u32 = 1920;
const H: u32 = 1080;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx-hybrid-bench"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn build_region_pixmap(w: u32, h: u32, seed: u32) -> Pixmap {
    // Render a small scene of rects into this region's pixmap.
    let mut p = Pixmap::new(w, h);
    let mut s = Scene::new();
    for i in 0..16 {
        let x = ((seed.wrapping_mul(i + 1)) % w.max(2)) as f64;
        let y = ((seed.wrapping_mul(i + 7)) % h.max(2)) as f64;
        let sx = 4.0 + ((seed.wrapping_mul(i + 13)) % 32) as f64;
        let sy = 4.0 + ((seed.wrapping_mul(i + 17)) % 32) as f64;
        let r = ((seed.wrapping_mul(i + 3)) & 0xff) as u8;
        let g = ((seed.wrapping_mul(i + 5)) & 0xff) as u8;
        let b = ((seed.wrapping_mul(i + 11)) & 0xff) as u8;
        s.push(DrawCommand::FillRect {
            rect: Rect::new(x, y, (x + sx).min(w as f64), (y + sy).min(h as f64)),
            radii: None,
            brush: Brush::Solid(Color::rgba8(r, g, b, 200)),
            transform: Affine::IDENTITY,
        });
    }
    CpuBackend::new().render(&s, &mut p).unwrap();
    p
}

/// Layout: N regions of equal size laid out in a grid.
fn region_layout(n_regions: usize) -> Vec<(RegionId, u32, u32, u32, u32)> {
    // Compute the closest square grid that fits n_regions.
    let cols = (n_regions as f32).sqrt().ceil() as u32;
    let rows = ((n_regions as f32) / (cols as f32)).ceil() as u32;
    let region_w = W / cols.max(1);
    let region_h = H / rows.max(1);
    let mut out = Vec::with_capacity(n_regions);
    for i in 0..n_regions {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        out.push((
            RegionId(i as u64),
            col * region_w,
            row * region_h,
            region_w,
            region_h,
        ));
    }
    out
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hybrid-bench-target"),
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

fn bench_cold_frame(c: &mut Criterion) {
    let Some((device, queue)) = init_device() else {
        eprintln!("no GPU adapter — skipping hybrid benches");
        return;
    };
    let mut g = c.benchmark_group("hybrid_cold_frame");
    for &n in &[8usize, 16, 64] {
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n_regions| {
            let layout = region_layout(n_regions);
            // Pre-build pixmaps OUTSIDE the timing loop — cold-frame
            // measures upload + composite, not CPU rasterise.
            let pixmaps: Vec<Pixmap> = layout.iter()
                .map(|(_, _, _, w, h)| build_region_pixmap(*w, *h, 42))
                .collect();
            let (_target, target_view) = make_target(&device);
            b.iter(|| {
                // Fresh HybridBackend per iter → fresh GPU textures.
                let mut backend = HybridBackend::new();
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                // Clear target.
                {
                    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &target_view,
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
                // Upload + composite.
                let mut instances = Vec::with_capacity(layout.len());
                for ((id, x, y, w, h), pm) in layout.iter().zip(pixmaps.iter()) {
                    backend.upsert_region_pixmap(&device, &queue, *id, pm);
                    instances.push((*id, QuadInstance::new(*x as f32, *y as f32, *w as f32, *h as f32)));
                }
                backend.composite(&device, &queue, &mut enc, &target_view,
                    wgpu::TextureFormat::Rgba8Unorm, W, H, &instances);
                queue.submit(Some(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                black_box(&backend);
            });
        });
    }
    g.finish();
}

fn bench_warm_static(c: &mut Criterion) {
    let Some((device, queue)) = init_device() else { return; };
    let mut g = c.benchmark_group("hybrid_warm_static");
    for &n in &[8usize, 16, 64] {
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n_regions| {
            let layout = region_layout(n_regions);
            let pixmaps: Vec<Pixmap> = layout.iter()
                .map(|(_, _, _, w, h)| build_region_pixmap(*w, *h, 42))
                .collect();
            let (_target, target_view) = make_target(&device);

            // Setup OUTSIDE timing: upload regions once. Inside the
            // bench loop only composite runs.
            let mut backend = HybridBackend::new();
            for ((id, _, _, _, _), pm) in layout.iter().zip(pixmaps.iter()) {
                backend.upsert_region_pixmap(&device, &queue, *id, pm);
            }
            let instances: Vec<_> = layout.iter()
                .map(|(id, x, y, w, h)| (*id, QuadInstance::new(*x as f32, *y as f32, *w as f32, *h as f32)))
                .collect();
            b.iter(|| {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                {
                    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &target_view,
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
                backend.composite(&device, &queue, &mut enc, &target_view,
                    wgpu::TextureFormat::Rgba8Unorm, W, H, &instances);
                queue.submit(Some(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                black_box(&backend);
            });
        });
    }
    g.finish();
}

/// Warm-delta: 10% of regions change every iteration. Re-uploads the
/// stale 10%, leaves the other 90% intact. Tests partial-upsert cost
/// in the steady state.
fn bench_warm_delta(c: &mut Criterion) {
    let Some((device, queue)) = init_device() else { return; };
    let mut g = c.benchmark_group("hybrid_warm_delta");
    for &n in &[8usize, 16, 64] {
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n_regions| {
            let layout = region_layout(n_regions);
            // Two seeds → two distinct pixmap colour generations.
            let pixmaps_v1: Vec<Pixmap> = layout.iter()
                .map(|(_, _, _, w, h)| build_region_pixmap(*w, *h, 42))
                .collect();
            let pixmaps_v2: Vec<Pixmap> = layout.iter()
                .map(|(_, _, _, w, h)| build_region_pixmap(*w, *h, 137))
                .collect();
            let (_target, target_view) = make_target(&device);

            // Seed all regions with v1 once OUTSIDE timing.
            let mut backend = HybridBackend::new();
            for ((id, _, _, _, _), pm) in layout.iter().zip(pixmaps_v1.iter()) {
                backend.upsert_region_pixmap(&device, &queue, *id, pm);
            }
            let instances: Vec<_> = layout.iter()
                .map(|(id, x, y, w, h)| (*id, QuadInstance::new(*x as f32, *y as f32, *w as f32, *h as f32)))
                .collect();
            // Number of regions to mutate per frame.
            let delta = (n_regions / 10).max(1);
            let mut tick = 0usize;
            b.iter(|| {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                {
                    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &target_view,
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
                // Re-upsert `delta` regions, alternating v1/v2 each tick.
                for i in 0..delta {
                    let idx = (tick + i) % layout.len();
                    let (id, _, _, _, _) = layout[idx];
                    let pm = if (tick & 1) == 0 { &pixmaps_v2[idx] } else { &pixmaps_v1[idx] };
                    backend.upsert_region_pixmap(&device, &queue, id, pm);
                }
                tick += 1;
                backend.composite(&device, &queue, &mut enc, &target_view,
                    wgpu::TextureFormat::Rgba8Unorm, W, H, &instances);
                queue.submit(Some(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                black_box(&backend);
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench_cold_frame, bench_warm_static, bench_warm_delta);
criterion_main!(benches);
