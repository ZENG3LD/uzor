//! Hybrid CPU/GPU rendering backend using `vello_hybrid`.
//!
//! `vello_hybrid` encodes geometry on the CPU using the sparse strips algorithm,
//! then hands off to a GPU fragment shader for the final rasterization step.
//! This makes it suitable for environments with weaker GPUs (WebGL2, mobile,
//! resource-constrained desktops) that cannot run the full `vello` compute backend.
//!
//! ## Dependency conflict warning
//!
//! `vello_hybrid 0.0.6` depends on `wgpu 27.0.1`.  The main GPU backend
//! (`uzor-backend-vello-gpu`) depends on `vello 0.6` which uses `wgpu 0.20`.
//! These two versions of wgpu **cannot coexist** in the same binary.  This
//! crate is therefore compiled separately and must not be a transitive
//! dependency of any crate that already pulls in `vello` or `wgpu 0.20`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use uzor_backend_vello_hybrid::VelloHybridRenderContext;
//!
//! // Create context (wgpu setup is handled externally)
//! let mut ctx = VelloHybridRenderContext::new(1.0);
//!
//! // Begin frame with wgpu handles
//! ctx.begin_frame(800, 600);
//!
//! // Draw using the uzor_core::render::RenderContext API
//! ctx.set_fill_color("#1e1e1e");
//! ctx.fill_rect(0.0, 0.0, 800.0, 600.0);
//!
//! // Submit to GPU via ctx.render(...)
//! ```

mod context;
pub use context::VelloHybridRenderContext;
