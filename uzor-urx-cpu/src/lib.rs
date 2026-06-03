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
pub mod fill;
pub mod stroke;
pub mod clip;
pub mod color;
pub mod path;
#[cfg(feature = "parallel")]
pub mod parallel;

pub use backend::{CpuBackend, RenderError};
pub use pixmap::Pixmap;

#[cfg(feature = "parallel")]
pub use parallel::render_parallel;
