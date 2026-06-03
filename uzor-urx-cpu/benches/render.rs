//! URX CPU render benchmarks.
//!
//! Run via `cargo bench -p uzor-urx-cpu`.
//!
//! Matrix: pixmap size × scene complexity × mode (sequential vs parallel).
//! Sizes: 400×300 (small panel), 1280×720 (typical), 1920×1080 (full HD).
//! Complexity: 10 / 100 / 1000 rects.
//! Mode: sequential CpuBackend.render vs parallel render_parallel (4 strips).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn build_scene(w: u32, h: u32, n_rects: usize) -> Scene {
    let mut s = Scene::new();
    let rng = |i: usize, m: usize| -> u32 {
        // Cheap LCG to avoid pulling rand dep.
        let v = (i.wrapping_mul(2654435761) ^ m) as u64;
        ((v >> 16) & 0xFFFF) as u32
    };
    for i in 0..n_rects {
        let x = (rng(i, 0xa) as f64 / 0xFFFF as f64) * (w as f64 - 50.0);
        let y = (rng(i, 0xb) as f64 / 0xFFFF as f64) * (h as f64 - 50.0);
        let sx = 8.0 + (rng(i, 0xc) as f64 / 0xFFFF as f64) * 40.0;
        let sy = 8.0 + (rng(i, 0xd) as f64 / 0xFFFF as f64) * 40.0;
        let r  = (rng(i, 0xe) & 0xFF) as u8;
        let g  = (rng(i, 0xf) & 0xFF) as u8;
        let b  = (rng(i, 0x10) & 0xFF) as u8;
        s.push(DrawCommand::FillRect {
            rect: Rect::new(x, y, x + sx, y + sy),
            radii: None,
            brush: Brush::Solid(Color::rgba8(r, g, b, 200)),
            transform: Affine::IDENTITY,
        });
    }
    s
}

fn bench_render(c: &mut Criterion) {
    let sizes: &[(u32, u32, &str)] = &[
        (400, 300, "400x300"),
        (1280, 720, "1280x720"),
        (1920, 1080, "1920x1080"),
    ];
    let complexities = [10usize, 100, 1000];

    let mut group = c.benchmark_group("cpu_render_sequential");
    for &(w, h, label) in sizes {
        for &n in &complexities {
            let scene = build_scene(w, h, n);
            let id = BenchmarkId::new(label, n);
            group.bench_with_input(id, &scene, |b, scene| {
                let backend = CpuBackend::new();
                b.iter(|| {
                    let mut p = Pixmap::new(w, h);
                    backend.render(black_box(scene), &mut p).unwrap();
                    black_box(p)
                });
            });
        }
    }
    group.finish();

    #[cfg(feature = "parallel")]
    {
        let mut group = c.benchmark_group("cpu_render_parallel_4strips");
        for &(w, h, label) in sizes {
            for &n in &complexities {
                let scene = build_scene(w, h, n);
                let id = BenchmarkId::new(label, n);
                group.bench_with_input(id, &scene, |b, scene| {
                    b.iter(|| {
                        let mut p = Pixmap::new(w, h);
                        uzor_urx_cpu::render_parallel(black_box(scene), &mut p, 4).unwrap();
                        black_box(p)
                    });
                });
            }
        }
        group.finish();
    }
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
