//! Bench: full-GPU compute pipeline vs CPU backend across N rect counts.
//!
//! Two backends, two measurement modes, four cmd-counts:
//!
//!   N ∈ {100, 1_000, 10_000, 100_000}
//!
//!   full_gpu_dispatch_only/<N>  — just encode → assign → sort → fine,
//!                                  measure compute pipeline cost only
//!                                  (what a real consumer sees when
//!                                  rendering in-place to a swapchain
//!                                  surface — no readback)
//!
//!   full_gpu_with_readback/<N>  — same plus copy_texture_to_buffer +
//!                                  map_async readback (headless /
//!                                  screenshot path; useful for
//!                                  diagnostics but NOT representative
//!                                  of frame-on-screen cost)
//!
//!   cpu_backend/<N>             — Scene + CpuBackend::render() into
//!                                  Pixmap (urx-cpu tile pipeline)
//!
//! Scale comparison reveals where the GPU dispatch overhead is amortised:
//! at N=100 the kernel launch + readback dominate; at N=10k+ the GPU
//! pipeline cost flattens while CPU scales ~linearly.
//!
//! Run: cargo bench -p uzor-urx-wgpu-full --bench full_gpu_vs_cpu

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use uzor_urx_cpu::{CpuBackend, Pixmap};
use uzor_urx_core::{
    math::{Color, Rect},
    scene::Scene,
};
use uzor_urx_wgpu_full::{
    cmd::SceneCmd,
    tile::{TileBuffers, TilePipeline, TILE_SIZE},
};

const W: u32 = 1920;
const H: u32 = 1080;
const N_VALUES: &[u32] = &[100, 1_000, 10_000, 100_000];

/// Splitmix64 deterministic RNG.
fn build_rng(seed: u64) -> impl FnMut() -> u32 {
    let mut s: u64 = seed;
    move || {
        s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
}

/// Build SceneCmd list (GPU encoding) for N rects.
fn build_gpu_cmds(n: u32) -> Vec<SceneCmd> {
    let mut rng = build_rng(0xa3b1_c2d4_e5f6_0708);
    let f01 = |x: u32| (x & 0xffff) as f32 / 65535.0;
    (0..n).map(|_| {
        let x0 = f01(rng()) * (W as f32 - 100.0);
        let y0 = f01(rng()) * (H as f32 - 100.0);
        // Smaller rects at higher N to keep total coverage bounded —
        // otherwise overdraw dominates and the bench is meaningless.
        let max_size = if n <= 1_000 { 200.0 }
                       else if n <= 10_000 { 80.0 }
                       else { 30.0 };
        let w  = 10.0 + f01(rng()) * max_size;
        let h  = 10.0 + f01(rng()) * max_size;
        let r  = (rng() & 0xff) as u8;
        let g  = (rng() & 0xff) as u8;
        let b  = (rng() & 0xff) as u8;
        SceneCmd::rect(x0, y0, x0 + w, y0 + h, [r, g, b, 255])
    }).collect()
}

/// Build Scene (CPU encoding) for N rects — same coords as GPU side.
fn build_cpu_scene(n: u32) -> Scene {
    let mut rng = build_rng(0xa3b1_c2d4_e5f6_0708);
    let f01 = |x: u32| (x & 0xffff) as f32 / 65535.0;
    let mut scene = Scene::new();
    let max_size = if n <= 1_000 { 200.0 }
                   else if n <= 10_000 { 80.0 }
                   else { 30.0 };
    for _ in 0..n {
        let x0 = f01(rng()) * (W as f32 - 100.0);
        let y0 = f01(rng()) * (H as f32 - 100.0);
        let w  = 10.0 + f01(rng()) * max_size;
        let h  = 10.0 + f01(rng()) * max_size;
        let r  = (rng() & 0xff) as u8;
        let g  = (rng() & 0xff) as u8;
        let b  = (rng() & 0xff) as u8;
        scene.fill_rect_solid(
            Rect::new(x0 as f64, y0 as f64, (x0 + w) as f64, (y0 + h) as f64),
            Color::rgba8(r, g, b, 255),
        );
    }
    scene
}

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let mut limits = wgpu::Limits::default();
    // For N=100_000 cmds we need a bigger storage buffer than the
    // default 128 MiB cap allows in some adapter limits. Default
    // wgpu::Limits already gives 128 MiB max_storage_buffer_binding_size
    // which covers 100k * 32 = 3.2 MB easily, but we bump just in case.
    limits.max_storage_buffer_binding_size = limits.max_storage_buffer_binding_size.max(256 << 20);
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-bench-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   limits,
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()
}

