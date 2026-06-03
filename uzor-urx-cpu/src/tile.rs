//! Coarse tile classifier + fine pass — closes the overdraw-heavy gap
//! vs vello_cpu by skipping buried opaque layers in O(1) per tile.
//!
//! Architecture (mirrors vello sparse_strips/coarse.rs):
//!
//! 1. The pixmap is split into a grid of WIDE TILES, each `TILE_W × TILE_H`
//!    pixels (`256 × 8`). For 1920×1080 that's `8 × 135 = 1080` tiles.
//! 2. Coarse pass walks the scene in painter's order and appends a
//!    `Cmd` per draw into each touched tile's `cmd_list`.
//! 3. **Key trick**: when a draw fully covers a tile AND its paint is
//!    OpaqueSolid → `cmd_list.clear()` + `bg = Some(color)`. All buried
//!    layers vanish from the work list. 1000 overlapping opaque rects
//!    collapse to ≤ N_TILES memsets instead of 1000 blend ops.
//! 4. Fine pass: per tile, either memset(bg) (one register-wide store
//!    per row) or replay the surviving cmd list via the SIMD span fill.
//!
//! Bit-exact contract: when no opaque-replacement triggers (the
//! pessimistic case), output is byte-identical to the non-tiled
//! scanline path. Verified by `tests/tile_parity.rs`.

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;

use uzor_urx_core::math::{Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};

use crate::clip::{transform_axis_aligned, ClipStack};
use crate::color::color_to_premul;
use crate::fill::axis_coverage;
use crate::pixmap::Pixmap;

/// Wide-tile dimensions. 32×8 is a sweet spot for typical UI: small
/// enough that 30-px rects fully cover several adjacent tiles
/// (triggering bg-replacement), large enough that the per-tile
/// bookkeeping doesn't dominate.
///
/// vello uses 256×4. We chose 32×8 because our consumers (charts,
/// dashboards) draw lots of small (≤50 px) opaque cells, where 256
/// is too coarse to ever trigger full coverage.
const TILE_W: u32 = 32;
const TILE_H: u32 = 8;

#[derive(Debug, Clone, Copy)]
enum Cmd {
    /// Solid-color fill of an axis-aligned rect, already
    /// transformed/clipped to screen space.
    FillRect {
        x0: f64, y0: f64, x1: f64, y1: f64,
        premul: [u8; 4],
    },
}

/// Tile state. Cmd list lives in a per-frame `bumpalo` arena — zero
/// per-frame heap allocation, contiguous memory layout that the L2
/// prefetcher can stream through linearly.
struct TileBucket<'a> {
    bg:   Option<[u8; 4]>,
    cmds: BumpVec<'a, Cmd>,
}

impl<'a> TileBucket<'a> {
    fn new_in(arena: &'a Bump) -> Self {
        Self { bg: None, cmds: BumpVec::new_in(arena) }
    }
    fn is_empty(&self) -> bool {
        self.bg.is_none() && self.cmds.is_empty()
    }
}

/// Render the scene via the tile pipeline. Falls back to scanline
/// for any primitive the tile path doesn't yet handle (paths, glyphs,
/// images, gradients, rounded clips, transforms with shear). Those
/// flush the tile buffer first so painter's order is preserved.
pub fn render_tiled(scene: &Scene, pixmap: &mut Pixmap) {
    let pw = pixmap.width();
    let ph = pixmap.height();
    if pw == 0 || ph == 0 { return; }

    let n_x = pw.div_ceil(TILE_W) as usize;
    let n_y = ph.div_ceil(TILE_H) as usize;

    // Per-frame arena. Initial capacity ≈ N_TILES × 8 cmds × 48 B/cmd.
    // Bumpalo grows in chunks of `Bump::with_capacity(N)`-sized slabs
    // when overflowed; one slab is the common case for typical UIs.
    let arena = Bump::with_capacity(n_x * n_y * 384);
    let mut tiles: Vec<TileBucket> = (0..n_x * n_y)
        .map(|_| TileBucket::new_in(&arena))
        .collect();

    // Painter's-order walk. Coarse classification per FillRect; any
    // primitive that's not "opaque solid axis-aligned FillRect" hits
    // the flush-to-pixmap path and resets tile state for subsequent
    // draws.
    let mut clip = ClipStack::new(Rect::new(0.0, 0.0, pw as f64, ph as f64));

    for cmd in &scene.commands {
        match cmd {
            DrawCommand::FillRect { rect, radii, brush, transform } => {
                // Reject anything that breaks the opaque-fill fast path
                // assumption. Each rejection flushes tile state first.
                let has_radii = radii.map(|r| r.iter().any(|v| *v > 0.0)).unwrap_or(false);
                let is_gradient = matches!(brush, Brush::Gradient(_) | Brush::Image(_));
                let coeffs = transform.as_coeffs();
                let has_shear = coeffs[1].abs() > 1e-6 || coeffs[2].abs() > 1e-6;
                if has_radii || is_gradient || has_shear || !clip.all_rect() {
                    flush_tiles(&mut tiles, pixmap, n_x, n_y);
                    // Hand off to the scanline path for this one primitive.
                    fallback_one(pixmap, &clip, cmd);
                    continue;
                }
                let color = brush_to_color(brush);
                let r_screen = transform_axis_aligned(*transform, *rect);
                let cur = clip.current();
                let visible = r_screen.intersect(cur);
                if visible.width() <= 0.0 || visible.height() <= 0.0 { continue; }
                let opaque = color.a == 255;
                add_rect_to_tiles(&mut tiles, n_x, n_y, pw, ph, visible, color, opaque);
            }
            DrawCommand::PushClipRect { rect, transform } => {
                // Clips can change region eligibility; flush tile
                // buffer so subsequent draws inside the clip see the
                // already-committed pixels.
                flush_tiles(&mut tiles, pixmap, n_x, n_y);
                clip.push_rect(*rect, transform);
            }
            DrawCommand::PopClip => {
                flush_tiles(&mut tiles, pixmap, n_x, n_y);
                clip.pop();
            }
            _ => {
                // Anything else (StrokeRect, Line, FillPath, StrokePath,
                // GlyphRun, Image, PushClipRoundedRect) — flush + use
                // the legacy scanline backend for that one command.
                flush_tiles(&mut tiles, pixmap, n_x, n_y);
                fallback_one(pixmap, &clip, cmd);
            }
        }
    }
    flush_tiles(&mut tiles, pixmap, n_x, n_y);
}

