//! The three CPU rendering legs.
//!
//! Same shape as `uzor-examples/src/parity_harness.rs`'s own
//! `render_via_urx_cpu`/`render_via_vello_cpu`/`record_via_urx_ctx` —
//! this module is that trio, promoted, minus the wgpu-native leg (this
//! crate is CPU-only by design: the owner brief's own goal is "figures'
//! proof renders run on ALL THREE CPU family legs," not a fourth
//! GPU-native leg, and dropping it keeps this crate's own dependency
//! list to CPU-only rasterizer crates, no `wgpu`/`pollster`).
//!
//! Every leg returns **premultiplied RGBA8**, `width * height * 4` bytes,
//! row-major — the one shared pixel contract [`compare_tight`](crate::compare_tight)
//! and [`write_composite_png`](crate::write_composite_png) both assume.

use uzor::render::RenderContext;
use uzor_urx_core::scene::Scene;

/// Render `draw` through `tiny-skia` (`uzor-render-tiny-skia`) — the
/// backend every proof render in this workspace currently hardcodes via
/// `uzor-export::render_to_png` (`uzor-export/src/lib.rs:46,162`). tiny-skia
/// is the last-resort fallback family, not a production reference — this
/// leg exists so its own output is compared AGAINST the two production
/// families, never assumed correct by default.
pub fn render_tiny_skia(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut ctx = uzor_render_tiny_skia::TinySkiaCpuRenderContext::new(width, height, 1.0);
    draw(&mut ctx);
    ctx.pixels().to_vec()
}

/// Render `draw` through `vello_cpu` (`uzor-render-vello-cpu`) — headless,
/// no device. Reuses the exact context type production `submit_cpu_vello`
/// drives; output is already premultiplied RGBA8
/// (`VelloCpuRenderContext::render_to_pixmap_rgba8`'s own documented
/// convention), matching every other leg here — no format conversion
/// needed anywhere in this crate.
pub fn render_vello_cpu(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut ctx = uzor_render_vello_cpu::VelloCpuRenderContext::new(1.0);
    ctx.begin_frame(width, height);
    draw(&mut ctx);
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    ctx.render_to_pixmap_rgba8(&mut buf, width as u16, height as u16);
    buf
}

/// Record `draw` into a `Scene` via `uzor_render_urx::UrxRenderContext`
/// (records, doesn't rasterize), then rasterize that `Scene` through the
/// URX family's own CPU backend (`uzor-urx-cpu::CpuBackend` — the URX
/// family's semantic reference, not a GPU backend, matching that crate's
/// own parity-harness convention).
///
/// This crate's own `uzor-urx-cpu` dependency turns on the `glyph`
/// feature (see `Cargo.toml`'s own comment) specifically so this
/// function's output is a genuine rendering of `draw`, not a build
/// silently missing every unrotated `fill_text` call.
pub fn render_urx_cpu(width: u32, height: u32, draw: impl FnOnce(&mut dyn RenderContext)) -> Vec<u8> {
    let mut rec_ctx = uzor_render_urx::UrxRenderContext::new(1.0);
    rec_ctx.begin_frame(width, height);
    draw(&mut rec_ctx);
    let scene: Scene = rec_ctx.take_scene();

    let mut pixmap = uzor_urx_cpu::Pixmap::new(width, height);
    let backend = uzor_urx_cpu::CpuBackend::new();
    if let Err(e) = backend.render(&scene, &mut pixmap) {
        eprintln!("[uzor-proof-harness] urx-cpu render error: {e:?}");
    }
    pixmap.pixels().to_vec()
}

/// One proof scene, rendered through all three CPU legs at the SAME
/// `width`/`height` — the unit [`crate::compare`] and
/// [`crate::composite`] both operate on.
pub struct ThreeLegRender {
    pub width: u32,
    pub height: u32,
    pub tiny_skia: Vec<u8>,
    pub vello_cpu: Vec<u8>,
    pub urx_cpu: Vec<u8>,
}

impl ThreeLegRender {
    /// Render `draw` through all three CPU legs at `width x height`.
    ///
    /// `draw` takes `&impl Fn` (re-callable), not `FnOnce` — every real
    /// `uzor-figures` proof draw closure already has this shape (e.g.
    /// `|ctx| figure.render(ctx, rect, &theme)`, capturing only shared
    /// borrows), and driving three separate render passes from ONE
    /// closure value (rather than requiring the caller to construct it
    /// three times) is the whole point of this entry point.
    pub fn capture(width: u32, height: u32, draw: impl Fn(&mut dyn RenderContext)) -> Self {
        Self {
            width,
            height,
            tiny_skia: render_tiny_skia(width, height, &draw),
            vello_cpu: render_vello_cpu(width, height, &draw),
            urx_cpu: render_urx_cpu(width, height, &draw),
        }
    }

    /// `(label, premultiplied RGBA8 pixels)` for all three legs, in the
    /// FIXED display order this crate's composite always uses
    /// (tiny-skia | vello-cpu | urx-cpu).
    pub fn legs(&self) -> [(&'static str, &[u8]); 3] {
        [("tiny-skia", &self.tiny_skia), ("vello-cpu", &self.vello_cpu), ("urx-cpu", &self.urx_cpu)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_red_rect(ctx: &mut dyn RenderContext) {
        ctx.set_fill_color("#ff0000");
        ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    }

    /// All three legs must produce a non-empty, correctly-sized,
    /// opaque-red buffer for a trivial solid fill with no rasterizer-
    /// specific behavior to diverge on — a plumbing sanity check, not a
    /// divergence test (this crate's real proof tests own that).
    #[test]
    fn all_three_legs_render_a_solid_fill_correctly_sized_and_opaque_red() {
        let render = ThreeLegRender::capture(4, 4, solid_red_rect);
        let center = ((2 * 4 + 2) * 4) as usize;
        for (label, pixels) in render.legs() {
            assert_eq!(pixels.len(), 4 * 4 * 4, "{label}: unexpected buffer length");
            assert_eq!(&pixels[center..center + 4], &[255, 0, 0, 255], "{label}: center pixel should be opaque red");
        }
    }
}
