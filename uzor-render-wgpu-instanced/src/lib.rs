//! High-performance instanced wgpu rendering backend for uzor.
//!
//! This backend replaces the vello 13-stage compute pipeline with a simple
//! instanced rendering approach:
//!
//! - All rectangles (filled, bordered, rounded) → `QuadInstance` list → 1 draw call
//! - All line segments → `LineInstance` list → 1 draw call
//! - All text → `TextAreaData` list → pending (text rasterizer to be added later)
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
