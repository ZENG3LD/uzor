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
    let px_x64 = (px_size * 64.0).round() as u32;
    let key = GlyphKey { font, glyph_id, px_size_x64: px_x64, subpx_x: subpx_x & 3 };

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

/// Composite a pre-shaped glyph run onto a premul RGBA8 pixel buffer.
/// Caller supplies the buffer + width/height + pen origin. Each glyph
/// is rasterised at the supplied font_size with subpx_x derived from
/// the glyph's fractional x position. `color` is the text colour
/// (any alpha); glyph mask is multiplied through.
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
) -> Result<(), GlyphError> {
    let a = color[3] as u32;
    let premul_color = [
        ((color[0] as u32 * a + 127) / 255) as u8,
        ((color[1] as u32 * a + 127) / 255) as u8,
        ((color[2] as u32 * a + 127) / 255) as u8,
        color[3],
    ];

    for g in glyphs {
        let px = origin_x + g.x;
        let py = origin_y + g.y;
        let frac = (px - px.floor()).max(0.0).min(1.0);
        let subpx = (frac * 4.0).floor() as u8 & 3;
        let bm = rasterise_glyph(font, g.glyph_id as u16, font_size, subpx)?;
        if bm.width == 0 || bm.height == 0 { continue; }
        let dst_x0 = (px.floor() as i32) + bm.left;
        // swash returns `top` as glyph height ABOVE baseline. Bitmap
        // top-left = baseline_y - top.
        let dst_y0 = (py.round() as i32) - bm.top;

        for gy in 0..bm.height as i32 {
            let dy = dst_y0 + gy;
            if dy < 0 || dy as u32 >= buf_h { continue; }
            for gx in 0..bm.width as i32 {
                let dx = dst_x0 + gx;
                if dx < 0 || dx as u32 >= buf_w { continue; }
                let mask = bm.alpha[(gy as u32 * bm.width + gx as u32) as usize] as u32;
                if mask == 0 { continue; }
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
