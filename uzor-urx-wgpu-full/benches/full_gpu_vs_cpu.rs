//! Bench: full-GPU pipeline vs CPU backend at 1920×1080 with N=100 random rects.
//!
//! GPU path: encode → tile_assign → tile_sort → fine → readback.
//! CPU path: Scene + CpuBackend::render() into Pixmap.
//!
//! Run: cargo bench -p uzor-urx-wgpu-full --bench full_gpu_vs_cpu

use criterion::{criterion_group, criterion_main, Criterion};
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
const N: u32 = 100;

// Splitmix64 deterministic RNG — same seed as tile_bin_demo.
fn build_rng() -> impl FnMut() -> u32 {
    let mut s: u64 = 0xa3b1_c2d4_e5f6_0708;
    move || {
        s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
}

/// Build SceneCmd list (GPU encoding).
fn build_gpu_cmds() -> Vec<SceneCmd> {
    let mut rng = build_rng();
    let f01 = |x: u32| (x & 0xffff) as f32 / 65535.0;
    (0..N).map(|_| {
        let x0 = f01(rng()) * (W as f32 - 200.0);
        let y0 = f01(rng()) * (H as f32 - 200.0);
        let w  = 20.0 + f01(rng()) * 200.0;
        let h  = 20.0 + f01(rng()) * 200.0;
        let r  = (rng() & 0xff) as u8;
        let g  = (rng() & 0xff) as u8;
        let b  = (rng() & 0xff) as u8;
        SceneCmd::rect(x0, y0, x0 + w, y0 + h, [r, g, b, 255])
    }).collect()
}

/// Build Scene (CPU encoding).
fn build_cpu_scene() -> Scene {
    let mut rng = build_rng();
    let f01 = |x: u32| (x & 0xffff) as f32 / 65535.0;
    let mut scene = Scene::new();
    for _ in 0..N {
        let x0 = f01(rng()) * (W as f32 - 200.0);
        let y0 = f01(rng()) * (H as f32 - 200.0);
        let w  = 20.0 + f01(rng()) * 200.0;
        let h  = 20.0 + f01(rng()) * 200.0;
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
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-bench-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
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

    let cmds = build_gpu_cmds();
    let (bufs, output_tex, output_view) =
        TileBuffers::with_output_texture(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    let tex_w = bufs.tile_count_x * TILE_SIZE;
    let tex_h = bufs.tile_count_y * TILE_SIZE;

    // Aligned readback buffer re-used each iteration to amortise allocation.
    let aligned_stride = (tex_w * 4 + 255) & !255;
    let buf_size       = (aligned_stride * tex_h) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bench-staging"),
        size:  buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    c.bench_function("full_gpu_1920x1080_100rects", |b| {
        b.iter(|| {
            let mut enc = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("bench-enc") },
            );
            pipeline.dispatch_full(&device, &queue, &mut enc, &bufs, &cmds, &output_view);
            // Copy texture to staging buffer in the same encoder pass.
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
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

            // Map + immediately unmap (forces pipeline completion; measures
            // full round-trip including GPU→CPU transfer).
            let slice = staging.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            rx.recv().unwrap().unwrap();
            drop(slice.get_mapped_range());
            staging.unmap();
        });
    });
}

fn bench_cpu_backend(c: &mut Criterion) {
    let scene = build_cpu_scene();
    let cpu   = CpuBackend::new();

    c.bench_function("cpu_backend_1920x1080_100rects", |b| {
        b.iter(|| {
            let mut pixmap = Pixmap::new(W, H);
            cpu.render(&scene, &mut pixmap).unwrap();
        });
    });
}

criterion_group!(benches, bench_full_gpu, bench_cpu_backend);
criterion_main!(benches);
