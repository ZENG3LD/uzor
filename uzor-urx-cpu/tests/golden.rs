//! Golden pixmap dump test — renders the URX-smoke reference scene
//! at 880x720, hashes the resulting pixels, prints the hash.
//!
//! Run: `cargo test -p uzor-urx-cpu --test golden -- --nocapture`
//!
//! First-run baseline: hash is captured + printed. Future regressions
//! show up as a hash mismatch. Hand-eye verify by writing pixels to
//! a PNG with the `--features dump-png` (feature TBD when we ship).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn make_reference_scene(viewport_w: f64, viewport_h: f64) -> Scene {
    let _ = viewport_h;
    let mut scene = Scene::new();

    // Background fill
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, viewport_w, viewport_h),
        radii: None,
        brush: Brush::Solid(Color::rgba8(13, 17, 23, 255)),
        transform: Affine::IDENTITY,
    });

    // Row of 3 fill rects
    for (i, &(r, g, b)) in [(31, 111, 235), (163, 113, 247), (251, 108, 44)].iter().enumerate() {
        let x = 20.0 + (i as f64) * 220.0;
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(x, 60.0, x + 200.0, 140.0),
            radii: None,
            brush: Brush::Solid(Color::rgba8(r, g, b, 255)),
            transform: Affine::IDENTITY,
        });
    }
    // Row of 3 stroke rects
    for (i, &(r, g, b)) in [(63, 185, 80), (88, 166, 255), (240, 136, 62)].iter().enumerate() {
        let x = 20.0 + (i as f64) * 220.0;
        scene.push(DrawCommand::StrokeRect {
            rect: Rect::new(x, 160.0, x + 200.0, 240.0),
            radii: None,
            stroke: Stroke { width: 3.0, ..Stroke::default() },
            brush: Brush::Solid(Color::rgba8(r, g, b, 255)),
            transform: Affine::IDENTITY,
        });
    }
    // Diagonal lines
    for i in 0..10 {
        let x0 = 20.0 + (i as f64) * 70.0;
        let x1 = x0 + 60.0;
        let color = if i & 1 == 0 { Color::rgba8(210, 153, 34, 255) } else { Color::rgba8(188, 140, 255, 255) };
        scene.push(DrawCommand::Line {
            from: Vec2 { x: x0, y: 280.0 },
            to:   Vec2 { x: x1, y: 330.0 },
            stroke: Stroke { width: 2.0, ..Stroke::default() },
            brush: Brush::Solid(color),
            transform: Affine::IDENTITY,
        });
    }
    // N-instance batch
    for ry in 0..6 {
        for cx in 0..40 {
            let x = 20.0 + (cx as f64) * 16.0;
            let y = 360.0 + (ry as f64) * 16.0;
            let color = if (cx + ry) & 1 == 0 { Color::rgba8(22, 27, 34, 255) } else { Color::rgba8(33, 38, 45, 255) };
            scene.push(DrawCommand::FillRect {
                rect: Rect::new(x, y, x + 14.0, y + 14.0),
                radii: None,
                brush: Brush::Solid(color),
                transform: Affine::IDENTITY,
            });
        }
    }
    scene
}

#[test]
fn golden_reference_scene_hash() {
    let (w, h) = (880u32, 720u32);
    let mut pixmap = Pixmap::new(w, h);
    let scene = make_reference_scene(w as f64, h as f64);
    let backend = CpuBackend::new();
    let t0 = std::time::Instant::now();
    backend.render(&scene, &mut pixmap).unwrap();
    let elapsed = t0.elapsed();
    let mut hasher = DefaultHasher::new();
    pixmap.pixels().hash(&mut hasher);
    let h64 = hasher.finish();

    let cmd_count = scene.commands.len();
    let pixel_count = (w as u64) * (h as u64);
    let bytes_per_sec = (pixel_count * 4) as f64 / elapsed.as_secs_f64();
    eprintln!(
        "[golden] {}x{} = {} pixels, {} commands, rendered in {:?} ({:.1} MB/s), pixel-hash = 0x{:016x}",
        w, h, pixel_count, cmd_count, elapsed, bytes_per_sec / 1_000_000.0, h64,
    );

    // Spot-check key pixels — same expectations as the URX-smoke rig.
    // bg pixel (top-left corner)
    assert_eq!(pixmap.get_pixel(2, 2), [13, 17, 23, 255]);
    // First fill rect center @ (120, 100)
    assert_eq!(pixmap.get_pixel(120, 100), [31, 111, 235, 255]);
    // Last fill rect (orange) center @ (560, 100)
    assert_eq!(pixmap.get_pixel(560, 100), [251, 108, 44, 255]);
    // Stroke rect border (top edge of green) @ (120, 160) — should be green-ish
    let c = pixmap.get_pixel(120, 160);
    assert!(c[1] > 30, "stroke top edge must have green, got {:?}", c);
    // Stroke rect INTERIOR — should be background (untouched)
    assert_eq!(pixmap.get_pixel(120, 200), [13, 17, 23, 255]);
}
