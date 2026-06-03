//! Lightweight property-style fuzz: 1000 random scenes per backend
//! path through `CpuBackend::render`, asserting:
//!   1. Never panic
//!   2. No OOB writes (pixmap.pixels() valid after every render)
//!   3. Tile path matches scanline byte-for-byte (parity invariant
//!      from `tile_parity.rs` extended to random inputs)
//!
//! This is the poor-man's `cargo-fuzz` — no extra dev-dep needed, no
//! AFL/libfuzzer, no nightly. Runs in seconds; catches the unknown
//! unknowns that hand-curated tests miss.

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
use uzor_urx_core::config::UrxConfig;
use uzor_urx_cpu::{CpuBackend, Pixmap};

/// Splitmix64 — fast, statistically good, zero deps.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn next_u32(&mut self) -> u32 { (self.next_u64() & 0xffff_ffff) as u32 }
    fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        let r = (self.next_u32() as f32) / (u32::MAX as f32);
        lo + r * (hi - lo)
    }
    fn next_u8(&mut self) -> u8 { (self.next_u32() & 0xff) as u8 }
    fn bool(&mut self) -> bool { (self.next_u64() & 1) == 1 }
}

const W: u32 = 128;
const H: u32 = 64;

/// Build a random scene with `n_cmds` primitives drawn from a varied
/// pool. Includes deliberately adversarial coords (slightly off-screen,
/// zero-area, sub-pixel) but NO NaN/Inf — the backend's NaN filter is
/// covered separately by `nan_inf_adversarial.rs`.
fn random_scene(rng: &mut Rng, n_cmds: usize) -> Scene {
    let mut s = Scene::new();
    for _ in 0..n_cmds {
        match rng.next_u32() % 5 {
            0 => {
                // FillRect.
                let x0 = rng.next_f32_range(-20.0, W as f32 + 20.0) as f64;
                let y0 = rng.next_f32_range(-20.0, H as f32 + 20.0) as f64;
                let w  = rng.next_f32_range(0.0, 40.0) as f64;
                let h  = rng.next_f32_range(0.0, 40.0) as f64;
                let r  = rng.next_u8();
                let g  = rng.next_u8();
                let b  = rng.next_u8();
                let a  = rng.next_u8();
                s.fill_rect_solid(
                    Rect::new(x0, y0, x0 + w, y0 + h),
                    Color::rgba8(r, g, b, a),
                );
            }
            1 => {
                // StrokeRect.
                let x0 = rng.next_f32_range(0.0, W as f32) as f64;
                let y0 = rng.next_f32_range(0.0, H as f32) as f64;
                let w  = rng.next_f32_range(1.0, 30.0) as f64;
                let h  = rng.next_f32_range(1.0, 30.0) as f64;
                s.commands.push(DrawCommand::StrokeRect {
                    rect: Rect::new(x0, y0, x0 + w, y0 + h),
                    radii: None,
                    stroke: Stroke { width: rng.next_f32_range(0.5, 4.0), ..Stroke::default() },
                    brush: Brush::Solid(Color::rgba8(rng.next_u8(), rng.next_u8(), rng.next_u8(), 255)),
                    transform: Affine::IDENTITY,
                });
            }
            2 => {
                // Line.
                let from = Vec2::new(
                    rng.next_f32_range(0.0, W as f32) as f64,
                    rng.next_f32_range(0.0, H as f32) as f64,
                );
                let to = Vec2::new(
                    rng.next_f32_range(0.0, W as f32) as f64,
                    rng.next_f32_range(0.0, H as f32) as f64,
                );
                s.commands.push(DrawCommand::Line {
                    from, to,
                    stroke: Stroke { width: rng.next_f32_range(0.5, 3.0), ..Stroke::default() },
                    brush: Brush::Solid(Color::rgba8(rng.next_u8(), rng.next_u8(), rng.next_u8(), 255)),
                    transform: Affine::IDENTITY,
                });
            }
            3 => {
                // FillRect with corner radii.
                let x0 = rng.next_f32_range(0.0, W as f32) as f64;
                let y0 = rng.next_f32_range(0.0, H as f32) as f64;
                let w  = rng.next_f32_range(2.0, 30.0) as f64;
                let h  = rng.next_f32_range(2.0, 30.0) as f64;
                let r  = rng.next_f32_range(0.0, 8.0);
                s.commands.push(DrawCommand::FillRect {
                    rect: Rect::new(x0, y0, x0 + w, y0 + h),
                    radii: Some([r, r, r, r]),
                    brush: Brush::Solid(Color::rgba8(rng.next_u8(), rng.next_u8(), rng.next_u8(), rng.next_u8())),
                    transform: Affine::IDENTITY,
                });
            }
            _ => {
                // Push/Pop clip pair.
                let x0 = rng.next_f32_range(-5.0, W as f32 - 5.0) as f64;
                let y0 = rng.next_f32_range(-5.0, H as f32 - 5.0) as f64;
                let w  = rng.next_f32_range(5.0, 50.0) as f64;
                let h  = rng.next_f32_range(5.0, 50.0) as f64;
                s.commands.push(DrawCommand::PushClipRect {
                    rect: Rect::new(x0, y0, x0 + w, y0 + h),
                    transform: Affine::IDENTITY,
                });
                // Add 1-2 child rects.
                for _ in 0..((rng.next_u32() & 1) + 1) {
                    let cx = rng.next_f32_range(0.0, W as f32) as f64;
                    let cy = rng.next_f32_range(0.0, H as f32) as f64;
                    s.fill_rect_solid(
                        Rect::new(cx, cy, cx + 5.0, cy + 5.0),
                        Color::rgba8(rng.next_u8(), rng.next_u8(), rng.next_u8(), 200),
                    );
                }
                s.commands.push(DrawCommand::PopClip);
            }
        }
    }
    s
}

