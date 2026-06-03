//! Cold-start skeleton painter — first-frame CPU rasterisation while
//! GPU shaders compile.
//!
//! The problem: every WGPU backend pays 100ms–2s shader compile cost
//! on the first frame. Without intervention the user sees a white
//! window for the entire cold start. Tessera hacked around this with
//! a backend-specific skeleton path; we promote it to a first-class
//! URX-core concept so EVERY URX consumer gets the same treatment.
//!
//! Design choices:
//!
//! - **Pure CPU**, zero GPU deps. The whole reason this exists is that
//!   GPU isn't ready yet.
//! - **Minimal vocabulary**: rect + text + optional logo (PNG bytes).
//!   No paths, no gradients, no images beyond logo. This is BOOT
//!   SCREEN, not a renderer.
//! - **~400 LOC implementation** budget. If we need more we picked the
//!   wrong scope.
//! - **One canonical implementation** in this crate. Every backend's
//!   `paint_skeleton(buf, spec)` calls into URX-core; consumers can
//!   never end up with two different-looking skeletons across backends.

use crate::math::{Color, Rect};

/// Skeleton specification — what to paint while GPU warms up.
#[derive(Debug, Clone)]
pub struct SkeletonSpec {
    /// Background fill (whole frame).
    pub bg:        Color,
    /// Optional centered logo PNG (decoded by skeleton painter).
    pub logo_png:  Option<Vec<u8>>,
    /// Optional spinner — animated indeterminate progress in centre.
    pub spinner:   bool,
    /// Optional one-line caption rendered below the centre.
    pub caption:   Option<String>,
    /// Caption color (defaults to a low-contrast grey if None).
    pub caption_color: Option<Color>,
}

impl Default for SkeletonSpec {
    fn default() -> Self {
        Self {
            bg:       Color::rgba8(13, 17, 23, 255), // dark
            logo_png: None,
            spinner:  true,
            caption:  None,
            caption_color: None,
        }
    }
}

/// A `SkeletonFrame` is a CPU pixmap the URX engine hands to the
/// GPU's swap chain via `queue.write_texture` (or to a CPU window
/// surface via softbuffer).
///
/// Each backend implementing skeleton support owns a `SkeletonFrame`
/// instance for the first N frames; after the real renderer is ready
/// it drops the skeleton frame and switches to normal paint.
pub struct SkeletonFrame {
    pub width:    u32,
    pub height:   u32,
    /// RGBA8 premultiplied pixels. Length = width × height × 4.
    pub pixels:   Vec<u8>,
    spec:         SkeletonSpec,
    started_us:   u64,
}

impl SkeletonFrame {
    pub fn new(width: u32, height: u32, spec: SkeletonSpec) -> Self {
        let pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        Self { width, height, pixels, spec, started_us: 0 }
    }

    /// Set the "started at" timestamp (μs since engine start). Used
    /// to animate the spinner phase. Caller-supplied so URX-core
    /// stays clock-free.
    pub fn set_started_us(&mut self, t_us: u64) { self.started_us = t_us; }

    pub fn spec(&self) -> &SkeletonSpec { &self.spec }

    /// Render one frame of the skeleton into the internal pixmap.
    /// `now_us` advances the spinner animation; caller supplies it.
    ///
    /// IMPLEMENTATION STATUS: this is the API surface; the actual
    /// scanline fill + bitmap text + spinner drawing land in a follow-up
    /// commit (~400 LOC implementation). The current body is a stub
    /// that fills the bg color — enough to prove the wiring.
    pub fn render(&mut self, now_us: u64) {
        let _ = now_us; // animation TBD

        // Background fill — Color → premultiplied RGBA8.
        let c = self.spec.bg;
        let bg_premul = [
            ((c.r as u32 * c.a as u32) / 255) as u8,
            ((c.g as u32 * c.a as u32) / 255) as u8,
            ((c.b as u32 * c.a as u32) / 255) as u8,
            c.a,
        ];
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bg_premul);
        }

        // TODO follow-up: spinner + caption + logo composite.
        // The doctrine is in place; the rasterisation is a follow-up
        // when the rest of the URX wiring needs it.
    }

    /// Reset to empty (release pixel memory). Called by the backend
    /// when handing off to the real renderer.
    pub fn discard(&mut self) {
        self.pixels.clear();
        self.pixels.shrink_to_fit();
    }
}

/// Bounding-box helper for centering content in a skeleton frame.
#[inline]
pub fn centered_rect(frame_w: u32, frame_h: u32, w: u32, h: u32) -> Rect {
    let x = ((frame_w.saturating_sub(w)) / 2) as f64;
    let y = ((frame_h.saturating_sub(h)) / 2) as f64;
    Rect::new(x, y, x + w as f64, y + h as f64)
}
