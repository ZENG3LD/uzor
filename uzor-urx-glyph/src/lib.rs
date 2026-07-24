//! URX glyph atlas + rasteriser.
//!
//! Scope:
//!   * Register fonts via raw bytes → opaque `FontId` (urx-core's).
//!   * Rasterise glyphs at any (px_size, subpx_bin) via swash.
//!   * Cache rasterised glyphs in an in-memory LRU keyed by the same
//!     tuple. R8 monochrome (alpha mask).
//!   * `draw_glyph_run` on a CPU `Pixmap` — premultiplied src-over of
//!     coloured glyphs.
//!
//! NON-scope (deliberate):
//!   * Text shaping (cosmic-text). Caller pre-shapes; Scene already
//!     carries `Vec<Glyph>` with positional offsets.
//!   * GPU upload / atlas page packing — that's urx-wgpu / urx-hybrid
//!     job. This crate stays CPU-only so it can serve every backend.
//!   * COLR/CBDT/CBLC colour glyphs — deferred; emoji not on the
//!     critical path. Monochrome is what charts + dashboards need.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};
use swash::{FontRef, GlyphId};

pub use uzor_urx_core::scene::{FontId, Glyph};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font:        FontId,
    pub glyph_id:    u16,
    /// Pixel height × 64 (24.8 fixed).
    pub px_size_x64: u32,
    /// X subpixel bin (0..3) — Y always snapped to integer.
    pub subpx_x:     u8,
}

impl GlyphKey {
    /// Extracted from `rasterise_glyph`'s inline key construction
    /// (URX Wave 2 design §0) — pure extraction, zero behavior change.
    /// Takes `glyph_id: u32` to match
    /// `uzor_urx_core::scene::Glyph::glyph_id`'s type directly, so
    /// callers outside this crate (e.g. `uzor-urx-wgpu`'s native
    /// atlas) never need to hand-duplicate the narrowing rule.
    pub fn new(font: FontId, glyph_id: u32, px_size: f32, subpx_x: u8) -> Self {
        Self {
            font,
            glyph_id: glyph_id as u16,
            px_size_x64: (px_size * 64.0).round() as u32,
            subpx_x: subpx_x & 3,
        }
    }
}

/// Bin a pen x-position into one of 4 horizontal subpixel phases.
///
/// Extracted from `draw_glyph_run`'s inline `frac`/`subpx` calc (URX
/// Wave 2 design §0) — pure extraction, zero behavior change.
/// `uzor-urx-wgpu`'s encode-time atlas lookup needs the IDENTICAL
/// formula this crate's own CPU compositor uses, so both backends
/// rasterise (and therefore atlas-key) the same glyph at the same
/// subpixel phase for the same pen position.
pub fn subpixel_bin_for_x(px: f32) -> u8 {
    let frac = (px - px.floor()).max(0.0).min(1.0);
    ((frac * 4.0).floor() as u8) & 3
}

#[derive(Debug)]
pub struct GlyphBitmap {
    pub width:    u32,
    pub height:   u32,
    /// Left/top offset from the glyph's pen position to the bitmap
    /// top-left corner (positive top = bitmap above baseline).
    pub left:     i32,
    pub top:      i32,
    pub alpha:    Vec<u8>, // R8 coverage, row-major
}

type GlyphBitmapArc = Arc<GlyphBitmap>;

#[derive(Default)]
struct GlyphLru {
    entries: Vec<(GlyphKey, GlyphBitmapArc, u64)>,
    tick:    u64,
}

const GLYPH_LRU_CAP: usize = 1024;

impl GlyphLru {
    fn get(&mut self, key: GlyphKey) -> Option<GlyphBitmapArc> {
        self.tick = self.tick.wrapping_add(1);
        for e in self.entries.iter_mut() {
            if e.0 == key { e.2 = self.tick; return Some(e.1.clone()); }
        }
        None
    }
    fn insert(&mut self, key: GlyphKey, bm: GlyphBitmapArc) {
        self.tick = self.tick.wrapping_add(1);
        if self.entries.len() >= GLYPH_LRU_CAP {
            if let Some((idx, _)) = self.entries.iter().enumerate()
                .min_by_key(|(_, e)| e.2)
            {
                self.entries.swap_remove(idx);
            }
        }
        self.entries.push((key, bm, self.tick));
    }
}

