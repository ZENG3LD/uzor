//! URX CPU backend — own scanline rasteriser.
//!
//! Owns its math end-to-end. Does NOT wrap `tiny_skia` or `vello_cpu`
//! (those crates have correctness bugs the owner doesn't want to
//! inherit). Implementation is plain scanline rasterisation with
//! analytic edge coverage AA — same approach Skia uses, well
//! understood, no GPU shader compile time.
//!
//! ## Scope (current)
//!
//! - `Pixmap` (premultiplied RGBA8 buffer)
//! - `CpuBackend::render(scene, pixmap)` — consumes `urx_core::Scene`,
//!   walks `DrawCommand` in painter's order, writes to the pixmap
//! - Primitives: `FillRect`, `StrokeRect`, `Line`, `PushClipRect`,
//!   `PopClip` (initial slice — text + image added when consumers
//!   need them across all backends)
//! - `PushClipRoundedRect`/`PopClip` — real per-pixel rounded-clip
//!   coverage masks (`rounded.rs`)
//! - `PushBlendLayer`/`PopBlendLayer` — real offscreen-pixmap group
//!   isolation (`blend.rs`, URX Wave 3 Commit 1,
//!   `nemo/docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`
//!   §5) — content between the two composites as ONE translucent unit
//!   (correct group opacity/blending), not each primitive fading
//!   independently against whatever's already drawn.
//! - `gradient.rs` (URX Wave 4 Commit 1,
//!   `nemo/docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//!   §0.1/§2.2) — three targeted correctness fixes: `FillRect` gradient
//!   fills now consult the rounded-clip mask (previously silently
//!   ignored `radii`); the gradient's own axis/center/radius/angles now
//!   transform with the rect (previously anchored to the untransformed
//!   frame); the module doc's stale "Sweep deferred" claim corrected
//!   (Sweep was already fully implemented). `build_lut`/`sample_stops`/
//!   `premul`/`lerp_u8`/`apply_spread`/`hash_stops`/`stop_rgba8` are now
//!   thin callers of the pure-function extraction in
//!   `uzor-urx-core::gradient_lut` — same "shared canonical
//!   implementation" discipline as Wave 2's glyph-key extraction, so
//!   `uzor-urx-wgpu`'s `GradientLutAtlas` builds byte-identical LUTs.
//! - `image_reg.rs` (URX Wave 4 Commit 1, design §0.4) — now a thin
//!   `pub use uzor_urx_image::*;` re-export shim; the actual
//!   `ImageData`/`ImageRegistry`/registry functions moved verbatim to
//!   the new shared crate `uzor-urx-image` (mirrors the `uzor-urx-glyph`
//!   precedent from Wave 2) so `uzor-urx-wgpu` can consume the SAME
//!   `ImageId` allocation without depending on this crate directly —
//!   call sites at this same `crate::image_reg::*` path are unchanged.
//!
//! ## Future
//!
//! - SIMD acceleration (AVX2 / NEON) — pure Rust intrinsics, no
//!   cross-compiler issues like clang's vector extensions
//! - Multi-threading via rayon (per horizontal strip)
//! - Glyph rasterisation via skrifa (own atlas, not cosmic-text alloc-
//!   per-frame mess)

pub mod pixmap;
pub mod backend;
mod blend;
pub mod fill;
pub mod stroke;
pub mod clip;
pub mod color;
pub mod gradient;
pub mod path;
pub mod rounded;
pub mod image_reg;
pub mod image_draw;
pub mod simd;
pub mod tile;
#[cfg(feature = "parallel")]
pub mod parallel;

pub use backend::{CpuBackend, RenderError};
pub use pixmap::Pixmap;
pub use image_reg::{ImageData, ImageError, ImageRegistry, register_image, lookup_image, unregister_image};

#[cfg(feature = "parallel")]
pub use parallel::render_parallel;
