//! URX-tile vs URX-scanline vs vello_cpu vs tiny_skia — same scene.
//!
//! Runs the SAME N-rect schedule through:
//!   * `CpuBackend::render` — scanline + SIMD span fill (1.4.1 default)
//!   * `tile::render_tiled` — coarse 256×8 tile classifier + bg-replace
//!   * vello_cpu — for reference
//!   * tiny_skia — for reference

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene as UrxScene};
use uzor_urx_cpu::{CpuBackend, Pixmap as UrxPixmap};

fn rng_u32(i: usize, m: usize) -> u32 {
    let v = (i.wrapping_mul(2654435761) ^ m) as u64;
    ((v >> 16) & 0xFFFF) as u32
}

struct CommonRects {
    rects: Vec<(f32, f32, f32, f32, [u8; 4])>,
}

impl CommonRects {
    fn build(w: u32, h: u32, n: usize) -> Self {
        let mut rects = Vec::with_capacity(n);
        for i in 0..n {
            let x = (rng_u32(i, 0xa) as f32 / 0xFFFF as f32) * (w as f32 - 50.0);
            let y = (rng_u32(i, 0xb) as f32 / 0xFFFF as f32) * (h as f32 - 50.0);
            let sx = 8.0 + (rng_u32(i, 0xc) as f32 / 0xFFFF as f32) * 40.0;
            let sy = 8.0 + (rng_u32(i, 0xd) as f32 / 0xFFFF as f32) * 40.0;
            let r  = (rng_u32(i, 0xe) & 0xFF) as u8;
            let g  = (rng_u32(i, 0xf) & 0xFF) as u8;
            let b  = (rng_u32(i, 0x10) & 0xFF) as u8;
            // 200 (semi-transparent) — same as original compare bench.
            rects.push((x, y, sx, sy, [r, g, b, 200]));
        }
        Self { rects }
    }

    /// Variant that uses OPAQUE alpha — meant to maximally exercise
    /// the tile bg-replacement fast path. This is the realistic UI
    /// scenario: solid panels stacked, with transparent overlays only
    /// at the top of the z-order.
    fn build_opaque(w: u32, h: u32, n: usize) -> Self {
        let mut s = Self::build(w, h, n);
        for r in s.rects.iter_mut() {
            r.4[3] = 255;
        }
        s
    }

    fn to_urx_scene(&self) -> UrxScene {
        let mut s = UrxScene::new();
        for &(x, y, sx, sy, c) in &self.rects {
            s.push(DrawCommand::FillRect {
                rect: Rect::new(x as f64, y as f64, (x + sx) as f64, (y + sy) as f64),
                radii: None,
                brush: Brush::Solid(Color::rgba8(c[0], c[1], c[2], c[3])),
                transform: Affine::IDENTITY,
            });
        }
        s
    }
}

fn bench_urx_scanline(b: &mut criterion::Bencher, rects: &CommonRects, w: u32, h: u32) {
    let scene = rects.to_urx_scene();
    let backend = CpuBackend::new();
    b.iter(|| {
        let mut p = UrxPixmap::new(w, h);
        backend.render(black_box(&scene), &mut p).unwrap();
        black_box(p)
    });
}

fn bench_urx_tiled(b: &mut criterion::Bencher, rects: &CommonRects, w: u32, h: u32) {
    let scene = rects.to_urx_scene();
    b.iter(|| {
        let mut p = UrxPixmap::new(w, h);
        uzor_urx_cpu::tile::render_tiled(black_box(&scene), &mut p);
        black_box(p)
    });
}

fn bench_vello_cpu(b: &mut criterion::Bencher, rects: &CommonRects, w: u32, h: u32) {
    use vello_cpu::{Pixmap as VPixmap, RenderContext as VCtx};
    use vello_cpu::color::{AlphaColor, Srgb};
    use vello_cpu::kurbo::Rect as KRect;
    let arc = Arc::new(rects.rects.clone());
    b.iter(|| {
        let mut ctx = VCtx::new(w as u16, h as u16);
        for &(x, y, sx, sy, c) in arc.iter() {
            let color = AlphaColor::<Srgb>::from_rgba8(c[0], c[1], c[2], c[3]);
            ctx.set_paint(color);
            ctx.fill_rect(&KRect::new(x as f64, y as f64, (x + sx) as f64, (y + sy) as f64));
        }
        let mut p = VPixmap::new(w as u16, h as u16);
        ctx.render_to_pixmap(&mut p);
        black_box(p)
    });
}

fn bench_compare(c: &mut Criterion) {
    let sizes: &[(u32, u32, &str)] = &[
        (400, 300, "400x300"),
        (1280, 720, "1280x720"),
        (1920, 1080, "1920x1080"),
    ];
    let complexities = [10usize, 100, 1000, 10_000];

    // Semi-transparent rects (alpha 200) — pessimistic case for tile
    // bg-replacement (NO triggers).
    let mut group = c.benchmark_group("semi_transparent");
    for &(w, h, label) in sizes {
        for &n in &complexities {
            let rects = CommonRects::build(w, h, n);
            group.bench_with_input(BenchmarkId::new(format!("URX-scanline/{}", label), n),
                &rects, |b, r| bench_urx_scanline(b, r, w, h));
            group.bench_with_input(BenchmarkId::new(format!("URX-tiled/{}", label), n),
                &rects, |b, r| bench_urx_tiled(b, r, w, h));
            group.bench_with_input(BenchmarkId::new(format!("vello_cpu/{}", label), n),
                &rects, |b, r| bench_vello_cpu(b, r, w, h));
        }
    }
    group.finish();

    // Opaque rects (alpha 255) — best case for tile bg-replacement.
    // 1000 opaque overlapping rects → most tiles see massive overdraw
    // collapse to a single memset.
    let mut group = c.benchmark_group("opaque");
    for &(w, h, label) in sizes {
        for &n in &complexities {
            let rects = CommonRects::build_opaque(w, h, n);
            group.bench_with_input(BenchmarkId::new(format!("URX-scanline/{}", label), n),
                &rects, |b, r| bench_urx_scanline(b, r, w, h));
            group.bench_with_input(BenchmarkId::new(format!("URX-tiled/{}", label), n),
                &rects, |b, r| bench_urx_tiled(b, r, w, h));
            group.bench_with_input(BenchmarkId::new(format!("vello_cpu/{}", label), n),
                &rects, |b, r| bench_vello_cpu(b, r, w, h));
        }
    }
    group.finish();
}

criterion_group!(benches, bench_compare);
criterion_main!(benches);