struct FontEntry {
    bytes:  Arc<Vec<u8>>,
    offset: u32,
}

#[derive(Default)]
struct Registry {
    fonts:    HashMap<u64, FontEntry>,
    next_id:  u64,
}

static REGISTRY: RwLock<Option<Registry>> = RwLock::new(None);
static CACHE:    RwLock<Option<GlyphLru>> = RwLock::new(None);
// ScaleContext is `!Sync` — thread-local. swash rasterisation is fast
// enough that constructing a fresh context per thread on first use is
// not a problem.
thread_local! {
    static SCALE_CTX: std::cell::RefCell<ScaleContext> = std::cell::RefCell::new(ScaleContext::new());
}

/// Register a font face (TTF/OTF bytes). Caller keeps no live reference
/// to the bytes; we Arc them internally. Returns the opaque `FontId`.
pub fn register_font(bytes: Vec<u8>) -> Result<FontId, GlyphError> {
    // Validate via swash — picks the first table offset (collection: 0).
    let font_ref = FontRef::from_index(&bytes, 0)
        .ok_or(GlyphError::InvalidFont)?;
    let offset = font_ref.offset;
    let mut g = REGISTRY.write().unwrap();
    let reg = g.get_or_insert_with(Registry::default);
    reg.next_id = reg.next_id.wrapping_add(1);
    let id = FontId(reg.next_id);
    reg.fonts.insert(id.0, FontEntry { bytes: Arc::new(bytes), offset });
    Ok(id)
}

pub fn unregister_font(id: FontId) -> bool {
    let mut g = REGISTRY.write().unwrap();
    if let Some(reg) = g.as_mut() {
        reg.fonts.remove(&id.0).is_some()
    } else { false }
}

#[derive(Debug)]
pub enum GlyphError {
    InvalidFont,
    UnknownFont,
    RasterFailed,
}

fn with_font_ref<R>(id: FontId, f: impl FnOnce(FontRef<'_>) -> R) -> Result<R, GlyphError> {
    let g = REGISTRY.read().unwrap();
    let reg = g.as_ref().ok_or(GlyphError::UnknownFont)?;
    let entry = reg.fonts.get(&id.0).ok_or(GlyphError::UnknownFont)?;
    let font_ref = FontRef {
        data: entry.bytes.as_slice(),
        offset: entry.offset,
        key: swash::CacheKey::new(),
    };
    Ok(f(font_ref))
}

/// Rasterise a single glyph at a given pixel size + subpixel bin.
/// Cached. Returns an `Arc<GlyphBitmap>` so cache lookups are cheap.
pub fn rasterise_glyph(
    font:     FontId,
    glyph_id: u16,
    px_size:  f32,
    subpx_x:  u8,
) -> Result<GlyphBitmapArc, GlyphError> {
    let key = GlyphKey::new(font, glyph_id as u32, px_size, subpx_x);

    {
        let mut g = CACHE.write().unwrap();
        let lru = g.get_or_insert_with(GlyphLru::default);
        if let Some(bm) = lru.get(key) { return Ok(bm); }
    }

    let bm = SCALE_CTX.with(|ctx| -> Result<GlyphBitmapArc, GlyphError> {
        let mut ctx = ctx.borrow_mut();
        with_font_ref(font, |font_ref| -> Result<GlyphBitmapArc, GlyphError> {
            let mut scaler = ctx.builder(font_ref)
                .size(px_size)
                .hint(true)
                .build();
            let offset_x = (subpx_x as f32) / 4.0;
            let image = Render::new(&[
                Source::ColorOutline(0),
                Source::ColorBitmap(StrikeWith::BestFit),
                Source::Outline,
            ])
                .format(Format::Alpha)
                .offset(Vector::new(offset_x, 0.0))
                .render(&mut scaler, GlyphId::from(glyph_id))
                .ok_or(GlyphError::RasterFailed)?;
            let w = image.placement.width;
            let h = image.placement.height;
            Ok(Arc::new(GlyphBitmap {
                width:  w,
                height: h,
                left:   image.placement.left,
                top:    image.placement.top,
                alpha:  image.data,
            }))
        })?
    })?;

    {
        let mut g = CACHE.write().unwrap();
        let lru = g.get_or_insert_with(GlyphLru::default);
        lru.insert(key, bm.clone());
    }
    Ok(bm)
}

/// Shared, lazily-built LUT for `uzor_urx_core::text_gamma::TEXT_GAMMA_CURVE`
/// — production callers (e.g. `uzor-urx-cpu::CpuBackend`) that just want
/// "the configured curve" call [`configured_text_gamma_lut`] instead of
/// building their own. Built exactly once per process, from the SAME
/// pure `build_text_gamma_lut` function `uzor-urx-wgpu`'s native glyph
/// pipeline calls at its own construction time — byte-identical bytes
/// on both backends, by construction (URX text-gamma design, §2.5).
static TEXT_GAMMA_LUT: std::sync::OnceLock<uzor_urx_core::text_gamma::TextGammaLut> = std::sync::OnceLock::new();

/// The shared, lazily-built LUT for `TEXT_GAMMA_CURVE`. See
/// [`TEXT_GAMMA_LUT`]'s own doc comment.
pub fn configured_text_gamma_lut() -> &'static uzor_urx_core::text_gamma::TextGammaLut {
    TEXT_GAMMA_LUT.get_or_init(|| {
        uzor_urx_core::text_gamma::build_text_gamma_lut(&uzor_urx_core::text_gamma::TEXT_GAMMA_CURVE)
    })
}