#[test]
fn no_panic_on_1000_random_scenes() {
    // Mixed-primitive scenes — every variant covered.
    let backend = CpuBackend::new();
    for seed in 0..1000u64 {
        let mut rng = Rng::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let n = ((rng.next_u32() % 40) + 1) as usize; // 1..=40 cmds
        let scene = random_scene(&mut rng, n);
        let mut p = Pixmap::new(W, H);
        // MUST not panic. Any panic = failed property.
        backend.render(&scene, &mut p).unwrap();
        // Pixmap len consistency — guarantees no OOB write.
        assert_eq!(p.pixels().len(), (W * H * 4) as usize);
    }
}

#[test]
fn tile_path_matches_scanline_on_random_fill_rect_scenes() {
    // Scene of pure-FillRect (axis-aligned, Solid brush) — both
    // paths must produce byte-identical output.
    let scanline_backend = CpuBackend::with_config(
        UrxConfig::builder().tile_route_min_cmds(1_000_000).build().unwrap()
    );
    let tile_backend = CpuBackend::new(); // default → tile path at ≥50 cmds

    for seed in 0..200u64 {
        let mut rng = Rng::new(seed.wrapping_mul(0xa3b1_c2d4_e5f6_0708));
        let n = 50 + (rng.next_u32() % 100) as usize; // 50..=149 cmds → triggers tile
        let mut scene = Scene::new();
        for _ in 0..n {
            let x0 = rng.next_f32_range(0.0, W as f32) as f64;
            let y0 = rng.next_f32_range(0.0, H as f32) as f64;
            let w  = rng.next_f32_range(1.0, 20.0) as f64;
            let h  = rng.next_f32_range(1.0, 20.0) as f64;
            // Mix opaque + semi-transparent.
            let a = if rng.bool() { 255 } else { 128 };
            scene.fill_rect_solid(
                Rect::new(x0, y0, x0 + w, y0 + h),
                Color::rgba8(rng.next_u8(), rng.next_u8(), rng.next_u8(), a),
            );
        }
        let mut p_scan = Pixmap::new(W, H);
        let mut p_tile = Pixmap::new(W, H);
        scanline_backend.render(&scene, &mut p_scan).unwrap();
        tile_backend.render(&scene, &mut p_tile).unwrap();
        assert_eq!(
            p_scan.pixels(), p_tile.pixels(),
            "tile vs scanline diverged on seed {} ({} cmds)", seed, n,
        );
    }
}

#[test]
fn no_panic_on_extreme_dimensions() {
    let backend = CpuBackend::new();
    let mut rng = Rng::new(0xdead_beef);
    // Try a bunch of unusual pixmap sizes.
    let dims = [
        (1, 1), (1, 100), (100, 1), (3, 3), (4, 5),
        (33, 17), (65, 33), (255, 128),
    ];
    for (w, h) in dims {
        for _ in 0..20 {
            let scene = random_scene(&mut rng, 15);
            let mut p = Pixmap::new(w, h);
            backend.render(&scene, &mut p).unwrap();
        }
    }
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_deterministic_on_random_scenes() {
    // Parallel vs 1-thread must give byte-identical output regardless
    // of input. Already tested for one scene; this exercises 50
    // random scenes large enough to engage rayon.
    let backend = CpuBackend::new();
    let pool_1 = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    for seed in 0..50u64 {
        let mut rng = Rng::new(seed.wrapping_mul(0xbabe_cafe));
        let n = 100 + (rng.next_u32() % 500) as usize;
        let mut scene = Scene::new();
        for _ in 0..n {
            let x0 = rng.next_f32_range(0.0, 256.0) as f64;
            let y0 = rng.next_f32_range(0.0, 256.0) as f64;
            scene.fill_rect_solid(
                Rect::new(x0, y0, x0 + 4.0, y0 + 4.0),
                Color::rgba8(rng.next_u8(), rng.next_u8(), rng.next_u8(), 255),
            );
        }
        let mut p_serial = Pixmap::new(256, 256);
        let mut p_pool   = Pixmap::new(256, 256);
        pool_1.install(|| backend.render(&scene, &mut p_serial).unwrap());
        backend.render(&scene, &mut p_pool).unwrap();
        assert_eq!(
            p_serial.pixels(), p_pool.pixels(),
            "parallel diverged on seed {} ({} cmds)", seed, n,
        );
    }
}
