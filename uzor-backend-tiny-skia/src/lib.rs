//! CPU software rasterizer backend using `tiny-skia` + `fontdue`.
//!
//! Provides a pure-Rust, zero-GPU rendering context that implements the
//! [`uzor_render::RenderContext`] trait via [`TinySkiaCpuRenderContext`].

mod context;
pub use context::TinySkiaCpuRenderContext;