/// Per-pixel clip test for [`draw_glyph_run`] — kept generic (a plain
/// bounds rect + an optional sampler closure, not a hard dependency on
/// any specific clip-stack TYPE) so this crate stays backend-agnostic.
/// `uzor-urx-cpu::CpuBackend` is the one real caller, adapting its own
/// `ClipStack` to this shape at the call site (`clip.current()` for
/// `bounds`, `|px, py| clip.pixel_coverage(px, py)` for `mask`) rather
/// than this crate depending on `uzor-urx-cpu` at all.
pub struct GlyphClip<'a> {
    /// Clip's current axis-aligned bounding rect in pixel space,
    /// half-open per axis (`[x0, x1) x [y0, y1)`) — a cheap per-pixel
    /// bbox test that alone is EXACT when the active clip is plain
    /// rects (no rounded clip on the stack). `(x0, y0, x1, y1)`.
    pub bounds: (f32, f32, f32, f32),
    /// Per-pixel coverage sampler (`0..=255`), present only when a
    /// rounded clip is active — mirrors every other CPU primitive's own
    /// `use_mask = !clip.all_rect()` split (`fill_rect_aa`/
    /// `stroke_rect_aa`/etc. all skip the equivalent per-pixel call
    /// entirely for the common plain-rect-clip case, `bounds` alone
    /// being exact there). `None` here is that same fast path.
    pub mask: Option<&'a dyn Fn(i64, i64) -> u8>,
}

