//! `Pixmap` — premultiplied RGBA8 buffer.
//!
//! Same layout as tiny_skia's `Pixmap` (premultiplied, row-major,
//! tightly packed) so the result can be uploaded straight to a
//! `wgpu::Texture` via `queue.write_texture` without conversion.

/// A 2D RGBA8 premultiplied pixel buffer.
///
/// Layout: row-major, tightly packed (no row padding). `pixels.len() =
/// width × height × 4`. Each pixel = `[r, g, b, a]` where the RGB
/// channels are already multiplied by alpha (`a` channel is full
/// straight alpha, `r/g/b` are `original × a / 255`).
#[derive(Debug, Clone)]
pub struct Pixmap {
    width:  u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Pixmap {
    /// Allocate a new pixmap, all-zero (transparent black).
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .expect("pixmap size overflow");
        Self { width, height, pixels: vec![0u8; n] }
    }

    /// Allocate filled with one premultiplied color.
    pub fn filled(width: u32, height: u32, premul_rgba: [u8; 4]) -> Self {
        let mut p = Self::new(width, height);
        p.fill(premul_rgba);
        p
    }

    pub fn width(&self)  -> u32 { self.width  }
    pub fn height(&self) -> u32 { self.height }
    pub fn pixels(&self) -> &[u8] { &self.pixels }
    pub fn pixels_mut(&mut self) -> &mut [u8] { &mut self.pixels }

    /// Stride in bytes (= width × 4 — no padding).
    pub fn stride(&self) -> usize { (self.width as usize) * 4 }

    /// Fill the whole pixmap with one premultiplied color.
    pub fn fill(&mut self, premul_rgba: [u8; 4]) {
        for chunk in self.pixels.chunks_exact_mut(4) {
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
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]]
    }

    /// Write one pixel (premultiplied, no blend, overwrite). Bounds-
    /// clamped — out-of-bounds writes silently no-op.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, premul_rgba: [u8; 4]) {
        if x >= self.width || y >= self.height { return; }
        let i = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        self.pixels[i .. i + 4].copy_from_slice(&premul_rgba);
    }

    /// Source-over blend a premultiplied pixel onto the buffer.
    /// Formula: `dst = src + dst × (1 - src.a)`. Operates entirely on
    /// premultiplied alpha → single multiply-add per channel.
    #[inline]
    pub fn blend_pixel(&mut self, x: u32, y: u32, src: [u8; 4]) {
        if x >= self.width || y >= self.height { return; }
        let i = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        let inv_a = 255 - src[3] as u32;
        // Integer arithmetic: round-half-up via +127.
        for c in 0..4 {
            let d = self.pixels[i + c] as u32;
            let s = src[c] as u32;
            self.pixels[i + c] = (s + (d * inv_a + 127) / 255).min(255) as u8;
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
        let mut remaining: &mut [u8] = &mut self.pixels;
        let mut y0: u32 = 0;
        for i in 0..count {
            let rows = (rows_per + if i < extra { 1 } else { 0 }) as u32;
            let byte_len = (rows as usize) * stride_bytes;
            let (head, tail) = remaining.split_at_mut(byte_len);
            out.push(PixmapStripMut {
                pixels: head, width: self.width, y0, rows,
            });
            y0 += rows;
            remaining = tail;
        }
        out
    }
}
