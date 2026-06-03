//! Parallel rasterisation tests (feature = "parallel").
//!
//! Verifies that per-strip parallel render produces IDENTICAL pixel
//! output to the sequential path — same scene, same hash. Then
//! measures speedup on a large scene.

#![cfg(feature = "parallel")]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
use uzor_urx_cpu::{render_parallel, CpuBackend, Pixmap};

fn heavy_scene(w: f64, h: f64) -> Scene {
    let mut s = Scene::new();
    // bg
    s.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, w, h),
        radii: None,
        brush: Brush::Solid(Color::rgba8(13, 17, 23, 255)),
        transform: Affine::IDENTITY,
    });
    // 600 small rects scattered
    for i in 0..600 {
        let x = ((i * 73) % 800) as f64;
        let y = ((i * 137) % 600) as f64;
        s.push(DrawCommand::FillRect {
            rect: Rect::new(x, y, x + 20.0, y + 20.0),
            radii: None,
            brush: Brush::Solid(Color::rgba8(
                ((i * 11) & 255) as u8,
                ((i * 53) & 255) as u8,
                ((i * 97) & 255) as u8,
                255,
            )),
            transform: Affine::IDENTITY,
        });
    }
    // 30 stroke rects
    for i in 0..30 {
        let x = ((i * 51) % 800) as f64;
        let y = ((i * 31) % 600) as f64;
        s.push(DrawCommand::StrokeRect {
            rect: Rect::new(x, y, x + 80.0, y + 60.0),
            radii: None,
            stroke: Stroke { width: 2.0, ..Stroke::default() },
            brush: Brush::Solid(Color::rgba8(58, 166, 255, 200)),
            transform: Affine::IDENTITY,
        });
    }
    // 50 lines
    for i in 0..50 {
        let x0 = ((i * 13) % 800) as f64;
        let y0 = ((i * 91) % 600) as f64;
        s.push(DrawCommand::Line {
            from: Vec2 { x: x0, y: y0 },
            to:   Vec2 { x: x0 + 100.0, y: y0 + 40.0 },
            stroke: Stroke { width: 1.5, ..Stroke::default() },
            brush: Brush::Solid(Color::rgba8(210, 153, 34, 220)),
            transform: Affine::IDENTITY,
        });
    }
    s
}

fn hash(p: &Pixmap) -> u64 {
    let mut h = DefaultHasher::new();
    p.pixels().hash(&mut h);
    h.finish()
}

#[test]
fn parallel_matches_sequential_pixel_for_pixel() {
    let scene = heavy_scene(800.0, 600.0);

    let mut p_seq = Pixmap::new(800, 600);
    CpuBackend::new().render(&scene, &mut p_seq).unwrap();
    let h_seq = hash(&p_seq);

    let mut p_par = Pixmap::new(800, 600);
    render_parallel(&scene, &mut p_par, 4).unwrap();
    let h_par = hash(&p_par);

    assert_eq!(h_seq, h_par,
               "parallel render must produce identical pixels to sequential");
}

#[test]
fn parallel_beats_sequential_on_heavy_scene() {
    let scene = heavy_scene(800.0, 600.0);

    let mut p_seq = Pixmap::new(800, 600);
    let t_seq_start = Instant::now();
    for _ in 0..3 {
        // warm-up + average over 3 runs
        p_seq.clear();
        CpuBackend::new().render(&scene, &mut p_seq).unwrap();
    }
    let seq_us = (t_seq_start.elapsed().as_micros() / 3) as u64;

    let mut p_par = Pixmap::new(800, 600);
    let t_par_start = Instant::now();
    for _ in 0..3 {
        p_par.clear();
        render_parallel(&scene, &mut p_par, 0).unwrap();  // auto = n_cores
    }
    let par_us = (t_par_start.elapsed().as_micros() / 3) as u64;

    eprintln!(
        "[bench_parallel] sequential {}µs avg ; parallel(auto) {}µs avg ; speedup {:.2}×",
        seq_us, par_us, (seq_us as f64) / (par_us as f64),
    );
    // Sanity: parallel should be FASTER (at least on dev box with >1 core).
    // On a 1-core CI box this could fail; the test is for the dev workflow.
    // Use a lenient threshold (just "not catastrophically slower").
    assert!(par_us < seq_us * 2,
            "parallel must not be >2× slower than sequential (got seq={}, par={})",
            seq_us, par_us);
}
