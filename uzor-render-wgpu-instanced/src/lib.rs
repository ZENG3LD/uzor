//! High-performance instanced wgpu rendering backend for uzor.
//!
//! This backend replaces the vello 13-stage compute pipeline with a simple
//! instanced rendering approach:
//!
//! - All rectangles (filled, bordered, rounded) → `QuadInstance` list → 1 draw call
//! - All line segments → `LineInstance` list → 1 draw call
//! - All text → `TextAreaData` list → pending (text rasterizer to be added later)
//!
//! ## Deprecation scope (2026-07-24, Wave 7 tail)
//!
//! For the **URX render family**, this crate is superseded — `uzor-urx-wgpu`'s
//! native pipelines (`NativeUrxRenderer`: own Quad SDF, Line capsule, Path
//! tessellation, Glyph atlas, stencil clip, gradients, images) now own every
//! production URX-family 2D submit path (ordinary + compose), reaching full
//! pixel parity with the CPU reference (`uzor-urx-wgpu/tests/parity.rs`,
//! 28/28). This crate's own adapter (`uzor-urx-wgpu::adapt_scene_into`) is
//! down to exactly one remaining URX-family caller, `uzor-urx-engine`'s
//! retained-mode Wgpu arm.
//!
//! This crate itself is **not** deprecated — it remains a first-class,
//! directly-selectable `Scene2DBackend::InstancedWgpu` for any consumer
//! that wants it explicitly (never autodetect-reachable; autodetect never
//! resolves to `InstancedWgpu` on either the Vello or URX family tree).
//! Known real consumers as of this pass, each the retirement gate on its
//! own schedule (this crate does not chase or migrate them):
//!
//! - `uzor-examples::urx-smoke-instanced` — A/B/C/D backend-bench demo.
//! - `uzor-tessera::stage2-demo` — same backend-bench pattern
//!   (`canvas2d_instanced_wgpu` button).
//! - `mylittlechart::mlc-app-render-context-instanced` (+ direct usage in
//!   `mlc-app-shell-vello`'s own GPU-thread render path) — a real product
//!   consumer, external to this workspace.
//!
//! ## Usage
//!
//! ```no_run
//! # async fn example() {
//! use uzor_render_wgpu_instanced::{InstancedRenderer, InstancedRenderContext};
//!
//! // Create the renderer once (owns GPU pipelines, atlas, etc.)
//! // let renderer = InstancedRenderer::new(&device, &queue, surface_format);
//!
//! // Each frame: create a context, draw into it, then render
//! // let mut ctx = InstancedRenderContext::new(width, height, 0.0, 0.0);
//! // ... draw calls ...
//! // renderer.render(&device, &queue, &target_view, width, height,
//! //     &ctx.quads, &ctx.lines, &ctx.text_areas, clear_color);
//! # }
//! ```

mod context;
mod glyph_instance;
mod instances;
mod renderer;
mod shaders;
mod text;
mod text_atlas;

pub use context::InstancedRenderContext;
pub use glyph_instance::GlyphInstance;
pub use instances::{DrawCmd, LineInstance, QuadInstance, TriangleInstance};
pub use renderer::InstancedRenderer;
pub use text::TextAreaData;
pub use text_atlas::{GlyphAtlas, GlyphEntry};