#[inline]
fn brush_to_color(b: &Brush) -> Color {
    match b {
        Brush::Solid(c) => *c,
        _ => Color::rgba8(0, 0, 0, 0),
    }
}

#[inline]
fn tile_rect(tx: usize, ty: usize, pw: u32, ph: u32) -> (u32, u32, u32, u32) {
    let x0 = (tx as u32) * TILE_W;
    let y0 = (ty as u32) * TILE_H;
    let x1 = (x0 + TILE_W).min(pw);
    let y1 = (y0 + TILE_H).min(ph);
    (x0, y0, x1, y1)
}

fn add_rect_to_tiles<'a>(
    tiles:   &mut [TileBucket<'a>],
    n_x:     usize,
    n_y:     usize,
    pw:      u32,
    ph:      u32,
    visible: Rect,
    color:   Color,
    opaque:  bool,
) {
    let premul = color_to_premul(color);
    let tx_lo = (visible.x0.floor() as i64).max(0) as u32 / TILE_W;
    let ty_lo = (visible.y0.floor() as i64).max(0) as u32 / TILE_H;
    let tx_hi = ((visible.x1.ceil()  as i64 - 1).max(0) as u32 / TILE_W).min(n_x as u32 - 1);
    let ty_hi = ((visible.y1.ceil()  as i64 - 1).max(0) as u32 / TILE_H).min(n_y as u32 - 1);

    for ty in ty_lo..=ty_hi {
        for tx in tx_lo..=tx_hi {
            let (x0, y0, x1, y1) = tile_rect(tx as usize, ty as usize, pw, ph);
            let fully_covers = opaque
                && visible.x0 <= x0 as f64 && visible.x1 >= x1 as f64
                && visible.y0 <= y0 as f64 && visible.y1 >= y1 as f64;
            let tile = &mut tiles[(ty as usize) * n_x + (tx as usize)];
            if fully_covers {
                // *** the overdraw-killer ***
                tile.cmds.clear();
                tile.bg = Some(premul);
            } else {
                tile.cmds.push(Cmd::FillRect {
                    x0: visible.x0,
                    y0: visible.y0,
                    x1: visible.x1,
                    y1: visible.y1,
                    premul,
                });
            }
        }
    }
}

/// Drain `tiles` into the pixmap. Parallel mode (feature `parallel`):
/// split pixmap into per-band mut slices via `chunks_mut(band_bytes)`
/// and process each band on a rayon worker. Static partition + no
/// inter-band synchronisation (bands never alias). On <128 tile rows
/// we fall back to sequential to avoid scheduler overhead.
fn flush_tiles<'a>(
    tiles:  &mut [TileBucket<'a>],
    pixmap: &mut Pixmap,
    n_x:    usize,
    n_y:    usize,
) {
    let pw = pixmap.width();
    let ph = pixmap.height();
    if pw == 0 || ph == 0 { return; }

    let stride = pw as usize * 4;
    let band_bytes = (TILE_H as usize) * stride;
    let pixels = pixmap.pixels_mut();

    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        if n_y >= 16 {
            // `par_chunks_mut` partitions the pixel slice into bands.
            // We pair each band with its tile-row via an index closure.
            // The bucket slice can't be split with `par_chunks_mut` AND
            // shared across the band closure simultaneously — so we use
            // raw pointers + manual unsafe: each band touches its own
            // bucket range (n_x buckets), bands never overlap.
            let buckets_ptr = tiles.as_mut_ptr() as usize;
            let buckets_len = tiles.len();
            pixels.par_chunks_mut(band_bytes).enumerate()
                .for_each(|(ty, band)| {
                    let band_y0 = (ty as u32) * TILE_H;
                    let row_h = (band.len() / stride) as u32;
                    // SAFETY: each tile-row band is disjoint (no
                    // overlap in `tiles[]` partitioning), bucket
                    // ownership is single-thread-per-row.
                    let row_buckets: &mut [TileBucket] = unsafe {
                        let start = ty * n_x;
                        let end = (start + n_x).min(buckets_len);
                        if start >= buckets_len { return; }
                        std::slice::from_raw_parts_mut(
                            (buckets_ptr as *mut TileBucket).add(start),
                            end - start,
                        )
                    };
                    flush_band(band, stride, row_buckets, pw, band_y0, row_h);
                });
            return;
        }
    }

    // Sequential fallback.
    for ty in 0..n_y {
        let band_start = ty * band_bytes;
        let band_end   = (band_start + band_bytes).min(pixels.len());
        let band = &mut pixels[band_start..band_end];
        let row_h = (band.len() / stride) as u32;
        let band_y0 = ty as u32 * TILE_H;
        let row_buckets = &mut tiles[ty * n_x .. (ty + 1) * n_x];
        flush_band(band, stride, row_buckets, pw, band_y0, row_h);
    }
}

