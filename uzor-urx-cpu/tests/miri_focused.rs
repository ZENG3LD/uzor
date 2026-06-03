//! Miri-focused subset: small, fast tests that exercise EVERY unsafe
//! block in urx-cpu so miri can audit Stacked Borrows + alignment +
//! UB. Skipped under non-miri runs (those run the full test suite).
//!
//! Run:
//!   cargo +nightly miri test -p uzor-urx-cpu --test miri_focused
//!
//! Coverage (PASSES under Stacked Borrows):
//!   - pixmap::AlignedBuf (alloc/clone/drop) — Layout-based unsafe
//!   - tile::flush_tiles SEQUENTIAL branch — bucket slice indexing
//!   - tile::memset_band align_to_mut::<u32> — alignment audit
//!   - simd::memset_u32_simd — try_into + slice indexing
//!
//! NOT covered (miri limitation, not a urx-cpu issue):
//!   - tile::flush_tiles PARALLEL branch — relies on crossbeam-epoch,
//!     which triggers a Stacked Borrows error inside its lock-free
//!     queue (well-known miri/crossbeam interaction; see
//!     crossbeam-rs/crossbeam#888). The raw-pointer-split pattern
//!     itself is the same one rayon uses for `par_chunks_mut`; we
//!     verify it indirectly via the byte-identical determinism test
//!     `config_thread_determinism::parallel_is_deterministic_*`.
//!     If a future crossbeam release makes that test miri-clean,
//!     enable the parallel branch test here too.

#![cfg(miri)]

use uzor_urx_core::math::{Color, Rect};
use uzor_urx_core::scene::Scene;
use uzor_urx_cpu::{CpuBackend, Pixmap};

const W: u32 = 32;
const H: u32 = 16;

#[test]
fn pixmap_alloc_clone_drop_no_ub() {
    let mut a = Pixmap::new(W, H);
    for (i, b) in a.pixels_mut().iter_mut().enumerate() {
        *b = (i & 0xff) as u8;
    }
    let _b = a.clone();
    // Drop both at end of scope — miri checks no double-free.
}

#[test]
fn zero_dim_pixmap_no_ub() {
    let _a = Pixmap::new(0, 0);
    let _b = Pixmap::new(0, 10);
    let _c = Pixmap::new(10, 0);
}

#[test]
fn tile_pipeline_no_ub() {
    // Trigger the tile path: 50+ cmds, all FillRect, axis-aligned.
    let mut s = Scene::new();
    for i in 0..60 {
        s.fill_rect_solid(
            Rect::new((i % 30) as f64, (i / 30) as f64,
                      ((i % 30) + 4) as f64, ((i / 30) + 4) as f64),
            Color { r: (i * 4) as u8, g: 100, b: 50, a: 255 },
        );
    }
    let mut p = Pixmap::new(W, H);
    CpuBackend::new().render(&s, &mut p).unwrap();
}

#[test]
fn scanline_path_no_ub() {
    // Force scanline by staying below tile route threshold.
    let mut s = Scene::new();
    for i in 0..10 {
        s.fill_rect_solid(
            Rect::new(i as f64, 0.0, i as f64 + 4.0, 4.0),
            Color { r: 255, g: 0, b: 0, a: 200 },
        );
    }
    let mut p = Pixmap::new(W, H);
    CpuBackend::new().render(&s, &mut p).unwrap();
}

// Note: the parallel-path test is intentionally disabled under miri
// because crossbeam-epoch trips Stacked Borrows in its lock-free
// queue. The raw-pointer-split logic in flush_tiles is verified
// indirectly via `config_thread_determinism` (byte-identical output
// between parallel + 1-thread).

#[test]
fn memset_band_align_to_no_ub() {
    // Render a solid-fill scenario that hits bg-replacement.
    // memset_band's align_to_mut::<u32> is exercised here.
    let mut s = Scene::new();
    for _ in 0..60 {
        s.fill_rect_solid(
            Rect::new(0.0, 0.0, W as f64, H as f64),
            Color { r: 200, g: 100, b: 50, a: 255 },
        );
    }
    let mut p = Pixmap::new(W, H);
    CpuBackend::new().render(&s, &mut p).unwrap();
}