/// Composite a pre-shaped glyph run onto a premul RGBA8 pixel buffer.
/// Caller supplies the buffer + width/height + pen origin. Each glyph
/// is rasterised at the supplied font_size with subpx_x derived from
/// the glyph's fractional x position. `color` is the text colour
/// (any alpha); glyph mask is multiplied through.
///
/// `gamma_lut` — `None` is today's exact code path, zero added cost (a
/// single branch skipped, no LUT indexing at all). `Some(lut)` looks up
/// the foreground-luma bucket ONCE for the whole run (loop-invariant,
/// same shape GPU's `encode_glyph_run` computes its own bin), then
/// remaps every glyph's raw swash coverage byte through `lut[bin]`
/// before the existing premultiply math — URX text-gamma compositing
/// design, 2026-07-26, §2.4.
///
/// `clip` — `None` renders unclipped (every caller before 2026-07-24
/// got exactly this; `uzor-urx-glyph/tests/smoke.rs`'s own direct
/// calls still do). `Some(c)` honors `c.bounds` as a cheap bbox test
/// per touched pixel, plus `c.mask` (if present) as a per-pixel
/// coverage multiplier — the fix for the bug where `GlyphRun` was the
/// only `uzor-urx-cpu` primitive that ignored `ClipStack` entirely
/// (every other arm — `FillRect`/`StrokeRect`/`Line`/`FillPath`/
/// `StrokePath`/`Image` — already threads `&clip` through; this arm's
/// call into this crate carried no clip information at all before this
/// param existed).
pub fn draw_glyph_run(
    pixels:    &mut [u8],
    buf_w:     u32,
    buf_h:     u32,
    origin_x:  f32,
    origin_y:  f32,
    glyphs:    &[Glyph],
    font:      FontId,
    font_size: f32,
    color:     [u8; 4],
    gamma_lut: Option<&uzor_urx_core::text_gamma::TextGammaLut>,
    clip:      Option<GlyphClip<'_>>,
) -> Result<(), GlyphError> {
    let a = color[3] as u32;
    let premul_color = [
        ((color[0] as u32 * a + 127) / 255) as u8,
        ((color[1] as u32 * a + 127) / 255) as u8,
        ((color[2] as u32 * a + 127) / 255) as u8,
        color[3],
    ];
    let gamma_bin = gamma_lut.map(|_| uzor_urx_core::text_gamma::luma_bin(color) as usize);

    for g in glyphs {
        let px = origin_x + g.x;
        let py = origin_y + g.y;
        let subpx = subpixel_bin_for_x(px);
        let bm = rasterise_glyph(font, g.glyph_id as u16, font_size, subpx)?;
        if bm.width == 0 || bm.height == 0 { continue; }
        let dst_x0 = (px.floor() as i32) + bm.left;
        // swash returns `top` as glyph height ABOVE baseline. Bitmap
        // top-left = baseline_y - top.
        let dst_y0 = (py.round() as i32) - bm.top;

        for gy in 0..bm.height as i32 {
            let dy = dst_y0 + gy;
            if dy < 0 || dy as u32 >= buf_h { continue; }
            if let Some(c) = &clip {
                if (dy as f32) < c.bounds.1 || (dy as f32) >= c.bounds.3 { continue; }
            }
            for gx in 0..bm.width as i32 {
                let dx = dst_x0 + gx;
                if dx < 0 || dx as u32 >= buf_w { continue; }
                if let Some(c) = &clip {
                    if (dx as f32) < c.bounds.0 || (dx as f32) >= c.bounds.2 { continue; }
                }
                let mut mask = bm.alpha[(gy as u32 * bm.width + gx as u32) as usize] as u32;
                if mask == 0 { continue; }
                if let Some(c) = &clip {
                    if let Some(sampler) = c.mask {
                        let mask_cov = sampler(dx as i64, dy as i64) as u32;
                        mask = (mask * mask_cov + 127) / 255;
                        if mask == 0 { continue; }
                    }
                }
                if let (Some(lut), Some(bin)) = (gamma_lut, gamma_bin) {
                    mask = lut[bin][mask as usize] as u32;
                }
                let scaled = [
                    ((premul_color[0] as u32 * mask + 127) / 255) as u8,
                    ((premul_color[1] as u32 * mask + 127) / 255) as u8,
                    ((premul_color[2] as u32 * mask + 127) / 255) as u8,
                    ((premul_color[3] as u32 * mask + 127) / 255) as u8,
                ];
                let i = ((dy as u32 * buf_w + dx as u32) * 4) as usize;
                let inv_a = 255 - scaled[3] as u32;
                pixels[i  ] = (scaled[0] as u32 + (pixels[i  ] as u32 * inv_a + 127) / 255).min(255) as u8;
                pixels[i+1] = (scaled[1] as u32 + (pixels[i+1] as u32 * inv_a + 127) / 255).min(255) as u8;
                pixels[i+2] = (scaled[2] as u32 + (pixels[i+2] as u32 * inv_a + 127) / 255).min(255) as u8;
                pixels[i+3] = (scaled[3] as u32 + (pixels[i+3] as u32 * inv_a + 127) / 255).min(255) as u8;
            }
        }
    }
    metrics::counter!(
        uzor_urx_core::metrics_keys::KEY_RENDER_GLYPH_INSTANCES,
    ).increment(glyphs.len() as u64);
    Ok(())
}

#[doc(hidden)]
pub fn _clear_caches_for_tests() {
    let mut c = CACHE.write().unwrap();
    *c = None;
    let mut r = REGISTRY.write().unwrap();
    *r = None;
}
