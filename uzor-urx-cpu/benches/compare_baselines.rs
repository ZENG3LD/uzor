//! Apples-to-apples baseline comparison: URX-CPU vs vello_cpu vs tiny_skia.
//!
//! All three rasterise the SAME scene of N opaque-coloured rects into
//! a fresh RGBA8 pixmap. The choice of "opaque-coloured rect" is
//! deliberate — it's what UI/charts dominate. Path rendering, gradient,
//! text, etc. are not measured here (separate concerns).
//!
//! Run:
//!   cargo bench -p uzor-urx-cpu --bench compare_baselines
//!
//! Numbers are per-frame wall-clock at criterion's default 100 samples.

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene as UrxScene};
use uzor_urx_cpu::{CpuBackend, Pixmap as UrxPixmap};

fn rng_u32(i: usize, m: usize) -> u32 {
    let v = (i.wrapping_mul(2654435761) ^ m) as u64;
    ((v >> 16) & 0xFFFF) as u32
}

/// Common rect schedule used by all three backends — same coords, same
/// colors, same z-order. Eliminates "different scene" as a source of
/// difference.
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
            rects.push((x, y, sx, sy, [r, g, b, 200]));
        }
        Self { rects }
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

fn bench_urx_cpu(b: &mut criterion::Bencher, rects: &CommonRects, w: u32, h: u32) {
    let scene = rects.to_urx_scene();
    let backend = CpuBackend::new();
    b.iter(|| {
        let mut p = UrxPixmap::new(w, h);
        backend.render(black_box(&scene), &mut p).unwrap();
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

fn bench_tiny_skia(b: &mut criterion::Bencher, rects: &CommonRects, w: u32, h: u32) {
    use tiny_skia::{Paint, Rect as TSRect, Transform};
    b.iter(|| {
        let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
        for &(x, y, sx, sy, c) in &rects.rects {
            let mut paint = Paint::default();
            paint.set_color_rgba8(c[0], c[1], c[2], c[3]);
            paint.anti_alias = true;
            let r = TSRect::from_xywh(x, y, sx, sy).unwrap();
            pixmap.fill_rect(r, &paint, Transform::identity(), None);
        }
        black_box(pixmap)
    });
}

fn bench_compare(c: &mut Criterion) {
    let sizes: &[(u32, u32, &str)] = &[
        (400, 300, "400x300"),
        (1280, 720, "1280x720"),
        (1920, 1080, "1920x1080"),
    ];
    let complexities = [10usize, 100, 1000];

    let mut group = c.benchmark_group("baseline_fill_rects");
    for &(w, h, label) in sizes {
        for &n in &complexities {
            let rects = CommonRects::build(w, h, n);
            let id_urx   = BenchmarkId::new(format!("URX/{}", label), n);
            let id_vello = BenchmarkId::new(format!("vello_cpu/{}", label), n);
            let id_ts    = BenchmarkId::new(format!("tiny_skia/{}", label), n);
            group.bench_with_input(id_urx,   &rects, |b, r| bench_urx_cpu  (b, r, w, h));
            group.bench_with_input(id_vello, &rects, |b, r| bench_vello_cpu(b, r, w, h));
            group.bench_with_input(id_ts,    &rects, |b, r| bench_tiny_skia(b, r, w, h));
        }
    }
    group.finish();
}

criterion_group!(benches, bench_compare);
criterion_main!(benches);
