//! Inner-loop micro-benchmarks — pinpoint where the per-pixel cost
//! lives BEFORE we throw SIMD at it.
//!
//! Run via `cargo bench -p uzor-urx-cpu --bench inner_loops`.
//! Goal: see ns/pixel for each primitive operation so the SIMD effort
//! lands on the hottest one.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use uzor_urx_cpu::color::premul_scale;
use uzor_urx_cpu::fill::axis_coverage;
use uzor_urx_cpu::Pixmap;

const N: usize = 1920; // one row of 1080p

fn bench_axis_coverage(c: &mut Criterion) {
    let mut g = c.benchmark_group("axis_coverage_per_pixel");
    g.throughput(Throughput::Elements(N as u64));
    g.bench_with_input(BenchmarkId::from_parameter(N), &N, |b, &n| {
        b.iter(|| {
            let mut acc: u32 = 0;
            // Same coverage call the rect fill does per pixel.
            for px in 0..n {
                let cov = axis_coverage(px as f64, px as f64 + 1.0, 1.2, (n - 1) as f64 - 0.7);
                acc = acc.wrapping_add(cov as u32);
            }
            black_box(acc)
        });
    });
    g.finish();
}

fn bench_premul_scale(c: &mut Criterion) {
    let mut g = c.benchmark_group("premul_scale_per_pixel");
    g.throughput(Throughput::Elements(N as u64));
    g.bench_with_input(BenchmarkId::from_parameter(N), &N, |b, &n| {
        let base = [200_u8, 100, 50, 200];
        b.iter(|| {
            let mut acc: u32 = 0;
            for px in 0..n {
                let cov = ((px * 7 + 11) & 0xFF) as u8;
                let out = premul_scale(base, cov);
                acc = acc.wrapping_add(out[3] as u32);
            }
            black_box(acc)
        });
    });
    g.finish();
}

fn bench_blend_pixel(c: &mut Criterion) {
    let mut g = c.benchmark_group("blend_pixel_per_pixel");
    g.throughput(Throughput::Elements(N as u64));
    g.bench_with_input(BenchmarkId::from_parameter(N), &N, |b, &n| {
        b.iter(|| {
            let mut p = Pixmap::new(n as u32, 1);
            for px in 0..n {
                p.blend_pixel(px as u32, 0, [120, 60, 90, 160]);
            }
            black_box(p)
        });
    });
    g.finish();
}

/// Whole rect-fill-row throughput — combines axis_coverage + premul_scale
/// + blend_pixel as they are in the real hot path. The number is the
/// per-pixel cost of the current scalar implementation; SIMD targets
/// this composite.
fn bench_rect_row_full(c: &mut Criterion) {
    use uzor_urx_core::math::{Affine, Brush, Color, Rect};
    use uzor_urx_core::scene::{DrawCommand, Scene};
    use uzor_urx_cpu::CpuBackend;
    let mut g = c.benchmark_group("rect_row_throughput");
    g.throughput(Throughput::Elements(N as u64));
    g.bench_with_input(BenchmarkId::from_parameter(N), &N, |b, &n| {
        let mut s = Scene::new();
        s.push(DrawCommand::FillRect {
            rect: Rect::new(0.5, 0.0, n as f64 - 0.5, 1.0),
            radii: None,
            brush: Brush::Solid(Color::rgba8(120, 60, 90, 200)),
            transform: Affine::IDENTITY,
        });
        let backend = CpuBackend::new();
        b.iter(|| {
            let mut p = Pixmap::new(n as u32, 1);
            backend.render(black_box(&s), &mut p).unwrap();
            black_box(p)
        });
    });
    g.finish();
}

criterion_group!(benches,
    bench_axis_coverage,
    bench_premul_scale,
    bench_blend_pixel,
    bench_rect_row_full,
);
criterion_main!(benches);
