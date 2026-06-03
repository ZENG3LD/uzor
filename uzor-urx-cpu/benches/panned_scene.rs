//! Panned-scene benchmark — scene 6 from 14-handoff:
//!
//! > 1000 rects shifted -800 px on x. Half are off-screen-left.
//! > Catches CulledWindings missing.
//!
//! We do three variants: in-view (no pan, baseline), half-pan (50%
//! off-screen-left), full-pan (90% off-screen-left). All three run
//! through URX-scanline, URX-tile, and vello_cpu for comparison.
//!
//! The win we expect from CPU-1 CulledWindings: tile path's
//! coordinate transformation already clips off-screen rects via
//! `Rect::intersect`, BUT the per-rect work is still O(1) per
//! off-screen rect. The point of CulledWindings is to make
//! AET-style algorithms (used by `fill_path_aa`, `stroke_path_aa`)
//! O(active rows) instead of O(visible-after-clip rows). For pure
//! axis-aligned FillRect — which is the tile path's target —
//! benefit is smaller. We bench both to know what we're getting.

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene as UrxScene};
use uzor_urx_cpu::{CpuBackend, Pixmap as UrxPixmap};

const W: u32 = 1920;
const H: u32 = 1080;

fn rng_u32(i: usize, m: usize) -> u32 {
    let v = (i.wrapping_mul(2654435761) ^ m) as u64;
    ((v >> 16) & 0xFFFF) as u32
}

/// Build N rects, then optionally apply a horizontal pan.
fn build_rects(n: usize, pan_x: f32) -> Vec<(f32, f32, f32, f32, [u8; 4])> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = (rng_u32(i, 0xa) as f32 / 0xFFFF as f32) * (W as f32 - 50.0);
        let y = (rng_u32(i, 0xb) as f32 / 0xFFFF as f32) * (H as f32 - 50.0);
        let sx = 8.0 + (rng_u32(i, 0xc) as f32 / 0xFFFF as f32) * 40.0;
        let sy = 8.0 + (rng_u32(i, 0xd) as f32 / 0xFFFF as f32) * 40.0;
        let r  = (rng_u32(i, 0xe) & 0xFF) as u8;
        let g  = (rng_u32(i, 0xf) & 0xFF) as u8;
        let b  = (rng_u32(i, 0x10) & 0xFF) as u8;
        out.push((x + pan_x, y, sx, sy, [r, g, b, 255]));
    }
    out
}

fn to_scene(rects: &[(f32, f32, f32, f32, [u8; 4])]) -> UrxScene {
    let mut s = UrxScene::new();
    for &(x, y, sx, sy, c) in rects {
        s.push(DrawCommand::FillRect {
            rect: Rect::new(x as f64, y as f64, (x + sx) as f64, (y + sy) as f64),
            radii: None,
            brush: Brush::Solid(Color::rgba8(c[0], c[1], c[2], c[3])),
            transform: Affine::IDENTITY,
        });
    }
    s
}

fn bench_urx_scanline(b: &mut criterion::Bencher, scene: &UrxScene) {
    let backend = CpuBackend::new();
    b.iter(|| {
        let mut p = UrxPixmap::new(W, H);
        backend.render(black_box(scene), &mut p).unwrap();
        black_box(p)
    });
}

fn bench_urx_tile(b: &mut criterion::Bencher, scene: &UrxScene) {
    b.iter(|| {
        let mut p = UrxPixmap::new(W, H);
        uzor_urx_cpu::tile::render_tiled(black_box(scene), &mut p);
        black_box(p)
    });
}

fn bench_vello_cpu(b: &mut criterion::Bencher, rects: &[(f32, f32, f32, f32, [u8; 4])]) {
    use vello_cpu::{Pixmap as VPixmap, RenderContext as VCtx};
    use vello_cpu::color::{AlphaColor, Srgb};
    use vello_cpu::kurbo::Rect as KRect;
    let arc = Arc::new(rects.to_vec());
    b.iter(|| {
        let mut ctx = VCtx::new(W as u16, H as u16);
        for &(x, y, sx, sy, c) in arc.iter() {
            let color = AlphaColor::<Srgb>::from_rgba8(c[0], c[1], c[2], c[3]);
            ctx.set_paint(color);
            ctx.fill_rect(&KRect::new(x as f64, y as f64, (x + sx) as f64, (y + sy) as f64));
        }
        let mut p = VPixmap::new(W as u16, H as u16);
        ctx.flush();
        ctx.render_to_pixmap(&mut p);
        black_box(p);
    });
}

fn bench_pan(c: &mut Criterion) {
    let mut g = c.benchmark_group("panned_scene_1920x1080");
    for &n in &[1_000usize, 10_000] {
        for &(label, pan_frac) in &[
            ("no_pan",   0.0_f32),
            ("half_pan", -(W as f32) * 0.5),
            ("full_pan", -(W as f32) * 0.9),
        ] {
            let rects = build_rects(n, pan_frac);
            let scene = to_scene(&rects);

            g.bench_with_input(
                BenchmarkId::new(format!("URX-scan/{label}"), n),
                &n,
                |b, _| bench_urx_scanline(b, &scene),
            );
            g.bench_with_input(
                BenchmarkId::new(format!("URX-tile/{label}"), n),
                &n,
                |b, _| bench_urx_tile(b, &scene),
            );
            g.bench_with_input(
                BenchmarkId::new(format!("vello/{label}"), n),
                &n,
                |b, _| bench_vello_cpu(b, &rects),
            );
        }
    }
    g.finish();
}

criterion_group!(benches, bench_pan);
criterion_main!(benches);
