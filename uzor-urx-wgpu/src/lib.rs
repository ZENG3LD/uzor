//! URX WGPU backend — `Scene` consumer + adapter to the existing
//! `uzor-render-wgpu-instanced` primitive pipelines.
//!
//! Why a wrapper, not a fork:
//! - The existing crate has 1500+ LOC of well-tested Quad/Line/Triangle
//!   SDF + cosmic-text atlas plumbing. Don't duplicate.
//! - URX value-add at this layer is consuming the shared `Scene` enum
//!   instead of the legacy ad-hoc `RenderContext` trait method calls.
//! - Renaming the underlying crate is a Phase 9 cleanup (deprecate
//!   + remove); for now we wrap.
//!
//! ## API
//!
//! ```ignore
//! let mut ctx = InstancedRenderContext::new(w, h, 0.0, 0.0);
//! UrxWgpuBackend::adapt_scene(&scene, &mut ctx);
//! // -> caller hands `ctx.draw_commands` to InstancedRenderer::render
//! //    (or via uzor-render-hub's submit_instanced)
//! ```

pub mod adapter;

pub use adapter::{adapt_scene_into, UrxWgpuBackend};

// Re-export the underlying primitive pieces so consumers don't need a
// separate `uzor-render-wgpu-instanced` dep just to call render.
pub use uzor_render_wgpu_instanced::{
    DrawCmd, InstancedRenderContext, InstancedRenderer, LineInstance, QuadInstance,
    TextAreaData, TriangleInstance,
};