fn bench_full_gpu(c: &mut Criterion) {
    let Some((device, queue)) = init_device() else {
        eprintln!("[bench] no GPU adapter — skipping full_gpu bench");
        return;
    };

    let pipeline = TilePipeline::new(&device);
    let (_dummy_tex_d, dummy_atlas_view_d) = TilePipeline::dummy_glyph_atlas(&device);
    let mut g_dispatch = c.benchmark_group("full_gpu_dispatch_only");
    g_dispatch.sample_size(50);

    for &n in N_VALUES {
        let cmds = build_gpu_cmds(n);
        let (bufs, _output_tex, output_view) =
            TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);

        g_dispatch.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut enc = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("bench-enc") },
                );
                pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &[], &output_view, &dummy_atlas_view_d);
                queue.submit(Some(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None, timeout: None,
                });
            });
        });
    }
    g_dispatch.finish();

    let mut g_readback = c.benchmark_group("full_gpu_with_readback");
    g_readback.sample_size(30);

    for &n in N_VALUES {
        let cmds = build_gpu_cmds(n);
        let (bufs, output_tex, output_view) =
            TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
        let tex_w = bufs.tile_count_x * TILE_SIZE;
        let tex_h = bufs.tile_count_y * TILE_SIZE;
        let aligned_stride = (tex_w * 4 + 255) & !255;
        let buf_size       = (aligned_stride * tex_h) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bench-staging"),
            size:  buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (_dummy_tex_rb, dummy_atlas_view_rb) = TilePipeline::dummy_glyph_atlas(&device);
        g_readback.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut enc = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("bench-enc-rb") },
                );
                pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &[], &output_view, &dummy_atlas_view_rb);
                enc.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture:   &output_tex,
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
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None, timeout: None,
                });

                let slice = staging.slice(..);
                let (tx, rx) = std::sync::mpsc::channel();
                slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None, timeout: None,
                });
                rx.recv().unwrap().unwrap();
                drop(slice.get_mapped_range());
                staging.unmap();
            });
        });
    }
    g_readback.finish();
}

fn bench_cpu_backend(c: &mut Criterion) {
    let cpu = CpuBackend::new();
    let mut g = c.benchmark_group("cpu_backend");
    g.sample_size(30);

    for &n in N_VALUES {
        // Skip N=100_000 on CPU — scene build alone is multi-second
        // and the render swamps the bench budget. Cover up to 10k.
        if n > 10_000 { continue; }
        let scene = build_cpu_scene(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut pixmap = Pixmap::new(W, H);
                cpu.render(&scene, &mut pixmap).unwrap();
            });
        });
    }
    g.finish();
}

/// Build a MIXED scene: 50% rects, 25% linear gradients, 12.5% radial,
/// 12.5% glyphs. Realistic UI dashboard composition.
fn build_mixed_gpu_cmds(n: u32) -> Vec<SceneCmd> {
    let mut rng = build_rng(0x1234_5678_9abc_def0);
    let f01 = |x: u32| (x & 0xffff) as f32 / 65535.0;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let x0 = f01(rng()) * (W as f32 - 100.0);
        let y0 = f01(rng()) * (H as f32 - 100.0);
        let max_size = if n <= 1_000 { 200.0 }
                       else if n <= 10_000 { 80.0 }
                       else { 30.0 };
        let w  = 10.0 + f01(rng()) * max_size;
        let h  = 10.0 + f01(rng()) * max_size;
        let r  = (rng() & 0xff) as u8;
        let g  = (rng() & 0xff) as u8;
        let b  = (rng() & 0xff) as u8;
        let r2 = (rng() & 0xff) as u8;
        let g2 = (rng() & 0xff) as u8;
        let b2 = (rng() & 0xff) as u8;
        // 50/25/12.5/12.5 split via lower bits of index.
        match i & 0x7 {
            0..=3 => out.push(SceneCmd::rect(x0, y0, x0 + w, y0 + h, [r, g, b, 255])),
            4..=5 => out.push(SceneCmd::lin_gradient(
                        x0, y0, x0 + w, y0 + h,
                        [r, g, b, 255], [r2, g2, b2, 255],
                        (i as u32) % 4,
                    )),
            6     => out.push(SceneCmd::rad_gradient(
                        x0, y0, x0 + w, y0 + h,
                        [r, g, b, 255], [r2, g2, b2, 255],
                    )),
            _     => out.push(SceneCmd::glyph(
                        x0, y0, x0 + w, y0 + h,
                        [r, g, b, 255],
                        [0.0, 0.0, 1.0, 1.0],
                    )),
        }
    }
    out
}

fn bench_full_gpu_mixed(c: &mut Criterion) {
    let Some((device, queue)) = init_device() else {
        eprintln!("[bench] no GPU adapter — skipping mixed bench");
        return;
    };
    let pipeline = TilePipeline::new(&device);
    // Build a small R8Unorm atlas for glyph cmds (constant white).
    let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench-glyph-atlas"),
        size:  wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format:    wgpu::TextureFormat::R8Unorm,
        usage:     wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &atlas_tex, mip_level: 0,
            origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
        },
        &vec![255u8; 64 * 64],
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(64), rows_per_image: Some(64) },
        wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
    );
    let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let mut g = c.benchmark_group("full_gpu_mixed_dispatch_only");
    g.sample_size(50);
    for &n in N_VALUES {
        let cmds = build_mixed_gpu_cmds(n);
        let (bufs, _output_tex, output_view) =
            TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut enc = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("mixed-bench-enc") },
                );
                pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &output_view, &atlas_view);
                queue.submit(Some(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None, timeout: None,
                });
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench_full_gpu, bench_full_gpu_mixed, bench_cpu_backend);
criterion_main!(benches);
