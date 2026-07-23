//! URX WGPU backend.
//!
//! Two render paths coexist during the URX Wave 1 cutover
//! (`docs/uzor-engines/plans/urx-wave1-native-pipelines-design-2026-07-25.md`,
//! crate map at `docs/uzor-engines/research/urx-wave1-crate-map-2026-07-25.md`):
//!
//! 1. **Legacy adapter path** (`adapter` module, [`UrxWgpuBackend`]) —
//!    translates a `Scene` into `uzor-render-wgpu-instanced` calls.
//!    Still the production path wired into `uzor-render-hub`; untouched
//!    by Wave 1.
//! 2. **Native pipeline path** ([`NativeUrxRenderer`]) — Wave 1's
//!    self-owned wgpu pipelines (Quad SDF in Commit 1, Line/capsule in
//!    Commit 2, Path/triangle + lyon tessellation in Commit 3),
//!    consuming `Scene` directly with no delegation to the legacy
//!    crate. Exercised by this crate's own tests and the pixel-parity
//!    harness (`tests/parity.rs`) until the Wave 5 cutover flips
//!    production traffic onto it.
//!
//! ## Legacy API
//!
//! ```ignore
//! let mut ctx = InstancedRenderContext::new(w, h, 0.0, 0.0);
//! UrxWgpuBackend::adapt_scene(&scene, &mut ctx);
//! // -> caller hands `ctx.draw_commands` to InstancedRenderer::render
//! //    (or via uzor-render-hub's submit_instanced)
//! ```

pub mod adapter;
mod encode;
mod msaa;
pub mod native_error;
mod pipelines;
pub mod renderer;
mod shaders;
mod tessellate;

pub use adapter::{adapt_scene_into, UrxWgpuBackend};
pub use native_error::NativeRenderError;
pub use renderer::{NativeUrxRenderer, Viewport};
pub use tessellate::TessCacheStats;

// Re-export the underlying primitive pieces so consumers don't need a
// separate `uzor-render-wgpu-instanced` dep just to call render.
pub use uzor_render_wgpu_instanced::{
    DrawCmd, InstancedRenderContext, InstancedRenderer, LineInstance, QuadInstance,
    TextAreaData, TriangleInstance,
};
