//! `Pixmap` — premultiplied RGBA8 buffer.
//!
//! Same layout as tiny_skia's `Pixmap` (premultiplied, row-major,
//! tightly packed) so the result can be uploaded straight to a
//! `wgpu::Texture` via `queue.write_texture` without conversion.
//!
//! The byte buffer is allocated 32-byte aligned. This is the
//! alignment required for `_mm256_load_si256` (AVX2 256-bit aligned
//! load) and friends, so future SIMD passes can use aligned variants
//! without per-row alignment checks. For typical 4K-aligned-on-x86
//! allocators, `Vec<u8>` only guarantees 1-byte alignment; we use
//! `alloc::alloc::Layout` + `from_raw_parts_mut` to get 32-byte
//! alignment with zero overhead per pixel.

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;

/// 32-byte aligned heap buffer of bytes. Used as `Pixmap`'s backing
/// store. Owns its memory; deallocates on drop.
///
/// Why a custom buffer instead of `Vec<u8>`? `Vec<u8>` only promises
/// 1-byte alignment. We want 32-byte aligned for AVX2 `_mm256_load_si256`
/// + aligned stores, and 16-byte aligned for SSE2 / NEON. 32-byte
/// alignment satisfies all current ISA targets.
struct AlignedBuf {
    ptr: NonNull<u8>,
    len: usize,
    layout: Layout,
}

const PIXMAP_ALIGN: usize = 32;

impl AlignedBuf {
    fn new_zeroed(len: usize) -> Self {
        // `Layout::from_size_align` errors if size + align would
        // overflow isize. We pre-checked size in `Pixmap::new`, but
        // belt-and-suspenders here. Empty len short-circuits to
        // dangling pointer (no alloc).
        if len == 0 {
            return Self {
                ptr: NonNull::dangling(),
                len: 0,
                layout: Layout::from_size_align(0, PIXMAP_ALIGN).unwrap(),
            };
        }
        let layout = Layout::from_size_align(len, PIXMAP_ALIGN)
            .expect("pixmap layout overflow");
        // SAFETY: layout has non-zero size; alloc_zeroed returns
        // either a 32-byte aligned zeroed pointer or null on OOM.
        let ptr = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(ptr).unwrap_or_else(|| {
            std::alloc::handle_alloc_error(layout)
        });
        Self { ptr, len, layout }
    }

    #[inline]
    fn as_slice(&self) -> &[u8] {
        if self.len == 0 { return &[]; }
        // SAFETY: ptr is 32-byte aligned, len bytes are initialised
        // to zero by alloc_zeroed (or never touched if len==0), and
        // the buffer is exclusively owned by Self.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    fn as_slice_mut(&mut self) -> &mut [u8] {
        if self.len == 0 { return &mut []; }
        // SAFETY: same as as_slice, and &mut self ensures no aliasing.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    #[allow(dead_code)]
    fn len(&self) -> usize { self.len }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        if self.len == 0 { return; }
        // SAFETY: ptr was allocated with `alloc_zeroed(layout)` and
        // never deallocated since; len > 0 guarantees ptr != dangling.
        unsafe { dealloc(self.ptr.as_ptr(), self.layout); }
    }
}

impl Clone for AlignedBuf {
    fn clone(&self) -> Self {
        let mut copy = Self::new_zeroed(self.len);
        if self.len > 0 {
            copy.as_slice_mut().copy_from_slice(self.as_slice());
        }
        copy
    }
}

impl std::fmt::Debug for AlignedBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlignedBuf")
            .field("len", &self.len)
            .field("align", &PIXMAP_ALIGN)
            .finish()
    }
}

// SAFETY: AlignedBuf is just an owned byte buffer; it has no thread
// affinity. The pointer is uniquely owned.
unsafe impl Send for AlignedBuf {}
unsafe impl Sync for AlignedBuf {}

/// A 2D RGBA8 premultiplied pixel buffer.
///
/// Layout: row-major, tightly packed (no row padding). `pixels.len() =
/// width × height × 4`. Each pixel = `[r, g, b, a]` where the RGB
/// channels are already multiplied by alpha (`a` channel is full
/// straight alpha, `r/g/b` are `original × a / 255`).
///
/// The underlying byte buffer is 32-byte aligned (see module docs).
#[derive(Debug, Clone)]
pub struct Pixmap {
    width:  u32,
    height: u32,
    pixels: AlignedBuf,
}