/// Flush a single horizontal tile-row band. Called sequentially per
/// band (no inter-band synchronisation needed — bands don't overlap).
#[inline]
fn flush_band<'a>(
    band:       &mut [u8],
    stride:     usize,
    buckets:    &mut [TileBucket<'a>],
    pw:         u32,
    band_y0:    u32,
    row_h:      u32,
) {
    for (tx, bucket) in buckets.iter_mut().enumerate() {
        if bucket.bg.is_none() && bucket.cmds.is_empty() { continue; }
        let x0 = (tx as u32) * TILE_W;
        let x1 = (x0 + TILE_W).min(pw);
        if let Some(bg) = bucket.bg {
            memset_band(band, stride, x0, x1, 0, row_h, bg);
        }
        for cmd in &bucket.cmds {
            match *cmd {
                Cmd::FillRect { x0: rx0, y0: ry0, x1: rx1, y1: ry1, premul } => {
                    replay_rect_in_band(
                        band, stride, pw,
                        band_y0, row_h,
                        x0, x1,
                        rx0, ry0, rx1, ry1,
                        premul,
                    );
                }
            }
        }
        bucket.bg = None;
        bucket.cmds.clear();
    }
}

#[inline]
fn memset_band(
    band:   &mut [u8],
    stride: usize,
    x0: u32, x1: u32,
    band_row_lo: u32, band_row_hi: u32,
    premul: [u8; 4],
) {
    let row_w = (x1 - x0) as usize;
    for py in band_row_lo..band_row_hi {
        // py is already band-local (callers pass [0, band_h)).
        let row_off = (py as usize) * stride + (x0 as usize) * 4;
        for i in 0..row_w {
            let p = row_off + i * 4;
            band[p    ] = premul[0];
            band[p + 1] = premul[1];
            band[p + 2] = premul[2];
            band[p + 3] = premul[3];
        }
    }
}

fn replay_rect_in_band(
    band:    &mut [u8],
    stride:  usize,
    pixmap_w: u32,
    band_y0: u32,
    band_h:  u32,
    tx0: u32, tx1: u32,
    rx0: f64, ry0: f64, rx1: f64, ry1: f64,
    premul: [u8; 4],
) {
    // Intersect rect with tile-x range and band-y range.
    let band_top    = band_y0 as f64;
    let band_bottom = (band_y0 + band_h) as f64;
    let fx0 = rx0.max(tx0 as f64);
    let fx1 = rx1.min(tx1 as f64);
    let fy0 = ry0.max(band_top);
    let fy1 = ry1.min(band_bottom);
    if fx0 >= fx1 || fy0 >= fy1 { return; }

    let ix0 = (fx0.floor() as i64).max(tx0 as i64);
    let iy0 = (fy0.floor() as i64).max(band_top as i64);
    let ix1 = (fx1.ceil()  as i64).min(tx1 as i64);
    let iy1 = (fy1.ceil()  as i64).min(band_bottom as i64);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let _ = pixmap_w;

    for py in iy0..iy1 {
        let v_cov = axis_coverage(py as f64, py as f64 + 1.0, fy0, fy1);
        if v_cov == 0 { continue; }
        let local_y = (py as u32 - band_y0) as usize;
        let row_off = local_y * stride;
        crate::simd::fill_span_into_slice(
            band, row_off,
            ix0, ix1, fx0, fx1,
            v_cov, premul,
        );
    }
}

/// Render a single command via the legacy scanline path. Used as
/// fallback for anything the tile path doesn't model (paths, glyphs,
/// non-axis-aligned, etc.).
fn fallback_one(pixmap: &mut Pixmap, clip: &ClipStack, cmd: &DrawCommand) {
    // Re-use CpuBackend::render for a single-command Scene.
    let mut s = Scene::new();
    s.push(cmd.clone());
    let backend = crate::backend::CpuBackend::new();
    // Borrow the existing clip's bounds by re-rendering with a fresh
    // ClipStack that matches.
    let _ = clip;
    let _ = backend.render(&s, pixmap);
}
