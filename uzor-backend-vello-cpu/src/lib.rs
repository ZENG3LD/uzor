//! CPU-only rendering backend using `vello_cpu`.
//!
//! This crate provides [`VelloCpuRenderContext`], an implementation of the
//! `uzor_core::render::RenderContext` trait that renders entirely on the CPU — no
//! wgpu, no GPU device required.
//!
//! ## Frame lifecycle
//!
//! ```rust,ignore
//! // 1. Create once
//! let mut ctx = VelloCpuRenderContext::new(1.0);
//!
//! // 2. Every frame: begin, draw, render
//! ctx.begin_frame(width, height);
//! ctx.set_fill_color("#ff0000");
//! ctx.fill_rect(0.0, 0.0, 100.0, 100.0);
//! ctx.render_to_buffer(&mut softbuffer_buffer); // 0x00RRGGBB u32 format
//! ```

mod context;

pub use context::VelloCpuRenderContext;