impl Pixmap {
    /// Allocate a new pixmap, all-zero (transparent black).
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .expect("pixmap size overflow");
        Self { width, height, pixels: AlignedBuf::new_zeroed(n) }
    }

    /// Allocate filled with one premultiplied color.
    pub fn filled(width: u32, height: u32, premul_rgba: [u8; 4]) -> Self {
        let mut p = Self::new(width, height);
        p.fill(premul_rgba);
        p
    }

    pub fn width(&self)  -> u32 { self.width  }
    pub fn height(&self) -> u32 { self.height }
    pub fn pixels(&self) -> &[u8] { self.pixels.as_slice() }
    pub fn pixels_mut(&mut self) -> &mut [u8] { self.pixels.as_slice_mut() }

    /// Stride in bytes (= width × 4 — no padding).
    pub fn stride(&self) -> usize { (self.width as usize) * 4 }

    /// Byte-alignment of the underlying pixel buffer. Always `32` for
    /// `Pixmap::new` allocations — guaranteed sufficient for AVX2
    /// 256-bit aligned loads/stores and 128-bit SSE2/NEON.
    #[inline]
    pub fn buffer_alignment() -> usize { PIXMAP_ALIGN }

    /// Fill the whole pixmap with one premultiplied color.
    pub fn fill(&mut self, premul_rgba: [u8; 4]) {
        for chunk in self.pixels.as_slice_mut().chunks_exact_mut(4) {
            chunk.copy_from_slice(&premul_rgba);
        }
    }

    /// Clear to fully transparent.
    pub fn clear(&mut self) {
        self.fill([0, 0, 0, 0]);
    }

    /// Read one pixel (clamped to bounds). Returns `[r, g, b, a]`
    /// premultiplied. Useful for tests + screenshot diff.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        let i = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        let s = self.pixels.as_slice();
        [s[i], s[i + 1], s[i + 2], s[i + 3]]
    }

    /// Write one pixel (premultiplied, no blend, overwrite). Bounds-
    /// clamped — out-of-bounds writes silently no-op.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, premul_rgba: [u8; 4]) {
        if x >= self.width || y >= self.height { return; }
        let i = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        self.pixels.as_slice_mut()[i .. i + 4].copy_from_slice(&premul_rgba);
    }

    /// Source-over blend a premultiplied pixel onto the buffer.
    /// Formula: `dst = src + dst × (1 - src.a)`. Operates entirely on
    /// premultiplied alpha → single multiply-add per channel.
    #[inline]
    pub fn blend_pixel(&mut self, x: u32, y: u32, src: [u8; 4]) {
        if x >= self.width || y >= self.height { return; }
        let i = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        let inv_a = 255 - src[3] as u32;
        let dst = self.pixels.as_slice_mut();
        // Integer arithmetic: round-half-up via +127.
        for c in 0..4 {
            let d = dst[i + c] as u32;
            let s = src[c] as u32;
            dst[i + c] = (s + (d * inv_a + 127) / 255).min(255) as u8;
        }
    }
}

/// Mutable view over a horizontal strip of a pixmap — a contiguous
/// slice of rows. Used by the `parallel` feature to give each rayon
/// worker exclusive access to its strip's bytes.
pub struct PixmapStripMut<'a> {
    pub(crate) pixels: &'a mut [u8],
    pub(crate) width:  u32,
    pub(crate) y0:     u32,    // strip's first row in the parent pixmap
    pub(crate) rows:   u32,
}

#[cfg(test)]
mod alignment_tests {
    use super::*;

    #[test]
    fn pixmap_buffer_is_32_byte_aligned() {
        for &(w, h) in &[(1u32, 1u32), (3, 5), (32, 8), (100, 100), (1920, 1080)] {
            let p = Pixmap::new(w, h);
            let addr = p.pixels().as_ptr() as usize;
            assert_eq!(addr % PIXMAP_ALIGN, 0,
                "{}×{}: ptr {:#x} not {}-byte aligned",
                w, h, addr, PIXMAP_ALIGN);
        }
    }

    #[test]
    fn zero_dim_pixmap_doesnt_crash() {
        let p = Pixmap::new(0, 0);
        assert_eq!(p.pixels().len(), 0);
        let p = Pixmap::new(0, 100);
        assert_eq!(p.pixels().len(), 0);
        let p = Pixmap::new(100, 0);
        assert_eq!(p.pixels().len(), 0);
    }

    #[test]
    fn clone_preserves_alignment_and_pixels() {
        let mut a = Pixmap::new(64, 16);
        for (i, b) in a.pixels_mut().iter_mut().enumerate() {
            *b = (i & 0xff) as u8;
        }
        let b = a.clone();
        assert_eq!(a.pixels(), b.pixels());
        assert_eq!(b.pixels().as_ptr() as usize % PIXMAP_ALIGN, 0);
    }
}

impl<'a> PixmapStripMut<'a> {
    pub fn width(&self)  -> u32 { self.width }
    pub fn rows(&self)   -> u32 { self.rows  }
    pub fn y0(&self)     -> u32 { self.y0    }

    /// `set_pixel`/`blend_pixel` with PARENT pixmap coordinates (not
    /// strip-local). Out-of-strip rows are silently skipped.
    #[inline]
    pub fn blend_pixel_parent(&mut self, x: u32, y: u32, src: [u8; 4]) {
        if y < self.y0 || y >= self.y0 + self.rows { return; }
        if x >= self.width { return; }
        let local_y = y - self.y0;
        let i = ((local_y as usize) * (self.width as usize) + (x as usize)) * 4;
        let inv_a = 255 - src[3] as u32;
        for c in 0..4 {
            let d = self.pixels[i + c] as u32;
            let s = src[c] as u32;
            self.pixels[i + c] = (s + (d * inv_a + 127) / 255).min(255) as u8;
        }
    }
}

impl Pixmap {
    /// Split this pixmap into N horizontal strips of (roughly) equal
    /// height. The total rows are distributed; if `count > height`
    /// the trailing strips get zero rows (still valid).
    ///
    /// Strips are returned in top-to-bottom order. Each strip owns
    /// its own row range exclusively — pass strips to threads.
    pub fn split_strips_mut(&mut self, count: usize) -> Vec<PixmapStripMut<'_>> {
        let count = count.max(1).min(self.height as usize);
        let stride_bytes = self.stride();
        let rows_per = (self.height as usize) / count;
        let extra    = (self.height as usize) % count;
        let mut out: Vec<PixmapStripMut<'_>> = Vec::with_capacity(count);
        let width = self.width;
        let mut remaining: &mut [u8] = self.pixels.as_slice_mut();
        let mut y0: u32 = 0;
        for i in 0..count {
            let rows = (rows_per + if i < extra { 1 } else { 0 }) as u32;
            let byte_len = (rows as usize) * stride_bytes;
            let (head, tail) = remaining.split_at_mut(byte_len);
            out.push(PixmapStripMut {
                pixels: head, width, y0, rows,
            });
            y0 += rows;
            remaining = tail;
        }
        out
    }
}
